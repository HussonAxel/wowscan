//! Téléchargement des enchères.
//!
//! Blizzard publie un instantané par heure : un par groupe de royaumes connectés
//! (équipement, mascottes, montures…) et un seul pour toute la région pour les
//! objets empilables (« commodities » : herbes, minerais, gemmes…).
//!
//! On ne garde de chaque enchère que l'objet, le prix unitaire et la quantité,
//! ce qui divise la taille par ~10. Un nouveau scan dans l'heure ne retélécharge
//! rien : la requête porte `If-Modified-Since` et l'API répond `304`.

use serde::{Deserialize, Serialize};

use crate::cache::{Cache, now};
use crate::client::{Fetched, Namespace};
use crate::{BlizzardClient, Money, Result};

/// En dessous de ce délai, on réutilise le cache sans même interroger l'API.
const FRESH_SECS: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Source {
    ConnectedRealm(u32),
    /// Marché régional des objets empilables.
    Commodities,
}

impl Source {
    fn path(self) -> String {
        match self {
            Self::ConnectedRealm(id) => format!("/data/wow/connected-realm/{id}/auctions"),
            Self::Commodities => "/data/wow/auctions/commodities".to_owned(),
        }
    }

    fn cache_file(self) -> String {
        match self {
            Self::ConnectedRealm(id) => format!("auctions-{id}.json.gz"),
            Self::Commodities => "commodities.json.gz".to_owned(),
        }
    }
}

/// Une enchère, réduite à ce dont on a besoin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Offer {
    #[serde(rename = "i")]
    pub item_id: u32,
    /// Prix d'une unité, achat immédiat.
    #[serde(rename = "p")]
    pub unit_price: Money,
    #[serde(rename = "q")]
    pub quantity: u32,
    /// Variantes (niveau d'objet, châsse…) ; vide pour la plupart des objets.
    #[serde(rename = "b", default, skip_serializing_if = "Vec::is_empty")]
    pub bonus_lists: Vec<u32>,
    /// Pour les mascottes en cage (objet 82800).
    #[serde(rename = "s", default, skip_serializing_if = "Option::is_none")]
    pub pet_species_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub source: Source,
    pub last_modified: Option<String>,
    /// Date (epoch, secondes) du dernier contrôle auprès de l'API.
    pub checked_at: u64,
    pub offers: Vec<Offer>,
}

#[derive(Deserialize)]
struct RawAuctions {
    #[serde(default)]
    auctions: Vec<RawAuction>,
}

#[derive(Deserialize)]
struct RawAuction {
    item: RawItem,
    /// Prix total d'achat immédiat (enchères par royaume).
    buyout: Option<u64>,
    /// Prix unitaire (objets empilables).
    unit_price: Option<u64>,
    quantity: u32,
}

#[derive(Deserialize)]
struct RawItem {
    id: u32,
    #[serde(default)]
    bonus_lists: Vec<u32>,
    pet_species_id: Option<u32>,
}

impl RawAuction {
    /// Les enchères sans achat immédiat sont ignorées : on ne peut pas les acheter tout de suite.
    fn into_offer(self) -> Option<Offer> {
        let quantity = self.quantity.max(1);
        let unit_price = self.unit_price.or_else(|| self.buyout.map(|b| b / u64::from(quantity)))?;
        Some(Offer {
            item_id: self.item.id,
            unit_price: Money(unit_price),
            quantity,
            bonus_lists: self.item.bonus_lists,
            pet_species_id: self.item.pet_species_id,
        })
    }
}

/// Instantané d'enchères et provenance (cache ou téléchargement).
#[derive(Debug)]
pub struct FetchedSnapshot {
    pub snapshot: Snapshot,
    pub downloaded: bool,
}

pub async fn fetch(client: &BlizzardClient, cache: &Cache, source: Source) -> Result<FetchedSnapshot> {
    let file = source.cache_file();
    let cached: Option<Snapshot> = cache.read(&file);

    if let Some(snapshot) = &cached
        && now().saturating_sub(snapshot.checked_at) < FRESH_SECS
    {
        return Ok(FetchedSnapshot { snapshot: snapshot.clone(), downloaded: false });
    }

    let since = cached.as_ref().and_then(|s| s.last_modified.as_deref());
    let fetched = client.get_json::<RawAuctions>(&source.path(), Namespace::Dynamic, since).await?;

    let (snapshot, downloaded) = match (fetched, cached) {
        (Fetched::NotModified, Some(mut snapshot)) => {
            snapshot.checked_at = now();
            (snapshot, false)
        }
        (Fetched::NotModified, None) => {
            // L'API ne devrait pas répondre 304 sans `If-Modified-Since` ; on retente sans.
            let value = client.get::<RawAuctions>(&source.path(), Namespace::Dynamic).await?;
            (reduce(source, value, None), true)
        }
        (Fetched::Fresh { value, last_modified }, _) => (reduce(source, value, last_modified), true),
    };

    cache.write(&file, &snapshot)?;
    Ok(FetchedSnapshot { snapshot, downloaded })
}

fn reduce(source: Source, raw: RawAuctions, last_modified: Option<String>) -> Snapshot {
    Snapshot {
        source,
        last_modified,
        checked_at: now(),
        offers: raw.auctions.into_iter().filter_map(RawAuction::into_offer).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduces_realm_and_commodity_auctions() {
        let raw: RawAuctions = serde_json::from_str(
            r#"{"auctions":[
                {"id":1,"item":{"id":19019,"bonus_lists":[6654]},"buyout":50000000,"quantity":1,"time_left":"LONG"},
                {"id":2,"item":{"id":82800,"pet_species_id":39,"pet_level":1},"buyout":3000000,"quantity":1},
                {"id":3,"item":{"id":2589},"bid":100,"quantity":1},
                {"id":4,"item":{"id":2589},"unit_price":250,"quantity":20},
                {"id":5,"item":{"id":2592},"buyout":1000,"quantity":4}
            ]}"#,
        )
        .unwrap();
        let snapshot = reduce(Source::ConnectedRealm(1080), raw, None);

        assert_eq!(snapshot.offers.len(), 4, "l'enchère sans achat immédiat est ignorée");
        assert_eq!(snapshot.offers[0].bonus_lists, vec![6654]);
        assert_eq!(snapshot.offers[1].pet_species_id, Some(39));
        assert_eq!(snapshot.offers[2].unit_price, Money(250));
        assert_eq!(snapshot.offers[3].unit_price, Money(250), "buyout total / quantité");
    }
}
