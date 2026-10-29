//! Pont entre l'interface React et le moteur `wowscan-core`.
//!
//! Les identifiants Blizzard sont rangés dans le coffre du système
//! (Gestionnaire d'identification sous Windows), jamais dans un fichier.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{Manager, State};
use tokio::sync::Mutex;
use wowscan_core::cache::Cache;
use wowscan_core::items::{self, ItemInfo};
use wowscan_core::optimize::{self, Plan, PlanInput};
use wowscan_core::realms::{self, ConnectedRealm};
use wowscan_core::refs::{self, ParsedRefs};
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
}

type CommandResult<T> = Result<T, CommandError>;

struct AppState {
    cache: Cache,
    client: Mutex<Option<Arc<BlizzardClient>>>,
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

/// Lance un scan ; l'avancement arrive par `on_event`, le résultat à la fin.
#[tauri::command]
async fn scan_items(
    state: State<'_, AppState>,
    ids: Vec<u32>,
    on_event: Channel<ScanEvent>,
) -> CommandResult<ScanResult> {
    let client = state.client().await?;
    let result = scan::scan(&client, &state.cache, &ids, scan::DEFAULT_CONCURRENCY, |event| {
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
        .setup(|app| {
            let cache_dir: PathBuf = app.path().app_cache_dir()?;
            app.manage(AppState { cache: Cache::new(cache_dir), client: Mutex::new(None) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            has_credentials,
            save_credentials,
            clear_credentials,
            parse_items,
            connected_realms,
            item_infos,
            scan_items,
            plan_purchases,
        ])
        .run(tauri::generate_context!())
        .expect("impossible de démarrer wowscan");
}
