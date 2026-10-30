//! Pont entre l'interface React et le moteur `wowscan-core`.
//!
//! Les identifiants Blizzard sont rangés dans le coffre du système
//! (Gestionnaire d'identification sous Windows), jamais dans un fichier.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{Manager, State};
use tokio::sync::Mutex;
use wowscan_core::cache::Cache;
use wowscan_core::collection::{self, AddonState, Character, SetProgress};
use wowscan_core::gamedata::{BossDrop, GameData, Source, Variant};
use wowscan_core::items::{self, ItemInfo};
use wowscan_core::optimize::{self, Plan, PlanInput};
use wowscan_core::realms::{self, ConnectedRealm};
use wowscan_core::model::LocalizedString;
use wowscan_core::refs::{self, ItemRef, ParsedRefs};
use wowscan_core::scan::{self, ScanEvent, ScanResult};
use wowscan_core::{BlizzardClient, Credentials, Region};

const KEYRING_SERVICE: &str = "wowscan";
const KEYRING_USER: &str = "blizzard-api";

/// Erreur envoyée à l'interface : un code à traduire, et le détail technique.
#[derive(Debug, Serialize)]
struct CommandError {
    code: &'static str,
    message: String,
}

impl From<wowscan_core::Error> for CommandError {
    fn from(err: wowscan_core::Error) -> Self {
        Self { code: err.code(), message: err.to_string() }
    }
}

impl From<keyring::Error> for CommandError {
    fn from(err: keyring::Error) -> Self {
        Self { code: "keyring", message: err.to_string() }
    }
}

impl CommandError {
    fn no_credentials() -> Self {
        Self { code: "no_credentials", message: "identifiants Blizzard non configurés".into() }
    }

    fn game_not_found() -> Self {
        Self { code: "game_not_found", message: "dossier de World of Warcraft introuvable".into() }
    }
}

type CommandResult<T> = Result<T, CommandError>;

struct AppState {
    cache: Cache,
    client: Mutex<Option<Arc<BlizzardClient>>>,
    gamedata: Mutex<Option<Arc<GameData>>>,
}

impl AppState {
    /// Client prêt à l'emploi, créé à partir du coffre au premier appel.
    async fn client(&self) -> CommandResult<Arc<BlizzardClient>> {
        let mut client = self.client.lock().await;
        if let Some(client) = client.as_ref() {
            return Ok(client.clone());
        }
        let credentials = load_credentials()?.ok_or_else(CommandError::no_credentials)?;
        let created = Arc::new(BlizzardClient::new(credentials, Region::Eu)?);
        *client = Some(created.clone());
        Ok(created)
    }
}

impl AppState {
    /// Tables du jeu, chargées au premier besoin (téléchargées une fois par semaine).
    async fn gamedata(&self) -> CommandResult<Arc<GameData>> {
        let mut data = self.gamedata.lock().await;
        if let Some(data) = data.as_ref() {
            return Ok(data.clone());
        }
        let loaded = Arc::new(GameData::load(&self.cache).await?);
        *data = Some(loaded.clone());
        Ok(loaded)
    }
}

fn keyring_entry() -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
}

