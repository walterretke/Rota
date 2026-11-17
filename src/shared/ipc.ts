// Typed IPC contract between the Rust core (src/main/ipc) and the renderer.
// Keep in sync with the Rust structs (serde camelCase).

export type KeyState = "missing" | "ok" | "invalid";

export interface LcuUser {
  gameName: string;
  tagLine: string;
  summonerLevel: number | null;
  profileIconId: number | null;
}

export interface LcuState {
  connected: boolean;
  phase: string | null;
  user: LcuUser | null;
}

export interface Counts {
  matches: number;
  withTimeline: number;
  remakes: number;
  champions: number;
  items: number;
  runes: number;
  summonerSpells: number;
}

export interface ImportProgress {
  step: string;
  done: number;
  total: number;
  newMatches: number;
  paused: boolean;
  finished: boolean;
  error: string | null;
}

export interface AppStatus {
  keyState: KeyState;
  lcu: LcuState;
  counts: Counts;
  ddragonVersion: string | null;
  userRiotId: string | null;
  importing: boolean;
  lastImport: ImportProgress | null;
}

export interface ConfigView {
  hasKey: boolean;
  keyHint: string;
  keyFromEnv: boolean;
  platform: string;
  region: string;
  lolInstallDir: string | null;
  riotId: string | null;
}

export interface ConfigInput {
  riotApiKey: string | null;
  platform: string;
  region: string;
  lolInstallDir: string | null;
  riotId: string | null;
}

export interface SyncResult {
  version: string;
  updated: boolean;
}

export interface MatchSummary {
  matchId: string;
  gameStart: number;
  durationS: number;
  patch: string;
  hasTimeline: boolean;
  championId: number;
  championName: string | null;
  championKey: string | null;
  position: string;
  win: boolean;
  kills: number;
  deaths: number;
  assists: number;
  cs: number;
}

/** Command name -> [arguments, result]. */
export interface Commands {
  get_config: [Record<string, never>, ConfigView];
  save_config: [{ input: ConfigInput }, ConfigView];
  test_key: [Record<string, never>, string];
  get_app_status: [Record<string, never>, AppStatus];
  sync_ddragon: [{ force: boolean }, SyncResult];
  start_import: [Record<string, never>, null];
  get_recent_matches: [{ limit: number }, MatchSummary[]];
}

/** Event name -> payload. */
export interface Events {
  "lcu-state": LcuState;
  "key-state": KeyState;
  "import-progress": ImportProgress;
}
