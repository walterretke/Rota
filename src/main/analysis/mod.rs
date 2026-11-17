//! Analysis helpers shared by the jobs and the database layer.
//! Phase 1 adds timeline metrics (CS@10, GD@15, deaths without vision...).

use serde_json::Value;

/// Matches shorter than this are remakes (guide section 9 / 14).
pub const REMAKE_MAX_SECONDS: i64 = 300;

/// `15.22.712.1234` → `15.22` (always the first two numbers of `gameVersion`).
pub fn patch_from_version(game_version: &str) -> String {
    game_version.split('.').take(2).collect::<Vec<_>>().join(".")
}

/// A remake is a match under 5 minutes or one that ended in an early surrender.
pub fn is_remake(match_json: &Value) -> bool {
    let info = &match_json["info"];
    let short = info["gameDuration"].as_i64().is_some_and(|d| d < REMAKE_MAX_SECONDS);
    let early_surrender = info["participants"]
        .as_array()
        .is_some_and(|ps| ps.iter().any(|p| p["gameEndedInEarlySurrender"].as_bool() == Some(true)));
    short || early_surrender
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn derives_patch() {
        assert_eq!(patch_from_version("15.22.712.1234"), "15.22");
        assert_eq!(patch_from_version(""), "");
    }

    #[test]
    fn detects_remakes() {
        assert!(is_remake(&json!({"info": {"gameDuration": 200, "participants": []}})));
        assert!(is_remake(&json!({"info": {"gameDuration": 900, "participants": [{"gameEndedInEarlySurrender": true}]}})));
        assert!(!is_remake(&json!({"info": {"gameDuration": 1800, "participants": [{"gameEndedInEarlySurrender": false}]}})));
    }
}
