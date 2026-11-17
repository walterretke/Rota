//! Data Dragon sync (pt_BR). Static CDN, does not count against the Riot rate limit.

use crate::db::{Db, StaticChampion, StaticData, StaticItem, StaticRune, StaticSpell};
use serde_json::Value;
use std::time::Duration;

const BASE: &str = "https://ddragon.leagueoflegends.com";

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub version: String,
    pub updated: bool,
}

async fn fetch(http: &reqwest::Client, url: &str) -> Result<Value, String> {
    let r = http.get(url).send().await.map_err(|_| "Não foi possível acessar o Data Dragon.".to_string())?;
    if !r.status().is_success() {
        return Err(format!("O Data Dragon respondeu {}.", r.status().as_u16()));
    }
    r.json().await.map_err(|_| "Resposta inválida do Data Dragon.".to_string())
}

/// Checks `versions.json` and, when it changed (or `force`), downloads champions, items,
/// runes and summoner spells. When Data Dragon lags on patch day we simply keep the old version.
pub async fn sync(db: &Db, force: bool) -> Result<SyncResult, String> {
    let http = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().map_err(|e| e.to_string())?;
    let versions = fetch(&http, &format!("{BASE}/api/versions.json")).await?;
    let version = versions.get(0).and_then(|v| v.as_str()).ok_or("versions.json vazio")?.to_string();

    let current = db.kv_get("ddragon_version");
    if !force && current.as_deref() == Some(version.as_str()) && db.counts().champions > 0 {
        return Ok(SyncResult { version, updated: false });
    }

    let url = |file: &str| format!("{BASE}/cdn/{version}/data/pt_BR/{file}");
    let urls = [url("champion.json"), url("item.json"), url("runesReforged.json"), url("summoner.json")];
    let (champions, items, runes, spells) =
        tokio::try_join!(fetch(&http, &urls[0]), fetch(&http, &urls[1]), fetch(&http, &urls[2]), fetch(&http, &urls[3]))?;
    let data = parse(&version, &champions, &items, &runes, &spells);
    db.save_static_data(&data).map_err(|e| format!("Erro no banco local: {e}"))?;
    tracing::info!(
        "Data Dragon {version}: {} champions, {} items, {} runes, {} spells",
        data.champions.len(),
        data.items.len(),
        data.runes.len(),
        data.spells.len()
    );
    Ok(SyncResult { version, updated: true })
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default()
}

/// Turns Data Dragon JSON files into database rows.
pub fn parse(version: &str, champions: &Value, items: &Value, runes: &Value, spells: &Value) -> StaticData {
    let mut d = StaticData { version: version.to_string(), ..Default::default() };

    for (file_id, c) in champions["data"].as_object().into_iter().flatten() {
        // `key` (numeric string) is match-v5's championId; `id` is the image file name
        if let Some(id) = c["key"].as_str().and_then(|k| k.parse::<i64>().ok()) {
            d.champions.push(StaticChampion {
                id,
                key: file_id.clone(),
                name: c["name"].as_str().unwrap_or(file_id).to_string(),
                tags: strings(&c["tags"]),
            });
        }
    }

    for (id_text, it) in items["data"].as_object().into_iter().flatten() {
        // Summoner's Rift only (Data Dragon also ships ARAM/Arena items)
        if it["maps"]["11"].as_bool() != Some(true) {
            continue;
        }
        let Ok(id) = id_text.parse::<i64>() else { continue };
        let gold = it["gold"]["total"].as_i64().unwrap_or(0);
        let tags = strings(&it["tags"]);
        let final_item = it["into"].as_array().is_none_or(|a| a.is_empty());
        let has_recipe = it["from"].as_array().is_some_and(|a| !a.is_empty());
        let is_legendary = final_item
            && has_recipe
            && gold >= 2200
            && it["gold"]["purchasable"].as_bool() != Some(false)
            && !tags.iter().any(|t| t == "Boots" || t == "Consumable");
        d.items.push(StaticItem { id, name: it["name"].as_str().unwrap_or_default().to_string(), gold, tags, is_legendary });
    }

    for tree in runes.as_array().into_iter().flatten() {
        let tree_id = tree["id"].as_i64().unwrap_or(0);
        let tree_name = tree["name"].as_str().unwrap_or_default().to_string();
        for slot in tree["slots"].as_array().into_iter().flatten() {
            for r in slot["runes"].as_array().into_iter().flatten() {
                if let Some(id) = r["id"].as_i64() {
                    d.runes.push(StaticRune {
                        id,
                        key: r["key"].as_str().unwrap_or_default().to_string(),
                        name: r["name"].as_str().unwrap_or_default().to_string(),
                        tree_id,
                        tree_name: tree_name.clone(),
                        icon: r["icon"].as_str().unwrap_or_default().to_string(),
                    });
                }
            }
        }
    }

    for (key, s) in spells["data"].as_object().into_iter().flatten() {
        if let Some(id) = s["key"].as_str().and_then(|k| k.parse::<i64>().ok()) {
            d.spells.push(StaticSpell { id, key: key.clone(), name: s["name"].as_str().unwrap_or_default().to_string() });
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn parses_fixtures() {
        let d = parse(
            "15.22.1",
            &fx(include_str!("../../../tests/fixtures/ddragon/champion.json")),
            &fx(include_str!("../../../tests/fixtures/ddragon/item.json")),
            &fx(include_str!("../../../tests/fixtures/ddragon/runesReforged.json")),
            &fx(include_str!("../../../tests/fixtures/ddragon/summoner.json")),
        );
        assert_eq!(d.champions.len(), 2);
        let ahri = d.champions.iter().find(|c| c.id == 103).unwrap();
        assert_eq!((ahri.key.as_str(), ahri.name.as_str()), ("Ahri", "Ahri"));
        assert_eq!(d.items.len(), 3, "Arena item must be filtered out");
        assert!(d.items.iter().find(|i| i.id == 3089).unwrap().is_legendary);
        assert!(!d.items.iter().find(|i| i.id == 1058).unwrap().is_legendary);
        assert!(!d.items.iter().find(|i| i.id == 3020).unwrap().is_legendary, "boots are not legendary");
        assert_eq!(d.runes.len(), 2);
        assert_eq!(d.runes[0].tree_name, "Dominação");
        assert_eq!(d.spells.len(), 2);

        let db = Db::in_memory();
        db.save_static_data(&d).unwrap();
        assert_eq!(db.kv_get("ddragon_version").as_deref(), Some("15.22.1"));
        assert_eq!(db.counts().items, 3);
    }
}
