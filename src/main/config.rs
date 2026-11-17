//! Local user configuration.
//!
//! Stored in `%APPDATA%\br.rota.app\config.json` on Windows. The Riot key only
//! lives in this file (typed in the Settings screen) or, in development, in
//! `.env` as `RIOT_API_KEY`. Never in code, Git, tests, logs or the installer.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    /// Riot key (RGAPI-...). Empty until the user provides one.
    pub riot_api_key: String,
    /// Platform routing value (league, mastery, spectator...).
    pub platform: String,
    /// Regional routing value (account, match).
    pub region: String,
    /// LoL install folder, used only if the client process cannot be found.
    pub lol_install_dir: Option<String>,
    /// Riot ID used when the LoL client is closed (`Name#TAG`).
    pub riot_id: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            riot_api_key: String::new(),
            platform: "br1".into(),
            region: "americas".into(),
            lol_install_dir: None,
            riot_id: None,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Config {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!("invalid config.json ({e}); using defaults");
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // write to a temp file then rename, so a crash never leaves a half-written file
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).expect("serializable config"))?;
        std::fs::rename(tmp, path)
    }

    pub fn has_key(&self) -> bool {
        !self.riot_api_key.trim().is_empty()
    }

    /// The key actually used: the saved one, or `RIOT_API_KEY` from the environment / `.env`
    /// in development builds.
    pub fn effective_key(&self) -> String {
        if self.has_key() {
            return self.riot_api_key.trim().to_string();
        }
        if cfg!(debug_assertions) {
            if let Some(k) = std::env::var("RIOT_API_KEY").ok().or_else(dev_env_key) {
                return k.trim().to_string();
            }
        }
        String::new()
    }

    /// Splits `Name#TAG` into (name, tag).
    pub fn parse_riot_id(riot_id: &str) -> Option<(String, String)> {
        let (name, tag) = riot_id.trim().rsplit_once('#')?;
        let (name, tag) = (name.trim(), tag.trim());
        if name.is_empty() || tag.is_empty() {
            return None;
        }
        Some((name.to_string(), tag.to_string()))
    }
}

/// Reads `RIOT_API_KEY` from the repository's `.env` (development only).
fn dev_env_key() -> Option<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.env");
    parse_env_value(&std::fs::read_to_string(path).ok()?, "RIOT_API_KEY")
}

fn parse_env_value(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (k, v) = line.trim().split_once('=')?;
        if k.trim() != name {
            return None;
        }
        let v = v.trim().trim_matches('"').trim_matches('\'');
        (!v.is_empty()).then(|| v.to_string())
    })
}

/// What the UI receives: the key itself never goes back to the renderer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigView {
    pub has_key: bool,
    pub key_hint: String,
    pub key_from_env: bool,
    pub platform: String,
    pub region: String,
    pub lol_install_dir: Option<String>,
    pub riot_id: Option<String>,
}

impl From<&Config> for ConfigView {
    fn from(c: &Config) -> Self {
        let key = c.effective_key();
        let key_hint = if key.len() > 4 { format!("…{}", &key[key.len() - 4..]) } else { String::new() };
        Self {
            has_key: !key.is_empty(),
            key_hint,
            key_from_env: !c.has_key() && !key.is_empty(),
            platform: c.platform.clone(),
            region: c.region.clone(),
            lol_install_dir: c.lol_install_dir.clone(),
            riot_id: c.riot_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_riot_id() {
        assert_eq!(Config::parse_riot_id(" retke#retke "), Some(("retke".into(), "retke".into())));
        assert_eq!(Config::parse_riot_id("Nome Com Espaço#BR1").unwrap().0, "Nome Com Espaço");
        assert_eq!(Config::parse_riot_id("notag"), None);
        assert_eq!(Config::parse_riot_id("#BR1"), None);
    }

    #[test]
    fn view_never_exposes_key() {
        let c = Config { riot_api_key: "FAKE-TEST-KEY-1234".into(), ..Default::default() };
        let v = ConfigView::from(&c);
        assert!(v.has_key);
        assert_eq!(v.key_hint, "…1234");
        assert!(!serde_json::to_string(&v).unwrap().contains("FAKE-TEST-KEY"));
    }

    #[test]
    fn parses_env_file() {
        let text = "# comment\nRIOT_PLATFORM=br1\nRIOT_API_KEY=\"fake-value\"\n";
        assert_eq!(parse_env_value(text, "RIOT_API_KEY").as_deref(), Some("fake-value"));
        assert_eq!(parse_env_value("RIOT_API_KEY=\n", "RIOT_API_KEY"), None);
    }

    #[test]
    fn saves_and_loads() {
        let dir = std::env::temp_dir().join(format!("rota-cfg-{}", std::process::id()));
        let path = dir.join("config.json");
        let c = Config { riot_id: Some("retke#retke".into()), ..Default::default() };
        c.save(&path).unwrap();
        assert_eq!(Config::load(&path), c);
        std::fs::remove_dir_all(dir).ok();
    }
}
