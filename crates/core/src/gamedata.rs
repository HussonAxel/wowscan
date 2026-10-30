//! Tables du jeu : versions des objets, apparences et ensembles de transmogrification.
//!
//! Un même objet existe en plusieurs versions (Normal, Héroïque, Mythique…) qui ne
//! diffèrent que par leurs *bonus* : l'API des enchères renvoie ces bonus, Wowhead les
//! met dans `?bonus=` et un lien en jeu les contient aussi. Ces tables traduisent :
//!
//! - bonus → libellé affiché sous le nom (« Mythique ») et modificateur d'apparence ;
//! - (objet, modificateur) → apparence, la vraie clé pour la transmogrification ;
//! - apparence → tous les objets qui la donnent ;
//! - ensemble (onglet « Ensembles » des collections) → ses pièces exactes.
//!
//! Elles viennent des fichiers du jeu publiés en CSV par wago.tools (données de la
//! communauté, pas de Blizzard), sont condensées puis gardées en cache une semaine.

use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::cache::{Cache, now};
use crate::model::LocalizedString;
use crate::{Error, Result};

const WAGO_BASE: &str = "https://wago.tools/db2";
const CACHE_FILE: &str = "gamedata.json.gz";
const TTL_SECS: u64 = 7 * 24 * 3600;
/// Incrémenté quand le format du cache change, pour le reconstruire.
const FORMAT: u32 = 1;

/// Types de bonus utiles (colonne `Type` de `ItemBonus`).
const BONUS_NAME_DESCRIPTION: u32 = 4;
const BONUS_APPEARANCE_MODIFIER: u32 = 7;

/// Langues téléchargées : clé wago → clé utilisée par l'API Blizzard.
const LOCALES: [(&str, &str); 2] = [("enUS", "en_GB"), ("frFR", "fr_FR")];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BonusEffect {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    modifier: Option<u16>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    labels: Vec<u32>,
}

/// Une pièce d'ensemble : un objet précis dans une version précise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub item_id: u32,
    pub modifier: u16,
    pub appearance_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransmogSet {
    pub id: u32,
    pub name: LocalizedString,
    /// Libellé de la version (« Mythique », « Héroïque »…), si l'ensemble en a un.
    pub label: Option<u32>,
    /// Classes autorisées (bit = identifiant de classe - 1) ; 0 = toutes.
    pub class_mask: u32,
    pub expansion: u32,
    pub pieces: Vec<Source>,
}

/// Version d'un objet déduite de ses bonus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    pub modifier: u16,
    pub appearance_id: Option<u32>,
    /// Libellés (identifiants de `ItemNameDescription`), dans l'ordre des bonus.
    pub labels: Vec<u32>,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    format: u32,
    fetched_at: u64,
    bonus: HashMap<u32, BonusEffect>,
    labels: HashMap<u32, LocalizedString>,
    /// Objet → (modificateur, apparence).
    item_appearances: HashMap<u32, Vec<(u16, u32)>>,
    sets: Vec<TransmogSet>,
}

pub struct GameData {
    stored: Stored,
    /// Apparence → objets qui la donnent (index inverse, recalculé au chargement).
    sources: HashMap<u32, Vec<(u32, u16)>>,
}

impl GameData {
    /// Tables depuis le cache si elles ont moins d'une semaine, sinon téléchargées.
    /// Si le téléchargement échoue, l'ancien cache sert encore.
    pub async fn load(cache: &Cache) -> Result<Self> {
        Self::load_from(cache, WAGO_BASE).await
    }

    pub async fn load_from(cache: &Cache, base: &str) -> Result<Self> {
        let cached = cache.read::<Stored>(CACHE_FILE).filter(|s| s.format == FORMAT);
        if cached.as_ref().is_some_and(|s| now().saturating_sub(s.fetched_at) < TTL_SECS) {
            return Ok(Self::index(cached.expect("vérifié juste avant")));
        }
        match download(base).await {
            Ok(stored) => {
                cache.write(CACHE_FILE, &stored)?;
                Ok(Self::index(stored))
            }
            Err(err) => cached.map(Self::index).ok_or(err),
        }
    }

