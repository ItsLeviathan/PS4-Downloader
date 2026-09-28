//! Catalog of PS4 homebrew, read from each project's official GitHub releases.
//! Only release metadata is fetched here; the files themselves go through the
//! download manager like any other link.

use crate::downloader::http::from_reqwest;
use crate::error::{AppError, AppResult, ErrorCode};
use futures_util::future::join_all;
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// GitHub rejects API requests without a User-Agent.
const USER_AGENT: &str = concat!(
    "PS4Downloader/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/ItsLeviathan/PS4-Downloader)"
);
/// Anonymous clients get 60 requests an hour, so one refresh of the catalog is kept this long.
const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
/// Some projects publish tag-only releases; look back this far for one with files.
const RELEASES_PER_REPO: &str = "10";
const NOTES_LIMIT: usize = 2000;
const DOWNLOADABLE: [&str; 4] = ["pkg", "bin", "zip", "7z"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Essentials,
    Tools,
    Payloads,
}

struct Entry {
    id: &'static str,
    name: &'static str,
    repo: &'static str,
    category: Category,
    description: &'static str,
}

const CATALOG: &[Entry] = &[
    Entry {
        id: "goldhen",
        name: "GoldHEN",
        repo: "GoldHEN/GoldHEN",
        category: Category::Essentials,
        description: "Homebrew enabler: loads unsigned apps, cheats, plugins and an FTP server after the exploit runs.",
    },
    Entry {
        id: "hb-store",
        name: "Homebrew Store",
        repo: "LightningMods/PS4-Store",
        category: Category::Essentials,
        description: "Browse and install homebrew apps straight from the console.",
    },
    Entry {
        id: "apollo",
        name: "Apollo Save Tool",
        repo: "bucanero/apollo-ps4",
        category: Category::Essentials,
        description: "Back up, restore, decrypt and patch your game saves.",
    },
    Entry {
        id: "goldhen-cheats",
        name: "GoldHEN Cheats Manager",
        repo: "GoldHEN/GoldHEN_Cheat_Manager",
        category: Category::Tools,
        description: "Download and manage GoldHEN cheat and patch files on the console.",
    },
    Entry {
        id: "goldhen-plugins",
        name: "GoldHEN Plugins",
        repo: "GoldHEN/GoldHEN_Plugins_Repository",
        category: Category::Tools,
        description: "Official GoldHEN plugins such as game patches, FPS counter and more.",
    },
    Entry {
        id: "remote-pkg-installer",
        name: "Remote PKG Installer",
        repo: "Backporter/ps4_remote_pkg_installer-OOSDK",
        category: Category::Tools,
        description: "Install PKG files sent from your PC over the local network.",
    },
    Entry {
        id: "payload-guest",
        name: "Payload Guest",
        repo: "Al-Azif/ps4-payload-guest",
        category: Category::Payloads,
        description: "Launch payloads stored on the console from a menu.",
    },
    Entry {
        id: "ftp",
        name: "FTP Server",
        repo: "hippie68/ps4-ftp",
        category: Category::Payloads,
        description: "FTP server payload for transferring files to and from the console.",
    },
    Entry {
        id: "ps4debug",
        name: "ps4debug",
        repo: "jogolden/ps4debug",
        category: Category::Payloads,
        description: "Debugging payload used by PC tools such as PS4 Cheater.",
    },
];

// ---- Types sent to the frontend ------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomebrewApp {
    pub id: String,
    pub name: String,
    pub category: Category,
    pub description: String,
    pub repo_url: String,
    pub release: Option<HomebrewRelease>,
    /// Set when this project's releases couldn't be read.
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomebrewRelease {
    pub version: String,
    pub published_at: Option<String>,
    pub notes: Option<String>,
    pub url: String,
    pub assets: Vec<HomebrewAsset>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomebrewAsset {
    pub name: String,
    pub size: u64,
    pub url: String,
    /// "sha256:<hex>" when GitHub publishes a digest for the file.
    pub checksum: Option<String>,
}

// ---- GitHub response ------------------------------------------------------

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    digest: Option<String>,
}

// ---- Client ---------------------------------------------------------------

