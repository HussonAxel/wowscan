//! Recherche d'une liste d'objets sur tous les royaumes de la région.
//!
//! Le coût d'un scan vient du téléchargement des instantanés, pas du nombre
//! d'objets cherchés : chercher 1 ou 200 objets prend le même temps.

use std::collections::{HashMap, HashSet};

use futures::StreamExt;
use serde::Serialize;

use crate::auctions::{self, Source};
use crate::cache::{Cache, now};
use crate::gamedata::{GameData, Variant};
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

/// Une version d'un objet, telle que la voit la transmogrification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantResult {
    /// Apparence ; `None` si les tables du jeu ne la connaissent pas.
    pub appearance_id: Option<u32>,
    pub modifier: u16,
    /// Libellés communs à toutes les enchères de cette version (« Mythique »…).
    pub labels: Vec<u32>,
    /// Prix sur le marché régional, si l'objet est empilable : il est alors le même partout.
    pub commodity: Option<PriceStats>,
    /// Prix par groupe de royaumes, du moins cher au plus cher.
    pub realms: Vec<RealmPrice>,
}

impl VariantResult {
    pub fn best(&self) -> Option<Money> {
        self.commodity.map(|c| c.min).or_else(|| self.realms.first().map(|r| r.stats.min))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    pub item_id: u32,
    /// Versions trouvées en vente, de la moins chère à la plus chère.
    pub variants: Vec<VariantResult>,
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

/// Clé d'une version : l'apparence si on la connaît, sinon le modificateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum VariantKey {
    Appearance(u32),
    Modifier(u16),
}

#[derive(Default)]
struct VariantAcc {
    modifier: u16,
    labels: Option<Vec<u32>>,
    commodity: Option<PriceStats>,
    realms: HashMap<u32, Option<PriceStats>>,
}

/// Scan de tous les royaumes. Avec les tables du jeu, chaque objet est découpé par
/// version (apparence) ; sans elles, toutes ses enchères forment une seule version.
pub async fn scan(
    client: &BlizzardClient,
    cache: &Cache,
    item_ids: &[u32],
    gamedata: Option<&GameData>,
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

    let mut found: HashMap<(u32, VariantKey), VariantAcc> = HashMap::new();
    // Les mêmes bonus reviennent sans cesse : on ne calcule chaque version qu'une fois.
    let mut variants: HashMap<(u32, Vec<u32>), Variant> = HashMap::new();
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
            let variant = variants
                .entry((offer.item_id, offer.bonus_lists.clone()))
                .or_insert_with(|| match gamedata {
                    Some(data) => data.variant(offer.item_id, &offer.bonus_lists),
                    None => Variant { modifier: 0, appearance_id: None, labels: Vec::new() },
                });
            let key = variant.appearance_id.map_or(VariantKey::Modifier(variant.modifier), VariantKey::Appearance);
            let acc = found.entry((offer.item_id, key)).or_default();
            acc.modifier = variant.modifier;
            acc.labels = Some(match acc.labels.take() {
                None => variant.labels.clone(),
                Some(common) => common.into_iter().filter(|l| variant.labels.contains(l)).collect(),
            });
            let stats = match realm_id {
                None => &mut acc.commodity,
                Some(realm) => acc.realms.entry(realm).or_default(),
            };
            PriceStats::add(stats, offer.unit_price, offer.quantity);
        }
        on_event(ScanEvent::SourceDone { realm_id, downloaded: fetched.downloaded, done, total });
    }

    let mut items: Vec<ItemResult> =
        item_ids.iter().map(|&item_id| ItemResult { item_id, variants: Vec::new() }).collect();
    let index: HashMap<u32, usize> = item_ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    for ((item_id, key), acc) in found {
        let mut realms: Vec<RealmPrice> = acc
            .realms
            .into_iter()
            .filter_map(|(connected_realm_id, stats)| stats.map(|stats| RealmPrice { connected_realm_id, stats }))
            .collect();
        realms.sort_by_key(|r| (r.stats.min, r.connected_realm_id));
        items[index[&item_id]].variants.push(VariantResult {
            appearance_id: match key {
                VariantKey::Appearance(id) => Some(id),
                VariantKey::Modifier(_) => None,
            },
            modifier: acc.modifier,
            labels: acc.labels.unwrap_or_default(),
            commodity: acc.commodity,
            realms,
        });
    }
    for item in &mut items {
        item.variants.sort_by_key(|v| (v.best(), v.appearance_id));
    }

    Ok(ScanResult { realms, items, failures, scanned_at: now() })
}