fn load_credentials() -> CommandResult<Option<Credentials>> {
    match keyring_entry()?.get_password() {
        // Format stocké : « client_id\nclient_secret ».
        Ok(stored) => Ok(stored.split_once('\n').map(|(id, secret)| Credentials {
            client_id: id.to_owned(),
            client_secret: secret.to_owned(),
        })),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

#[tauri::command]
fn has_credentials() -> CommandResult<bool> {
    Ok(load_credentials()?.is_some())
}

/// Vérifie les identifiants auprès de Blizzard avant de les enregistrer.
#[tauri::command]
async fn save_credentials(
    state: State<'_, AppState>,
    client_id: String,
    client_secret: String,
) -> CommandResult<()> {
    let credentials = Credentials {
        client_id: client_id.trim().to_owned(),
        client_secret: client_secret.trim().to_owned(),
    };
    let client = BlizzardClient::new(credentials.clone(), Region::Eu)?;
    client.check_credentials().await?;

    keyring_entry()?.set_password(&format!("{}\n{}", credentials.client_id, credentials.client_secret))?;
    *state.client.lock().await = Some(Arc::new(client));
    Ok(())
}

#[tauri::command]
async fn clear_credentials(state: State<'_, AppState>) -> CommandResult<()> {
    match keyring_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(err) => return Err(err.into()),
    }
    *state.client.lock().await = None;
    Ok(())
}

#[tauri::command]
fn parse_items(input: String) -> ParsedRefs {
    refs::parse_item_refs(&input)
}

#[tauri::command]
async fn connected_realms(state: State<'_, AppState>) -> CommandResult<Vec<ConnectedRealm>> {
    let client = state.client().await?;
    Ok(realms::connected_realms(&client, &state.cache).await?)
}

#[tauri::command]
async fn item_infos(state: State<'_, AppState>, ids: Vec<u32>) -> CommandResult<HashMap<u32, ItemInfo>> {
    let client = state.client().await?;
    Ok(items::item_infos(&client, &state.cache, &ids).await?)
}

/// Objets dont le nom correspond exactement, pour les copies du chat sans liens.
#[tauri::command]
async fn find_items_by_name(
    state: State<'_, AppState>,
    names: Vec<String>,
) -> CommandResult<HashMap<String, Vec<u32>>> {
    let client = state.client().await?;
    Ok(wowscan_core::search::find_items_by_name(&client, &state.cache, &names).await?)
}

/// Versions demandées (apparence et libellés) pour des objets saisis avec leurs bonus.
#[tauri::command]
async fn resolve_variants(state: State<'_, AppState>, items: Vec<ItemRef>) -> CommandResult<Vec<Variant>> {
    let data = state.gamedata().await?;
    Ok(items.iter().map(|i| data.variant(i.item_id, &i.bonus_lists)).collect())
}

/// Libellés de version (« Mythique »…), pour les afficher dans la langue choisie.
#[tauri::command]
async fn variant_labels(state: State<'_, AppState>) -> CommandResult<HashMap<u32, LocalizedString>> {
    Ok(state.gamedata().await?.labels().clone())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetSummary {
    id: u32,
    name: LocalizedString,
    label: Option<u32>,
    class_mask: u32,
    expansion: u32,
    /// Nombre d'apparences distinctes (une par emplacement, en général).
    appearances: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetPiece {
    appearance_id: u32,
    /// Emplacement (`InventoryType` du jeu), s'il est connu.
    slot: Option<u8>,
    /// Objets de l'ensemble qui donnent cette apparence (versions Alliance/Horde…).
    set_sources: Vec<Source>,
    /// Tous les objets du jeu qui donnent cette apparence, ensemble ou non, avec leur origine.
    all_sources: Vec<PieceSource>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PieceSource {
    #[serde(flatten)]
    source: Source,
    /// Origine (`TransmogSourceTypeEnum` : 1 boss, 2 quête, 3 marchand, 4 butin du monde…).
    kind: u8,
    /// Boss qui laissent tomber l'objet.
    drops: Vec<BossDrop>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetDetails {
    #[serde(flatten)]
    summary: SetSummary,
    pieces: Vec<SetPiece>,
}

fn summary(set: &wowscan_core::gamedata::TransmogSet) -> SetSummary {
    let mut appearances: Vec<u32> = set.pieces.iter().map(|p| p.appearance_id).collect();
    appearances.sort_unstable();
    appearances.dedup();
    SetSummary {
        id: set.id,
        name: set.name.clone(),
        label: set.label,
        class_mask: set.class_mask,
        expansion: set.expansion,
        appearances: appearances.len(),
    }
}

#[tauri::command]
async fn search_sets(state: State<'_, AppState>, query: String) -> CommandResult<Vec<SetSummary>> {
    let data = state.gamedata().await?;
    Ok(data.search_sets(&query, 60).into_iter().map(summary).collect())
}

#[tauri::command]
async fn set_details(state: State<'_, AppState>, id: u32) -> CommandResult<Option<SetDetails>> {
    let data = state.gamedata().await?;
    let Some(set) = data.set(id) else { return Ok(None) };
    let mut pieces: Vec<SetPiece> = Vec::new();
    for source in &set.pieces {
        match pieces.iter_mut().find(|p| p.appearance_id == source.appearance_id) {
            Some(piece) => piece.set_sources.push(*source),
            None => pieces.push(SetPiece {
                appearance_id: source.appearance_id,
                slot: data.slot(source.item_id),
                set_sources: vec![*source],
                all_sources: data
                    .sources(source.appearance_id)
                    .iter()
                    .map(|&(item_id, modifier)| PieceSource {
                        source: Source { item_id, modifier, appearance_id: source.appearance_id },
                        kind: data.source_kind(item_id, modifier),
                        drops: data.drops(item_id),
                    })
                    .collect(),
            }),
        }
    }
    Ok(Some(SetDetails { summary: summary(set), pieces }))
}

/// Objets qui donnent chacune de ces apparences (pour scanner les pièces manquantes).
#[tauri::command]
async fn appearance_items(state: State<'_, AppState>, ids: Vec<u32>) -> CommandResult<HashMap<u32, Vec<u32>>> {
    let data = state.gamedata().await?;
    Ok(ids
        .into_iter()
        .map(|id| {
            let mut items: Vec<u32> = data.sources(id).iter().map(|(item, _)| *item).collect();
            items.dedup();
            (id, items)
        })
        .collect())
}

/// État de la collection : dossier du jeu, addon et dernier export lu.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CollectionStatus {
    game_dir: Option<PathBuf>,
    addon: Option<AddonState>,
    /// Date de l'analyse en jeu (secondes Unix), si l'addon a déjà exporté.
    scanned_at: Option<u64>,
    appearances: usize,
    characters: Vec<Character>,
}

fn game_dir(hint: Option<String>) -> Option<PathBuf> {
    collection::find_game_dir(hint.as_deref().map(Path::new))
}

fn collection_status_in(dir: Option<PathBuf>) -> CommandResult<CollectionStatus> {
    let Some(dir) = dir else {
        return Ok(CollectionStatus { game_dir: None, addon: None, scanned_at: None, appearances: 0, characters: Vec::new() });
    };
    let export = collection::read(&dir)?;
    Ok(CollectionStatus {
        addon: Some(collection::addon_state(&dir)),
        scanned_at: export.as_ref().map(|e| e.scanned_at),
        appearances: export.as_ref().map_or(0, |e| e.appearances.len()),
        characters: export.map(|e| e.characters).unwrap_or_default(),
        game_dir: Some(dir),
    })
}

#[tauri::command]
fn collection_status(game_dir: Option<String>) -> CommandResult<CollectionStatus> {
    collection_status_in(self::game_dir(game_dir))
}

#[tauri::command]
fn install_addon(game_dir: Option<String>) -> CommandResult<CollectionStatus> {
    let dir = self::game_dir(game_dir).ok_or_else(CommandError::game_not_found)?;
    collection::install_addon(&dir)?;
    collection_status_in(Some(dir))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetCollection {
    #[serde(flatten)]
    summary: SetSummary,
    #[serde(flatten)]
    progress: SetProgress,
}

/// Progression de chaque ensemble d'après le dernier export de l'addon.
#[tauri::command]
async fn collection_sets(state: State<'_, AppState>, game_dir: Option<String>) -> CommandResult<Vec<SetCollection>> {
    let dir = self::game_dir(game_dir).ok_or_else(CommandError::game_not_found)?;
    let collected = collection::read(&dir)?.map(|e| e.appearances).unwrap_or_default();
    let data = state.gamedata().await?;
    Ok(data
        .sets()
        .iter()
        .filter(|s| !s.pieces.is_empty())
        .map(|set| SetCollection { summary: summary(set), progress: collection::set_progress(&data, set, &collected) })
        .collect())
}

/// Lance un scan ; l'avancement arrive par `on_event`, le résultat à la fin.
#[tauri::command]
async fn scan_items(
    state: State<'_, AppState>,
    ids: Vec<u32>,
    on_event: Channel<ScanEvent>,
) -> CommandResult<ScanResult> {
    let client = state.client().await?;
    // Sans les tables du jeu (wago.tools injoignable), on scanne quand même, versions mêlées.
    let data = state.gamedata().await.ok();
    let result = scan::scan(&client, &state.cache, &ids, data.as_deref(), scan::DEFAULT_CONCURRENCY, |event| {
        // Fenêtre fermée pendant le scan : on continue sans afficher la progression.
        let _ = on_event.send(event);
    })
    .await?;
    Ok(result)
}

#[tauri::command]
fn plan_purchases(input: PlanInput) -> Plan {
    optimize::plan(&input)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let cache_dir: PathBuf = app.path().app_cache_dir()?;
            app.manage(AppState { cache: Cache::new(cache_dir), client: Mutex::new(None), gamedata: Mutex::new(None) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            has_credentials,
            save_credentials,
            clear_credentials,
            parse_items,
            find_items_by_name,
            resolve_variants,
            variant_labels,
            search_sets,
            set_details,
            appearance_items,
            collection_status,
            install_addon,
            collection_sets,
            connected_realms,
            item_infos,
            scan_items,
            plan_purchases,
        ])
        .run(tauri::generate_context!())
        .expect("impossible de démarrer wowscan");
}
