//! Moteur de wowscan.
//!
//! - [`cache`] : fichiers JSON compressés dans le dossier de cache.
//! - [`client`] : authentification et requêtes vers l'API Blizzard (Game Data).
//! - [`realms`] : liste des royaumes connectés de la région.
//! - [`auctions`] : téléchargement des enchères, réduites à l'essentiel et mises en cache.
//! - [`gamedata`] : tables du jeu (versions des objets, apparences, ensembles), via wago.tools.
//! - [`items`] : noms (fr/en), qualité et icône des objets.
//! - [`scan`] : recherche d'une liste d'objets sur tous les royaumes.
//! - [`optimize`] : choix des royaumes où acheter pour payer le moins cher.
//! - [`search`] : objets retrouvés par leur nom exact (copies du chat sans liens).
//! - [`refs`] : lecture d'identifiants, de liens Wowhead et de liens en jeu.

pub mod auctions;
pub mod cache;
pub mod client;
pub mod error;
pub mod gamedata;
pub mod items;
pub mod model;
pub mod optimize;
pub mod realms;
pub mod refs;
pub mod scan;
pub mod search;

pub use client::{BlizzardClient, Credentials, Region};
pub use error::{Error, Result};
pub use model::{LocalizedString, Money};
