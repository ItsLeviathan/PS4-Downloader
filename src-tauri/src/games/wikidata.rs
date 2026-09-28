//! Wikidata: search by name and read game facts from item statements.
//! https://www.wikidata.org/w/api.php

use super::{api_url, get_json, CriticScore, Service};
use crate::error::{AppError, AppResult};
use futures_util::future::try_join_all;
use reqwest::Client;
use serde_json::{Map, Value};
use std::collections::HashMap;

const API: &str = "https://www.wikidata.org/w/api.php";
/// Most items the API accepts per request.
const BATCH: usize = 50;
/// Name matches per search request.
pub const PAGE: u32 = 50;
/// Concurrent requests per lookup; Wikimedia asks clients to keep this low.
pub const MAX_PARALLEL: usize = 4;

const VIDEO_GAME: &str = "Q7889";
/// Classes that carry platforms but aren't a single game.
const NOT_A_GAME: [&str; 3] = [
    "Q7058673",  // video game series
    "Q196600",   // media franchise
    "Q141546978", // video game franchise
];
/// Review aggregators whose scores are out of 100, most trusted first.
const AGGREGATORS: [(&str, &str); 2] = [("Q150248", "Metacritic"), ("Q21039459", "OpenCritic")];
/// Many names now live under "mul" (all languages) instead of "en".
const LANGUAGES: &str = "en|mul";

pub struct SearchBatch {
    pub ids: Vec<String>,
    pub next_offset: Option<u32>,
}

/// One Wikidata item with its English label, description, aliases and statements.
pub struct Entity {
    pub id: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub aliases: Vec<String>,
    pub wiki_title: Option<String>,
    claims: Map<String, Value>,
}

/// Short descriptions of things that mention games but aren't one.
const NOT_GAME_WORDS: [&str; 16] = [
    "series", "franchise", "character", "soundtrack", "album", "film", "episode", "television",
    "company", "engine", "console", "award", "board game", "card game", "tabletop", "role-playing game system",
];

/// Whether a search hit's short description ("2015 video game") describes one video game.
fn looks_like_game(description: &str) -> bool {
    let d = description.to_ascii_lowercase();
    d.contains("game") && !NOT_GAME_WORDS.iter().any(|w| d.contains(w))
}

pub fn is_item_id(id: &str) -> bool {
    id.len() > 1 && id.starts_with('Q') && id[1..].bytes().all(|b| b.is_ascii_digit())
}

/// Prefix search on labels and aliases, ranked by Wikidata.
pub async fn search(client: &Client, query: &str, offset: u32) -> AppResult<SearchBatch> {
    let offset = offset.to_string();
    let limit = PAGE.to_string();
    let url = api_url(
        API,
        &[
            ("action", "wbsearchentities"),
            ("search", query),
            ("language", "en"),
            ("uselang", "en"),
            ("type", "item"),
            ("limit", &limit),
            ("continue", &offset),
            ("format", "json"),
            ("formatversion", "2"),
        ],
    )?;
    let json = get_json(client, url, Service::Wikimedia).await?;
    Ok(parse_search(&json))
}

fn parse_search(json: &Value) -> SearchBatch {
    // Full items are large, so only fetch the ones whose short description says "game".
    let ids = json["search"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["description"].as_str().is_some_and(looks_like_game))
        .filter_map(|r| r["id"].as_str())
        .filter(|id| is_item_id(id))
        .map(String::from)
        .collect();
    let next_offset = json["search-continue"].as_u64().map(|n| n as u32);
    SearchBatch { ids, next_offset }
}

/// Items in the same order as `ids`; missing ones are skipped. Full items are
/// large (every statement carries its citations), so they're fetched in a few
/// small parallel requests rather than one slow one.
pub async fn entities(client: &Client, ids: &[String]) -> AppResult<Vec<Entity>> {
    let chunk_size = ids.len().div_ceil(MAX_PARALLEL).clamp(1, BATCH);
    let requests = ids.chunks(chunk_size).map(|chunk| async move {
        let joined = chunk.join("|");
        let url = api_url(
            API,
            &[
                ("action", "wbgetentities"),
                ("ids", &joined),
                ("props", "labels|descriptions|aliases|claims|sitelinks"),
                ("languages", LANGUAGES),
                ("sitefilter", "enwiki"),
                ("format", "json"),
                ("formatversion", "2"),
            ],
        )?;
        let json = get_json(client, url, Service::Wikimedia).await?;
        Ok::<_, AppError>(chunk.iter().filter_map(|id| Entity::from_json(&json["entities"][id])).collect::<Vec<_>>())
    });
    Ok(try_join_all(requests).await?.into_iter().flatten().collect())
}