pub struct HomebrewCatalog {
    client: Client,
    cache: Mutex<Option<(Instant, Vec<HomebrewApp>)>>,
}

impl Default for HomebrewCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl HomebrewCatalog {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(25))
            .build()
            .expect("static HTTP client configuration is valid");
        Self { client, cache: Mutex::new(None) }
    }

    /// Every catalog entry with its newest release. `refresh` skips the cache.
    pub async fn list(&self, refresh: bool) -> AppResult<Vec<HomebrewApp>> {
        if !refresh {
            if let Some((at, apps)) = &*self.lock() {
                if at.elapsed() < CACHE_TTL {
                    return Ok(apps.clone());
                }
            }
        }

        let results = join_all(CATALOG.iter().map(|e| latest_release(&self.client, e.repo))).await;
        // When nothing could be read (offline, rate limited) one error says it all.
        if let Some(Err(first)) = results.iter().find(|r| r.is_err()).filter(|_| results.iter().all(Result::is_err)) {
            return Err(first.clone());
        }

        let apps: Vec<HomebrewApp> = CATALOG
            .iter()
            .zip(results)
            .map(|(e, result)| {
                let (release, error) = match result {
                    Ok(release) => (release, None),
                    Err(err) => (None, Some(err)),
                };
                HomebrewApp {
                    id: e.id.to_string(),
                    name: e.name.to_string(),
                    category: e.category,
                    description: e.description.to_string(),
                    repo_url: format!("https://github.com/{}", e.repo),
                    release,
                    error,
                }
            })
            .collect();
        // Don't keep a partial catalog around; the next visit retries the failed ones.
        if apps.iter().all(|a| a.error.is_none()) {
            *self.lock() = Some((Instant::now(), apps.clone()));
        }
        Ok(apps)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<(Instant, Vec<HomebrewApp>)>> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The newest stable release that has files worth downloading.
async fn latest_release(client: &Client, repo: &str) -> AppResult<Option<HomebrewRelease>> {
    let url = Url::parse_with_params(
        &format!("https://api.github.com/repos/{repo}/releases"),
        [("per_page", RELEASES_PER_REPO)],
    )
    .map_err(|e| AppError::internal(e.to_string()))?;
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| from_reqwest(&e.without_url()))?;
    let status = response.status();
    if !status.is_success() {
        let remaining = header_u64(&response, "x-ratelimit-remaining");
        let reset = header_u64(&response, "x-ratelimit-reset");
        return Err(status_error(status, remaining, reset));
    }
    let body = response.bytes().await.map_err(|e| from_reqwest(&e.without_url()))?;
    let releases: Vec<GhRelease> = serde_json::from_slice(&body).map_err(|e| {
        AppError::new(ErrorCode::ServerError, "GitHub sent a response PS4 Downloader couldn't read.")
            .with_details(e.to_string())
    })?;
    Ok(pick_release(releases))
}

fn pick_release(releases: Vec<GhRelease>) -> Option<HomebrewRelease> {
    // Prereleases only count when a project has nothing stable with files.
    let stable = releases.iter().any(|r| !r.draft && !r.prerelease && has_files(r));
    let r = releases.into_iter().find(|r| !r.draft && (!stable || !r.prerelease) && has_files(r))?;
    let mut assets: Vec<HomebrewAsset> = r
        .assets
        .into_iter()
        .filter(|a| is_downloadable(&a.name))
        .map(|a| HomebrewAsset {
            checksum: a.digest.filter(|d| d.starts_with("sha256:")),
            name: a.name,
            size: a.size,
            url: a.browser_download_url,
        })
        .collect();
    // .pkg files are what gets installed, so list them first.
    assets.sort_by_key(|a| !a.name.to_ascii_lowercase().ends_with(".pkg"));
    Some(HomebrewRelease {
        version: r.tag_name,
        published_at: r.published_at,
        notes: r.body.map(|b| clip(b.trim(), NOTES_LIMIT)).filter(|b| !b.is_empty()),
        url: r.html_url,
        assets,
    })
}

fn has_files(release: &GhRelease) -> bool {
    release.assets.iter().any(|a| is_downloadable(&a.name))
}

fn is_downloadable(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| DOWNLOADABLE.iter().any(|d| ext.eq_ignore_ascii_case(d)))
}

