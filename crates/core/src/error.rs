use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("identifiants Blizzard refusés (vérifie le client ID et le secret)")]
    InvalidCredentials,

    #[error("requête HTTP échouée : {0}")]
    Http(#[from] reqwest::Error),

    #[error("l'API Blizzard a répondu {status} pour {url}")]
    Api { status: u16, url: String },

    #[error("réponse JSON invalide pour {url} : {source}")]
    Json {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("cache illisible ({path}) : {source}")]
    Cache {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl Error {
    /// Code stable, que l'interface traduit dans la langue choisie.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "invalid_credentials",
            Self::Http(_) => "network",
            Self::Api { status: 429, .. } => "rate_limited",
            Self::Api { .. } => "api",
            Self::Json { .. } => "bad_response",
            Self::Cache { .. } => "cache",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
