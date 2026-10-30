//! Tables du jeu : versions des objets, apparences et ensembles de transmogrification.
//!
//! Un même objet existe en plusieurs versions (Normal, Héroïque, Mythique…) qui ne
//! diffèrent que par leurs *bonus* : l'API des enchères renvoie ces bonus, Wowhead les
//! met dans `?bonus=` et un lien en jeu les contient aussi. Ces tables traduisent :
//!
//! - bonus → libellé affiché sous le nom (« Mythique ») et modificateur d'apparence ;
//! - (objet, modificateur) → apparence, la vraie clé pour la transmogrification ;
//! - apparence → tous les objets qui la donnent ;
//! - ensemble (onglet « Ensembles » des collections) → ses pièces exactes ;
//! - objet → d'où il vient (boss, quête, marchand…), quel boss le laisse et son emplacement.
//!
//! Elles viennent des fichiers du jeu publiés en CSV par wago.tools (données de la
//! communauté, pas de Blizzard), sont condensées puis gardées en cache une semaine.

use std::collections::{HashMap, HashSet};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::cache::{Cache, now};
use crate::model::LocalizedString;
use crate::mounts::{self, Mount};
use crate::{Error, Result};

const WAGO_BASE: &str = "https://wago.tools/db2";
const CACHE_FILE: &str = "gamedata.json.gz";
const TTL_SECS: u64 = 7 * 24 * 3600;
/// Incrémenté quand le format du cache change, pour le reconstruire.
const FORMAT: u32 = 9;

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
    #[serde(default)]
    pub category: SetCategory,
    /// Raid et difficulté, si on sait les déterminer.
    #[serde(default)]
    pub raid: Option<SetRaid>,
    pub pieces: Vec<Source>,
}

/// Raid d'où vient un ensemble.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRaid {
    pub instance_id: u32,
    pub name: LocalizedString,
    pub difficulty: SetDifficulty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetDifficulty {
    RaidFinder,
    Normal,
    Heroic,
    Mythic,
}

/// Difficulté d'après le libellé (anglais) ; sans libellé, un ensemble de raid est en Normal.
fn raid_difficulty(label: &str) -> Option<SetDifficulty> {
    if label.starts_with("Mythic+") || label.contains("World Tier") {
        None
    } else if label.is_empty() || label.contains("Normal") {
        Some(SetDifficulty::Normal)
    } else if label.starts_with("Raid Finder") {
        Some(SetDifficulty::RaidFinder)
    } else if label.contains("Heroic") {
        Some(SetDifficulty::Heroic)
    } else if label.starts_with("Mythic") {
        Some(SetDifficulty::Mythic)
    } else {
        None
    }
}

/// Instance la plus citée (à égalité, la plus récente).
fn most_voted(votes: &[u32]) -> Option<(u32, usize)> {
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for &vote in votes {
        *counts.entry(vote).or_default() += 1;
    }
    counts.into_iter().max_by_key(|&(instance, count)| (count, instance))
}

/// Raid hérité des autres ensembles du groupe : seulement avec au moins deux voix et une nette majorité,
/// car certains groupes mêlent plusieurs raids.
fn group_instance(votes: &[u32]) -> Option<u32> {
    most_voted(votes).filter(|&(_, count)| count >= 2 && count * 2 > votes.len()).map(|(instance, _)| instance)
}

/// Grande famille d'un ensemble, déduite de son libellé, de ses pièces et du journal des aventures.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetCategory {
    Raid,
    Dungeon,
    Pvp,
    TradingPost,
    #[default]
    Other,
}

/// Type d'instance (`InstanceType` de la table `Map`).
pub(crate) const INSTANCE_DUNGEON: u8 = 1;
pub(crate) const INSTANCE_RAID: u8 = 2;
/// `TransmogSourceTypeEnum` : comptoir.
const SOURCE_TRADING_POST: u8 = 10;
/// `Flags` de `JournalInstance` : boss de zone (et Baradin, dont le butin vient des autres raids).
pub(crate) const INSTANCE_FLAG_WORLD: u32 = 2;
/// « Draenor » : boss de zone rangés sur la carte de Hautchamp, sans le drapeau.
pub(crate) const INSTANCE_DRAENOR: u32 = 557;

