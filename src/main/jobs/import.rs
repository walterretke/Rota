//! Imports the user's latest ranked Solo/Duo matches (match + timeline).
//!
//! - A match already in the database is never downloaded again.
//! - Pauses while the user is in a match.
//! - An expired key (401/403) stops the job and warns the user, with no retry.

use super::wait_while_in_game;
use crate::db::Db;
use crate::lcu::LcuState;
use crate::riot::{Priority, RiotClient, RiotError};
use serde::Serialize;
use tokio::sync::watch;

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    /// Human-readable step (pt-BR, shown in the UI).
    pub step: String,
    pub done: u32,
    pub total: u32,
    pub new_matches: u32,
    pub paused: bool,
    pub finished: bool,
    pub error: Option<String>,
}

pub struct RiotId {
    pub name: String,
    pub tag: String,
}

pub async fn import_recent(
    db: &Db,
    riot: &RiotClient,
    platform: &str,
    who: RiotId,
    count: u32,
    mut lcu: watch::Receiver<LcuState>,
    report: impl Fn(&ImportProgress),
) -> Result<ImportProgress, RiotError> {
    let mut p = ImportProgress { step: format!("Buscando a conta {}#{}…", who.name, who.tag), ..Default::default() };
    report(&p);

    // The LCU PUUID differs from the API PUUID: always resolve by Riot ID.
    let account = riot
        .account_by_riot_id(&who.name, &who.tag)
        .await?
        .ok_or_else(|| RiotError::NotFound(format!("Conta {}#{} não encontrada. Confira o Riot ID.", who.name, who.tag)))?;
    let puuid = account["puuid"].as_str().unwrap_or_default().to_string();
    let name = account["gameName"].as_str().unwrap_or(&who.name).to_string();
    let tag = account["tagLine"].as_str().unwrap_or(&who.tag).to_string();

    let summoner = riot.summoner_by_puuid(&puuid).await?;
    log_db(db.upsert_player(&puuid, &name, &tag, platform, summoner.as_ref()));
    if let Some(entries) = riot.league_entries_by_puuid(&puuid).await? {
        log_db(db.insert_rank_snapshots(&puuid, &entries).map(|_| ()));
    }
    log_db(db.kv_set("user_puuid", &puuid));
    log_db(db.kv_set("user_riot_id", &format!("{name}#{tag}")));

    p.step = "Listando suas ranqueadas…".into();
    report(&p);
    let ids = riot.ranked_match_ids(&puuid, 0, count).await?;
    p.total = ids.len() as u32;

    for id in ids {
        if lcu.borrow().in_game() {
            p.paused = true;
            p.step = "Pausado: você está em partida. Continua quando o jogo acabar.".into();
            report(&p);
            wait_while_in_game(&mut lcu).await;
            p.paused = false;
        }

        let (exists, has_timeline) = db.match_state(&id);
        if !(exists && has_timeline) {
            p.step = format!("Baixando {id}…");
            report(&p);
            let fetched = if exists { None } else { riot.match_by_id(&id, Priority::High).await? };
            let timeline = riot.timeline_by_id(&id, Priority::High).await?;
            let raw = fetched.or_else(|| if exists { db.match_raw(&id) } else { None });
            match raw {
                Some(m) => {
                    if db.save_match(&m, timeline.as_ref()).is_ok() && !exists {
                        p.new_matches += 1;
                    }
                }
                None => tracing::warn!("match {id} not found at Riot"),
            }
        }
        p.done += 1;
        report(&p);
    }

    p.step = if p.new_matches > 0 {
        format!("Pronto: {} partidas novas importadas.", p.new_matches)
    } else {
        "Pronto: tudo já estava atualizado.".into()
    };
    p.finished = true;
    report(&p);
    Ok(p)
}

fn log_db<T>(r: rusqlite::Result<T>) {
    if let Err(e) = r {
        tracing::warn!("database write failed: {e}");
    }
}