fn clip(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((cut, _)) => format!("{}…", text[..cut].trim_end()),
        None => text.to_string(),
    }
}

fn header_u64(response: &reqwest::Response, name: &str) -> Option<u64> {
    response.headers().get(name)?.to_str().ok()?.parse().ok()
}

fn status_error(status: StatusCode, remaining: Option<u64>, reset: Option<u64>) -> AppError {
    let details = format!("GitHub returned HTTP {}", status.as_u16());
    let limited = status == StatusCode::TOO_MANY_REQUESTS || (status == StatusCode::FORBIDDEN && remaining == Some(0));
    let (code, message) = if limited {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let minutes = reset.map(|r| r.saturating_sub(now).div_ceil(60).max(1));
        let when = match minutes {
            Some(1) => "in about a minute".to_string(),
            Some(m) => format!("in about {m} minutes"),
            None => "later".to_string(),
        };
        (ErrorCode::RateLimited, format!("GitHub limits how often release lists can be read. Try again {when}."))
    } else if status == StatusCode::NOT_FOUND {
        (ErrorCode::NotFound, "This project's releases are no longer on GitHub.".to_string())
    } else if status.is_server_error() {
        (ErrorCode::ServerError, "GitHub is temporarily unavailable. Try again later.".to_string())
    } else {
        (ErrorCode::HttpStatus, "GitHub returned an unexpected response.".to_string())
    };
    AppError::new(code, message).with_details(details)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, prerelease: bool, files: &[&str]) -> GhRelease {
        GhRelease {
            tag_name: tag.to_string(),
            html_url: format!("https://github.com/o/r/releases/tag/{tag}"),
            body: Some("  notes  ".to_string()),
            published_at: None,
            draft: false,
            prerelease,
            assets: files
                .iter()
                .map(|f| GhAsset {
                    name: f.to_string(),
                    size: 1,
                    browser_download_url: format!("https://github.com/o/r/releases/download/{tag}/{f}"),
                    digest: Some("sha256:ab".to_string()),
                })
                .collect(),
        }
    }

    #[test]
    fn picks_newest_stable_release_with_files() {
        let picked = pick_release(vec![
            release("3.0-beta", true, &["app.pkg"]),
            release("2.1", false, &[]),
            release("2.0", false, &["source.tar.gz", "readme.txt", "payload.bin", "app.pkg"]),
        ])
        .unwrap();
        assert_eq!(picked.version, "2.0");
        assert_eq!(picked.notes.as_deref(), Some("notes"));
        let names: Vec<_> = picked.assets.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["app.pkg", "payload.bin"]);
        assert_eq!(picked.assets[0].checksum.as_deref(), Some("sha256:ab"));
    }

    #[test]
    fn falls_back_to_prerelease_and_none() {
        assert_eq!(pick_release(vec![release("1.0-rc", true, &["a.PKG"])]).unwrap().version, "1.0-rc");
        assert!(pick_release(vec![release("1.0", false, &["a.txt"])]).is_none());
    }

    #[test]
    fn maps_rate_limits() {
        assert_eq!(status_error(StatusCode::FORBIDDEN, Some(0), None).code, ErrorCode::RateLimited);
        assert_eq!(status_error(StatusCode::FORBIDDEN, Some(10), None).code, ErrorCode::HttpStatus);
        assert_eq!(status_error(StatusCode::NOT_FOUND, None, None).code, ErrorCode::NotFound);
    }

    #[test]
    fn clips_notes() {
        assert_eq!(clip("abcdef", 3), "abc…");
        assert_eq!(clip("abc", 3), "abc");
    }

    /// Hits the real GitHub API: `cargo test --lib homebrew -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn reads_live_catalog() {
        let apps = HomebrewCatalog::new().list(true).await.unwrap();
        for app in &apps {
            assert!(app.error.is_none(), "{}: {:?}", app.name, app.error);
            let release = app.release.as_ref().unwrap_or_else(|| panic!("{} has no release", app.name));
            println!("{} {} {:?}", app.name, release.version, release.assets.iter().map(|a| &a.name).collect::<Vec<_>>());
        }
    }
}
