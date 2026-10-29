//! Fiches d'objets (nom en fr/en, qualité, type, icône), mises en cache pour de bon.

use std::collections::HashMap;

use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::cache::Cache;
use crate::client::Namespace;
use crate::model::LocalizedString;
use crate::{BlizzardClient, Error, Result};

const CACHE_FILE: &str = "items.json.gz";

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
struct RawQuality {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct RawNamed {
    #[serde(default)]
    name: LocalizedString,
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
    let mut known: HashMap<u32, ItemInfo> = cache.read(CACHE_FILE).unwrap_or_default();
    let missing: Vec<u32> = ids.iter().copied().filter(|id| !known.contains_key(id)).collect();

    if !missing.is_empty() {
        let fetched: Vec<Result<Option<ItemInfo>>> = futures::stream::iter(missing)
            .map(|id| fetch_one(client, id))
            .buffer_unordered(8)
            .collect()
            .await;
        for info in fetched {
            if let Some(info) = info? {
                known.insert(info.id, info);
            }
        }
        cache.write(CACHE_FILE, &known)?;
    }

    Ok(ids.iter().filter_map(|id| known.get(id).map(|i| (*id, i.clone()))).collect())
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
