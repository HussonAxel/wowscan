//! Recherche d'une liste d'objets sur tous les royaumes de la région.
//!
//! Le coût d'un scan vient du téléchargement des instantanés, pas du nombre
//! d'objets cherchés : chercher 1 ou 200 objets prend le même temps.

use std::collections::{HashMap, HashSet};

use futures::StreamExt;
use serde::Serialize;

use crate::auctions::{self, Source};
use crate::cache::{Cache, now};
use crate::realms::{self, ConnectedRealm};
use crate::{BlizzardClient, Money, Result};

/// Téléchargements simultanés. L'API autorise 100 requêtes/s, mais chaque
/// réponse pèse plusieurs Mo : au-delà, on sature la connexion sans rien gagner.
pub const DEFAULT_CONCURRENCY: usize = 8;

/// Avancement d'un scan, pour la barre de progression.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ScanEvent {
    #[serde(rename_all = "camelCase")]
    Started { total: usize },
    /// `realm_id` vaut `None` pour le marché régional des objets empilables.
    #[serde(rename_all = "camelCase")]
    SourceDone { realm_id: Option<u32>, downloaded: bool, done: usize, total: usize },
    #[serde(rename_all = "camelCase")]
    SourceFailed { realm_id: Option<u32>, error: String, done: usize, total: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceStats {
    /// Prix unitaire le plus bas.
    pub min: Money,
    /// Nombre d'enchères.
    pub auctions: u32,
    /// Nombre total d'unités en vente.
    pub quantity: u32,
}

impl PriceStats {
    fn add(stats: &mut Option<Self>, price: Money, quantity: u32) {
        let s = stats.get_or_insert(Self { min: price, auctions: 0, quantity: 0 });
        s.min = s.min.min(price);
        s.auctions += 1;
        s.quantity += quantity;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RealmPrice {
    pub connected_realm_id: u32,
    pub stats: PriceStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    pub item_id: u32,
    /// Prix sur le marché régional, si l'objet est empilable : il est alors le même partout.
    pub commodity: Option<PriceStats>,
    /// Prix par groupe de royaumes, du moins cher au plus cher.
    pub realms: Vec<RealmPrice>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub realm_id: Option<u32>,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub realms: Vec<ConnectedRealm>,
    pub items: Vec<ItemResult>,
    /// Royaumes qui n'ont pas pu être lus : le résultat est alors partiel.
    pub failures: Vec<Failure>,
    pub scanned_at: u64,
}

pub async fn scan(
    client: &BlizzardClient,
    cache: &Cache,
    item_ids: &[u32],
    concurrency: usize,
    on_event: impl Fn(ScanEvent),
) -> Result<ScanResult> {
    let realms = realms::connected_realms(client, cache).await?;
    let mut wanted = HashSet::new();
    let item_ids: Vec<u32> = item_ids.iter().copied().filter(|id| wanted.insert(*id)).collect();

    let sources: Vec<Source> = std::iter::once(Source::Commodities)
        .chain(realms.iter().map(|r| Source::ConnectedRealm(r.id)))
        .collect();
    let total = sources.len();
    on_event(ScanEvent::Started { total });

    let mut commodity: HashMap<u32, Option<PriceStats>> = HashMap::new();
    let mut per_realm: HashMap<(u32, u32), Option<PriceStats>> = HashMap::new();
    let mut failures = Vec::new();
    let mut done = 0;

    let mut downloads = futures::stream::iter(sources)
        .map(|source| async move { (source, auctions::fetch(client, cache, source).await) })
        .buffer_unordered(concurrency.max(1));

    while let Some((source, fetched)) = downloads.next().await {
        done += 1;
        let realm_id = match source {
            Source::ConnectedRealm(id) => Some(id),
            Source::Commodities => None,
        };
        let fetched = match fetched {
            Ok(fetched) => fetched,
            Err(err) => {
                let error = err.to_string();
                on_event(ScanEvent::SourceFailed { realm_id, error: error.clone(), done, total });
                failures.push(Failure { realm_id, error });
                continue;
            }
        };

        for offer in fetched.snapshot.offers.iter().filter(|o| wanted.contains(&o.item_id)) {
            let stats = match realm_id {
                None => commodity.entry(offer.item_id).or_default(),
                Some(realm) => per_realm.entry((offer.item_id, realm)).or_default(),
            };
            PriceStats::add(stats, offer.unit_price, offer.quantity);
        }
        on_event(ScanEvent::SourceDone { realm_id, downloaded: fetched.downloaded, done, total });
    }

    let mut items: Vec<ItemResult> = item_ids
        .iter()
        .map(|&item_id| ItemResult {
            item_id,
            commodity: commodity.get(&item_id).copied().flatten(),
            realms: Vec::new(),
        })
        .collect();
    let index: HashMap<u32, usize> = item_ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    for ((item_id, connected_realm_id), stats) in per_realm {
        if let (Some(&i), Some(stats)) = (index.get(&item_id), stats) {
            items[i].realms.push(RealmPrice { connected_realm_id, stats });
        }
    }
    for item in &mut items {
        item.realms.sort_by_key(|r| (r.stats.min, r.connected_realm_id));
    }

    Ok(ScanResult { realms, items, failures, scanned_at: now() })
}
