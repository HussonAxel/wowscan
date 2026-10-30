//! Fiches d'objets (nom en fr/en, qualité, type, icône), mises en cache pour de bon.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::cache::Cache;
use crate::client::Namespace;
use crate::model::LocalizedString;
use crate::{BlizzardClient, Error, Result};

const CACHE_FILE: &str = "items.json.gz";
/// Les demandes simultanées lisent, complètent et réécrivent le même fichier.
static CACHE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemInfo {
    pub id: u32,
    pub name: LocalizedString,
    /// `POOR`, `COMMON`, `UNCOMMON`, `RARE`, `EPIC`, `LEGENDARY`, `ARTIFACT`, `HEIRLOOM`.
    pub quality: String,
    pub item_class: LocalizedString,
    pub item_subclass: LocalizedString,
    pub inventory_type: LocalizedString,
    pub icon: Option<String>,
}

#[derive(Deserialize)]
struct RawItem {
    id: u32,
    name: LocalizedString,
    quality: RawQuality,
    item_class: RawNamed,
    item_subclass: RawNamed,
    inventory_type: RawNamed,
}

#[derive(Deserialize)]
pub(crate) struct RawQuality {
    #[serde(rename = "type")]
    pub(crate) kind: String,
}

#[derive(Deserialize)]
pub(crate) struct RawNamed {
    #[serde(default)]
    pub(crate) name: LocalizedString,
}

#[derive(Deserialize)]
struct RawMedia {
    #[serde(default)]
    assets: Vec<RawAsset>,
}

#[derive(Deserialize)]
struct RawAsset {
    key: String,
    value: String,
}

/// Fiches des objets demandés ; ceux inconnus de l'API sont absents du résultat.
pub async fn item_infos(
    client: &BlizzardClient,
    cache: &Cache,
    ids: &[u32],
) -> Result<HashMap<u32, ItemInfo>> {
    let mut known = read_known(cache);
    let missing: Vec<u32> = ids.iter().copied().filter(|id| !known.contains_key(id)).collect();

    let mut failure = None;
    if !missing.is_empty() {
        let fetched: Vec<Result<Option<ItemInfo>>> = futures::stream::iter(missing)
            .map(|id| fetch_one(client, id))
            .buffer_unordered(8)
            .collect()
            .await;
        // Un échec ne doit pas faire perdre les fiches obtenues : elles sont gardées et l'objet en échec sera redemandé.
        let mut new_items = Vec::new();
        for info in fetched {
            match info {
                Ok(Some(info)) => new_items.push(info),
                Ok(None) => {}
                Err(err) => failure = failure.or(Some(err)),
            }
        }
        if !new_items.is_empty() {
            let _guard = CACHE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
            known = cache.read(CACHE_FILE).unwrap_or_default();
            known.extend(new_items.into_iter().map(|info| (info.id, info)));
            // Sans cache écrit, les fiches restent valables pour cette réponse.
            let _ = cache.write(CACHE_FILE, &known);
        }
    }

    let found: HashMap<u32, ItemInfo> = ids.iter().filter_map(|id| known.get(id).map(|i| (*id, i.clone()))).collect();
    match failure {
        Some(err) if found.is_empty() => Err(err),
        _ => Ok(found),
    }
}

fn read_known(cache: &Cache) -> HashMap<u32, ItemInfo> {
    let _guard = CACHE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    cache.read(CACHE_FILE).unwrap_or_default()
}

async fn fetch_one(client: &BlizzardClient, id: u32) -> Result<Option<ItemInfo>> {
    let raw: RawItem = match client.get(&format!("/data/wow/item/{id}"), Namespace::Static).await {
        Ok(raw) => raw,
        Err(Error::Api { status: 404, .. }) => return Ok(None),
        Err(err) => return Err(err),
    };
    // Une icône manquante ne doit pas faire échouer la fiche.
    let icon = client
        .get::<RawMedia>(&format!("/data/wow/media/item/{id}"), Namespace::Static)
        .await
        .ok()
        .and_then(|m| m.assets.into_iter().find(|a| a.key == "icon"))
        .map(|a| a.value);

    Ok(Some(ItemInfo {
        id: raw.id,
        name: raw.name,
        quality: raw.quality.kind,
        item_class: raw.item_class.name,
        item_subclass: raw.item_subclass.name,
        inventory_type: raw.inventory_type.name,
        icon,
    }))
}
