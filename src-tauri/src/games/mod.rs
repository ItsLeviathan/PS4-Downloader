//! Game search and details. Wikidata supplies the facts and Wikipedia the
//! description, cover art and screenshots, so no account is needed. With a RAWG
//! API key, trailers, more screenshots and user ratings are added on top.
//! Only metadata is fetched here; downloads still go through the download manager.

mod rawg;
mod wikidata;
mod wikipedia;

use crate::downloader::http::from_reqwest;
use crate::error::{AppError, AppResult, ErrorCode};
use futures_util::future::try_join_all;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use wikidata::Entity;

pub use wikipedia::{Block, Section};

/// Wikimedia asks API clients to identify themselves with a way to reach the maintainer.
const USER_AGENT: &str = concat!(
    "PS4Downloader/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/ItsLeviathan/PS4-Downloader)"
);
/// Name matches come 50 at a time and most aren't games, so a search may read a few batches.
const SEARCH_EXTRA_BATCHES: usize = 2;
const SEARCH_MIN_CANDIDATES: usize = 20;
const SEARCH_CACHE_LIMIT: usize = 64;
const CARD_GENRES: usize = 2;
const CARD_IMAGE_WIDTH: u32 = 400;
const DETAILS_CACHE_LIMIT: usize = 64;

const PS4: &str = "Q5014725";
const PS5: &str = "Q63184502";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlatformFilter {
    Ps4,
    Ps5,
    All,
}

impl PlatformFilter {
    fn matches(self, entity: &Entity) -> bool {
        match self {
            Self::Ps4 => entity.has_item("P400", PS4),
            Self::Ps5 => entity.has_item("P400", PS5),
            Self::All => true,
        }
    }
}

