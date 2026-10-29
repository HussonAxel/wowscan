use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};

use crate::cache::{Cache, now};
use crate::client::Namespace;
use crate::model::{Href, LocalizedString};
use crate::{BlizzardClient, Result};

/// Les fusions de royaumes sont rares : une semaine de cache suffit.
const REALMS_TTL_SECS: u64 = 7 * 24 * 3600;
const CACHE_FILE: &str = "realms.json.gz";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Realm {
    pub id: u32,
    pub name: LocalizedString,
    pub slug: String,
}

/// Groupe de royaumes qui partagent le même hôtel des ventes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectedRealm {
    pub id: u32,
    pub realms: Vec<Realm>,
}

impl ConnectedRealm {
    /// « Hyjal », ou « Kirin Tor, Sinstralis, … » pour un groupe.
    pub fn display_name(&self, locale: &str) -> String {
        let mut names: Vec<_> = self.realms.iter().map(|r| r.name.get(locale)).collect();
        names.sort_unstable();
        names.join(", ")
    }
}

#[derive(Serialize, Deserialize)]
struct CachedRealms {
    fetched_at: u64,
    realms: Vec<ConnectedRealm>,
}

/// Liste des royaumes connectés de la région, depuis le cache s'il est récent.
pub async fn connected_realms(client: &BlizzardClient, cache: &Cache) -> Result<Vec<ConnectedRealm>> {
    if let Some(cached) = cache.read::<CachedRealms>(CACHE_FILE)
        && now().saturating_sub(cached.fetched_at) < REALMS_TTL_SECS
        && !cached.realms.is_empty()
    {
        return Ok(cached.realms);
    }

    #[derive(Deserialize)]
    struct Index {
        connected_realms: Vec<Href>,
    }
    let index: Index = client.get("/data/wow/connected-realm/index", Namespace::Dynamic).await?;
    let ids: Vec<u32> = index.connected_realms.iter().filter_map(|h| id_from_href(&h.href)).collect();

    let mut realms: Vec<ConnectedRealm> = futures::stream::iter(ids)
        .map(|id| async move {
            client.get(&format!("/data/wow/connected-realm/{id}"), Namespace::Dynamic).await
        })
        .buffer_unordered(16)
        .try_collect()
        .await?;
    realms.sort_by_key(|r| r.id);

    cache.write(CACHE_FILE, &CachedRealms { fetched_at: now(), realms: realms.clone() })?;
    Ok(realms)
}

/// `https://…/connected-realm/1080?namespace=dynamic-eu` → `1080`.
fn id_from_href(href: &str) -> Option<u32> {
    href.split('?').next()?.rsplit('/').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ids_from_hrefs() {
        assert_eq!(
            id_from_href("https://eu.api.blizzard.com/data/wow/connected-realm/1080?namespace=dynamic-eu"),
            Some(1080)
        );
        assert_eq!(id_from_href("nope"), None);
    }

    #[test]
    fn display_name_sorts_group_members() {
        let realm = |id, name: &str| Realm {
            id,
            name: serde_json::from_value(serde_json::json!({ "fr_FR": name })).unwrap(),
            slug: name.to_lowercase(),
        };
        let group = ConnectedRealm { id: 1, realms: vec![realm(2, "Sinstralis"), realm(3, "Kirin Tor")] };
        assert_eq!(group.display_name("fr_FR"), "Kirin Tor, Sinstralis");
    }
}
