//! Finds the League client's port and password.
//!
//! 1. Command-line of `LeagueClientUx.exe` (`--app-port`, `--remoting-auth-token`),
//!    which works for any install folder.
//! 2. The `lockfile` in the LoL folder (configured or `C:\Riot Games\League of Legends`).
//!
//! Port and password change every time the client opens: never persist them.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub port: u16,
    pub password: String,
}

/// Parses `LeagueClient:<pid>:<port>:<password>:https`.
pub fn parse_lockfile(text: &str) -> Option<Credentials> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.len() < 5 {
        return None;
    }
    Some(Credentials { port: parts[2].parse().ok()?, password: parts[3].to_string() })
}

/// Reads `--app-port=1234` and `--remoting-auth-token=xyz` (quoted or not).
pub fn parse_args<S: AsRef<str>>(args: &[S]) -> Option<Credentials> {
    let mut port = None;
    let mut password = None;
    for a in args {
        let a = a.as_ref().trim_matches('"');
        if let Some(v) = a.strip_prefix("--app-port=") {
            port = v.parse().ok();
        } else if let Some(v) = a.strip_prefix("--remoting-auth-token=") {
            password = Some(v.to_string());
        }
    }
    Some(Credentials { port: port?, password: password? })
}

fn from_process() -> Option<Credentials> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    // names first (cheap), then the command line of the one process we care about
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let pid = ["LeagueClientUx.exe", "LeagueClientUx"]
        .iter()
        .find_map(|n| sys.processes_by_exact_name(OsStr::new(n)).next().map(|p| p.pid()))?;
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        false,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always).with_exe(UpdateKind::Always),
    );
    let p = sys.process(pid)?;
    let args: Vec<String> = p.cmd().iter().map(|s| s.to_string_lossy().into_owned()).collect();
    parse_args(&args).or_else(|| {
        // no access to the command line: try the lockfile next to the executable
        let dir = p.exe().and_then(Path::parent)?;
        parse_lockfile(&std::fs::read_to_string(dir.join("lockfile")).ok()?)
    })
}

fn from_lockfile(configured_dir: Option<&str>) -> Option<Credentials> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = configured_dir.map(str::trim).filter(|d| !d.is_empty()) {
        dirs.push(PathBuf::from(d));
    }
    dirs.push(PathBuf::from(r"C:\Riot Games\League of Legends"));
    dirs.iter().find_map(|d| parse_lockfile(&std::fs::read_to_string(d.join("lockfile")).ok()?))
}

/// Looks for a running client. `None` when it is closed.
pub fn find_client(configured_dir: Option<&str>) -> Option<Credentials> {
    from_process().or_else(|| from_lockfile(configured_dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lockfile_fixture() {
        let c = parse_lockfile(include_str!("../../../tests/fixtures/lcu/lockfile")).unwrap();
        assert_eq!(c, Credentials { port: 54321, password: "fakeLocalPassword".into() });
        assert!(parse_lockfile("garbage").is_none());
    }

    #[test]
    fn parses_process_args() {
        let args = [
            "C:/Riot Games/League of Legends/LeagueClientUx.exe",
            "\"--riotclient-auth-token=not-this-one\"",
            "\"--app-port=60123\"",
            "--remoting-auth-token=fakeLocalPassword",
        ];
        assert_eq!(parse_args(&args), Some(Credentials { port: 60123, password: "fakeLocalPassword".into() }));
        assert_eq!(parse_args(&["--app-port=1"]), None);
    }

    #[test]
    fn lockfile_in_configured_dir() {
        let dir = std::env::temp_dir().join(format!("rota-lcu-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lockfile"), "LeagueClient:1:4000:pw:https").unwrap();
        assert_eq!(from_lockfile(dir.to_str()).unwrap().port, 4000);
        std::fs::remove_dir_all(dir).ok();
    }
}
