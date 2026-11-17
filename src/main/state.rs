//! Shared application state (one instance, managed by Tauri).

use crate::config::Config;
use crate::db::Db;
use crate::jobs::import::ImportProgress;
use crate::lcu::Watcher;
use crate::riot::RiotClient;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

pub struct AppState {
    pub config: RwLock<Config>,
    pub config_path: PathBuf,
    pub db: Arc<Db>,
    /// The single Riot client: every Riot call goes through it.
    pub riot: Arc<RiotClient>,
    pub lcu: Arc<Watcher>,
    pub importing: AtomicBool,
    pub last_import: Mutex<Option<ImportProgress>>,
}
