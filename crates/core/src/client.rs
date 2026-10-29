use std::time::{Duration, Instant};

use reqwest::{StatusCode, header};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

use crate::{Error, Result};

/// Région de jeu. Seule l'Europe est gérée pour l'instant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Region {
    #[default]
    Eu,
}

impl Region {
    pub fn api_base(self) -> &'static str {
        match self {
            Self::Eu => "https://eu.api.blizzard.com",
        }
    }

    pub fn dynamic_namespace(self) -> &'static str {
        match self {
            Self::Eu => "dynamic-eu",
        }
    }

    pub fn static_namespace(self) -> &'static str {
        match self {
            Self::Eu => "static-eu",
        }
    }
}

const OAUTH_URL: &str = "https://oauth.battle.net/token";
const MAX_ATTEMPTS: u32 = 4;

/// Identifiants d'un client créé sur <https://develop.battle.net/access/clients>.
#[derive(Debug, Clone)]
pub struct Credentials {
    pub client_id: String,
    pub client_secret: String,
}

/// Espace de noms de l'API : `dynamic` (enchères, royaumes) ou `static` (objets).
#[derive(Debug, Clone, Copy)]
pub enum Namespace {
    Dynamic,
    Static,
}

/// Résultat d'une requête conditionnelle (`If-Modified-Since`).
#[derive(Debug)]
pub enum Fetched<T> {
    Fresh { value: T, last_modified: Option<String> },
    NotModified,
}

struct Token {
    value: String,
    expires_at: Instant,
}

pub struct BlizzardClient {
    http: reqwest::Client,
    credentials: Credentials,
    region: Region,
    api_base: String,
    oauth_url: String,
    token: Mutex<Option<Token>>,
}

impl BlizzardClient {
    pub fn new(credentials: Credentials, region: Region) -> Result<Self> {
        Self::with_urls(credentials, region, region.api_base(), OAUTH_URL)
    }

    /// Variante utilisée par les tests pour viser un faux serveur.
    pub fn with_urls(
        credentials: Credentials,
        region: Region,
        api_base: &str,
        oauth_url: &str,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("wowscan/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .build()?;
        Ok(Self {
            http,
            credentials,
            region,
            api_base: api_base.trim_end_matches('/').to_owned(),
            oauth_url: oauth_url.to_owned(),
            token: Mutex::new(None),
        })
    }

    pub fn region(&self) -> Region {
        self.region
    }

    /// Vérifie les identifiants en demandant un jeton.
    pub async fn check_credentials(&self) -> Result<()> {
        self.token(true).await.map(|_| ())
    }

    async fn token(&self, force_refresh: bool) -> Result<String> {
        let mut guard = self.token.lock().await;
        if let Some(token) = guard.as_ref()
            && !force_refresh
            && token.expires_at > Instant::now()
        {
            return Ok(token.value.clone());
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: u64,
        }

        let response = self
            .http
            .post(&self.oauth_url)
            .basic_auth(&self.credentials.client_id, Some(&self.credentials.client_secret))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await?;
        match response.status() {
            StatusCode::UNAUTHORIZED | StatusCode::BAD_REQUEST | StatusCode::FORBIDDEN => {
                return Err(Error::InvalidCredentials);
            }
            status if !status.is_success() => {
                return Err(Error::Api { status: status.as_u16(), url: self.oauth_url.clone() });
            }
            _ => {}
        }
        let body: TokenResponse = response.json().await?;
        // Marge d'une minute pour ne jamais envoyer un jeton qui expire en route.
        let lifetime = Duration::from_secs(body.expires_in.saturating_sub(60));
        *guard = Some(Token { value: body.access_token.clone(), expires_at: Instant::now() + lifetime });
        Ok(body.access_token)
    }

    /// `GET` JSON sur l'API, avec nouvel essai en cas de 429 / 5xx et de jeton expiré.
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        namespace: Namespace,
        if_modified_since: Option<&str>,
    ) -> Result<Fetched<T>> {
        let url = format!("{}{}", self.api_base, path);
        let namespace = match namespace {
            Namespace::Dynamic => self.region.dynamic_namespace(),
            Namespace::Static => self.region.static_namespace(),
        };
        let mut refreshed_token = false;

        for attempt in 0..MAX_ATTEMPTS {
            let token = self.token(false).await?;
            let mut request = self
                .http
                .get(&url)
                .bearer_auth(&token)
                .header("Battlenet-Namespace", namespace);
            if let Some(since) = if_modified_since {
                request = request.header(header::IF_MODIFIED_SINCE, since);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(err) if attempt + 1 < MAX_ATTEMPTS && (err.is_timeout() || err.is_connect()) => {
                    backoff(attempt).await;
                    continue;
                }
                Err(err) => return Err(err.into()),
            };

            match response.status() {
                StatusCode::NOT_MODIFIED => return Ok(Fetched::NotModified),
                StatusCode::UNAUTHORIZED if !refreshed_token => {
                    refreshed_token = true;
                    self.token(true).await?;
                }
                status if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() => {
                    if attempt + 1 == MAX_ATTEMPTS {
                        return Err(Error::Api { status: status.as_u16(), url });
                    }
                    backoff(attempt).await;
                }
                status if !status.is_success() => {
                    return Err(Error::Api { status: status.as_u16(), url });
                }
                _ => {
                    let last_modified = response
                        .headers()
                        .get(header::LAST_MODIFIED)
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned);
                    let bytes = response.bytes().await?;
                    let value = serde_json::from_slice(&bytes)
                        .map_err(|source| Error::Json { url: url.clone(), source })?;
                    return Ok(Fetched::Fresh { value, last_modified });
                }
            }
        }
        Err(Error::Api { status: StatusCode::UNAUTHORIZED.as_u16(), url })
    }

    /// Comme [`get_json`](Self::get_json), sans requête conditionnelle.
    pub async fn get<T: DeserializeOwned>(&self, path: &str, namespace: Namespace) -> Result<T> {
        match self.get_json(path, namespace, None).await? {
            Fetched::Fresh { value, .. } => Ok(value),
            // Impossible sans `If-Modified-Since`, mais on ne panique pas pour autant.
            Fetched::NotModified => Err(Error::Api { status: 304, url: path.to_owned() }),
        }
    }
}

async fn backoff(attempt: u32) {
    tokio::time::sleep(Duration::from_millis(500 * 2u64.pow(attempt))).await;
}
