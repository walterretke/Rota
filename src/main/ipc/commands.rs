//! Tauri commands invoked by the renderer. Errors are user-facing pt-BR strings.

use super::events;
use crate::config::{Config, ConfigView};
use crate::db::{Counts, MatchSummary};
use crate::ddragon::{self, SyncResult};
use crate::jobs::import::{import_recent, ImportProgress, RiotId};
use crate::lcu::LcuState;
use crate::riot::KeyState;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInput {
    /// `None` or empty keeps the current key.
    pub riot_api_key: Option<String>,
    pub platform: String,
    pub region: String,
    pub lol_install_dir: Option<String>,
    pub riot_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub key_state: KeyState,
    pub lcu: LcuState,
    pub counts: Counts,
    pub ddragon_version: Option<String>,
    pub user_riot_id: Option<String>,
    pub importing: bool,
    pub last_import: Option<ImportProgress>,
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> ConfigView {
    ConfigView::from(&*state.config.read().unwrap())
}

#[tauri::command]
pub fn save_config(state: State<'_, AppState>, input: ConfigInput) -> Result<ConfigView, String> {
    let riot_id = clean(input.riot_id);
    if let Some(id) = &riot_id {
        if Config::parse_riot_id(id).is_none() {
            return Err("Riot ID inválido. Use o formato Nome#TAG.".into());
        }
    }
    let platform = input.platform.trim().to_lowercase();
    let region = input.region.trim().to_lowercase();
    if platform.is_empty() || region.is_empty() {
        return Err("Informe a plataforma e a região.".into());
    }
    let mut cfg = state.config.write().unwrap();
    if let Some(key) = clean(input.riot_api_key) {
        cfg.riot_api_key = key;
    }
    cfg.platform = platform;
    cfg.region = region;
    cfg.lol_install_dir = clean(input.lol_install_dir);
    cfg.riot_id = riot_id;
    cfg.save(&state.config_path).map_err(|_| "Não foi possível salvar as configurações.".to_string())?;
    state.riot.configure(&cfg.effective_key(), &cfg.platform, &cfg.region);
    state.lcu.set_lol_dir(cfg.lol_install_dir.clone());
    Ok(ConfigView::from(&*cfg))
}

#[tauri::command]
pub async fn test_key(state: State<'_, AppState>) -> Result<String, String> {
    // re-arm the client: the user may be testing a key that was just fixed
    {
        let cfg = state.config.read().unwrap();
        state.riot.configure(&cfg.effective_key(), &cfg.platform, &cfg.region);
    }
    match state.riot.platform_status().await {
        Ok(_) => Ok("Chave válida. Tudo certo com a Riot.".into()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn get_app_status(state: State<'_, AppState>) -> AppStatus {
    AppStatus {
        key_state: state.riot.key_state(),
        lcu: state.lcu.state(),
        counts: state.db.counts(),
        ddragon_version: state.db.kv_get("ddragon_version"),
        user_riot_id: state.db.kv_get("user_riot_id"),
        importing: state.importing.load(Ordering::SeqCst),
        last_import: state.last_import.lock().unwrap().clone(),
    }
}

#[tauri::command]
pub async fn sync_ddragon(state: State<'_, AppState>, force: bool) -> Result<SyncResult, String> {
    ddragon::sync(&state.db, force).await
}

#[tauri::command]
pub fn start_import(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let who = {
        let lcu_user = state.lcu.state().user;
        let cfg = state.config.read().unwrap();
        match lcu_user {
            Some(u) if !u.tag_line.is_empty() => RiotId { name: u.game_name, tag: u.tag_line },
            _ => match cfg.riot_id.as_deref().and_then(Config::parse_riot_id) {
                Some((name, tag)) => RiotId { name, tag },
                None => {
                    return Err("Abra o cliente do LoL ou informe seu Riot ID em Configurações.".into());
                }
            },
        }
    };
    if state.importing.swap(true, Ordering::SeqCst) {
        return Err("Uma importação já está em andamento.".into());
    }

    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let platform = state.config.read().unwrap().platform.clone();
        let report = |p: &ImportProgress| {
            *state.last_import.lock().unwrap() = Some(p.clone());
            let _ = app.emit(events::IMPORT_PROGRESS, p);
        };
        let result = import_recent(&state.db, &state.riot, &platform, who, 100, state.lcu.subscribe(), report).await;
        if let Err(e) = result {
            let msg = e.to_string();
            tracing::warn!("import stopped: {msg}");
            let mut last = state.last_import.lock().unwrap().clone().unwrap_or_default();
            last.error = Some(msg);
            last.finished = true;
            last.step = "A importação parou.".into();
            *state.last_import.lock().unwrap() = Some(last.clone());
            let _ = app.emit(events::IMPORT_PROGRESS, &last);
        }
        state.importing.store(false, Ordering::SeqCst);
    });
    Ok(())
}

#[tauri::command]
pub fn get_recent_matches(state: State<'_, AppState>, limit: u32) -> Result<Vec<MatchSummary>, String> {
    let Some(puuid) = state.db.kv_get("user_puuid") else { return Ok(vec![]) };
    state.db.recent_matches(&puuid, limit.min(100)).map_err(|_| "Erro ao ler o banco local.".to_string())
}

/// Forwards core state changes to the renderer as events.
pub fn spawn_event_forwarders(app: &AppHandle, state: &AppState) {
    let mut lcu_rx = state.lcu.subscribe();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while lcu_rx.changed().await.is_ok() {
            let s = lcu_rx.borrow_and_update().clone();
            let _ = handle.emit(events::LCU_STATE, s);
        }
    });

    let mut key_rx = state.riot.subscribe_key_state();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while key_rx.changed().await.is_ok() {
            let s = *key_rx.borrow_and_update();
            let _ = handle.emit(events::KEY_STATE, s);
        }
    });
}

/// Keeps static data fresh: syncs Data Dragon at startup (outside matches).
pub fn spawn_startup_sync(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = handle.state::<AppState>();
        crate::jobs::wait_while_in_game(&mut state.lcu.subscribe()).await;
        if let Err(e) = ddragon::sync(&state.db, false).await {
            tracing::warn!("Data Dragon sync failed: {e}");
        }
    });
}
