//! English Wikipedia: article text, cover art and in-article screenshots.
//! https://en.wikipedia.org/w/api.php

use super::{api_url, get_json, Service};
use crate::error::AppResult;
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

const API: &str = "https://en.wikipedia.org/w/api.php";
const BATCH: usize = 50;
const COVER_WIDTH: &str = "600";
const SCREENSHOT_WIDTH: &str = "1280";
const MIN_SCREENSHOT_WIDTH: u64 = 300;
const MAX_SCREENSHOTS: usize = 12;
const MAX_SECTIONS: usize = 8;
const MAX_SECTION_CHARS: usize = 9000;

/// Sections that are lists of links or citations rather than prose.
const SKIPPED_SECTIONS: [&str; 10] = [
    "references",
    "notes",
    "external links",
    "see also",
    "further reading",
    "citations",
    "footnotes",
    "bibliography",
    "sources",
    "works cited",
];

/// File names that suggest an in-game image.
const SCREENSHOT_HINTS: [&str; 5] = ["screenshot", "gameplay", "screen", "in-game", "ingame"];

/// File names that are icons or logos rather than screenshots.
const SKIPPED_FILES: [&str; 9] =
    ["logo", "icon", "cover", "wordmark", "symbol", "ambox", "commons-", "question_book", "edit-clear"];

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "text", rename_all = "camelCase")]
pub enum Block {
    Heading(String),
    Text(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub title: String,
    pub blocks: Vec<Block>,
}

pub struct Page {
    pub url: String,
    pub cover: Option<String>,
    pub sections: Vec<Section>,
}

/// Lead image (usually the box art) for each title, keyed by the title as given.
pub async fn thumbnails(client: &Client, titles: &[String], width: u32) -> AppResult<HashMap<String, String>> {
    let width = width.to_string();
    let mut out = HashMap::new();
    for chunk in titles.chunks(BATCH) {
        let joined = chunk.join("|");
        let url = api_url(
            API,
            &[
                ("action", "query"),
                ("prop", "pageimages"),
                ("piprop", "thumbnail"),
                ("pithumbsize", &width),
                ("pilimit", "50"),
                // Box art is non-free, so it must be allowed explicitly.
                ("pilicense", "any"),
                ("titles", &joined),
                ("format", "json"),
                ("formatversion", "2"),
            ],
        )?;
        let json = get_json(client, url, Service::Wikimedia).await?;
        out.extend(parse_thumbnails(&json));
    }
    Ok(out)
}

fn parse_thumbnails(json: &Value) -> HashMap<String, String> {
    // Map normalized titles back to the ones we asked for.
    let mut original: HashMap<&str, &str> = HashMap::new();
    for n in json["query"]["normalized"].as_array().into_iter().flatten() {
        if let (Some(from), Some(to)) = (n["from"].as_str(), n["to"].as_str()) {
            original.insert(to, from);
        }
    }
    let mut out = HashMap::new();
    for page in json["query"]["pages"].as_array().into_iter().flatten() {
        if let (Some(title), Some(src)) = (page["title"].as_str(), page["thumbnail"]["source"].as_str()) {
            let key = original.get(title).copied().unwrap_or(title);
            out.insert(key.to_string(), src.to_string());
        }
    }
    out
}

/// The article's text split into sections, its URL and its cover art.
pub async fn page(client: &Client, title: &str) -> AppResult<Page> {
    let url = api_url(
        API,
        &[
            ("action", "query"),
            ("prop", "extracts|pageimages|info"),
            ("explaintext", "1"),
            ("exsectionformat", "wiki"),
            ("piprop", "thumbnail"),
            ("pithumbsize", COVER_WIDTH),
            ("pilicense", "any"),
            ("inprop", "url"),
            ("redirects", "1"),
            ("titles", title),
            ("format", "json"),
            ("formatversion", "2"),
        ],
    )?;
    let json = get_json(client, url, Service::Wikimedia).await?;
    let page = &json["query"]["pages"][0];
    Ok(Page {
        url: page["fullurl"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| format!("https://en.wikipedia.org/wiki/{}", title.replace(' ', "_"))),
        cover: page["thumbnail"]["source"].as_str().map(String::from),
        sections: split_sections(page["extract"].as_str().unwrap_or_default()),
    })
}

/// Screenshots used in the article. Other images (the cover, logos, photos of
/// the team) are left out.
pub async fn screenshots(client: &Client, title: &str) -> AppResult<Vec<String>> {
    let url = api_url(
        API,
        &[
            ("action", "query"),
            ("generator", "images"),
            ("gimlimit", "40"),
            ("prop", "imageinfo"),
            ("iiprop", "url|mime|size"),
            ("iiurlwidth", SCREENSHOT_WIDTH),
            ("redirects", "1"),
            ("titles", title),
            ("format", "json"),
            ("formatversion", "2"),
        ],
    )?;
    let json = get_json(client, url, Service::Wikimedia).await?;
    Ok(parse_screenshots(&json))
}

fn parse_screenshots(json: &Value) -> Vec<String> {
    json["query"]["pages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let name = p["title"].as_str().unwrap_or_default().to_ascii_lowercase();
            if SKIPPED_FILES.iter().any(|s| name.contains(s)) {
                return None;
            }
            let info = &p["imageinfo"][0];
            let mime = info["mime"].as_str()?;
            let wide = info["width"].as_u64()? >= MIN_SCREENSHOT_WIDTH;
            if !wide || !(mime == "image/jpeg" || mime == "image/png") {
                return None;
            }
            if !SCREENSHOT_HINTS.iter().any(|h| name.contains(h)) {
                return None;
            }
            info["thumburl"].as_str().or(info["url"].as_str()).map(String::from)
        })
        .take(MAX_SCREENSHOTS)
        .collect()
}