/// Le libellé (anglais) suffit souvent : « Mythic », « Gladiator », « Dungeons »… Sinon on regarde
/// d'où viennent les pièces : comptoir, ou boss de raid ou de donjon pour au moins la moitié d'entre elles.
/// `pieces` : pour chaque pièce, (origine, types d'instance où un boss la laisse tomber).
fn categorize(name: &str, label: &str, pieces: &[(u8, Vec<u8>)]) -> SetCategory {
    const PVP_LABELS: [&str; 7] = ["Gladiator", "Elite", "Combatant", "Aspirant", "Honor", "War Mode", "PVP"];
    const PVP_NAMES: [&str; 3] = ["Gladiator's", "Combatant's", "Aspirant's"];
    const RAID_LABELS: [&str; 4] = ["Raid Finder", "Normal", "Heroic", "Mythic"];
    let player_count = label.split_once(" Player").is_some_and(|(n, _)| n.parse::<u8>().is_ok());
    if PVP_LABELS.iter().any(|l| label.starts_with(l)) || PVP_NAMES.iter().any(|n| name.contains(n)) {
        return SetCategory::Pvp;
    }
    if RAID_LABELS.iter().any(|l| label.starts_with(l)) || player_count {
        return SetCategory::Raid;
    }
    if label.starts_with("Dungeon") || label.starts_with("Timewarped") {
        return SetCategory::Dungeon;
    }
    let half = |count: usize| !pieces.is_empty() && count * 2 >= pieces.len();
    if half(pieces.iter().filter(|(kind, _)| *kind == SOURCE_TRADING_POST).count()) {
        SetCategory::TradingPost
    } else if half(pieces.iter().filter(|(_, types)| types.contains(&INSTANCE_RAID)).count()) {
        SetCategory::Raid
    } else if half(pieces.iter().filter(|(_, types)| types.contains(&INSTANCE_DUNGEON)).count()) {
        SetCategory::Dungeon
    } else {
        SetCategory::Other
    }
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

/// Boss qui laisse tomber un objet, d'après le journal des aventures.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BossDrop {
    pub encounter_id: u32,
    pub encounter: LocalizedString,
    pub instance_id: u32,
    pub instance: LocalizedString,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Encounter {
    name: LocalizedString,
    instance: u32,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    format: u32,
    fetched_at: u64,
    bonus: HashMap<u32, BonusEffect>,
    labels: HashMap<u32, LocalizedString>,
    /// Objet → (modificateur, apparence, origine), l'origine étant `TransmogSourceTypeEnum`
    /// (1 boss, 2 quête, 3 marchand, 4 butin du monde, 7 haut fait, 8 métier…).
    item_appearances: HashMap<u32, Vec<(u16, u32, u8)>>,
    sets: Vec<TransmogSet>,
    /// Objet → rencontres du journal qui le donnent.
    drops: HashMap<u32, Vec<u32>>,
    encounters: HashMap<u32, Encounter>,
    instances: HashMap<u32, LocalizedString>,
    /// Pièces d'ensemble → emplacement (`InventoryType` : 1 tête, 3 épaules, 5 torse…).
    slots: HashMap<u32, u8>,
    mounts: Vec<Mount>,
    /// Fichier d'icône → son nom dans le jeu, sans extension : il retrouve l'icône d'un pack installé.
    icon_names: HashMap<u32, String>,
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
            for &(modifier, appearance, _) in list {
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
            .find(|(m, _, _)| *m == modifier)
            .or_else(|| list.iter().find(|(m, _, _)| *m == 0))
            .or_else(|| list.first())
            .map(|(_, a, _)| *a)
    }

    /// Origine d'un objet dans une version (`TransmogSourceTypeEnum`, 0 si inconnue).
    pub fn source_kind(&self, item_id: u32, modifier: u16) -> u8 {
        let list = self.stored.item_appearances.get(&item_id);
        list.and_then(|l| l.iter().find(|(m, _, _)| *m == modifier).or_else(|| l.first())).map_or(0, |(_, _, k)| *k)
    }

    /// Boss qui laissent tomber cet objet.
    pub fn drops(&self, item_id: u32) -> Vec<BossDrop> {
        let ids = self.stored.drops.get(&item_id).map_or(&[][..], Vec::as_slice);
        ids.iter()
            .filter_map(|id| {
                let encounter = self.stored.encounters.get(id)?;
                Some(BossDrop {
                    encounter_id: *id,
                    encounter: encounter.name.clone(),
                    instance_id: encounter.instance,
                    instance: self.stored.instances.get(&encounter.instance).cloned().unwrap_or_default(),
                })
            })
            .collect()
    }

    /// Emplacement d'une pièce d'ensemble (`InventoryType`), s'il est connu.
    pub fn slot(&self, item_id: u32) -> Option<u8> {
        self.stored.slots.get(&item_id).copied()
    }

    pub fn icon_name(&self, file_id: u32) -> Option<&str> {
        self.stored.icon_names.get(&file_id).map(String::as_str)
    }

    pub fn mounts(&self) -> &[Mount] {
        &self.stored.mounts
    }

    pub fn sets(&self) -> &[TransmogSet] {
        &self.stored.sets
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
    let (journal_items, encounters_en, encounters_fr, instances_en, instances_fr, items, maps) = tokio::try_join!(
        get("JournalEncounterItem", None),
        get("JournalEncounter", Some(LOCALES[0].0)),
        get("JournalEncounter", Some(LOCALES[1].0)),
        get("JournalInstance", Some(LOCALES[0].0)),
        get("JournalInstance", Some(LOCALES[1].0)),
        get("Item", None),
        get("Map", None),
    )?;
    let (mounts_en, mounts_fr, item_effects, item_x_effects, tiers, tier_instances, spell_misc) = tokio::try_join!(
        get("Mount", Some(LOCALES[0].0)),
        get("Mount", Some(LOCALES[1].0)),
        get("ItemEffect", None),
        get("ItemXItemEffect", None),
        get("JournalTier", None),
        get("JournalTierXInstance", None),
        get("SpellMisc", None),
    )?;
    let mounts = mounts::build(&mounts::Tables {
        mounts_en: &mounts_en,
        mounts_fr: &mounts_fr,
        item_effects: &item_effects,
        item_x_effects: &item_x_effects,
        journal_items: &journal_items,
        encounters: &encounters_en,
        instances: &instances_en,
        maps: &maps,
        tiers: &tiers,
        tier_instances: &tier_instances,
        spell_misc: &spell_misc,
    })?;

    #[derive(Deserialize)]
    struct ManifestRow {
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "FilePath")]
        path: String,
        #[serde(rename = "FileName")]
        name: String,
    }
    let icon_names: HashMap<u32, String> = rows::<ManifestRow>(&get("ManifestInterfaceData", None).await?)?
        .into_iter()
        .filter(|r| r.path.eq_ignore_ascii_case("interface\\icons\\"))
        .filter_map(|r| r.name.rsplit_once('.').map(|(stem, _)| (r.id, stem.to_owned())))
        .collect();

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
        #[serde(rename = "TransmogSourceTypeEnum")]
        kind: u8,
    }
    let mut item_appearances: HashMap<u32, Vec<(u16, u32, u8)>> = HashMap::new();
    let mut by_source_id: HashMap<u32, Source> = HashMap::new();
    for row in rows::<ModifiedRow>(&modified)? {
        if row.appearance == 0 {
            continue;
        }
        item_appearances.entry(row.item).or_default().push((row.modifier, row.appearance, row.kind));
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
        #[serde(rename = "TransmogSetGroupID")]
        group: u32,
    }
    let mut sets: HashMap<u32, TransmogSet> = HashMap::new();
    let mut set_groups: HashMap<u32, u32> = HashMap::new();
    for ((_, locale), file) in LOCALES.iter().zip([&sets_en, &sets_fr]) {
        for row in rows::<SetRow>(file)? {
            set_groups.insert(row.id, row.group);
            let set = sets.entry(row.id).or_insert_with(|| TransmogSet {
                id: row.id,
                name: LocalizedString::default(),
                label: (row.label > 0).then_some(row.label),
                // Le masque est un entier signé dans les tables : on garde ses 32 bits.
                class_mask: row.class_mask as u32,
                expansion: row.expansion,
                category: SetCategory::Other,
                raid: None,
                pieces: pieces.remove(&row.id).unwrap_or_default(),
            });
            if !row.name.is_empty() {
                set.name.0.insert((*locale).to_owned(), row.name);
            }
        }
    }
    let mut sets: Vec<TransmogSet> = sets.into_values().filter(|s| !s.name.0.is_empty()).collect();
    sets.sort_by_key(|s| s.id);

    #[derive(Deserialize)]
    struct JournalItemRow {
        #[serde(rename = "JournalEncounterID")]
        encounter: u32,
        #[serde(rename = "ItemID")]
        item: u32,
    }
    let mut drops: HashMap<u32, Vec<u32>> = HashMap::new();
    for row in rows::<JournalItemRow>(&journal_items)? {
        if item_appearances.contains_key(&row.item) {
            let list = drops.entry(row.item).or_default();
            if !list.contains(&row.encounter) {
                list.push(row.encounter);
            }
        }
    }

    #[derive(Deserialize)]
    struct EncounterRow {
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "Name_lang")]
        name: String,
        #[serde(rename = "JournalInstanceID")]
        instance: u32,
    }
    let wanted: HashSet<u32> = drops.values().flatten().copied().collect();
    let mut encounters: HashMap<u32, Encounter> = HashMap::new();
    for ((_, locale), file) in LOCALES.iter().zip([&encounters_en, &encounters_fr]) {
        for row in rows::<EncounterRow>(file)?.into_iter().filter(|r| wanted.contains(&r.id)) {
            let encounter =
                encounters.entry(row.id).or_insert_with(|| Encounter { name: LocalizedString::default(), instance: row.instance });
            if !row.name.is_empty() {
                encounter.name.0.insert((*locale).to_owned(), row.name);
            }
        }
    }

    #[derive(Deserialize)]
    struct InstanceRow {
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "Name_lang")]
        name: String,
        #[serde(rename = "MapID")]
        map: u32,
        #[serde(rename = "Flags", default)]
        flags: u32,
    }
    let wanted: HashSet<u32> = encounters.values().map(|e| e.instance).collect();
    let mut instances: HashMap<u32, LocalizedString> = HashMap::new();
    let mut instance_maps: HashMap<u32, u32> = HashMap::new();
    // Boss de zone (Pandarie, Îles Dragons…) : typés comme des raids, ils ne doivent pas attirer les ensembles.
    let mut world_instances: HashSet<u32> = HashSet::from([INSTANCE_DRAENOR]);
    for ((_, locale), file) in LOCALES.iter().zip([&instances_en, &instances_fr]) {
        for row in rows::<InstanceRow>(file)?.into_iter().filter(|r| wanted.contains(&r.id) && !r.name.is_empty()) {
            instance_maps.insert(row.id, row.map);
            if row.flags & INSTANCE_FLAG_WORLD != 0 {
                world_instances.insert(row.id);
            }
            instances.entry(row.id).or_default().0.insert((*locale).to_owned(), row.name);
        }
    }

    #[derive(Deserialize)]
    struct MapRow {
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "InstanceType")]
        kind: u8,
    }
    let map_kinds: HashMap<u32, u8> = rows::<MapRow>(&maps)?.into_iter().map(|r| (r.id, r.kind)).collect();
    let instance_kind = |encounter: &u32| {
        let instance = encounters.get(encounter)?.instance;
        map_kinds.get(instance_maps.get(&instance)?).copied()
    };
    let raid_instance = |encounter: &u32| {
        let instance = encounters.get(encounter)?.instance;
        (instance_kind(encounter) == Some(INSTANCE_RAID) && !world_instances.contains(&instance)).then_some(instance)
    };
    let mut set_instances: HashMap<u32, u32> = HashMap::new();
    let mut group_votes: HashMap<u32, Vec<u32>> = HashMap::new();
    for set in &mut sets {
        let pieces: Vec<(u8, Vec<u8>)> = set
            .pieces
            .iter()
            .map(|p| {
                let kind = item_appearances
                    .get(&p.item_id)
                    .and_then(|list| list.iter().find(|(m, a, _)| *m == p.modifier && *a == p.appearance_id))
                    .map_or(0, |&(_, _, kind)| kind);
                let types = drops.get(&p.item_id).map_or_else(Vec::new, |ids| ids.iter().filter_map(instance_kind).collect());
                (kind, types)
            })
            .collect();
        let label = set.label.and_then(|id| labels.get(&id)).map_or("", |l| l.get("en_GB"));
        set.category = categorize(set.name.get("en_GB"), label, &pieces);
        if set.category != SetCategory::Raid {
            continue;
        }
        let mut votes = Vec::new();
        for piece in &set.pieces {
            let mut piece_instances: Vec<u32> =
                drops.get(&piece.item_id).map_or_else(Vec::new, |ids| ids.iter().filter_map(raid_instance).collect());
            piece_instances.sort_unstable();
            piece_instances.dedup();
            votes.extend(piece_instances);
        }
        if let Some((instance, _)) = most_voted(&votes) {
            set_instances.insert(set.id, instance);
            let group = set_groups.get(&set.id).copied().unwrap_or(0);
            if group != 0 {
                group_votes.entry(group).or_default().push(instance);
            }
        }
    }
    for set in sets.iter_mut().filter(|s| s.category == SetCategory::Raid) {
        let inherited = || group_instance(group_votes.get(set_groups.get(&set.id)?)?);
        let Some(instance) = set_instances.get(&set.id).copied().or_else(inherited) else { continue };
        let label = set.label.and_then(|id| labels.get(&id)).map_or("", |l| l.get("en_GB"));
        let (Some(difficulty), Some(name)) = (raid_difficulty(label), instances.get(&instance)) else { continue };
        set.raid = Some(SetRaid { instance_id: instance, name: name.clone(), difficulty });
    }

    #[derive(Deserialize)]
    struct ItemRow {
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "InventoryType")]
        slot: u8,
    }
    let wanted: HashSet<u32> = sets.iter().flat_map(|s| &s.pieces).map(|p| p.item_id).collect();
    let slots: HashMap<u32, u8> =
        rows::<ItemRow>(&items)?.into_iter().filter(|r| r.slot > 0 && wanted.contains(&r.id)).map(|r| (r.id, r.slot)).collect();

    Ok(Stored {
        format: FORMAT,
        fetched_at: now(),
        bonus: bonus_map,
        labels,
        item_appearances,
        sets,
        drops,
        encounters,
        instances,
        slots,
        mounts,
        icon_names,
    })
}