    fn index(stored: Stored) -> Self {
        let mut sources: HashMap<u32, Vec<(u32, u16)>> = HashMap::new();
        for (&item, list) in &stored.item_appearances {
            for &(modifier, appearance) in list {
                sources.entry(appearance).or_default().push((item, modifier));
            }
        }
        for list in sources.values_mut() {
            list.sort_unstable();
        }
        Self { stored, sources }
    }

    pub fn fetched_at(&self) -> u64 {
        self.stored.fetched_at
    }

    /// Version d'un objet d'après ses bonus (enchère, lien Wowhead ou lien en jeu).
    pub fn variant(&self, item_id: u32, bonus_lists: &[u32]) -> Variant {
        let mut modifier = None;
        let mut labels = Vec::new();
        for effect in bonus_lists.iter().filter_map(|b| self.stored.bonus.get(b)) {
            modifier = modifier.or(effect.modifier);
            for label in &effect.labels {
                if !labels.contains(label) {
                    labels.push(*label);
                }
            }
        }
        let modifier = modifier.unwrap_or(0);
        Variant { modifier, appearance_id: self.appearance(item_id, modifier), labels }
    }

    /// Apparence d'un objet pour un modificateur ; à défaut, sa version de base.
    pub fn appearance(&self, item_id: u32, modifier: u16) -> Option<u32> {
        let list = self.stored.item_appearances.get(&item_id)?;
        list.iter()
            .find(|(m, _)| *m == modifier)
            .or_else(|| list.iter().find(|(m, _)| *m == 0))
            .or_else(|| list.first())
            .map(|(_, a)| *a)
    }

    /// Objets (et version) qui donnent cette apparence.
    pub fn sources(&self, appearance_id: u32) -> &[(u32, u16)] {
        self.sources.get(&appearance_id).map_or(&[], Vec::as_slice)
    }

    pub fn label(&self, id: u32) -> Option<&LocalizedString> {
        self.stored.labels.get(&id)
    }

    pub fn labels(&self) -> &HashMap<u32, LocalizedString> {
        &self.stored.labels
    }

    pub fn set(&self, id: u32) -> Option<&TransmogSet> {
        self.stored.sets.iter().find(|s| s.id == id)
    }

    /// Ensembles dont le nom (français ou anglais) contient tous les mots cherchés,
    /// sans tenir compte des accents ni de la casse. Les noms qui commencent par la
    /// recherche passent en premier.
    pub fn search_sets(&self, query: &str, limit: usize) -> Vec<&TransmogSet> {
        let query = fold(query);
        let words: Vec<&str> = query.split_whitespace().collect();
        if words.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<(bool, &TransmogSet)> = self
            .stored
            .sets
            .iter()
            .filter(|s| !s.pieces.is_empty())
            .filter_map(|set| {
                let names: Vec<String> = set.name.0.values().map(|n| fold(n)).collect();
                let matches = names.iter().any(|n| words.iter().all(|w| n.contains(w)));
                matches.then(|| (names.iter().any(|n| n.starts_with(&query)), set))
            })
            .collect();
        found.sort_by(|(a_prefix, a), (b_prefix, b)| {
            b_prefix.cmp(a_prefix).then_with(|| a.name.get("en_GB").cmp(b.name.get("en_GB"))).then(a.id.cmp(&b.id))
        });
        found.into_iter().take(limit).map(|(_, s)| s).collect()
    }
}

/// Minuscules sans accents, pour comparer « Héroïque » et « heroique ».
fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'ç' => 'c',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' | 'ì' => 'i',
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ÿ' => 'y',
            '’' | '‘' => '\'',
            c => c,
        })
        .collect::<String>()
        .replace('œ', "oe")
        .replace('æ', "ae")
}

