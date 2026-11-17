//! Client watcher: finds the client, connects to its WebSocket (WAMP) and reacts
//! to events instead of polling. While the client is closed it only checks
//! every few seconds, which is cheap.

use super::discovery::{find_client, Credentials};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::Value;
use std::sync::RwLock;
use std::time::Duration;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::HeaderValue, Message};

const SEARCH_INTERVAL: Duration = Duration::from_secs(5);

/// Only the events we need, so the client does not stream everything to us.
const EVENT_PHASE: &str = "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase";
const EVENT_SUMMONER: &str = "OnJsonApiEvent_lol-summoner_v1_current-summoner";

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LcuUser {
    pub game_name: String,
    pub tag_line: String,
    pub summoner_level: Option<i64>,
    pub profile_icon_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LcuState {
    pub connected: bool,
    /// `None`, `Lobby`, `Matchmaking`, `ReadyCheck`, `ChampSelect`, `InProgress`, `EndOfGame`...
    pub phase: Option<String>,
    pub user: Option<LcuUser>,
}

impl LcuState {
    pub fn in_game(&self) -> bool {
        self.connected && self.phase.as_deref() == Some("InProgress")
    }
}

pub struct Watcher {
    state: watch::Sender<LcuState>,
    lol_dir: RwLock<Option<String>>,
}

/// HTTP client that accepts the LCU's self-signed certificate.
/// Used ONLY for https://127.0.0.1; the Riot API client verifies TLS normally.
fn loopback_http() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(5))
        .no_proxy()
        .build()
        .expect("loopback client")
}

fn basic_auth(password: &str) -> String {
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("riot:{password}")))
}

/// Reads the logged-in user. The LCU PUUID is deliberately ignored: it is not the
/// same as the API PUUID, so we resolve the account by Riot ID instead.
pub fn parse_user(v: &Value) -> Option<LcuUser> {
    let name = v["gameName"].as_str().filter(|s| !s.is_empty())?;
    Some(LcuUser {
        game_name: name.to_string(),
        tag_line: v["tagLine"].as_str().unwrap_or_default().to_string(),
        summoner_level: v["summonerLevel"].as_i64(),
        profile_icon_id: v["profileIconId"].as_i64(),
    })
}

/// Applies one WAMP message (`[8, "<event>", {data, eventType, uri}]`) to the state.
/// Returns `true` when the state changed.
pub fn apply_event(state: &mut LcuState, msg: &Value) -> bool {
    if msg[0].as_i64() != Some(8) {
        return false;
    }
    let body = &msg[2];
    let before = state.clone();
    match msg[1].as_str() {
        Some(EVENT_PHASE) => state.phase = body["data"].as_str().map(String::from),
        Some(EVENT_SUMMONER) => {
            if body["eventType"].as_str() == Some("Delete") {
                state.user = None;
            } else if let Some(u) = parse_user(&body["data"]) {
                state.user = Some(u);
            }
        }
        _ => {}
    }
    *state != before
}

impl Watcher {
    pub fn new(lol_dir: Option<String>) -> Self {
        Self { state: watch::channel(LcuState::default()).0, lol_dir: RwLock::new(lol_dir) }
    }