pub(crate) fn rows<T: DeserializeOwned>((url, text): &(String, String)) -> Result<Vec<T>> {
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
                (124182, vec![(0, 25447, 4), (1, 25419, 4), (3, 25526, 4)]),
                // Objets fictifs : l'un partage l'apparence mythique, l'autre vient d'un boss.
                (999_001, vec![(0, 25526, 3)]),
                (999_002, vec![(3, 30000, 1)]),
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
                category: SetCategory::Pvp,
                raid: None,
                pieces: vec![
                    Source { item_id: 124182, modifier: 3, appearance_id: 25526 },
                    Source { item_id: 999_002, modifier: 3, appearance_id: 30000 },
                ],
            }],
            drops: HashMap::from([(999_002, vec![1438])]),
            encounters: HashMap::from([(1438, Encounter { name: labels(&[(0, "Archimonde", "Archimonde")])[&0].clone(), instance: 669 })]),
            instances: labels(&[(669, "Hellfire Citadel", "Citadelle des Flammes infernales")]),
            slots: HashMap::from([(124182, 6), (999_002, 1)]),
            mounts: Vec::new(),
            icon_names: HashMap::new(),
        })
    }

    #[test]
    fn set_categories() {
        use SetCategory::*;
        let boss = |kinds: &[u8]| (1, kinds.to_vec());
        assert_eq!(categorize("Erudite Occultist's Vestments", "Mythic", &[]), Raid);
        assert_eq!(categorize("Sanctified Bloodmage's Regalia", "25 Player (Heroic)", &[]), Raid);
        assert_eq!(categorize("Obsidian Gladiator's Silk Armor", "Elite", &[]), Pvp);
        assert_eq!(categorize("Verdant Gladiator's Chain Armor", "", &[]), Pvp);
        assert_eq!(categorize("Coreway Regalia", "Dungeons", &[]), Dungeon);
        assert_eq!(categorize("Golden Clockwork Attire", "Gold", &[(10, vec![]), (10, vec![])]), TradingPost);
        // Sans libellé utile : la majorité des pièces tombe sur des boss de raid.
        assert_eq!(categorize("Diabolic Raiment", "", &[boss(&[2]), boss(&[2, 1]), (4, vec![])]), Raid);
        assert_eq!(categorize("Nighthide Coat", "Green", &[boss(&[1]), (4, vec![])]), Dungeon);
        assert_eq!(categorize("Faewoven Regalia", "Campaign", &[(2, vec![]), boss(&[2]), (2, vec![])]), Other);
    }

    #[test]
    fn raid_difficulties_from_labels() {
        use SetDifficulty::*;
        for (label, expected) in [
            ("", Some(Normal)),
            ("Normal", Some(Normal)),
            ("10 Player (Normal)", Some(Normal)),
            ("25 Player (Normal)", Some(Normal)),
            ("Raid Finder", Some(RaidFinder)),
            ("Heroic", Some(Heroic)),
            ("25 Player (Heroic)", Some(Heroic)),
            ("Mythic", Some(Mythic)),
            ("Mythic+", None),
            ("Heroic World Tier", None),
            ("Gold", None),
        ] {
            assert_eq!(raid_difficulty(label), expected, "{label}");
        }
    }

    #[test]
    fn raid_instance_votes() {
        assert_eq!(most_voted(&[1, 2, 2]), Some((2, 2)));
        assert_eq!(most_voted(&[1, 2]), Some((2, 1)), "tie goes to the latest");
        assert_eq!(most_voted(&[]), None);
        assert_eq!(group_instance(&[7, 7, 8]), Some(7));
        assert_eq!(group_instance(&[7]), None, "a single vote is not enough");
        assert_eq!(group_instance(&[7, 7, 8, 8]), None, "no clear majority");
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

    #[test]
    fn origins_drops_and_slots() {
        let data = sample();
        assert_eq!(data.source_kind(124182, 3), 4);
        assert_eq!(data.source_kind(999_002, 3), 1);
        assert_eq!(data.source_kind(1, 0), 0);
        let drops = data.drops(999_002);
        assert_eq!(drops.len(), 1);
        assert_eq!((drops[0].encounter_id, drops[0].instance_id), (1438, 669));
        assert_eq!(drops[0].instance.get("fr_FR"), "Citadelle des Flammes infernales");
        assert!(data.drops(124182).is_empty());
        assert_eq!((data.slot(124182), data.slot(1)), (Some(6), None));
    }
}
