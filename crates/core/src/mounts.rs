//! Montures du jeu, rangées par extension puis par catégorie.
//!
//! Les tables ne donnent ni l'une ni l'autre directement. La catégorie vient du type de source,
//! du texte de source (`Faction:`, `Zone:`…) et du journal des aventures. L'extension vient du
//! journal quand la monture y figure, sinon de son identifiant, à peu près chronologique : c'est
//! une approximation, en particulier pour les montures de boutique ou de promotion.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::cache::Cache;
use crate::client::Namespace;
use crate::gamedata::{INSTANCE_DRAENOR, INSTANCE_DUNGEON, INSTANCE_FLAG_WORLD, INSTANCE_RAID, rows};
use crate::items::{RawNamed, RawQuality};
use crate::model::LocalizedString;
use crate::{BlizzardClient, Error, Result};

const TOOLTIPS_FILE: &str = "mount-tooltips.json.gz";
/// Les demandes simultanées lisent, complètent et réécrivent le même fichier.
static CACHE_LOCK: Mutex<()> = Mutex::new(());

/// `SourceTypeEnum` de la table `Mount`.
const SOURCE_DROP: i32 = 0;
const SOURCE_QUEST: i32 = 1;
const SOURCE_VENDOR: i32 = 2;
const SOURCE_PROFESSION: i32 = 3;
const SOURCE_ACHIEVEMENT: i32 = 5;
const SOURCE_EVENT: i32 = 6;
const SOURCE_PROMOTION: i32 = 7;
const SOURCE_TCG: i32 = 8;
const SOURCE_SHOP: i32 = 9;
const SOURCE_TRADING_POST: i32 = 11;

/// Premier identifiant de monture de chaque extension, relevé sur les montures du journal.
const EXPANSION_STARTS: [(u32, u32); 11] = [
    (129, 1),
    (236, 2),
    (383, 3),
    (448, 4),
    (600, 5),
    (763, 6),
    (993, 7),
    (1350, 8),
    (1588, 9),
    (2140, 10),
    (2600, 11),
];

/// Mots d'une source de butin qui désignent un contenant ou une activité, pas une créature.
const NOT_A_CREATURE: [&str; 36] = [
    "drop", "chest", "cache", "treasure", "crate", "box", "boxe", "bag", "coffer", "stash", "satchel", "hoard", "mission",
    "expedition", "reward", "world", "bounty", "pack", "egg", "crystal", "paragon", "assault", "vessel", "gift", "container",
    "supplie", "trove", "strongbox", "casket", "sack", "purse", "spoil", "offering", "rift", "pile", "footlocker",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MountCategory {
    Raid,
    Dungeon,
    Reputation,
    Quest,
    Elite,
    Drop,
    Vendor,
    Achievement,
    Profession,
    Pvp,
    Event,
    Promotion,
    TradingPost,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    pub id: u32,
    pub spell_id: u32,
    /// Objet qui apprend la monture, s'il existe : il donne sa fiche.
    pub item_id: Option<u32>,
    /// Fichier d'icône du sort : toutes les montures en ont un, même sans objet.
    pub icon: Option<u32>,
    pub name: LocalizedString,
    pub source: LocalizedString,
    pub category: MountCategory,
    pub expansion: u32,
}

/// Lieu d'une rencontre du journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Raid,
    Dungeon,
    World,
}

fn place(kind: Option<u8>, flags: u32, instance: u32) -> Option<Place> {
    if flags & INSTANCE_FLAG_WORLD != 0 || instance == INSTANCE_DRAENOR {
        return Some(Place::World);
    }
    match kind? {
        INSTANCE_RAID => Some(Place::Raid),
        INSTANCE_DUNGEON => Some(Place::Dungeon),
        _ => None,
    }
}

static FIELD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\|c[0-9a-f]{8}([^:|]+):? ?\|r ?([^|]*)").unwrap());
static CODES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\|c[0-9a-f]{8}|\|r|\|H[^|]*\|h|\|h|\|T[^|]*\|t").unwrap());

/// Texte de source sans les codes de couleur, de lien et d'icône du jeu.
fn clean_source(text: &str) -> String {
    let text = CODES.replace_all(text, "").replace("|n", "\n");
    text.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(" · ")
}