async fn download(base: &str) -> Result<Stored> {
    let http = reqwest::Client::builder()
        .user_agent(concat!("wowscan/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let get = |table: &'static str, locale: Option<&'static str>| {
        let http = http.clone();
        let url = match locale {
            Some(l) => format!("{base}/{table}/csv?locale={l}"),
            None => format!("{base}/{table}/csv"),
        };
        async move {
            let response = http.get(&url).send().await?;
            if !response.status().is_success() {
                return Err(Error::Api { status: response.status().as_u16(), url });
            }
            Ok::<_, Error>((url, response.text().await?))
        }
    };

    let (bonus, modified, set_items, labels_en, labels_fr, sets_en, sets_fr) = tokio::try_join!(
        get("ItemBonus", None),
        get("ItemModifiedAppearance", None),
        get("TransmogSetItem", None),
        get("ItemNameDescription", Some(LOCALES[0].0)),
        get("ItemNameDescription", Some(LOCALES[1].0)),
        get("TransmogSet", Some(LOCALES[0].0)),
        get("TransmogSet", Some(LOCALES[1].0)),
    )?;

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct BonusRow {
        #[serde(rename = "Value_0")]
        value_0: i64,
        #[serde(rename = "ParentItemBonusListID")]
        parent: u32,
        #[serde(rename = "Type")]
        kind: u32,
    }
    let mut bonus_map: HashMap<u32, BonusEffect> = HashMap::new();
    for row in rows::<BonusRow>(&bonus)? {
        let value = u32::try_from(row.value_0).unwrap_or(0);
        match row.kind {
            BONUS_NAME_DESCRIPTION if value > 0 => bonus_map.entry(row.parent).or_default().labels.push(value),
            BONUS_APPEARANCE_MODIFIER => {
                bonus_map.entry(row.parent).or_default().modifier = u16::try_from(value).ok();
            }
            _ => {}
        }
    }

    #[derive(Deserialize)]
    struct ModifiedRow {
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "ItemID")]
        item: u32,
        #[serde(rename = "ItemAppearanceModifierID")]
        modifier: u16,
        #[serde(rename = "ItemAppearanceID")]
        appearance: u32,
    }
    let mut item_appearances: HashMap<u32, Vec<(u16, u32)>> = HashMap::new();
    let mut by_source_id: HashMap<u32, Source> = HashMap::new();
    for row in rows::<ModifiedRow>(&modified)? {
        if row.appearance == 0 {
            continue;
        }
        item_appearances.entry(row.item).or_default().push((row.modifier, row.appearance));
        by_source_id.insert(row.id, Source { item_id: row.item, modifier: row.modifier, appearance_id: row.appearance });
    }

    #[derive(Deserialize)]
    struct LabelRow {
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "Description_lang")]
        text: String,
    }
    let mut labels: HashMap<u32, LocalizedString> = HashMap::new();
    for ((_, locale), file) in LOCALES.iter().zip([&labels_en, &labels_fr]) {
        for row in rows::<LabelRow>(file)? {
            if !row.text.is_empty() {
                labels.entry(row.id).or_default().0.insert((*locale).to_owned(), row.text);
            }
        }
    }

    #[derive(Deserialize)]
    struct SetItemRow {
        #[serde(rename = "TransmogSetID")]
        set: u32,
        #[serde(rename = "ItemModifiedAppearanceID")]
        source: u32,
    }
    let mut pieces: HashMap<u32, Vec<Source>> = HashMap::new();
    for row in rows::<SetItemRow>(&set_items)? {
        if let Some(source) = by_source_id.get(&row.source) {
            pieces.entry(row.set).or_default().push(*source);
        }
    }

    #[derive(Deserialize)]
    struct SetRow {
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "Name_lang")]
        name: String,
        #[serde(rename = "ClassMask")]
        class_mask: i64,
        #[serde(rename = "ItemNameDescriptionID")]
        label: u32,
        #[serde(rename = "ExpansionID")]
        expansion: u32,
    }
    let mut sets: HashMap<u32, TransmogSet> = HashMap::new();
    for ((_, locale), file) in LOCALES.iter().zip([&sets_en, &sets_fr]) {
        for row in rows::<SetRow>(file)? {
            let set = sets.entry(row.id).or_insert_with(|| TransmogSet {
                id: row.id,
                name: LocalizedString::default(),
                label: (row.label > 0).then_some(row.label),
                // Le masque est un entier signé dans les tables : on garde ses 32 bits.
                class_mask: row.class_mask as u32,
                expansion: row.expansion,
                pieces: pieces.remove(&row.id).unwrap_or_default(),
            });
            if !row.name.is_empty() {
                set.name.0.insert((*locale).to_owned(), row.name);
            }
        }
    }
    let mut sets: Vec<TransmogSet> = sets.into_values().filter(|s| !s.name.0.is_empty()).collect();
    sets.sort_by_key(|s| s.id);

    Ok(Stored { format: FORMAT, fetched_at: now(), bonus: bonus_map, labels, item_appearances, sets })
}

