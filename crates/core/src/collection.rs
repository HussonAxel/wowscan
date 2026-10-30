//! Collection du compte : apparences déjà débloquées et progression des ensembles.
//!
//! L'API publique de Blizzard ne donne pas les apparences d'un compte sans connexion
//! Battle.net : l'addon WowScan (dossier `addon/` du dépôt) les parcourt en jeu et les
//! écrit dans ses SavedVariables (`WTF/Account/<compte>/SavedVariables/WowScan.lua`),
//! que le jeu enregistre au /reload ou à la déconnexion. Ce module retrouve le dossier
//! du jeu, installe l'addon et lit ce fichier.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::UNIX_EPOCH;

use regex::Regex;
use serde::Serialize;

use crate::gamedata::{GameData, TransmogSet};
use crate::{Error, Result};

const ADDON_NAME: &str = "WowScan";
const ADDON_FILES: [(&str, &str); 2] = [
    ("WowScan.toc", include_str!("../../../addon/WowScan/WowScan.toc")),
    ("WowScan.lua", include_str!("../../../addon/WowScan/WowScan.lua")),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    pub name: String,
    pub realm: String,
    /// Identifiant de classe du jeu (1 Guerrier … 13 Évocateur).
    pub class_id: u8,
}

#[derive(Debug, Clone)]
pub struct Collection {
    pub file: PathBuf,
    /// Date de l'analyse en jeu (secondes Unix).
    pub scanned_at: u64,
    pub appearances: HashSet<u32>,
    pub characters: Vec<Character>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AddonState {
    Missing,
    Outdated,
    Installed,
}

/// Progression d'un ensemble, comptée par emplacement comme dans le jeu : un emplacement
/// est acquis dès qu'une de ses pièces (ou la même apparence venue d'ailleurs) l'est.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetProgress {
    pub collected: u32,
    pub total: u32,
    /// Apparences des pièces des emplacements manquants.
    pub missing: Vec<u32>,
}

static SCANNED_AT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\["scannedAt"\]\s*=\s*(\d+)"#).unwrap());
static APPEARANCES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\["appearances"\]\s*=\s*"([0-9,]*)""#).unwrap());
static CHARACTERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\["characters"\]\s*=\s*\{([^}]*)\}"#).unwrap());
static CHARACTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\["([^"]+)-([^"-]+)"\]\s*=\s*(\d+)"#).unwrap());

/// Lit le contenu de `WowScan.lua` ; `None` si l'addon n'a encore rien exporté.
pub fn parse(text: &str) -> Option<(u64, HashSet<u32>, Vec<Character>)> {
    let appearances = APPEARANCES.captures(text)?[1].split(',').filter_map(|id| id.parse().ok()).collect();
    let scanned_at = SCANNED_AT.captures(text).and_then(|c| c[1].parse().ok()).unwrap_or(0);
    let mut characters: Vec<Character> = CHARACTERS
        .captures(text)
        .map(|block| {
            CHARACTER
                .captures_iter(&block[1])
                .filter_map(|c| {
                    Some(Character { name: c[1].to_owned(), realm: c[2].to_owned(), class_id: c[3].parse().ok()? })
                })
                .collect()
        })
        .unwrap_or_default();
    characters.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.realm.cmp(&b.realm)));
    Some((scanned_at, appearances, characters))
}

/// Dossier `_retail_` du jeu : celui indiqué (racine ou `_retail_`), sinon les emplacements habituels.
pub fn find_game_dir(hint: Option<&Path>) -> Option<PathBuf> {
    if let Some(hint) = hint.filter(|h| !h.as_os_str().is_empty()) {
        return retail_dir(hint);
    }
    let mut candidates = Vec::new();
    for drive in b'C'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\", drive as char));
        for sub in ["Program Files (x86)", "Program Files", "", "Games", "Jeux", "Blizzard", "Battle.net"] {
            candidates.push(root.join(sub).join("World of Warcraft"));
        }
    }
    candidates.push(PathBuf::from("/Applications/World of Warcraft"));
    candidates.iter().find_map(|c| retail_dir(c))
}

fn retail_dir(path: &Path) -> Option<PathBuf> {
    let is_game = |p: &Path| p.join("WTF").is_dir() || p.join("Wow.exe").is_file() || p.join("World of Warcraft.app").exists();
    let retail = path.join("_retail_");
    if is_game(&retail) {
        Some(retail)
    } else if is_game(path) {
        Some(path.to_path_buf())
    } else {
        None
    }
}

/// Export le plus récent parmi les comptes du jeu.
pub fn read(game_dir: &Path) -> Result<Option<Collection>> {
    let accounts = game_dir.join("WTF").join("Account");
    let Ok(entries) = std::fs::read_dir(&accounts) else { return Ok(None) };
    let latest = entries
        .flatten()
        .map(|e| e.path().join("SavedVariables").join(format!("{ADDON_NAME}.lua")))
        .filter_map(|file| {
            let modified = file.metadata().ok()?.modified().ok()?;
            Some((modified, file))
        })
        .max_by_key(|(modified, _)| *modified);
    let Some((modified, file)) = latest else { return Ok(None) };
    let text = std::fs::read_to_string(&file).map_err(|source| Error::Game { path: file.clone(), source })?;
    Ok(parse(&text).map(|(scanned_at, appearances, characters)| Collection {
        scanned_at: if scanned_at > 0 {
            scanned_at
        } else {
            modified.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
        },
        file,
        appearances,
        characters,
    }))
}