fn expansion_from_id(id: u32) -> u32 {
    EXPANSION_STARTS.iter().rev().find(|(start, _)| id >= *start).map_or(0, |(_, expansion)| *expansion)
}

fn is_creature(source: &str) -> bool {
    !source.split(|c: char| !c.is_alphanumeric()).any(|word| {
        let word = word.to_lowercase();
        let singular = word.strip_suffix('s').unwrap_or(&word);
        NOT_A_CREATURE.contains(&singular)
    })
}

/// `journal` : lieu de la rencontre qui donne la monture. `zone_place` : lieu d'une zone, d'après son nom anglais.
fn categorize(source_kind: i32, text: &str, journal: Option<Place>, zone_place: impl Fn(&str) -> Option<Place>) -> MountCategory {
    use MountCategory::*;
    match journal {
        Some(Place::World) => return Elite,
        Some(Place::Raid) => return Raid,
        Some(Place::Dungeon) => return Dungeon,
        None => {}
    }
    let fields: Vec<(&str, &str)> = FIELD
        .captures_iter(text)
        .map(|c| (c.get(1).map_or("", |m| m.as_str().trim()), c.get(2).map_or("", |m| m.as_str().trim())))
        .collect();
    let has = |prefix: &str| fields.iter().any(|(header, _)| header.starts_with(prefix));
    let value = |name: &str| fields.iter().find(|(header, _)| *header == name).map(|(_, value)| *value);

    if source_kind == SOURCE_TRADING_POST || has("Trading Post") {
        return TradingPost;
    }
    if matches!(source_kind, SOURCE_PROMOTION | SOURCE_TCG | SOURCE_SHOP) || has("In-Game Shop") || has("Promotion") || has("Recruit") {
        return Promotion;
    }
    if has("PvP") {
        return Pvp;
    }
    if has("Faction") || has("Renown") || has("Covenant") {
        return Reputation;
    }
    if source_kind == SOURCE_ACHIEVEMENT || has("Achievement") {
        return Achievement;
    }
    if source_kind == SOURCE_EVENT || has("World Event") || has("Holiday") {
        return Event;
    }
    if source_kind == SOURCE_PROFESSION || has("Profession") {
        return Profession;
    }
    if source_kind == SOURCE_QUEST || has("Quest") {
        return Quest;
    }
    let zone = value("Zone").or_else(|| value("Location"));
    let instance = zone.and_then(|z| zone_place(z.split(" (").next().unwrap_or(z)));
    match instance {
        Some(Place::Raid) => return Raid,
        Some(Place::Dungeon) => return Dungeon,
        _ => {}
    }
    if let Some(drop) = value("Drop") {
        return if zone.is_some() && is_creature(drop) { Elite } else { Drop };
    }
    if source_kind == SOURCE_VENDOR || has("Vendor") {
        return Vendor;
    }
    if source_kind == SOURCE_DROP || has("Treasure") {
        return Drop;
    }
    Other
}

/// Tables nécessaires, telles que téléchargées (url, CSV).
pub(crate) struct Tables<'a> {
    pub mounts_en: &'a (String, String),
    pub mounts_fr: &'a (String, String),
    pub item_effects: &'a (String, String),
    pub item_x_effects: &'a (String, String),
    pub journal_items: &'a (String, String),
    pub encounters: &'a (String, String),
    pub instances: &'a (String, String),
    pub maps: &'a (String, String),
    pub tiers: &'a (String, String),
    pub tier_instances: &'a (String, String),
    pub spell_misc: &'a (String, String),
}

