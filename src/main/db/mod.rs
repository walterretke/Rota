//! Local SQLite database (`%APPDATA%\br.rota.app\rota.db`), accessed from Rust
//! through `rusqlite` (the Tauri equivalent of `better-sqlite3`).

use crate::analysis::{is_remake, patch_from_version};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::sync::Mutex;

const SCHEMA: &str = include_str!("schema.sql");
const SCHEMA_VERSION: i32 = 1;

pub struct Db {
    conn: Mutex<Connection>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MatchSummary {
    pub match_id: String,
    pub game_start: i64,
    pub duration_s: i64,
    pub patch: String,
    pub has_timeline: bool,
    pub champion_id: i64,
    pub champion_name: Option<String>,
    pub champion_key: Option<String>,
    pub position: String,
    pub win: bool,
    pub kills: i64,
    pub deaths: i64,
    pub assists: i64,
    pub cs: i64,
}

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub matches: i64,
    pub with_timeline: i64,
    pub remakes: i64,
    pub champions: i64,
    pub items: i64,
    pub runes: i64,
    pub summoner_spells: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticChampion {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticItem {
    pub id: i64,
    pub name: String,
    pub gold: i64,
    pub tags: Vec<String>,
    pub is_legendary: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticRune {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub tree_id: i64,
    pub tree_name: String,
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticSpell {
    pub id: i64,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct StaticData {
    pub version: String,
    pub champions: Vec<StaticChampion>,
    pub items: Vec<StaticItem>,
    pub runes: Vec<StaticRune>,
    pub spells: Vec<StaticSpell>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn json_text<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Db> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn in_memory() -> Db {
        Self::init(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn init(conn: Connection) -> rusqlite::Result<Db> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version < SCHEMA_VERSION {
            conn.execute_batch(SCHEMA)?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(Db { conn: Mutex::new(conn) })
    }

    fn c(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn kv_get(&self, key: &str) -> Option<String> {
        self.c()
            .query_row("SELECT value FROM app_kv WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    pub fn kv_set(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.c().execute(
            "INSERT INTO app_kv (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn upsert_player(
        &self,
        puuid: &str,
        game_name: &str,
        tag_line: &str,
        platform: &str,
        summoner: Option<&Value>,
    ) -> rusqlite::Result<()> {
        let icon = summoner.and_then(|s| s["profileIconId"].as_i64());
        let level = summoner.and_then(|s| s["summonerLevel"].as_i64());
        self.c().execute(
            "INSERT INTO players (puuid, game_name, tag_line, platform, profile_icon_id, summoner_level, last_updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(puuid) DO UPDATE SET game_name = excluded.game_name, tag_line = excluded.tag_line,
               platform = excluded.platform,
               profile_icon_id = COALESCE(excluded.profile_icon_id, profile_icon_id),
               summoner_level = COALESCE(excluded.summoner_level, summoner_level),
               last_updated = excluded.last_updated",
            params![puuid, game_name, tag_line, platform, icon, level, now_ms()],
        )?;
        Ok(())
    }

    /// Stores one snapshot per queue returned by league-v4 (the API only gives the current value).
    pub fn insert_rank_snapshots(&self, puuid: &str, entries: &Value) -> rusqlite::Result<usize> {
        let Some(list) = entries.as_array() else { return Ok(0) };
        let c = self.c();
        let now = now_ms();
        for e in list {
            c.execute(
                "INSERT INTO rank_snapshots (puuid, queue, tier, division, lp, wins, losses, taken_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    puuid,
                    e["queueType"].as_str(),
                    e["tier"].as_str(),
                    e["rank"].as_str(),
                    e["leaguePoints"].as_i64(),
                    e["wins"].as_i64(),
                    e["losses"].as_i64(),
                    now
                ],
            )?;
        }
        Ok(list.len())
    }

    /// (exists, has timeline)
    pub fn match_state(&self, match_id: &str) -> (bool, bool) {
        self.c()
            .query_row("SELECT timeline IS NOT NULL FROM matches WHERE match_id = ?1", [match_id], |r| {
                r.get::<_, bool>(0)
            })
            .optional()
            .ok()
            .flatten()
            .map_or((false, false), |t| (true, t))
    }

    pub fn match_raw(&self, match_id: &str) -> Option<Value> {
        let text: String = self
            .c()
            .query_row("SELECT raw FROM matches WHERE match_id = ?1", [match_id], |r| r.get(0))
            .optional()
            .ok()
            .flatten()?;
        serde_json::from_str(&text).ok()
    }

    /// Saves a match and its participants. Idempotent (primary key = matchId):
    /// importing twice never duplicates, and a stored timeline is never dropped.
    pub fn save_match(&self, m: &Value, timeline: Option<&Value>) -> rusqlite::Result<String> {
        let info = &m["info"];
        let match_id = m["metadata"]["matchId"].as_str().unwrap_or_default().to_string();
        if match_id.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName("match without matchId".into()));
        }
        let version = info["gameVersion"].as_str().unwrap_or_default();
        let participants = info["participants"].as_array().cloned().unwrap_or_default();

        let mut c = self.c();
        let tx = c.transaction()?;
        tx.execute(
            "INSERT INTO matches (match_id, queue_id, game_version, patch, game_start, duration_s, is_remake, raw, timeline)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(match_id) DO UPDATE SET raw = excluded.raw, is_remake = excluded.is_remake,
               timeline = COALESCE(excluded.timeline, matches.timeline)",
            params![
                match_id,
                info["queueId"].as_i64(),
                version,
                patch_from_version(version),
                info["gameStartTimestamp"].as_i64().or(info["gameCreation"].as_i64()),
                info["gameDuration"].as_i64(),
                is_remake(m),
                json_text(m),
                timeline.map(json_text)
            ],
        )?;
        for p in &participants {
            let items: Vec<i64> = (0..=6).map(|i| p[format!("item{i}")].as_i64().unwrap_or(0)).collect();
            let spells = [p["summoner1Id"].as_i64().unwrap_or(0), p["summoner2Id"].as_i64().unwrap_or(0)];
            let cs = p["totalMinionsKilled"].as_i64().unwrap_or(0) + p["neutralMinionsKilled"].as_i64().unwrap_or(0);
            tx.execute(
                "INSERT OR REPLACE INTO participants
                 (match_id, puuid, participant_id, team_id, position, champion_id, win,
                  kills, deaths, assists, cs, gold, damage, vision, items, runes, summoners)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    match_id,
                    p["puuid"].as_str(),
                    p["participantId"].as_i64(),
                    p["teamId"].as_i64(),
                    p["teamPosition"].as_str().unwrap_or_default(),
                    p["championId"].as_i64(),
                    p["win"].as_bool().unwrap_or(false),
                    p["kills"].as_i64(),
                    p["deaths"].as_i64(),
                    p["assists"].as_i64(),
                    cs,
                    p["goldEarned"].as_i64(),
                    p["totalDamageDealtToChampions"].as_i64(),
                    p["visionScore"].as_i64(),
                    json_text(&items),
                    json_text(&p["perks"]),
                    json_text(&spells),
                ],
            )?;
        }
        tx.commit()?;
        Ok(match_id)
    }

    pub fn save_static_data(&self, d: &StaticData) -> rusqlite::Result<()> {
        let mut c = self.c();
        let tx = c.transaction()?;
        tx.execute_batch(
            "DELETE FROM static_champions; DELETE FROM static_items;
             DELETE FROM static_runes; DELETE FROM static_summoner_spells;",
        )?;
        {
            let mut st = tx.prepare("INSERT INTO static_champions VALUES (?1, ?2, ?3, ?4, ?5)")?;
            for x in &d.champions {
                st.execute(params![x.id, x.key, x.name, json_text(&x.tags), d.version])?;
            }
            let mut st = tx.prepare("INSERT INTO static_items VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
            for x in &d.items {
                st.execute(params![x.id, x.name, x.gold, json_text(&x.tags), x.is_legendary, d.version])?;
            }
            let mut st = tx.prepare("INSERT OR REPLACE INTO static_runes VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?;
            for x in &d.runes {
                st.execute(params![x.id, x.key, x.name, x.tree_id, x.tree_name, x.icon, d.version])?;
            }
            let mut st = tx.prepare("INSERT INTO static_summoner_spells VALUES (?1, ?2, ?3, ?4)")?;
            for x in &d.spells {
                st.execute(params![x.id, x.key, x.name, d.version])?;
            }
        }
        tx.execute(
            "INSERT INTO app_kv (key, value) VALUES ('ddragon_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [&d.version],
        )?;
        tx.commit()
    }

    /// The user's most recent ranked matches, remakes excluded.
    pub fn recent_matches(&self, puuid: &str, limit: u32) -> rusqlite::Result<Vec<MatchSummary>> {
        let c = self.c();
        let mut st = c.prepare(
            "SELECT m.match_id, m.game_start, m.duration_s, m.patch, m.timeline IS NOT NULL,
                    p.champion_id, sc.name, sc.key, p.position, p.win, p.kills, p.deaths, p.assists, p.cs
             FROM participants p
             JOIN matches m ON m.match_id = p.match_id
             LEFT JOIN static_champions sc ON sc.champion_id = p.champion_id
             WHERE p.puuid = ?1 AND m.is_remake = 0
             ORDER BY m.game_start DESC
             LIMIT ?2",
        )?;
        let rows = st.query_map(params![puuid, limit], |r| {
            Ok(MatchSummary {
                match_id: r.get(0)?,
                game_start: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                duration_s: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                patch: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                has_timeline: r.get(4)?,
                champion_id: r.get(5)?,
                champion_name: r.get(6)?,
                champion_key: r.get(7)?,
                position: r.get::<_, Option<String>>(8)?.unwrap_or_default(),
                win: r.get(9)?,
                kills: r.get::<_, Option<i64>>(10)?.unwrap_or(0),
                deaths: r.get::<_, Option<i64>>(11)?.unwrap_or(0),
                assists: r.get::<_, Option<i64>>(12)?.unwrap_or(0),
                cs: r.get::<_, Option<i64>>(13)?.unwrap_or(0),
            })
        })?;
        rows.collect()
    }

    pub fn counts(&self) -> Counts {
        let c = self.c();
        let n = |sql: &str| c.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0);
        Counts {
            matches: n("SELECT COUNT(*) FROM matches"),
            with_timeline: n("SELECT COUNT(*) FROM matches WHERE timeline IS NOT NULL"),
            remakes: n("SELECT COUNT(*) FROM matches WHERE is_remake = 1"),
            champions: n("SELECT COUNT(*) FROM static_champions"),
            items: n("SELECT COUNT(*) FROM static_items"),
            runes: n("SELECT COUNT(*) FROM static_runes"),
            summoner_spells: n("SELECT COUNT(*) FROM static_summoner_spells"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MATCH: &str = include_str!("../../../tests/fixtures/match_ranked.json");
    const REMAKE: &str = include_str!("../../../tests/fixtures/match_remake.json");
    const TIMELINE: &str = include_str!("../../../tests/fixtures/timeline_min.json");

    fn fixture(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn save_match_is_idempotent() {
        let db = Db::in_memory();
        let m = fixture(MATCH);
        db.save_match(&m, Some(&fixture(TIMELINE))).unwrap();
        db.save_match(&m, None).unwrap(); // re-import without timeline keeps the timeline
        assert_eq!(db.match_state("BR1_0000000001"), (true, true));
        assert_eq!(db.counts().matches, 1);
        let n: i64 = db.c().query_row("SELECT COUNT(*) FROM participants", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 10);

        let r = db.recent_matches("test-puuid-01", 10).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].cs, 192);
        assert_eq!(r[0].patch, "15.22");
        assert!(r[0].win);
        assert!(r[0].has_timeline);
    }

    #[test]
    fn remakes_are_excluded() {
        let db = Db::in_memory();
        db.save_match(&fixture(REMAKE), None).unwrap();
        assert_eq!(db.counts().remakes, 1);
        assert!(db.recent_matches("test-puuid-01", 10).unwrap().is_empty());
    }

    #[test]
    fn kv_and_rank_snapshots() {
        let db = Db::in_memory();
        db.kv_set("a", "1").unwrap();
        db.kv_set("a", "2").unwrap();
        assert_eq!(db.kv_get("a").as_deref(), Some("2"));
        db.upsert_player("pu", "Jogador", "TESTE", "br1", None).unwrap();
        let entries = serde_json::json!([{"queueType": "RANKED_SOLO_5x5", "tier": "GOLD", "rank": "II",
            "leaguePoints": 40, "wins": 10, "losses": 8}]);
        assert_eq!(db.insert_rank_snapshots("pu", &entries).unwrap(), 1);
    }
}
