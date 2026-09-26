//! Game search and details from the RAWG video game database (https://rawg.io/apidocs).
//! Only metadata is fetched here; downloads still go through the download manager.

use crate::downloader::http::from_reqwest;
use crate::error::{AppError, AppResult, ErrorCode};
use reqwest::{Client, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const API_BASE: &str = "https://api.rawg.io/api";
const PAGE_SIZE: u32 = 24;
const MAX_TAGS: usize = 12;

/// RAWG platform ids.
pub const PLATFORM_PS4: u32 = 18;
pub const PLATFORM_PS5: u32 = 187;

pub struct GameDb {
    client: Client,
}

impl Default for GameDb {
    fn default() -> Self {
        Self::new()
    }
}

// ---- Types sent to the frontend ------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
    pub id: u64,
    pub slug: String,
    pub name: String,
    pub released: Option<String>,
    pub tba: bool,
    pub image: Option<String>,
    pub rating: f32,
    pub ratings_count: u32,
    pub metacritic: Option<u32>,
    pub genres: Vec<String>,
    pub platforms: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSearchPage {
    pub results: Vec<GameSummary>,
    pub count: u64,
    pub next_page: Option<u32>,
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
pub struct GameDetails {
    pub id: u64,
    pub slug: String,
    pub name: String,
    pub alternative_names: Vec<String>,
    pub description: String,
    pub released: Option<String>,
    pub tba: bool,
    pub image: Option<String>,
    pub image_additional: Option<String>,
    pub website: Option<String>,
    pub rating: f32,
    pub rating_top: u32,
    pub ratings_count: u32,
    pub metacritic: Option<u32>,
    pub playtime: u32,
    pub esrb: Option<String>,
    pub genres: Vec<String>,
    pub platforms: Vec<PlatformRelease>,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    pub stores: Vec<String>,
    pub tags: Vec<String>,
    pub screenshots: Vec<String>,
    pub trailers: Vec<Trailer>,
    pub rawg_url: String,
}

// ---- RAWG response shapes (only the fields we use) ------------------------

#[derive(Deserialize)]
struct Named {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct Tag {
    #[serde(default)]
    name: String,
    #[serde(default)]
    language: String,
}

#[derive(Deserialize)]
struct PlatformEntry {
    platform: Named,
    #[serde(default)]
    released_at: Option<String>,
}

#[derive(Deserialize)]
struct StoreEntry {
    store: Named,
}

#[derive(Deserialize)]
struct RawGame {
    id: u64,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    released: Option<String>,
    #[serde(default)]
    tba: bool,
    #[serde(default)]
    background_image: Option<String>,
    #[serde(default)]
    rating: f32,
    #[serde(default)]
    ratings_count: u32,
    #[serde(default)]
    metacritic: Option<u32>,
    #[serde(default, deserialize_with = "null_as_default")]
    genres: Vec<Named>,
    #[serde(default, deserialize_with = "null_as_default")]
    platforms: Vec<PlatformEntry>,
}

#[derive(Deserialize)]
struct RawDetails {
    #[serde(flatten)]
    game: RawGame,
    #[serde(default, deserialize_with = "null_as_default")]
    alternative_names: Vec<String>,
    #[serde(default)]
    description_raw: String,
    #[serde(default)]
    background_image_additional: Option<String>,
    #[serde(default)]
    website: Option<String>,
    #[serde(default)]
    rating_top: u32,
    #[serde(default)]
    playtime: u32,
    #[serde(default)]
    esrb_rating: Option<Named>,
    #[serde(default, deserialize_with = "null_as_default")]
    developers: Vec<Named>,
    #[serde(default, deserialize_with = "null_as_default")]
    publishers: Vec<Named>,
    #[serde(default, deserialize_with = "null_as_default")]
    stores: Vec<StoreEntry>,
    #[serde(default, deserialize_with = "null_as_default")]
    tags: Vec<Tag>,
}

#[derive(Deserialize)]
struct RawList<T> {
    #[serde(default)]
    count: u64,
    #[serde(default)]
    next: Option<String>,
    #[serde(default = "Vec::new")]
    results: Vec<T>,
}

#[derive(Deserialize)]
struct RawScreenshot {
    image: String,
}

#[derive(Deserialize)]
struct RawMovie {
    #[serde(default)]
    name: String,
    #[serde(default)]
    preview: Option<String>,
    data: RawMovieData,
}

#[derive(Deserialize)]
struct RawMovieData {
    #[serde(rename = "480", default)]
    low: Option<String>,
    #[serde(default)]
    max: Option<String>,
}

fn null_as_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

fn names(items: Vec<Named>) -> Vec<String> {
    items.into_iter().map(|n| n.name).filter(|n| !n.is_empty()).collect()
}

/// RAWG sometimes returns empty strings instead of null.
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

impl From<RawGame> for GameSummary {
    fn from(g: RawGame) -> Self {
        Self {
            id: g.id,
            slug: g.slug,
            name: g.name,
            released: non_empty(g.released),
            tba: g.tba,
            image: non_empty(g.background_image),
            rating: g.rating,
            ratings_count: g.ratings_count,
            metacritic: g.metacritic,
            genres: names(g.genres),
            platforms: g.platforms.into_iter().map(|p| p.platform.name).collect(),
        }
    }
}

// ---- Client ---------------------------------------------------------------

impl GameDb {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(concat!("PS4Downloader/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("static HTTP client configuration is valid");
        Self { client }
    }

    /// Searches RAWG. `platforms` narrows results to those RAWG platform ids (empty = all).
    pub async fn search(&self, key: &str, query: &str, page: u32, platforms: &[u32]) -> AppResult<GameSearchPage> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(GameSearchPage { results: Vec::new(), count: 0, next_page: None });
        }
        let page = page.max(1);
        let mut params = vec![
            ("search", query.to_string()),
            ("page", page.to_string()),
            ("page_size", PAGE_SIZE.to_string()),
        ];
        if !platforms.is_empty() {
            let ids: Vec<String> = platforms.iter().map(u32::to_string).collect();
            params.push(("platforms", ids.join(",")));
        }
        let list: RawList<RawGame> = self.get(key, "games", &params).await?;
        Ok(GameSearchPage {
            next_page: list.next.is_some().then_some(page + 1),
            count: list.count,
            results: list.results.into_iter().map(GameSummary::from).collect(),
        })
    }

    /// Full details for one game, including screenshots and trailers.
    pub async fn details(&self, key: &str, id: u64) -> AppResult<GameDetails> {
        let base = format!("games/{id}");
        let screenshots_path = format!("{base}/screenshots");
        let movies_path = format!("{base}/movies");
        let (raw, screenshots, movies) = tokio::join!(
            self.get::<RawDetails>(key, &base, &[]),
            self.get::<RawList<RawScreenshot>>(key, &screenshots_path, &[]),
            self.get::<RawList<RawMovie>>(key, &movies_path, &[]),
        );
        let raw = raw?;
        // Media is optional; a failure there shouldn't hide the details.
        let screenshots = screenshots.map(|l| l.results).unwrap_or_default();
        let movies = movies.map(|l| l.results).unwrap_or_default();

        let RawDetails {
            game,
            alternative_names,
            description_raw,
            background_image_additional,
            website,
            rating_top,
            playtime,
            esrb_rating,
            developers,
            publishers,
            stores,
            tags,
        } = raw;
        Ok(GameDetails {
            id: game.id,
            rawg_url: format!("https://rawg.io/games/{}", game.slug),
            slug: game.slug,
            name: game.name,
            alternative_names,
            description: description_raw.trim().to_string(),
            released: non_empty(game.released),
            tba: game.tba,
            image: non_empty(game.background_image),
            image_additional: non_empty(background_image_additional),
            website: non_empty(website),
            rating: game.rating,
            rating_top,
            ratings_count: game.ratings_count,
            metacritic: game.metacritic,
            playtime,
            esrb: esrb_rating.map(|e| e.name).filter(|n| !n.is_empty()),
            genres: names(game.genres),
            platforms: game
                .platforms
                .into_iter()
                .map(|p| PlatformRelease { name: p.platform.name, released_at: non_empty(p.released_at) })
                .collect(),
            developers: names(developers),
            publishers: names(publishers),
            stores: stores.into_iter().map(|s| s.store.name).filter(|n| !n.is_empty()).collect(),
            tags: tags
                .into_iter()
                .filter(|t| t.language == "eng" && !t.name.is_empty())
                .map(|t| t.name)
                .take(MAX_TAGS)
                .collect(),
            screenshots: screenshots.into_iter().map(|s| s.image).collect(),
            trailers: movies
                .into_iter()
                .filter_map(|m| {
                    let url = m.data.max.or(m.data.low)?;
                    Some(Trailer { name: m.name, preview: non_empty(m.preview), url })
                })
                .collect(),
        })
    }

    async fn get<T: DeserializeOwned>(&self, key: &str, path: &str, params: &[(&str, String)]) -> AppResult<T> {
        let key = key.trim();
        if key.is_empty() {
            return Err(missing_key());
        }
        let mut url = Url::parse(&format!("{API_BASE}/{path}")).map_err(|e| AppError::internal(e.to_string()))?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("key", key);
            for (name, value) in params {
                query.append_pair(name, value);
            }
        }
        let response = self.client.get(url).send().await.map_err(|e| from_reqwest(&e.without_url()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(status_error(status));
        }
        let body = response.bytes().await.map_err(|e| from_reqwest(&e.without_url()))?;
        serde_json::from_slice(&body).map_err(|e| {
            AppError::new(ErrorCode::ServerError, "RAWG sent a response PS4 Downloader couldn't read. Try again later.")
                .with_details(e.to_string())
        })
    }
}

pub fn missing_key() -> AppError {
    AppError::new(
        ErrorCode::ApiKey,
        "Add your free RAWG API key to search for games. You can get one at rawg.io/apidocs.",
    )
}

fn status_error(status: StatusCode) -> AppError {
    let details = format!("RAWG returned HTTP {}", status.as_u16());
    let (code, message) = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            (ErrorCode::ApiKey, "RAWG didn't accept your API key. Check the key in Settings.")
        }
        StatusCode::NOT_FOUND => (ErrorCode::NotFound, "RAWG doesn't have a page for that game."),
        StatusCode::TOO_MANY_REQUESTS => {
            (ErrorCode::RateLimited, "Too many searches in a short time. Wait a moment and try again.")
        }
        s if s.is_server_error() => (ErrorCode::ServerError, "RAWG is temporarily unavailable. Try again later."),
        _ => (ErrorCode::HttpStatus, "RAWG returned an unexpected response."),
    };
    AppError::new(code, message).with_details(details)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_results() {
        let json = r#"{
            "count": 2, "next": "https://api.rawg.io/api/games?page=2",
            "results": [
                {"id": 1, "slug": "bloodborne", "name": "Bloodborne", "released": "2015-03-24",
                 "background_image": "https://media.rawg.io/a.jpg", "rating": 4.4, "ratings_count": 10,
                 "metacritic": 92, "genres": [{"name": "Action"}],
                 "platforms": [{"platform": {"id": 18, "name": "PlayStation 4"}}]},
                {"id": 2, "slug": "x", "name": "X", "released": null, "background_image": "",
                 "genres": null, "platforms": null}
            ]
        }"#;
        let list: RawList<RawGame> = serde_json::from_str(json).unwrap();
        assert_eq!(list.count, 2);
        let games: Vec<GameSummary> = list.results.into_iter().map(GameSummary::from).collect();
        assert_eq!(games[0].platforms, ["PlayStation 4"]);
        assert_eq!(games[0].metacritic, Some(92));
        assert_eq!(games[1].image, None);
        assert!(games[1].genres.is_empty());
    }

    #[test]
    fn parses_details_and_movies() {
        let json = r#"{"id": 1, "slug": "s", "name": "S", "description_raw": " Text ",
            "esrb_rating": null, "developers": [{"name": "FromSoftware"}], "publishers": null,
            "tags": [{"name": "Souls-like", "language": "eng"}, {"name": "x", "language": "rus"}],
            "stores": [{"store": {"name": "PlayStation Store"}}]}"#;
        let raw: RawDetails = serde_json::from_str(json).unwrap();
        assert_eq!(raw.game.name, "S");
        assert_eq!(names(raw.developers), ["FromSoftware"]);
        assert_eq!(raw.tags.len(), 2);

        let movie: RawMovie =
            serde_json::from_str(r#"{"name": "Trailer", "preview": "p.jpg", "data": {"480": "a.mp4", "max": "b.mp4"}}"#)
                .unwrap();
        assert_eq!(movie.data.max.as_deref(), Some("b.mp4"));
    }

    #[test]
    fn maps_http_errors() {
        assert_eq!(status_error(StatusCode::UNAUTHORIZED).code, ErrorCode::ApiKey);
        assert_eq!(status_error(StatusCode::TOO_MANY_REQUESTS).code, ErrorCode::RateLimited);
        assert_eq!(status_error(StatusCode::BAD_GATEWAY).code, ErrorCode::ServerError);
    }
}