pub(crate) fn build(tables: &Tables) -> Result<Vec<Mount>> {
    #[derive(Deserialize)]
    struct EffectRow {
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "SpellID")]
        spell: i64,
    }
    #[derive(Deserialize)]
    struct ItemEffectRow {
        #[serde(rename = "ItemEffectID")]
        effect: u32,
        #[serde(rename = "ItemID")]
        item: u32,
    }
    #[derive(Deserialize)]
    struct JournalItemRow {
        #[serde(rename = "JournalEncounterID")]
        encounter: u32,
        #[serde(rename = "ItemID")]
        item: u32,
    }
    #[derive(Deserialize)]
    struct EncounterRow {
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "JournalInstanceID")]
        instance: u32,
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
    #[derive(Deserialize)]
    struct MapRow {
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "InstanceType")]
        kind: u8,
    }
    #[derive(Deserialize)]
    struct TierRow {
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "Expansion")]
        expansion: i64,
    }
    #[derive(Deserialize)]
    struct TierInstanceRow {
        #[serde(rename = "JournalTierID")]
        tier: u32,
        #[serde(rename = "JournalInstanceID")]
        instance: u32,
    }
    #[derive(Deserialize)]
    struct MountRow {
        #[serde(rename = "Name_lang")]
        name: String,
        #[serde(rename = "SourceText_lang")]
        source: String,
        #[serde(rename = "ID")]
        id: u32,
        #[serde(rename = "SourceTypeEnum")]
        source_kind: i32,
        #[serde(rename = "SourceSpellID")]
        spell: u32,
    }
    #[derive(Deserialize)]
    struct SpellMiscRow {
        #[serde(rename = "SpellID")]
        spell: u32,
        #[serde(rename = "SpellIconFileDataID")]
        icon: u32,
    }

    let effects: HashMap<u32, i64> = rows::<EffectRow>(tables.item_effects)?.into_iter().map(|r| (r.id, r.spell)).collect();
    let mut spell_items: HashMap<i64, Vec<u32>> = HashMap::new();
    for row in rows::<ItemEffectRow>(tables.item_x_effects)? {
        if let Some(spell) = effects.get(&row.effect) {
            spell_items.entry(*spell).or_default().push(row.item);
        }
    }
    let mut item_encounters: HashMap<u32, Vec<u32>> = HashMap::new();
    for row in rows::<JournalItemRow>(tables.journal_items)? {
        item_encounters.entry(row.item).or_default().push(row.encounter);
    }
    let encounter_instances: HashMap<u32, u32> = rows::<EncounterRow>(tables.encounters)?.into_iter().map(|r| (r.id, r.instance)).collect();
    let map_kinds: HashMap<u32, u8> = rows::<MapRow>(tables.maps)?.into_iter().map(|r| (r.id, r.kind)).collect();

    let mut instance_places: HashMap<u32, Place> = HashMap::new();
    let mut zone_places: HashMap<String, Place> = HashMap::new();
    for row in rows::<InstanceRow>(tables.instances)? {
        if let Some(found) = place(map_kinds.get(&row.map).copied(), row.flags, row.id) {
            instance_places.insert(row.id, found);
            zone_places.insert(row.name, found);
        }
    }

    // « Expansion » vaut 100 pour Classique, 200 pour Burning Crusade… ; 9000 pour la saison en cours, ignorée.
    let tiers: HashMap<u32, u32> = rows::<TierRow>(tables.tiers)?
        .into_iter()
        .filter_map(|r| u32::try_from(r.expansion / 100 - 1).ok().filter(|e| *e < 50).map(|e| (r.id, e)))
        .collect();
    let mut instance_expansions: HashMap<u32, u32> = HashMap::new();
    for row in rows::<TierInstanceRow>(tables.tier_instances)? {
        if let Some(&expansion) = tiers.get(&row.tier) {
            let entry = instance_expansions.entry(row.instance).or_insert(expansion);
            *entry = expansion.min(*entry);
        }
    }

    // Un sort a une ligne par difficulté : la première suffit.
    let mut spell_icons: HashMap<u32, u32> = HashMap::new();
    for row in rows::<SpellMiscRow>(tables.spell_misc)? {
        if row.icon != 0 {
            spell_icons.entry(row.spell).or_insert(row.icon);
        }
    }

    let mut french: HashMap<u32, MountRow> = rows::<MountRow>(tables.mounts_fr)?.into_iter().map(|r| (r.id, r)).collect();
    let localized = |en: String, fr: Option<&str>| {
        let mut text = LocalizedString::default();
        for (locale, value) in [("en_GB", Some(en.as_str())), ("fr_FR", fr)] {
            if let Some(value) = value.filter(|v| !v.is_empty()) {
                text.0.insert(locale.to_owned(), value.to_owned());
            }
        }
        text
    };

    let mut mounts = Vec::new();
    for row in rows::<MountRow>(tables.mounts_en)? {
        // Montures cachées ou de test du jeu.
        if row.source.trim().is_empty() || row.name.starts_with('[') {
            continue;
        }
        let items = spell_items.get(&i64::from(row.spell)).map_or(&[][..], Vec::as_slice);
        let instance = items
            .iter()
            .flat_map(|item| item_encounters.get(item).into_iter().flatten())
            .find_map(|encounter| encounter_instances.get(encounter).copied().filter(|i| instance_places.contains_key(i)));
        let journal = instance.and_then(|i| instance_places.get(&i).copied());
        let category = categorize(row.source_kind, &row.source, journal, |zone| zone_places.get(zone).copied());
        let expansion = instance.and_then(|i| instance_expansions.get(&i).copied()).unwrap_or_else(|| expansion_from_id(row.id));
        let fr = french.remove(&row.id);
        mounts.push(Mount {
            id: row.id,
            spell_id: row.spell,
            item_id: items.iter().copied().min(),
            icon: spell_icons.get(&row.spell).copied(),
            source: localized(clean_source(&row.source), fr.as_ref().map(|f| clean_source(&f.source)).as_deref()),
            name: localized(row.name, fr.as_ref().map(|f| f.name.as_str())),
            category,
            expansion,
        });
    }
    mounts.sort_by_key(|m| m.id);
    Ok(mounts)
}