// ---- Types sent to the frontend ------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
    /// Wikidata item id, e.g. "Q17154554".
    pub id: String,
    pub name: String,
    /// Short description such as "2015 action role-playing game".
    pub summary: Option<String>,
    /// "YYYY-MM-DD", "YYYY-MM" or "YYYY", depending on what is known.
    pub released: Option<String>,
    pub image: Option<String>,
    pub critic_score: Option<CriticScore>,
    pub genres: Vec<String>,
    /// Short names of the PlayStation consoles it's on ("PS4", "PS5").
    pub consoles: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CriticScore {
    /// Out of 100.
    pub score: u32,
    /// "Metacritic" or "OpenCritic".
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSearchPage {
    pub results: Vec<GameSummary>,
    pub next_offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformRelease {
    pub name: String,
    pub released_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trailer {
    pub name: String,
    pub preview: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawgRating {
    pub rating: f32,
    pub rating_top: u32,
    pub ratings_count: u32,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetails {
    pub id: String,
    pub name: String,
    pub summary: Option<String>,
    pub sections: Vec<Section>,
    pub released: Option<String>,
    pub image: Option<String>,
    /// Wide artwork for the page banner (from RAWG when available).
    pub backdrop: Option<String>,
    pub website: Option<String>,
    pub wikipedia_url: Option<String>,
    pub wikidata_url: String,
    pub critic_score: Option<CriticScore>,
    pub genres: Vec<String>,
    pub platforms: Vec<PlatformRelease>,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    pub series: Vec<String>,
    pub modes: Vec<String>,
    pub age_ratings: Vec<String>,
    pub alternative_names: Vec<String>,
    pub consoles: Vec<String>,
    pub screenshots: Vec<String>,
    pub trailers: Vec<Trailer>,
    pub rawg: Option<RawgRating>,
}

// ---- Client ---------------------------------------------------------------

pub struct GameDb {
    client: Client,
    searches: Mutex<HashMap<(String, u32, PlatformFilter), GameSearchPage>>,
    /// Keyed by item id and RAWG key, since the key changes what a page includes.
    details: Mutex<HashMap<(String, String), GameDetails>>,
}

impl Default for GameDb {
    fn default() -> Self {
        Self::new()
    }
}

impl GameDb {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(25))
            .build()
            .expect("static HTTP client configuration is valid");
        Self { client, searches: Mutex::new(HashMap::new()), details: Mutex::new(HashMap::new()) }
    }

    pub async fn search(&self, query: &str, offset: u32, platform: PlatformFilter) -> AppResult<GameSearchPage> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(GameSearchPage { results: Vec::new(), next_offset: None });
        }

        let cache_key = (query.to_lowercase(), offset, platform);
        if let Some(hit) = lock(&self.searches).get(&cache_key) {
            return Ok(hit.clone());
        }

        // Name search is cheap; collect likely games first, then fetch their full items once.
        // The first batch usually decides; the next ones are read together when it's thin.
        let first = wikidata::search(&self.client, query, offset).await?;
        let mut candidates = first.ids;
        let mut next_offset = first.next_offset;
        if let Some(next) = next_offset.filter(|_| candidates.len() < SEARCH_MIN_CANDIDATES) {
            let offsets: Vec<u32> = (0..SEARCH_EXTRA_BATCHES as u32).map(|i| next + i * wikidata::PAGE).collect();
            let batches = try_join_all(offsets.iter().map(|o| wikidata::search(&self.client, query, *o))).await?;
            next_offset = None;
            for batch in batches {
                next_offset = batch.next_offset;
                for id in batch.ids {
                    if !candidates.contains(&id) {
                        candidates.push(id);
                    }
                }
                if next_offset.is_none() {
                    break;
                }
            }
        }
        let games: Vec<Entity> = wikidata::entities(&self.client, &candidates)
            .await?
            .into_iter()
            .filter(|e| e.is_game() && platform.matches(e))
            .collect();

        // Genre names and cover art only decorate the cards; don't fail the search over them.
        let genre_ids: Vec<String> =
            games.iter().flat_map(|g| g.items("P136").into_iter().take(CARD_GENRES)).collect();
        let titles: Vec<String> = games.iter().filter_map(|g| g.wiki_title.clone()).collect();
        let (labels, covers) = tokio::join!(
            wikidata::labels(&self.client, &genre_ids),
            wikipedia::thumbnails(&self.client, &titles, CARD_IMAGE_WIDTH),
        );
        let labels = labels.unwrap_or_default();
        let covers = covers.unwrap_or_default();

        let results = games
            .into_iter()
            .map(|g| GameSummary {
                image: g.wiki_title.as_ref().and_then(|t| covers.get(t).cloned()),
                genres: names(&labels, g.items("P136").iter().take(CARD_GENRES)),
                released: g.earliest_release(),
                critic_score: g.critic_score(Some(PS4)),
                consoles: consoles(&g),
                summary: g.description.clone(),
                name: g.name(),
                id: g.id,
            })
            .collect();
        let page = GameSearchPage { results, next_offset };
        let mut cache = lock(&self.searches);
        if cache.len() >= SEARCH_CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(cache_key, page.clone());
        Ok(page)
    }

    /// Full details for one game. `rawg_key` (may be empty) adds RAWG media and ratings.
    pub async fn details(&self, id: &str, rawg_key: &str) -> AppResult<GameDetails> {
        if !wikidata::is_item_id(id) {
            return Err(AppError::new(ErrorCode::NotFound, "That game couldn't be found."));
        }
        let rawg_key = rawg_key.trim();
        let with_rawg = !rawg_key.is_empty();
        let cache_key = (id.to_string(), rawg_key.to_string());
        if let Some(hit) = lock(&self.details).get(&cache_key) {
            return Ok(hit.clone());
        }

        let entity = wikidata::entities(&self.client, &[id.to_string()])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "That game couldn't be found on Wikidata."))?;
        let name = entity.name();

        let mut referenced = Vec::new();
        for prop in ["P136", "P178", "P123", "P400", "P179", "P404", "P852", "P908"] {
            referenced.extend(entity.items(prop));
        }
        let title = entity.wiki_title.clone();
        let (labels, page, images, extras) = tokio::join!(
            wikidata::labels(&self.client, &referenced),
            async {
                match &title {
                    Some(t) => wikipedia::page(&self.client, t).await.map(Some),
                    None => Ok(None),
                }
            },
            async {
                match &title {
                    Some(t) => wikipedia::screenshots(&self.client, t).await,
                    None => Ok(Vec::new()),
                }
            },
            async {
                if with_rawg {
                    rawg::find(&self.client, rawg_key, &name).await.ok().flatten()
                } else {
                    None
                }
            },
        );
        let labels = labels?;
        // The article is the description; without it the facts alone are still useful.
        let page = page.unwrap_or(None);
        let images = images.unwrap_or_default();

        let releases = entity.release_dates();
        let platforms = entity
            .items("P400")
            .iter()
            .filter_map(|q| {
                let name = labels.get(q)?.clone();
                // Dates without a platform apply to all of them.
                let released_at = releases
                    .iter()
                    .filter(|(p, _)| p.as_deref() == Some(q))
                    .map(|(_, d)| d)
                    .min()
                    .or_else(|| releases.iter().filter(|(p, _)| p.is_none()).map(|(_, d)| d).min())
                    .cloned();
                Some(PlatformRelease { name, released_at })
            })
            .collect();

        let cover = page.as_ref().and_then(|p| p.cover.clone());
        let mut screenshots = images;
        let mut trailers = Vec::new();
        let mut backdrop = None;
        let mut rating = None;
        if let Some(extras) = extras {
            // RAWG screenshots are larger and cleaner, so they go first.
            screenshots = extras.screenshots.into_iter().chain(screenshots).collect();
            trailers = extras.trailers;
            backdrop = extras.background;
            rating = extras.rating;
        }
        screenshots.dedup();

        let details = GameDetails {
            summary: entity.description.clone(),
            sections: page.as_ref().map(|p| p.sections.clone()).unwrap_or_default(),
            released: entity.earliest_release(),
            image: cover,
            backdrop,
            website: entity.strings("P856").into_iter().next(),
            wikipedia_url: page.map(|p| p.url),
            wikidata_url: format!("https://www.wikidata.org/wiki/{}", entity.id),
            critic_score: entity.critic_score(Some(PS4)),
            genres: names(&labels, entity.items("P136").iter()),
            platforms,
            developers: names(&labels, entity.items("P178").iter()),
            publishers: names(&labels, entity.items("P123").iter()),
            series: names(&labels, entity.items("P179").iter()),
            modes: names(&labels, entity.items("P404").iter()),
            age_ratings: names(&labels, entity.items("P852").iter().chain(entity.items("P908").iter())),
            alternative_names: entity.aliases.iter().filter(|a| **a != name).cloned().collect(),
            consoles: consoles(&entity),
            screenshots,
            trailers,
            rawg: rating,
            id: entity.id.clone(),
            name,
        };

        let mut cache = lock(&self.details);
        if cache.len() >= DETAILS_CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(cache_key, details.clone());
        Ok(details)
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn names<'a>(labels: &HashMap<String, String>, ids: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in ids {
        if let Some(label) = labels.get(id) {
            if !out.contains(label) {
                out.push(label.clone());
            }
        }
    }
    out
}

