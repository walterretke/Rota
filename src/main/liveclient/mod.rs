//! Live Client Data API (`https://127.0.0.1:2999/liveclientdata/`), available only
//! while a match is running. It is the ONLY allowed source of live data (no memory reading).
//!
//! Planned for the live panel (guide section 7.10): poll `/activeplayer`, `/playerlist`
//! and `/eventdata` every ~2 s (local, no rate limit), dedupe events by `EventID`, and never
//! `/allgamedata` in a loop. Forbidden here (7.10.3): ability/spell/camp timers, power-spike
//! alerts, `respawnTimer` alerts, objective respawn timers, auto-buy or auto-level.