/// English labels for the given item ids.
pub async fn labels(client: &Client, ids: &[String]) -> AppResult<HashMap<String, String>> {
    let mut unique: Vec<String> = ids.to_vec();
    unique.sort();
    unique.dedup();
    let mut out = HashMap::new();
    for chunk in unique.chunks(BATCH) {
        let joined = chunk.join("|");
        let url = api_url(
            API,
            &[
                ("action", "wbgetentities"),
                ("ids", &joined),
                ("props", "labels"),
                ("languages", LANGUAGES),
                ("format", "json"),
                ("formatversion", "2"),
            ],
        )?;
        let json = get_json(client, url, Service::Wikimedia).await?;
        for id in chunk {
            if let Some(label) = localized(&json["entities"][id]["labels"]) {
                out.insert(id.clone(), label);
            }
        }
    }
    Ok(out)
}

impl Entity {
    fn from_json(json: &Value) -> Option<Self> {
        if json.get("missing").is_some() {
            return None;
        }
        let text = |v: &Value| v.as_str().map(str::to_string).filter(|s| !s.is_empty());
        Some(Self {
            id: json["id"].as_str()?.to_string(),
            label: localized(&json["labels"]),
            description: localized(&json["descriptions"]),
            aliases: ["en", "mul"]
                .iter()
                .flat_map(|lang| json["aliases"][lang].as_array().into_iter().flatten())
                .filter_map(|x| text(&x["value"]))
                .collect(),
            wiki_title: text(&json["sitelinks"]["enwiki"]["title"]),
            claims: json["claims"].as_object().cloned().unwrap_or_default(),
        })
    }

    pub fn name(&self) -> String {
        self.label
            .clone()
            .or_else(|| self.wiki_title.as_deref().map(strip_disambiguation))
            .unwrap_or_else(|| self.id.clone())
    }

    /// A single game: a video game, or something released on a platform that isn't a series.
    pub fn is_game(&self) -> bool {
        let classes = self.items("P31");
        if classes.iter().any(|c| NOT_A_GAME.contains(&c.as_str())) {
            return false;
        }
        classes.iter().any(|c| c == VIDEO_GAME) || !self.items("P400").is_empty()
    }

    /// Statements for `prop`, skipping deprecated ones.
    fn statements(&self, prop: &str) -> impl Iterator<Item = &Value> {
        self.claims
            .get(prop)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|s| s["rank"].as_str() != Some("deprecated"))
    }

    /// Item ids referenced by `prop`, deduplicated, in statement order.
    pub fn items(&self, prop: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for id in self.statements(prop).filter_map(|s| s["mainsnak"]["datavalue"]["value"]["id"].as_str()) {
            if !out.iter().any(|o| o == id) {
                out.push(id.to_string());
            }
        }
        out
    }

    pub fn has_item(&self, prop: &str, id: &str) -> bool {
        self.statements(prop).any(|s| s["mainsnak"]["datavalue"]["value"]["id"].as_str() == Some(id))
    }

    pub fn strings(&self, prop: &str) -> Vec<String> {
        self.statements(prop)
            .filter_map(|s| s["mainsnak"]["datavalue"]["value"].as_str())
            .map(String::from)
            .collect()
    }

    /// (platform item id, date) for every publication date statement.
    pub fn release_dates(&self) -> Vec<(Option<String>, String)> {
        let mut out = Vec::new();
        for s in self.statements("P577") {
            let Some(date) = parse_time(&s["mainsnak"]["datavalue"]["value"]) else { continue };
            let platforms = qualifier_items(s, "P400");
            if platforms.is_empty() {
                out.push((None, date));
            } else {
                out.extend(platforms.into_iter().map(|p| (Some(p), date.clone())));
            }
        }
        out
    }

    pub fn earliest_release(&self) -> Option<String> {
        // Partial dates ("2015", "2015-03") sort before full ones in the same period.
        self.release_dates().into_iter().map(|(_, d)| d).min()
    }

    /// The most trusted aggregate critic score, preferring the one for `platform`
    /// when an aggregator lists scores per platform.
    pub fn critic_score(&self, platform: Option<&str>) -> Option<CriticScore> {
        for (aggregator, source) in AGGREGATORS {
            let scores: Vec<(&Value, u32)> = self
                .statements("P444")
                .filter(|s| qualifier_items(s, "P447").iter().any(|q| q == aggregator))
                .filter_map(|s| Some((s, parse_score(s["mainsnak"]["datavalue"]["value"].as_str()?)?)))
                .collect();
            let best = platform
                .and_then(|p| scores.iter().find(|(s, _)| qualifier_items(s, "P400").iter().any(|q| q == p)))
                .or_else(|| scores.first());
            if let Some((_, score)) = best {
                return Some(CriticScore { score: *score, source: source.to_string() });
            }
        }
        None
    }
}

fn localized(values: &Value) -> Option<String> {
    ["en", "mul"]
        .iter()
        .find_map(|lang| values[lang]["value"].as_str().filter(|s| !s.is_empty()))
        .map(String::from)
}

