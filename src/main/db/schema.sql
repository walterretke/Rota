-- Local SQLite schema, based on section 11 of the guide:
-- JSONB/arrays -> TEXT holding JSON; TIMESTAMPTZ -> INTEGER (epoch ms).

CREATE TABLE IF NOT EXISTS app_kv (
  key    TEXT PRIMARY KEY,
  value  TEXT
);

-- Known players
CREATE TABLE IF NOT EXISTS players (
  puuid            TEXT PRIMARY KEY,
  game_name        TEXT,
  tag_line         TEXT,
  platform         TEXT DEFAULT 'br1',
  profile_icon_id  INTEGER,
  summoner_level   INTEGER,
  last_updated     INTEGER
);

-- Rank history (our own snapshots: the API only returns the current value)
CREATE TABLE IF NOT EXISTS rank_snapshots (
  id        INTEGER PRIMARY KEY AUTOINCREMENT,
  puuid     TEXT REFERENCES players(puuid),
  queue     TEXT,
  tier      TEXT,
  division  TEXT,
  lp        INTEGER,
  wins      INTEGER,
  losses    INTEGER,
  taken_at  INTEGER
);
CREATE INDEX IF NOT EXISTS idx_rank_snapshots_puuid ON rank_snapshots(puuid, taken_at);

-- Raw matches (immutable)
CREATE TABLE IF NOT EXISTS matches (
  match_id      TEXT PRIMARY KEY,
  queue_id      INTEGER,
  game_version  TEXT,
  patch         TEXT,
  game_start    INTEGER,
  duration_s    INTEGER,
  is_remake     INTEGER,
  avg_tier      TEXT,
  raw           TEXT,
  timeline      TEXT
);
CREATE INDEX IF NOT EXISTS idx_matches_start ON matches(game_start);

-- One row per participant
CREATE TABLE IF NOT EXISTS participants (
  match_id        TEXT REFERENCES matches(match_id),
  puuid           TEXT,
  participant_id  INTEGER,
  team_id         INTEGER,
  position        TEXT,
  champion_id     INTEGER,
  win             INTEGER,
  kills INTEGER, deaths INTEGER, assists INTEGER,
  cs INTEGER, gold INTEGER, damage INTEGER, vision INTEGER,
  items           TEXT,
  runes           TEXT,
  summoners       TEXT,
  PRIMARY KEY (match_id, participant_id)
);
CREATE INDEX IF NOT EXISTS idx_participants_puuid ON participants(puuid);

-- Timeline-derived metrics (recomputable; filled in phase 1)
CREATE TABLE IF NOT EXISTS participant_metrics (
  match_id        TEXT,
  participant_id  INTEGER,
  cs_at_10 INTEGER, cs_at_15 INTEGER,
  gd_at_10 INTEGER, gd_at_15 INTEGER,
  xpd_at_10 INTEGER, xpd_at_15 INTEGER,
  deaths_pre_15 INTEGER,
  deaths_no_vision INTEGER,
  objective_participation REAL,
  first_legendary_s INTEGER,
  skill_order TEXT,
  item_path TEXT,
  metrics_version INTEGER,
  PRIMARY KEY (match_id, participant_id)
);

-- Meta aggregates (phase 2)
CREATE TABLE IF NOT EXISTS meta_champion_stats (
  patch TEXT, tier_group TEXT, position TEXT, champion_id INTEGER,
  games INTEGER, wins INTEGER, picks_rate REAL, ban_rate REAL,
  PRIMARY KEY (patch, tier_group, position, champion_id)
);

CREATE TABLE IF NOT EXISTS meta_matchups (
  patch TEXT, tier_group TEXT, position TEXT,
  champion_id INTEGER, opponent_id INTEGER,
  games INTEGER, wins INTEGER, avg_gd_15 REAL,
  PRIMARY KEY (patch, tier_group, position, champion_id, opponent_id)
);

-- Static data (Data Dragon, pt_BR)
CREATE TABLE IF NOT EXISTS static_champions (
  champion_id INTEGER PRIMARY KEY, key TEXT, name TEXT, tags TEXT, version TEXT
);
CREATE TABLE IF NOT EXISTS static_items (
  item_id INTEGER PRIMARY KEY, name TEXT, gold INTEGER, tags TEXT, is_legendary INTEGER, version TEXT
);
CREATE TABLE IF NOT EXISTS static_runes (
  rune_id INTEGER PRIMARY KEY, key TEXT, name TEXT, tree_id INTEGER, tree_name TEXT, icon TEXT, version TEXT
);
CREATE TABLE IF NOT EXISTS static_summoner_spells (
  spell_id INTEGER PRIMARY KEY, key TEXT, name TEXT, version TEXT
);