    pub fn state(&self) -> LcuState {
        self.state.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<LcuState> {
        self.state.subscribe()
    }

    pub fn set_lol_dir(&self, dir: Option<String>) {
        *self.lol_dir.write().unwrap() = dir;
    }

    fn publish(&self, new: LcuState) {
        self.state.send_if_modified(|s| {
            let changed = *s != new;
            *s = new;
            changed
        });
    }

    /// Main loop; runs forever on a background task.
    pub async fn run(self: std::sync::Arc<Self>) {
        let http = loopback_http();
        loop {
            let dir = self.lol_dir.read().unwrap().clone();
            let creds = tokio::task::spawn_blocking(move || find_client(dir.as_deref())).await.ok().flatten();
            if let Some(c) = creds {
                tracing::info!("League client found on port {}", c.port);
                if let Err(e) = self.session(&http, &c).await {
                    tracing::info!("League client connection closed: {e}");
                }
            }
            self.publish(LcuState::default());
            tokio::time::sleep(SEARCH_INTERVAL).await;
        }
    }

    async fn get(&self, http: &reqwest::Client, c: &Credentials, path: &str) -> Option<Value> {
        let r = http
            .get(format!("https://127.0.0.1:{}{path}", c.port))
            .header("Authorization", basic_auth(&c.password))
            .send()
            .await
            .ok()?;
        if !r.status().is_success() {
            return None;
        }
        r.json().await.ok()
    }

    async fn current_user(&self, http: &reqwest::Client, c: &Credentials) -> Option<LcuUser> {
        self.get(http, c, "/lol-summoner/v1/current-summoner").await.as_ref().and_then(parse_user)
    }

    /// One connection, from connect until the client closes.
    async fn session(&self, http: &reqwest::Client, c: &Credentials) -> Result<(), String> {
        let mut req = format!("wss://127.0.0.1:{}/", c.port).into_client_request().map_err(|e| e.to_string())?;
        req.headers_mut().insert(
            "Authorization",
            HeaderValue::from_str(&basic_auth(&c.password)).map_err(|e| e.to_string())?,
        );
        // self-signed certificate accepted for this loopback connection only
        let tls = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .map_err(|e| e.to_string())?;
        let (mut ws, _) = tokio_tungstenite::connect_async_tls_with_config(
            req,
            None,
            false,
            Some(tokio_tungstenite::Connector::NativeTls(tls)),
        )
        .await
        .map_err(|e| format!("websocket: {e}"))?;

        for event in [EVENT_PHASE, EVENT_SUMMONER] {
            ws.send(Message::text(serde_json::json!([5, event]).to_string())).await.map_err(|e| e.to_string())?;
        }

        let mut state = LcuState {
            connected: true,
            phase: self
                .get(http, c, "/lol-gameflow/v1/gameflow-phase")
                .await
                .and_then(|v| v.as_str().map(String::from)),
            user: self.current_user(http, c).await,
        };
        self.publish(state.clone());

        while let Some(msg) = ws.next().await {
            let text = match msg.map_err(|e| e.to_string())? {
                Message::Text(t) => t.to_string(),
                Message::Close(_) => break,
                _ => continue,
            };
            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
            if apply_event(&mut state, &v) {
                // the user may log in after we connected
                if state.user.is_none() {
                    state.user = self.current_user(http, c).await;
                }
                tracing::debug!("client phase: {:?}", state.phase);
                self.publish(state.clone());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_user_fixture() {
        let v: Value = serde_json::from_str(include_str!("../../../tests/fixtures/lcu/current_summoner.json")).unwrap();
        let u = parse_user(&v).unwrap();
        assert_eq!((u.game_name.as_str(), u.tag_line.as_str()), ("Jogador1", "TESTE"));
        assert!(parse_user(&serde_json::json!({})).is_none());
    }

    #[test]
    fn applies_gameflow_event() {
        let v: Value = serde_json::from_str(include_str!("../../../tests/fixtures/lcu/event_gameflow.json")).unwrap();
        let mut s = LcuState { connected: true, ..Default::default() };
        assert!(apply_event(&mut s, &v));
        assert_eq!(s.phase.as_deref(), Some("ChampSelect"));
        assert!(!apply_event(&mut s, &v), "same event twice is not a change");
        assert!(!apply_event(&mut s, &serde_json::json!([0, "welcome"])));
    }

    #[test]
    fn detects_in_game() {
        let s = LcuState { connected: true, phase: Some("InProgress".into()), user: None };
        assert!(s.in_game());
        assert!(!LcuState::default().in_game());
    }

    #[test]
    fn basic_auth_header() {
        assert_eq!(basic_auth("abc"), "Basic cmlvdDphYmM=");
    }
}
