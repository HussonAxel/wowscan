//! Recherche d'objets par leur nom exact.
//!
//! Sert aux copies du chat de WoW : le jeu retire les codes des liens au copier-coller,
//! il ne reste que `[Nom de l'objet]`. La recherche de l'API est approximative (par mots) ;
//! on demande une grande page et on ne garde que les noms identiques, en français ou en
//! anglais. Les réponses, y compris « aucun objet », sont gardées en cache.

use std::collections::HashMap;

use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::cache::Cache;
use crate::client::Namespace;
use crate::model::LocalizedString;
use crate::{BlizzardClient, Result};

const CACHE_FILE: &str = "names.json.gz";
const PAGE_SIZE: u32 = 1000;
/// Langues essayées, dans l'ordre : le client de jeu de l'utilisateur est souvent en français.
const LOCALES: [&str; 2] = ["fr_FR", "en_US"];

#[derive(Deserialize)]
struct SearchPage {
    #[serde(default)]
    results: Vec<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
    data: SearchItem,
}

#[derive(Deserialize)]
struct SearchItem {
    id: u32,
    name: LocalizedString,
}

#[derive(Default, Serialize, Deserialize)]
struct CachedNames(HashMap<String, Vec<u32>>);

fn normalize(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Objets dont le nom correspond exactement, pour chaque nom demandé (liste vide si aucun).
pub async fn find_items_by_name(
    client: &BlizzardClient,
    cache: &Cache,
    names: &[String],
) -> Result<HashMap<String, Vec<u32>>> {
    let mut known: CachedNames = cache.read(CACHE_FILE).unwrap_or_default();
    // Noms possédés (et non empruntés) : le futur reste `Send`, ce qu'exige Tauri.
    let missing: Vec<String> = names.iter().filter(|n| !known.0.contains_key(&normalize(n))).cloned().collect();

    if !missing.is_empty() {
        let found: Vec<(String, Result<Vec<u32>>)> = futures::stream::iter(missing)
            .map(|name| async move { (normalize(&name), search_exact(client, &name).await) })
            .buffer_unordered(4)
            .collect()
            .await;
        for (name, ids) in found {
            known.0.insert(name, ids?);
        }
        cache.write(CACHE_FILE, &known)?;
    }

    Ok(names.iter().map(|n| (n.clone(), known.0.get(&normalize(n)).cloned().unwrap_or_default())).collect())
}

async fn search_exact(client: &BlizzardClient, name: &str) -> Result<Vec<u32>> {
    let wanted = normalize(name);
    let query: String = url_encode(name.trim());
    for locale in LOCALES {
        let path = format!("/data/wow/search/item?name.{locale}={query}&_pageSize={PAGE_SIZE}");
        let page: SearchPage = client.get(&path, Namespace::Static).await?;
        let mut ids: Vec<u32> = page
            .results
            .into_iter()
            .filter(|r| r.data.name.0.values().any(|n| normalize(n) == wanted))
            .map(|r| r.data.id)
            .collect();
        if !ids.is_empty() {
            ids.sort_unstable();
            ids.dedup();
            return Ok(ids);
        }
    }
    Ok(Vec::new())
}

/// Encodage d'URL minimal pour un paramètre (lettres et chiffres ASCII gardés tels quels).
fn url_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_accents_and_spaces() {
        assert_eq!(url_encode("Claymore de Noir-Mâtin"), "Claymore%20de%20Noir-M%C3%A2tin");
        assert_eq!(url_encode("Corde d'or"), "Corde%20d%27or");
    }
}