/// Fiche d'une monture, telle que le jeu l'affiche. Sans objet, seule la description est connue.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MountTooltip {
    pub quality: Option<String>,
    pub level: Option<LocalizedString>,
    pub binding: Option<LocalizedString>,
    pub kind: Option<LocalizedString>,
    pub spells: Vec<LocalizedString>,
    pub requirements: Vec<LocalizedString>,
    pub description: Option<LocalizedString>,
}

#[derive(Deserialize)]
struct RawDisplay {
    #[serde(default)]
    display_string: LocalizedString,
}

#[derive(Deserialize)]
struct RawSpell {
    #[serde(default)]
    description: LocalizedString,
}

#[derive(Default, Deserialize)]
struct RawRequirements {
    level: Option<RawDisplay>,
    skill: Option<RawDisplay>,
}

#[derive(Default, Deserialize)]
struct RawPreview {
    level: Option<RawDisplay>,
    binding: Option<RawNamed>,
    item_subclass: Option<RawNamed>,
    #[serde(default)]
    spells: Vec<RawSpell>,
    #[serde(default)]
    requirements: RawRequirements,
    description: Option<LocalizedString>,
}

#[derive(Deserialize)]
struct RawItem {
    quality: RawQuality,
    #[serde(default)]
    preview_item: RawPreview,
}

#[derive(Deserialize)]
struct RawMount {
    description: Option<LocalizedString>,
}

fn filled(text: LocalizedString) -> Option<LocalizedString> {
    (!text.0.is_empty()).then_some(text)
}

impl From<RawItem> for MountTooltip {
    fn from(item: RawItem) -> Self {
        let preview = item.preview_item;
        let requirements = [preview.requirements.level, preview.requirements.skill];
        Self {
            quality: Some(item.quality.kind),
            level: preview.level.and_then(|l| filled(l.display_string)),
            binding: preview.binding.and_then(|b| filled(b.name)),
            kind: preview.item_subclass.and_then(|k| filled(k.name)),
            spells: preview.spells.into_iter().filter_map(|s| filled(s.description)).collect(),
            requirements: requirements.into_iter().flatten().filter_map(|r| filled(r.display_string)).collect(),
            description: preview.description.and_then(filled),
        }
    }
}