fn addon_dir(game_dir: &Path) -> PathBuf {
    game_dir.join("Interface").join("AddOns").join(ADDON_NAME)
}

pub fn addon_state(game_dir: &Path) -> AddonState {
    let dir = addon_dir(game_dir);
    if !dir.join("WowScan.toc").is_file() {
        return AddonState::Missing;
    }
    let same = |name: &str, content: &str| {
        std::fs::read_to_string(dir.join(name)).is_ok_and(|on_disk| on_disk.replace("\r\n", "\n") == content.replace("\r\n", "\n"))
    };
    if ADDON_FILES.iter().all(|(name, content)| same(name, content)) {
        AddonState::Installed
    } else {
        AddonState::Outdated
    }
}

/// Copie l'addon dans `Interface/AddOns/WowScan` (le remplace s'il existe déjà).
pub fn install_addon(game_dir: &Path) -> Result<PathBuf> {
    let dir = addon_dir(game_dir);
    std::fs::create_dir_all(&dir).map_err(|source| Error::Game { path: dir.clone(), source })?;
    for (name, content) in ADDON_FILES {
        let path = dir.join(name);
        std::fs::write(&path, content).map_err(|source| Error::Game { path, source })?;
    }
    Ok(dir)
}

/// Emplacement d'une pièce : robe et torse comptent pour le même.
fn slot_key(data: &GameData, item_id: u32, appearance_id: u32) -> (u8, u32) {
    match data.slot(item_id) {
        Some(20) => (5, 0),
        Some(slot) => (slot, 0),
        None => (0, appearance_id),
    }
}

pub fn set_progress(data: &GameData, set: &TransmogSet, collected: &HashSet<u32>) -> SetProgress {
    let mut slots: BTreeMap<(u8, u32), Vec<u32>> = BTreeMap::new();
    for piece in &set.pieces {
        let list = slots.entry(slot_key(data, piece.item_id, piece.appearance_id)).or_default();
        if !list.contains(&piece.appearance_id) {
            list.push(piece.appearance_id);
        }
    }
    let mut progress = SetProgress { collected: 0, total: slots.len() as u32, missing: Vec::new() };
    for appearances in slots.values() {
        if appearances.iter().any(|a| collected.contains(a)) {
            progress.collected += 1;
        } else {
            progress.missing.extend(appearances);
        }
    }
    progress
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gamedata::tests::sample;

    const SAVED: &str = r#"
WowScanDB = {
["count"] = 3,
["scannedAt"] = 1761800000,
["characters"] = {
["Ayije-Elune"] = 3,
["Funel-Kirin Tor"] = 13,
},
["version"] = 1,
["appearances"] = "25447,25526,40000",
}
"#;

    #[test]
    fn parses_saved_variables() {
        let (scanned_at, appearances, characters) = parse(SAVED).unwrap();
        assert_eq!(scanned_at, 1_761_800_000);
        assert_eq!(appearances, HashSet::from([25447, 25526, 40000]));
        assert_eq!(
            characters,
            vec![
                Character { name: "Ayije".into(), realm: "Elune".into(), class_id: 3 },
                Character { name: "Funel".into(), realm: "Kirin Tor".into(), class_id: 13 },
            ]
        );
        assert!(parse("WowScanDB = {}").is_none());
        assert_eq!(parse(r#"["appearances"] = """#).unwrap().1.len(), 0);
    }

    #[test]
    fn progress_by_slot() {
        let data = sample();
        let set = data.set(13).unwrap();
        let progress = set_progress(&data, set, &HashSet::from([25526]));
        assert_eq!(progress, SetProgress { collected: 1, total: 2, missing: vec![30000] });
        assert_eq!(set_progress(&data, set, &HashSet::new()).missing, vec![30000, 25526], "ordered by slot");
        assert_eq!(set_progress(&data, set, &HashSet::from([25526, 30000])).collected, 2);
    }

    #[test]
    fn installs_and_reads_from_game_folder() {
        let root = tempfile::tempdir().unwrap();
        let retail = root.path().join("_retail_");
        std::fs::create_dir_all(retail.join("WTF/Account/1#1/SavedVariables")).unwrap();
        assert_eq!(find_game_dir(Some(root.path())), Some(retail.clone()));
        assert_eq!(find_game_dir(Some(&retail)), Some(retail.clone()));
        assert!(read(&retail).unwrap().is_none());

        assert_eq!(addon_state(&retail), AddonState::Missing);
        install_addon(&retail).unwrap();
        assert_eq!(addon_state(&retail), AddonState::Installed);
        std::fs::write(retail.join("Interface/AddOns/WowScan/WowScan.lua"), "-- old").unwrap();
        assert_eq!(addon_state(&retail), AddonState::Outdated);

        std::fs::write(retail.join("WTF/Account/1#1/SavedVariables/WowScan.lua"), SAVED).unwrap();
        let collection = read(&retail).unwrap().unwrap();
        assert_eq!((collection.appearances.len(), collection.characters.len()), (3, 2));
    }
}
