//! `wowscan scan 19019 https://www.wowhead.com/item=2589 --owned 1080`

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand, ValueEnum};
use wowscan_core::cache::Cache;
use wowscan_core::optimize::{self, Candidate, PlanInput, Want};
use wowscan_core::realms::{self, ConnectedRealm};
use wowscan_core::scan::{self, ScanEvent};
use wowscan_core::{BlizzardClient, Credentials, Money, Region, items, refs};

#[derive(Parser)]
#[command(version, about = "Prix de l'hôtel des ventes de WoW sur tous les royaumes européens")]
struct Cli {
    /// Client ID créé sur https://develop.battle.net/access/clients
    #[arg(long, env = "WOWSCAN_CLIENT_ID", hide_env_values = true)]
    client_id: String,
    #[arg(long, env = "WOWSCAN_CLIENT_SECRET", hide_env_values = true)]
    client_secret: String,
    /// Dossier de cache (par défaut : dossier de cache du système / wowscan).
    #[arg(long, env = "WOWSCAN_CACHE_DIR")]
    cache_dir: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = Lang::Fr)]
    lang: Lang,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum Lang {
    Fr,
    En,
}

impl Lang {
    fn locale(self) -> &'static str {
        match self {
            Self::Fr => "fr_FR",
            Self::En => "en_GB",
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Vérifie les identifiants Blizzard.
    Check,
    /// Liste les groupes de royaumes connectés et leur identifiant.
    Realms,
    /// Cherche des objets sur tous les royaumes.
    Scan {
        /// Identifiants, liens Wowhead ou liens en jeu.
        #[arg(required = true)]
        items: Vec<String>,
        /// Royaumes connectés où tu as déjà un personnage (identifiants de `wowscan realms`).
        #[arg(long, value_delimiter = ',')]
        owned: Vec<u32>,
        /// Coût estimé, en or, d'un royaume où il faut créer un personnage.
        #[arg(long, default_value_t = 0)]
        new_realm_cost: u64,
        /// Nombre de royaumes affichés par objet.
        #[arg(long, default_value_t = 5)]
        top: usize,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cache_dir = match cli.cache_dir {
        Some(dir) => dir,
        None => dirs::cache_dir().context("dossier de cache introuvable, utilise --cache-dir")?.join("wowscan"),
    };
    let cache = Cache::new(cache_dir);
    let client = BlizzardClient::new(
        Credentials { client_id: cli.client_id, client_secret: cli.client_secret },
        Region::Eu,
    )?;
    let locale = cli.lang.locale();

    match cli.command {
        Command::Check => {
            client.check_credentials().await?;
            println!("Identifiants valides.");
        }
        Command::Realms => {
            for realm in realms::connected_realms(&client, &cache).await? {
                println!("{:>5}  {}", realm.id, realm.display_name(locale));
            }
        }
        Command::Scan { items: inputs, owned, new_realm_cost, top } => {
            let parsed = refs::parse_item_refs(&inputs.join(" "));
            for token in &parsed.unrecognized {
                eprintln!("Ignoré (format inconnu) : {token}");
            }
            if parsed.item_ids.is_empty() {
                bail!("aucun objet reconnu");
            }

            let infos = items::item_infos(&client, &cache, &parsed.item_ids).await?;
            let result = scan::scan(&client, &cache, &parsed.item_ids, scan::DEFAULT_CONCURRENCY, progress).await?;
            eprintln!();

            let names: HashMap<u32, &ConnectedRealm> = result.realms.iter().map(|r| (r.id, r)).collect();
            let realm_name = |id: u32| names.get(&id).map_or_else(|| id.to_string(), |r| r.display_name(locale));
            for failure in &result.failures {
                let source = failure.realm_id.map_or("marché régional".to_owned(), realm_name);
                eprintln!("Échec ({source}) : {}", failure.error);
            }

            let owned: HashSet<u32> = owned.into_iter().collect();
            let mut wants = Vec::new();
            for item in &result.items {
                let name = infos.get(&item.item_id).map_or("objet inconnu", |i| i.name.get(locale));
                println!("\n{name} ({})", item.item_id);
                if let Some(stats) = item.commodity {
                    println!("  objet empilable, même prix partout : {} ({} en vente)", stats.min, stats.quantity);
                    continue;
                }
                if item.realms.is_empty() {
                    println!("  aucune enchère");
                }
                for realm in item.realms.iter().take(top) {
                    let mark = if owned.contains(&realm.connected_realm_id) { "*" } else { " " };
                    println!(
                        " {mark}{:>18}  {}  ({} enchère(s))",
                        realm.stats.min.to_string(),
                        realm_name(realm.connected_realm_id),
                        realm.stats.auctions
                    );
                }
                wants.push(Want {
                    item_id: item.item_id,
                    candidates: item
                        .realms
                        .iter()
                        .map(|r| Candidate { connected_realm_id: r.connected_realm_id, price: r.stats.min })
                        .collect(),
                });
            }

            if wants.len() > 1 {
                let plan = optimize::plan(&PlanInput {
                    wants,
                    owned_realms: owned,
                    new_realm_cost: Money::from_gold(new_realm_cost),
                });
                println!("\nListe d'achat optimisée : {} au total", plan.total);
                for realm in &plan.realms {
                    let new = if plan.new_realms.contains(realm) { " (nouveau personnage)" } else { "" };
                    println!("  {}{new}", realm_name(*realm));
                    for p in plan.purchases.iter().filter(|p| p.connected_realm_id == *realm) {
                        let name = infos.get(&p.item_id).map_or("?", |i| i.name.get(locale));
                        println!("    {:>18}  {name}", p.price.to_string());
                    }
                }
            }
        }
    }
    Ok(())
}

fn progress(event: ScanEvent) {
    if let ScanEvent::SourceDone { done, total, .. } | ScanEvent::SourceFailed { done, total, .. } = event {
        eprint!("\rRoyaumes lus : {done}/{total}");
        let _ = std::io::stderr().flush();
    }
}