/// "God of War (2018 video game)" to "God of War".
fn strip_disambiguation(title: &str) -> String {
    match title.rsplit_once(" (") {
        Some((name, rest)) if rest.ends_with(')') => name.to_string(),
        _ => title.to_string(),
    }
}

fn qualifier_items(statement: &Value, prop: &str) -> Vec<String> {
    statement["qualifiers"][prop]
        .as_array()
        .map(|a| a.iter().filter_map(|q| q["datavalue"]["value"]["id"].as_str()).map(String::from).collect())
        .unwrap_or_default()
}

/// Wikidata time value to "YYYY-MM-DD", "YYYY-MM" or "YYYY" per its precision.
fn parse_time(value: &Value) -> Option<String> {
    let time = value["time"].as_str()?.strip_prefix('+')?;
    let date = time.split('T').next()?;
    let len = match value["precision"].as_u64()? {
        11.. => 10,
        10 => 7,
        9 => 4,
        _ => return None,
    };
    date.get(..len).map(String::from)
}

/// "92/100" or "92" to 92. Percentages ("98% recommend") aren't scores.
fn parse_score(text: &str) -> Option<u32> {
    let text = text.trim();
    let number = text.strip_suffix("/100").unwrap_or(text);
    number.parse().ok().filter(|n| *n <= 100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(id: &str) -> Value {
        json!({ "id": id })
    }

    fn statement(prop_value: Value, qualifiers: Value) -> Value {
        json!({ "rank": "normal", "mainsnak": { "datavalue": { "value": prop_value } }, "qualifiers": qualifiers })
    }

    fn game() -> Entity {
        Entity::from_json(&json!({
            "id": "Q1",
            "labels": { "mul": { "value": "Bloodborne" } },
            "descriptions": { "en": { "value": "2015 video game" } },
            "sitelinks": { "enwiki": { "title": "Bloodborne" } },
            "claims": {
                "P31": [statement(item(VIDEO_GAME), json!({}))],
                "P400": [statement(item("Q5014725"), json!({}))],
                "P577": [
                    statement(json!({ "time": "+2015-03-24T00:00:00Z", "precision": 11 }), json!({ "P400": [{ "datavalue": { "value": item("Q5014725") } }] })),
                    statement(json!({ "time": "+2015-00-00T00:00:00Z", "precision": 9 }), json!({})),
                ],
                "P444": [
                    statement(json!("75/100"), json!({ "P447": [{ "datavalue": { "value": item("Q2") } }] })),
                    statement(json!("98%"), json!({ "P447": [{ "datavalue": { "value": item("Q21039459") } }] })),
                    statement(json!("91/100"), json!({ "P447": [{ "datavalue": { "value": item("Q21039459") } }] })),
                ],
            }
        }))
        .unwrap()
    }

    #[test]
    fn reads_game_facts() {
        let g = game();
        assert!(g.is_game());
        assert!(g.has_item("P400", "Q5014725"));
        let score = g.critic_score(Some("Q5014725")).unwrap();
        assert_eq!((score.score, score.source.as_str()), (91, "OpenCritic"));
        assert_eq!(g.name(), "Bloodborne");
        assert_eq!(g.earliest_release().as_deref(), Some("2015"));
        assert_eq!(g.release_dates()[0], (Some("Q5014725".into()), "2015-03-24".into()));
        assert_eq!(g.wiki_title.as_deref(), Some("Bloodborne"));
    }

    #[test]
    fn rejects_series_and_missing() {
        let series = Entity::from_json(&json!({
            "id": "Q3",
            "claims": { "P31": [statement(item("Q7058673"), json!({}))], "P400": [statement(item("Q5014725"), json!({}))] }
        }))
        .unwrap();
        assert!(!series.is_game());
        assert!(Entity::from_json(&json!({ "id": "Q4", "missing": "" })).is_none());
    }

    #[test]
    fn parses_values() {
        assert_eq!(parse_score("94/100"), Some(94));
        assert_eq!(parse_score("98%"), None);
        assert_eq!(parse_score("87"), Some(87));
        assert_eq!(strip_disambiguation("God of War (2018 video game)"), "God of War");
        assert_eq!(strip_disambiguation("Bloodborne"), "Bloodborne");
        assert_eq!(parse_time(&json!({ "time": "+2018-04-20T00:00:00Z", "precision": 10 })).as_deref(), Some("2018-04"));
        assert!(is_item_id("Q17154554"));
        assert!(!is_item_id("Q"));
        assert!(!is_item_id("P31"));
        let batch = parse_search(&json!({ "search": [
            { "id": "Q1", "description": "2015 action role-playing video game" },
            { "id": "Q2", "description": "video game series" },
            { "id": "Q3", "description": "2023 American TV series" },
            { "id": "Q4" },
            { "id": "L5", "description": "video game" }
        ], "search-continue": 50 }));
        assert_eq!(batch.ids, ["Q1"]);
        assert_eq!(batch.next_offset, Some(50));
    }
}
