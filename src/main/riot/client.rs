//! Riot API HTTP client: two independent buckets (platform `br1` and regional
//! `americas`), retry with backoff, and an automatic pause when the key
//! expires (401/403). The key never appears in logs or error messages.

use super::limiter::{parse_limits_header, Limiter, Priority, DEFAULT_LIMITS};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routing {
    /// `br1`: summoner, league, mastery, spectator, status...
    Platform,
    /// `americas`: account and match.
    Regional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyState {
    /// No key configured.
    Missing,
    /// Key configured, no auth errors so far.
    Ok,
    /// Riot answered 401/403: expired or invalid key. Queue paused.
    Invalid,
}

/// User-facing errors (pt-BR). Never include the key.
#[derive(Debug, thiserror::Error)]
pub enum RiotError {
    #[error("Nenhuma chave da Riot configurada. Abra Configurações e cole a sua chave.")]
    MissingKey,
    #[error("A chave da Riot expirou ou é inválida. Gere uma nova no portal e cole em Configurações.")]
    InvalidKey,
    #[error("Falha de rede ao falar com a Riot.")]
    Network,
    #[error("A Riot respondeu com erro {0}.")]
    Http(u16),
    #[error("A Riot não respondeu depois de {0} tentativas.")]
    Exhausted(u32),
    #[error("{0}")]
    NotFound(String),
}

struct Settings {
    key: String,
    platform: String,
    region: String,
}

pub struct RiotClient {
    http: reqwest::Client,
    settings: RwLock<Settings>,
    buckets: RwLock<HashMap<String, Arc<Limiter>>>,
    key_state: watch::Sender<KeyState>,
    base_override: Option<String>,
}

const ATTEMPTS: u32 = 4;

fn state_for(key: &str) -> KeyState {
    if key.trim().is_empty() {
        KeyState::Missing
    } else {
        KeyState::Ok
    }
}

impl RiotClient {
    pub fn new(key: &str, platform: &str, region: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(concat!("Rota/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("http client");
        Self {
            http,
            settings: RwLock::new(Settings {
                key: key.trim().to_string(),
                platform: platform.to_string(),
                region: region.to_string(),
            }),
            buckets: RwLock::new(HashMap::new()),
            key_state: watch::channel(state_for(key)).0,
            base_override: None,
        }
    }

    /// Tests only: point at a local server instead of `*.api.riotgames.com`.
    #[cfg(test)]
    pub fn with_base(mut self, base: &str) -> Self {
        self.base_override = Some(base.to_string());
        self
    }

    pub fn configure(&self, key: &str, platform: &str, region: &str) {
        {
            let mut s = self.settings.write().unwrap();
            s.key = key.trim().to_string();
            s.platform = platform.to_string();
            s.region = region.to_string();
        }
        self.key_state.send_replace(state_for(key));
    }

    pub fn key_state(&self) -> KeyState {
        *self.key_state.borrow()
    }

    pub fn subscribe_key_state(&self) -> watch::Receiver<KeyState> {
        self.key_state.subscribe()
    }

    fn bucket(&self, routing_value: &str) -> Arc<Limiter> {
        if let Some(b) = self.buckets.read().unwrap().get(routing_value) {
            return b.clone();
        }
        self.buckets
            .write()
            .unwrap()
            .entry(routing_value.to_string())
            .or_insert_with(|| Arc::new(Limiter::new(DEFAULT_LIMITS)))
            .clone()
    }

    /// Generic GET. `Ok(None)` means 404.
    pub async fn get(
        &self,
        routing: Routing,
        path: &str,
        query: &[(&str, String)],
        priority: Priority,
    ) -> Result<Option<Value>, RiotError> {
        let (key, routing_value) = {
            let s = self.settings.read().unwrap();
            let rv = match routing {
                Routing::Platform => s.platform.clone(),
                Routing::Regional => s.region.clone(),
            };
            (s.key.clone(), rv)
        };
        match self.key_state() {
            KeyState::Missing => return Err(RiotError::MissingKey),
            KeyState::Invalid => return Err(RiotError::InvalidKey),
            KeyState::Ok => {}
        }

        let base = self
            .base_override
            .clone()
            .unwrap_or_else(|| format!("https://{routing_value}.api.riotgames.com"));
        let url = format!("{base}{path}");
        let bucket = self.bucket(&routing_value);

        for attempt in 0..ATTEMPTS {
            bucket.acquire(priority).await;
            // the key may have expired while we waited in the queue
            if self.key_state() == KeyState::Invalid {
                return Err(RiotError::InvalidKey);
            }
            let resp = match self.http.get(&url).header("X-Riot-Token", &key).query(query).send().await {
                Ok(r) => r,
                Err(e) => {
                    // reqwest errors carry the URL only, never headers
                    tracing::warn!("network error on {path}: {e} (attempt {})", attempt + 1);
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                    continue;
                }
            };

            if let Some(limits) = resp
                .headers()
                .get("X-App-Rate-Limit")
                .and_then(|v| v.to_str().ok())
                .and_then(parse_limits_header)
            {
                bucket.update_limits(&limits).await;
            }

            let status = resp.status().as_u16();
            match status {
                200 => return resp.json::<Value>().await.map(Some).map_err(|_| RiotError::Network),
                404 => return Ok(None),
                429 => {
                    let header = |name: &str| resp.headers().get(name).and_then(|v| v.to_str().ok()).map(String::from);
                    let wait = header("Retry-After").and_then(|v| v.parse::<u64>().ok()).unwrap_or(5);
                    let kind = header("X-Rate-Limit-Type").unwrap_or_else(|| "unknown".into());
                    tracing::warn!("429 ({kind}) on {path}; waiting {wait}s");
                    bucket.block_for(Duration::from_secs(wait)).await;
                }
                401 | 403 => {
                    // no retry: pause everything until the user pastes a new key
                    tracing::error!("Riot answered {status}: key expired or invalid; pausing all calls");
                    self.key_state.send_replace(KeyState::Invalid);
                    return Err(RiotError::InvalidKey);
                }
                s if s >= 500 => {
                    tracing::warn!("Riot {s} on {path} (attempt {})", attempt + 1);
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                }
                s => {
                    tracing::warn!("Riot {s} on {path}");
                    return Err(RiotError::Http(s));
                }
            }
        }
        Err(RiotError::Exhausted(ATTEMPTS))
    }

    // ---- Endpoints used in phase 0 ----

    /// Cheap call used to validate the key (server status).
    pub async fn platform_status(&self) -> Result<Option<Value>, RiotError> {
        self.get(Routing::Platform, "/lol/status/v4/platform-data", &[], Priority::High).await
    }

    pub async fn account_by_riot_id(&self, name: &str, tag: &str) -> Result<Option<Value>, RiotError> {
        let path = format!("/riot/account/v1/accounts/by-riot-id/{}/{}", encode_segment(name), encode_segment(tag));
        self.get(Routing::Regional, &path, &[], Priority::High).await
    }

    pub async fn summoner_by_puuid(&self, puuid: &str) -> Result<Option<Value>, RiotError> {
        let path = format!("/lol/summoner/v4/summoners/by-puuid/{puuid}");
        self.get(Routing::Platform, &path, &[], Priority::High).await
    }

    pub async fn league_entries_by_puuid(&self, puuid: &str) -> Result<Option<Value>, RiotError> {
        let path = format!("/lol/league/v4/entries/by-puuid/{puuid}");
        self.get(Routing::Platform, &path, &[], Priority::High).await
    }

    /// Ranked Solo/Duo match ids (queue 420). `count` is capped at 100.
    pub async fn ranked_match_ids(&self, puuid: &str, start: u32, count: u32) -> Result<Vec<String>, RiotError> {
        let path = format!("/lol/match/v5/matches/by-puuid/{puuid}/ids");
        let q = [
            ("queue", "420".to_string()),
            ("type", "ranked".to_string()),
            ("start", start.to_string()),
            ("count", count.min(100).to_string()),
        ];
        let v = self.get(Routing::Regional, &path, &q, Priority::High).await?;
        Ok(v.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default())
    }

    pub async fn match_by_id(&self, id: &str, p: Priority) -> Result<Option<Value>, RiotError> {
        self.get(Routing::Regional, &format!("/lol/match/v5/matches/{id}"), &[], p).await
    }

    pub async fn timeline_by_id(&self, id: &str, p: Priority) -> Result<Option<Value>, RiotError> {
        self.get(Routing::Regional, &format!("/lol/match/v5/matches/{id}/timeline"), &[], p).await
    }
}

/// Percent-encodes one URL path segment (Riot IDs may contain spaces and accents).
fn encode_segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    const FAKE_KEY: &str = "fake-test-key";

    /// Minimal HTTP server that answers a fixed sequence of responses. Never the real API.
    fn server(responses: Vec<(u16, &'static str, &'static str)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, extra, body) in responses {
                let (mut s, _) = l.accept().unwrap();
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap();
                requests.push(String::from_utf8_lossy(&buf[..n]).to_string());
                let r = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
                    body.len()
                );
                s.write_all(r.as_bytes()).unwrap();
            }
            requests
        });
        (base, h)
    }

    #[test]
    fn encodes_riot_id() {
        assert_eq!(encode_segment("retke"), "retke");
        assert_eq!(encode_segment("Nome Ç"), "Nome%20%C3%87");
    }

    #[tokio::test]
    async fn no_key_no_call() {
        let c = RiotClient::new("", "br1", "americas");
        assert!(matches!(c.platform_status().await, Err(RiotError::MissingKey)));
    }

    #[tokio::test]
    async fn invalid_key_pauses_without_retry() {
        let (base, h) = server(vec![(403, "", "{}")]);
        let c = RiotClient::new(FAKE_KEY, "br1", "americas").with_base(&base);
        let err = c.platform_status().await.unwrap_err();
        assert!(matches!(err, RiotError::InvalidKey));
        assert!(!err.to_string().contains(FAKE_KEY), "error message must not leak the key");
        assert_eq!(c.key_state(), KeyState::Invalid);
        // second call never leaves the app
        assert!(matches!(c.platform_status().await, Err(RiotError::InvalidKey)));
        let requests = h.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].to_lowercase().contains(&format!("x-riot-token: {FAKE_KEY}")));
        // a new key resumes
        c.configure("another-fake-key", "br1", "americas");
        assert_eq!(c.key_state(), KeyState::Ok);
    }

    #[tokio::test]
    async fn retries_after_429_and_handles_404() {
        let (base, h) = server(vec![
            (429, "Retry-After: 1\r\nX-Rate-Limit-Type: application\r\n", "{}"),
            (200, "X-App-Rate-Limit: 20:1,100:120\r\n", r#"["BR1_1","BR1_2"]"#),
            (404, "", "{}"),
        ]);
        let c = RiotClient::new(FAKE_KEY, "br1", "americas").with_base(&base);
        let ids = c.ranked_match_ids("puuid", 0, 500).await.unwrap();
        assert_eq!(ids, vec!["BR1_1", "BR1_2"]);
        assert!(c.match_by_id("BR1_0", Priority::High).await.unwrap().is_none());
        let requests = h.join().unwrap();
        assert!(requests[1].contains("count=100"), "count must be capped at 100");
        assert!(requests[1].contains("queue=420"));
    }
}
