//! Rota core: the Rust side of the Tauri app (equivalent of Electron's main process).

pub mod analysis;
pub mod config;
pub mod db;
pub mod ddragon;
pub mod ipc;
pub mod jobs;
pub mod lcu;
pub mod liveclient;
pub mod riot;
pub mod state;

use config::Config;
use ipc::commands;
use state::AppState;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use tauri::Manager;

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

fn init_logging(dir: &std::path::Path) {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let _ = std::fs::create_dir_all(dir);
    let (file, guard) = tracing_appender::non_blocking(tracing_appender::rolling::daily(dir, "rota.log"));
    let _ = LOG_GUARD.set(guard);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,rota_lib=debug"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(file).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stderr))
        .try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let paths = app.path();
            let data_dir = paths.app_data_dir()?;
            init_logging(&paths.app_log_dir().unwrap_or_else(|_| data_dir.join("logs")));

            let config_path = paths.app_config_dir()?.join("config.json");
            let config = Config::load(&config_path);
            let db = Arc::new(db::Db::open(&data_dir.join("rota.db"))?);
            let riot = Arc::new(riot::RiotClient::new(&config.effective_key(), &config.platform, &config.region));
            let lcu = Arc::new(lcu::Watcher::new(config.lol_install_dir.clone()));
            tracing::info!("Rota {} started; data in {}", env!("CARGO_PKG_VERSION"), data_dir.display());

            tauri::async_runtime::spawn(lcu.clone().run());
            let state = AppState {
                config: RwLock::new(config),
                config_path,
                db,
                riot,
                lcu,
                importing: AtomicBool::new(false),
                last_import: Mutex::new(None),
            };
            commands::spawn_event_forwarders(app.handle(), &state);
            app.manage(state);
            commands::spawn_startup_sync(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::test_key,
            commands::get_app_status,
            commands::sync_ddragon,
            commands::start_import,
            commands::get_recent_matches,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rota");
}