fn consoles(entity: &Entity) -> Vec<String> {
    [(PS4, "PS4"), (PS5, "PS5")]
        .into_iter()
        .filter(|(q, _)| entity.has_item("P400", q))
        .map(|(_, n)| n.to_string())
        .collect()
}

// ---- HTTP -----------------------------------------------------------------

#[derive(Clone, Copy)]
enum Service {
    Wikimedia,
    Rawg,
}

async fn get_json(client: &Client, url: Url, service: Service) -> AppResult<Value> {
    let response = client.get(url).send().await.map_err(|e| from_reqwest(&e.without_url()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(status_error(status, service));
    }
    let body = response.bytes().await.map_err(|e| from_reqwest(&e.without_url()))?;
    serde_json::from_slice(&body).map_err(|e| {
        AppError::new(ErrorCode::ServerError, "The game database sent a response PS4 Downloader couldn't read.")
            .with_details(e.to_string())
    })
}

fn status_error(status: StatusCode, service: Service) -> AppError {
    let name = match service {
        Service::Wikimedia => "Wikipedia",
        Service::Rawg => "RAWG",
    };
    let details = format!("{name} returned HTTP {}", status.as_u16());
    let (code, message) = match (status, service) {
        (StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN, Service::Rawg) => {
            (ErrorCode::ApiKey, "RAWG didn't accept your API key. Check the key in Settings.".to_string())
        }
        (StatusCode::TOO_MANY_REQUESTS, _) => (
            ErrorCode::RateLimited,
            format!("{name} is getting too many requests right now. Wait a few seconds and try again."),
        ),
        (s, _) if s.is_server_error() => {
            (ErrorCode::ServerError, format!("{name} is temporarily unavailable. Try again later."))
        }
        (StatusCode::NOT_FOUND, _) => (ErrorCode::NotFound, "That game couldn't be found.".to_string()),
        _ => (ErrorCode::HttpStatus, format!("{name} returned an unexpected response.")),
    };
    AppError::new(code, message).with_details(details)
}

fn api_url(base: &str, params: &[(&str, &str)]) -> AppResult<Url> {
    Url::parse_with_params(base, params).map_err(|e| AppError::internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_http_errors() {
        assert_eq!(status_error(StatusCode::UNAUTHORIZED, Service::Rawg).code, ErrorCode::ApiKey);
        assert_eq!(status_error(StatusCode::FORBIDDEN, Service::Wikimedia).code, ErrorCode::HttpStatus);
        assert_eq!(status_error(StatusCode::TOO_MANY_REQUESTS, Service::Wikimedia).code, ErrorCode::RateLimited);
        assert_eq!(status_error(StatusCode::BAD_GATEWAY, Service::Rawg).code, ErrorCode::ServerError);
    }
}