/// Fiche d'une monture ; la description vient de l'objet, sinon de la monture elle-même.
pub async fn mount_tooltip(client: &BlizzardClient, cache: &Cache, mount_id: u32, item_id: Option<u32>) -> Result<MountTooltip> {
    let cached: HashMap<u32, MountTooltip> = {
        let _guard = CACHE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        cache.read(TOOLTIPS_FILE).unwrap_or_default()
    };
    if let Some(tooltip) = cached.get(&mount_id) {
        return Ok(tooltip.clone());
    }

    let mut tooltip = match item_id {
        Some(id) => match client.get::<RawItem>(&format!("/data/wow/item/{id}"), Namespace::Static).await {
            Ok(item) => MountTooltip::from(item),
            Err(Error::Api { status: 404, .. }) => MountTooltip::default(),
            Err(err) => return Err(err),
        },
        None => MountTooltip::default(),
    };
    if tooltip.description.is_none() {
        tooltip.description = match client.get::<RawMount>(&format!("/data/wow/mount/{mount_id}"), Namespace::Static).await {
            Ok(mount) => mount.description.and_then(filled),
            Err(Error::Api { status: 404, .. }) => None,
            Err(err) => return Err(err),
        };
    }

    let _guard = CACHE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let mut known: HashMap<u32, MountTooltip> = cache.read(TOOLTIPS_FILE).unwrap_or_default();
    known.insert(mount_id, tooltip.clone());
    let _ = cache.write(TOOLTIPS_FILE, &known);
    Ok(tooltip)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "|cFFFFD200";

    fn source(fields: &[(&str, &str)]) -> String {
        fields.iter().map(|(h, v)| format!("{HEADER}{h}: |r{v}|n")).collect()
    }

    fn category(kind: i32, fields: &[(&str, &str)]) -> MountCategory {
        categorize(kind, &source(fields), None, |zone| (zone == "Firelands").then_some(Place::Raid))
    }

    #[test]
    fn tooltip_from_item() {
        let item: RawItem = serde_json::from_str(
            r#"{"quality":{"type":"EPIC"},"preview_item":{
                "level":{"value":10,"display_string":{"en_GB":"Item Level 10"}},
                "binding":{"name":{"en_GB":"Binds when picked up"}},
                "item_subclass":{"name":{"en_GB":"Mount"}},
                "spells":[{"description":{"en_GB":"Teaches you"}},{"description":{"en_GB":null}}],
                "requirements":{"level":{"display_string":{"en_GB":"Requires Level 10"}},"skill":{"display_string":{"en_GB":"Requires Riding"}}},
                "description":{"en_GB":"Tamed."}}}"#,
        )
        .unwrap();
        let tooltip = MountTooltip::from(item);
        assert_eq!(tooltip.quality.as_deref(), Some("EPIC"));
        assert_eq!(tooltip.level.unwrap().get("en_GB"), "Item Level 10");
        assert_eq!(tooltip.spells.len(), 1, "empty spell texts are dropped");
        assert_eq!(tooltip.requirements.len(), 2);
        assert_eq!(tooltip.description.unwrap().get("en_GB"), "Tamed.");
    }

    #[test]
    fn categories_from_source() {
        use MountCategory::*;
        assert_eq!(categorize(0, "", Some(Place::Raid), |_| None), Raid);
        assert_eq!(categorize(0, "", Some(Place::World), |_| None), Elite);
        assert_eq!(category(0, &[("Drop", "Alysrazor"), ("Zone", "Firelands")]), Raid);
        assert_eq!(category(0, &[("Drop", "Time-Lost Proto-Drake"), ("Zone", "The Storm Peaks")]), Elite);
        assert_eq!(category(0, &[("Drop", "Invader's Forgotten Treasure"), ("Zone", "Tanaan Jungle")]), Drop);
        assert_eq!(category(0, &[("Drop", "Time Rifts"), ("Zone", "Tyrhold")]), Drop);
        assert_eq!(category(2, &[("Vendor", "Someone"), ("Faction", "Kirin Tor")]), Reputation);
        assert_eq!(category(0, &[("Vendor", "Drakma"), ("Zone", "Orgrimmar")]), Vendor);
        assert_eq!(category(1, &[("Quest", "A quest")]), Quest);
        assert_eq!(category(5, &[("Achievement", "Glory")]), Achievement);
        assert_eq!(category(0, &[("PvP", "Season 1")]), Pvp);
        assert_eq!(category(11, &[("Trading Post", "April")]), TradingPost);
        assert_eq!(category(9, &[("In-Game Shop", "")]), Promotion);
        assert_eq!(category(-1, &[("Talebound", "")]), Other);
    }

    #[test]
    fn expansions_from_ids() {
        assert_eq!(expansion_from_id(69), 0);
        assert_eq!(expansion_from_id(129), 1);
        assert_eq!(expansion_from_id(448), 4);
        assert_eq!(expansion_from_id(2599), 10);
        assert_eq!(expansion_from_id(3058), 11);
    }

    #[test]
    fn source_text_without_game_codes() {
        let text = "|cFFFFD200Vendor: |rMagovu|n|cFFFFD200Cost: |r8000|Hcurrency:3316|h|TInterface\\ICONS\\x.blp:0|t|h\n";
        assert_eq!(clean_source(text), "Vendor: Magovu · Cost: 8000");
    }
}