/// Splits plain-text extract with "== Heading ==" markers into sections.
fn split_sections(text: &str) -> Vec<Section> {
    let mut sections = vec![Section { title: "Overview".into(), blocks: Vec::new() }];
    let mut paragraph = String::new();

    fn flush(paragraph: &mut String, section: &mut Section) {
        let text = paragraph.trim();
        if !text.is_empty() {
            section.blocks.push(Block::Text(text.to_string()));
        }
        paragraph.clear();
    }

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some((level, heading)) = parse_heading(trimmed) {
            let current = sections.last_mut().expect("at least one section");
            flush(&mut paragraph, current);
            if level == 2 {
                sections.push(Section { title: heading.to_string(), blocks: Vec::new() });
            } else if !heading.is_empty() {
                current.blocks.push(Block::Heading(heading.to_string()));
            }
        } else {
            // Each line of the plain-text extract is one paragraph.
            paragraph.push_str(trimmed);
            flush(&mut paragraph, sections.last_mut().expect("at least one section"));
        }
    }
    flush(&mut paragraph, sections.last_mut().expect("at least one section"));

    sections
        .into_iter()
        .filter(|s| !SKIPPED_SECTIONS.contains(&s.title.to_ascii_lowercase().as_str()))
        .map(|mut s| {
            // Drop headings with nothing after them, then cap very long sections.
            let mut kept: Vec<Block> = Vec::new();
            let mut chars = 0;
            for block in s.blocks {
                if chars >= MAX_SECTION_CHARS {
                    break;
                }
                if let Block::Text(t) = &block {
                    chars += t.len();
                }
                kept.push(block);
            }
            while matches!(kept.last(), Some(Block::Heading(_))) {
                kept.pop();
            }
            s.blocks = kept;
            s
        })
        .filter(|s| s.blocks.iter().any(|b| matches!(b, Block::Text(_))))
        .take(MAX_SECTIONS)
        .collect()
}

fn parse_heading(line: &str) -> Option<(usize, &str)> {
    let level = line.bytes().take_while(|b| *b == b'=').count();
    if level < 2 || !line.ends_with("==") {
        return None;
    }
    Some((level, line.trim_matches('=').trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn splits_article_sections() {
        let text = "Bloodborne is a game.\nIt was released in 2015.\n\n\n== Gameplay ==\nPlayers fight.\n\n=== Combat ===\nFast.\n=== Empty ===\n\n== Plot ==\n\n== References ==\nCite.\n";
        let sections = split_sections(text);
        let titles: Vec<&str> = sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Overview", "Gameplay"]);
        assert_eq!(sections[0].blocks.len(), 2);
        assert!(matches!(&sections[0].blocks[1], Block::Text(t) if t == "It was released in 2015."));
        assert_eq!(sections[1].blocks.len(), 3);
        assert!(matches!(&sections[1].blocks[1], Block::Heading(h) if h == "Combat"));
    }

    #[test]
    fn maps_normalized_thumbnail_titles() {
        let json = json!({ "query": {
            "normalized": [{ "from": "god of war", "to": "God of war" }],
            "pages": [{ "title": "God of war", "thumbnail": { "source": "a.jpg" } }, { "title": "X" }]
        }});
        let thumbs = parse_thumbnails(&json);
        assert_eq!(thumbs.get("god of war").map(String::as_str), Some("a.jpg"));
        assert_eq!(thumbs.len(), 1);
    }

    #[test]
    fn keeps_only_screenshots() {
        let json = json!({ "query": { "pages": [
            { "title": "File:Bloodborne gameplay.jpg", "imageinfo": [{ "mime": "image/jpeg", "width": 1920, "thumburl": "shot.jpg" }] },
            { "title": "File:Bloodborne logo.png", "imageinfo": [{ "mime": "image/png", "width": 1920, "thumburl": "logo.png" }] },
            { "title": "File:Tiny.png", "imageinfo": [{ "mime": "image/png", "width": 64, "thumburl": "tiny.png" }] },
            { "title": "File:Director at GDC.jpg", "imageinfo": [{ "mime": "image/jpeg", "width": 406, "thumburl": "person.jpg" }] },
            { "title": "File:Map.svg", "imageinfo": [{ "mime": "image/svg+xml", "width": 900, "thumburl": "map.png" }] }
        ]}});
        assert_eq!(parse_screenshots(&json), ["shot.jpg"]);
    }
}