fn rows<T: DeserializeOwned>((url, text): &(String, String)) -> Result<Vec<T>> {
    csv::Reader::from_reader(text.as_bytes())
        .deserialize()
        .collect::<std::result::Result<Vec<T>, _>>()
        .map_err(|err| Error::Table { url: url.clone(), message: err.to_string() })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Extraits réels des tables pour la ceinture 124182 (Citadelle des Flammes infernales).
    pub fn sample() -> GameData {
        let labels = |pairs: &[(u32, &str, &str)]| {
            pairs
                .iter()
                .map(|(id, en, fr)| {
                    (*id, LocalizedString([("en_GB", *en), ("fr_FR", *fr)].map(|(k, v)| (k.to_owned(), v.to_owned())).into()))
                })
                .collect()
        };
        let effect = |modifier: Option<u16>, labels: &[u32]| BonusEffect { modifier, labels: labels.to_vec() };
        GameData::index(Stored {
            format: FORMAT,
            fetched_at: now(),
            bonus: HashMap::from([
                (9079, effect(None, &[])),
                (9080, effect(Some(1), &[2015])),
                (9081, effect(Some(3), &[13145])),
                (1799, effect(Some(3), &[13145])),
                (3441, effect(None, &[11428])),
            ]),
            labels: labels(&[(2015, "Heroic", "Héroïque"), (13145, "Mythic", "Mythique"), (11428, "Warforged", "De guerre")]),
            item_appearances: HashMap::from([
                (124182, vec![(0, 25447), (1, 25419), (3, 25526)]),
                // Objet fictif qui partage l'apparence mythique.
                (999_001, vec![(0, 25526)]),
            ]),
            sets: vec![TransmogSet {
                id: 13,
                name: LocalizedString(
                    [("en_GB", "Wild Combatant's Chain Armor"), ("fr_FR", "Cotte d’anneaux du combattant farouche")]
                        .map(|(k, v)| (k.to_owned(), v.to_owned()))
                        .into(),
                ),
                label: Some(13145),
                class_mask: 0,
                expansion: 6,
                pieces: vec![Source { item_id: 124182, modifier: 3, appearance_id: 25526 }],
            }],
        })
    }

    #[test]
    fn variants_from_bonus_lists() {
        let data = sample();
        assert_eq!(data.variant(124182, &[9081]), Variant { modifier: 3, appearance_id: Some(25526), labels: vec![13145] });
        assert_eq!(data.variant(124182, &[9080]).appearance_id, Some(25419));
        assert_eq!(data.variant(124182, &[9079]).appearance_id, Some(25447));
        // Ancien encodage : 1799 + 1473 (bonus sans effet utile).
        assert_eq!(data.variant(124182, &[1799, 1473]).appearance_id, Some(25526));
        // « De guerre » sans modificateur : apparence normale.
        let warforged = data.variant(124182, &[1801, 563, 3441]);
        assert_eq!((warforged.appearance_id, warforged.labels), (Some(25447), vec![11428]));
        assert_eq!(data.variant(124182, &[]).appearance_id, Some(25447));
        assert_eq!(data.variant(1, &[9081]).appearance_id, None);
    }

    #[test]
    fn sources_and_set_search() {
        let data = sample();
        assert_eq!(data.sources(25526), &[(124182, 3), (999_001, 0)]);
        assert_eq!(data.search_sets("combattant farouche", 10).len(), 1);
        assert_eq!(data.search_sets("COTTE D'ANNEAUX", 10).len(), 1, "accents, case and apostrophes folded");
        assert_eq!(data.search_sets("wild chain", 10)[0].id, 13);
        assert!(data.search_sets("gladiator", 10).is_empty());
        assert!(data.search_sets("  ", 10).is_empty());
    }
}
