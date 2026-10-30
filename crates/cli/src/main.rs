//! `wowscan scan 19019 https://www.wowhead.com/item=2589 --owned 1080`

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand, ValueEnum};
use wowscan_core::cache::Cache;
use wowscan_core::gamedata::GameData;
use wowscan_core::optimize::{self, Candidate, PlanInput, Want};
use wowscan_core::realms::{self, ConnectedRealm};
use wowscan_core::scan::{self, ScanEvent, VariantResult};
use wowscan_core::{BlizzardClient, Credentials, Money, Region, items, refs, search};

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
    /// Cherche un ensemble de transmogrification par son nom (français ou anglais).
    Sets {
        #[arg(required = true)]
        query: Vec<String>,
    },
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
        /// Écrit le résultat brut en JSON (celui que reçoit l'interface) au lieu du tableau.
        #[arg(long)]
        json: bool,
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
        Command::Sets { query } => {
            let data = GameData::load(&cache).await?;
            let label = |id: Option<u32>| {
                id.and_then(|id| data.label(id)).map_or(String::new(), |l| format!(" [{}]", l.get(locale)))
            };
            for set in data.search_sets(&query.join(" "), 20) {
                println!("{:>6}  {}{}  ({} pièces)", set.id, set.name.get(locale), label(set.label), set.pieces.len());
                for piece in &set.pieces {
                    println!("          objet {} · apparence {}", piece.item_id, piece.appearance_id);
                }
            }
        }
        Command::Scan { items: inputs, owned, new_realm_cost, top, json } => {
            let mut parsed = refs::parse_item_refs(&inputs.join(" "));
            for token in &parsed.unrecognized {
                eprintln!("Ignoré (format inconnu) : {token}");
            }
            // Noms entre crochets sans lien (copie du chat) : retrouvés par leur nom exact.
            if !parsed.names.is_empty() {
                let found = search::find_items_by_name(&client, &cache, &parsed.names).await?;
                for name in &parsed.names {
                    match found.get(name).map(Vec::as_slice) {
                        Some([]) | None => eprintln!("Aucun objet nommé « {name} »"),
                        Some(ids) => parsed.items.extend(ids.iter().map(|&item_id| refs::ItemRef { item_id, bonus_lists: Vec::new() })),
                    }
                }
            }
            if parsed.items.is_empty() {
                bail!("aucun objet reconnu");
            }
            let item_ids = parsed.item_ids();

            // Sans les tables du jeu, le scan marche encore mais mélange les versions.
            let data = GameData::load(&cache)
                .await
                .inspect_err(|err| eprintln!("Tables du jeu indisponibles, versions non distinguées : {err}"))
                .ok();
            let infos = items::item_infos(&client, &cache, &item_ids).await?;
            let result =
                scan::scan(&client, &cache, &item_ids, data.as_ref(), scan::DEFAULT_CONCURRENCY, progress).await?;
            eprintln!();
            if json {
                println!("{}", serde_json::to_string(&result)?);
                return Ok(());
            }

            let names: HashMap<u32, &ConnectedRealm> = result.realms.iter().map(|r| (r.id, r)).collect();
            let realm_name = |id: u32| names.get(&id).map_or_else(|| id.to_string(), |r| r.display_name(locale));
            for failure in &result.failures {
                let source = failure.realm_id.map_or("marché régional".to_owned(), realm_name);
                eprintln!("Échec ({source}) : {}", failure.error);
            }
            let label = |variant: &VariantResult| {
                let labels: Vec<&str> =
                    variant.labels.iter().filter_map(|l| data.as_ref()?.label(*l)).map(|l| l.get(locale)).collect();
                let appearance = variant.appearance_id.map_or(String::new(), |a| format!("apparence {a}"));
                [labels.join(" · "), appearance].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
            };

            let owned: HashSet<u32> = owned.into_iter().collect();
            let mut wants = Vec::new();
            let mut want_names = Vec::new();
            for wanted in &parsed.items {
                let item = result.items.iter().find(|i| i.item_id == wanted.item_id).expect("objet scanné");
                let name = infos.get(&item.item_id).map_or("objet inconnu", |i| i.name.get(locale));
                // Version demandée (bonus d'un lien Wowhead ou en jeu) : seule elle compte.
                let requested = (!wanted.bonus_lists.is_empty())
                    .then(|| data.as_ref().and_then(|d| d.variant(item.item_id, &wanted.bonus_lists).appearance_id))
                    .flatten();
                println!("\n{name} ({}){}", item.item_id, requested.map_or(String::new(), |a| format!(" — version demandée : apparence {a}")));
                if item.variants.is_empty() {
                    println!("  aucune enchère");
                }
                for variant in &item.variants {
                    let mark = if requested.is_some() && variant.appearance_id == requested { "→" } else { " " };
                    println!(" {mark} {}", label(variant));
                    if let Some(stats) = variant.commodity {
                        println!("     objet empilable, même prix partout : {} ({} en vente)", stats.min, stats.quantity);
                        continue;
                    }
                    for realm in variant.realms.iter().take(top) {
                        let owned_mark = if owned.contains(&realm.connected_realm_id) { "*" } else { " " };
                        println!(
                            "    {owned_mark}{:>18}  {}  ({} enchère(s))",
                            realm.stats.min.to_string(),
                            realm_name(realm.connected_realm_id),
                            realm.stats.auctions
                        );
                    }
                }

                let chosen = match requested {
                    Some(appearance) => item.variants.iter().find(|v| v.appearance_id == Some(appearance)),
                    None => item.variants.iter().find(|v| v.commodity.is_none()),
                };
                if let Some(variant) = chosen.filter(|v| v.commodity.is_none()) {
                    want_names.push(format!("{name} ({})", label(variant)));
                    wants.push(Want {
                        // Clé de la ligne d'achat : son rang (un objet peut être demandé en deux versions).
                        item_id: wants.len() as u32,
                        candidates: variant
                            .realms
                            .iter()
                            .map(|r| Candidate { connected_realm_id: r.connected_realm_id, price: r.stats.min })
                            .collect(),
                    });
                }
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
                        println!("    {:>18}  {}", p.price.to_string(), want_names[p.item_id as usize]);
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
