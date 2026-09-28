//! Optional RAWG enrichment (https://rawg.io/apidocs): trailers, extra screenshots,
//! banner art and user ratings, matched to the Wikidata game by name.

use super::{api_url, get_json, RawgRating, Service, Trailer};
use crate::error::AppResult;
use reqwest::Client;
use serde_json::Value;

const API: &str = "https://api.rawg.io/api";

pub struct Extras {
    pub rating: Option<RawgRating>,
    pub background: Option<String>,
    pub screenshots: Vec<String>,
    pub trailers: Vec<Trailer>,
}

/// Looks up `name` on RAWG. Returns `None` when there's no exact match, so the
/// wrong game's media is never shown.
pub async fn find(client: &Client, key: &str, name: &str) -> AppResult<Option<Extras>> {
    let key = key.trim();
    let url = api_url(
        &format!("{API}/games"),
        &[("key", key), ("search", name), ("search_precise", "true"), ("page_size", "5")],
    )?;
    let json = get_json(client, url, Service::Rawg).await?;
    let wanted = normalize(name);
    let Some(game) = json["results"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|g| g["name"].as_str().is_some_and(|n| normalize(n) == wanted))
    else {
        return Ok(None);
    };
    let Some(id) = game["id"].as_u64() else { return Ok(None) };

    let shots_url = api_url(&format!("{API}/games/{id}/screenshots"), &[("key", key)])?;
    let movies_url = api_url(&format!("{API}/games/{id}/movies"), &[("key", key)])?;
    let (shots, movies) =
        tokio::join!(get_json(client, shots_url, Service::Rawg), get_json(client, movies_url, Service::Rawg));

    let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(String::from);
    let rating = game["rating"].as_f64().filter(|r| *r > 0.0).map(|r| RawgRating {
        rating: r as f32,
        rating_top: game["rating_top"].as_u64().filter(|t| *t > 0).unwrap_or(5) as u32,
        ratings_count: game["ratings_count"].as_u64().unwrap_or(0) as u32,
        url: format!("https://rawg.io/games/{}", game["slug"].as_str().unwrap_or_default()),
    });
    Ok(Some(Extras {
        rating,
        background: text(&game["background_image"]),
        screenshots: shots
            .map(|j| j["results"].as_array().into_iter().flatten().filter_map(|s| text(&s["image"])).collect())
            .unwrap_or_default(),
        trailers: movies.map(|j| parse_trailers(&j)).unwrap_or_default(),
    }))
}

fn parse_trailers(json: &Value) -> Vec<Trailer> {
    json["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let url = m["data"]["max"].as_str().or(m["data"]["480"].as_str())?.to_string();
            Some(Trailer {
                name: m["name"].as_str().unwrap_or_default().to_string(),
                preview: m["preview"].as_str().filter(|p| !p.is_empty()).map(String::from),
                url,
            })
        })
        .collect()
}

/// Case- and punctuation-insensitive form for matching names across databases.
fn normalize(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn matches_names_loosely() {
        assert_eq!(normalize("Marvel's Spider-Man"), normalize("marvels spider man"));
        assert_ne!(normalize("God of War"), normalize("God of War III"));
    }

    #[test]
    fn parses_trailers() {
        let json = json!({ "results": [
            { "name": "Launch", "preview": "p.jpg", "data": { "480": "a.mp4", "max": "b.mp4" } },
            { "name": "Broken", "data": {} }
        ]});
        let trailers = parse_trailers(&json);
        assert_eq!(trailers.len(), 1);
        assert_eq!(trailers[0].url, "b.mp4");
    }
}
