//! HTTP client setup, URL validation, metadata probing and response parsing.

use crate::error::{error_chain, AppError, AppResult, ErrorCode};
use crate::storage::filename;
use percent_encoding::percent_decode_str;
use reqwest::header::{self, HeaderMap};
use reqwest::{redirect, Client, StatusCode, Url};
use std::time::Duration;

const MAX_REDIRECTS: usize = 10;

pub fn build_client() -> Client {
    let policy = redirect::Policy::custom(|attempt| {
        let secure_before = attempt.previous().iter().any(|u| u.scheme() == "https");
        if attempt.previous().len() >= MAX_REDIRECTS {
            attempt.error("too many redirects")
        } else if !matches!(attempt.url().scheme(), "http" | "https") {
            attempt.error("redirected to an unsupported protocol")
        } else if secure_before && attempt.url().scheme() == "http" {
            attempt.error("redirected from HTTPS to insecure HTTP")
        } else {
            attempt.follow()
        }
    });
    Client::builder()
        .user_agent(concat!("PS4Downloader/", env!("CARGO_PKG_VERSION")))
        .redirect(policy)
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .build()
        .expect("static HTTP client configuration is valid")
}

pub fn validate_url(input: &str) -> AppResult<Url> {
    let trimmed = input.trim();
    let url = Url::parse(trimmed).map_err(|e| {
        AppError::new(
            ErrorCode::InvalidUrl,
            "That doesn't look like a valid link. Paste a full URL starting with http:// or https://.",
        )
        .with_details(e.to_string())
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::new(
            ErrorCode::UnsupportedProtocol,
            format!("Only http:// and https:// links are supported (this link uses {}://).", url.scheme()),
        ));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(AppError::new(ErrorCode::InvalidUrl, "The link is missing a server name."));
    }
    Ok(url)
}

/// What the server told us about a file before downloading it.
#[derive(Debug, Clone)]
pub struct Probe {
    pub final_url: Url,
    pub file_name: String,
    pub total_size: Option<u64>,
    pub supports_range: bool,
    /// ETag or Last-Modified, used with If-Range to detect a changed file on resume.
    pub validator: Option<String>,
}

/// Asks for the first byte of the file. A `206` with `Content-Range` proves
/// that range requests work and reveals the full size in a single request.
pub async fn probe(client: &Client, url: &Url) -> AppResult<Probe> {
    let response = client
        .get(url.clone())
        .header(header::RANGE, "bytes=0-0")
        .send()
        .await
        .map_err(|e| from_reqwest(&e))?;

    // An empty file can't satisfy "bytes=0-0"; ask again without a range.
    let response = if response.status() == StatusCode::RANGE_NOT_SATISFIABLE {
        client.get(url.clone()).send().await.map_err(|e| from_reqwest(&e))?
    } else {
        response
    };

    let status = response.status();
    if !status.is_success() {
        return Err(status_error(status));
    }
    let headers = response.headers();
    if is_html(headers) {
        return Err(AppError::new(
            ErrorCode::NotAFile,
            "This link opens a web page, not a file. It may need a browser, a login, or a click on a download button first — PS4 Downloader only handles direct file links.",
        )
        .with_details(format!("Content-Type: {}", header_str(headers, header::CONTENT_TYPE).unwrap_or_default())));
    }

    let content_range = header_str(headers, header::CONTENT_RANGE).and_then(parse_content_range);
    let (supports_range, total_size) = match (status, content_range) {
        (StatusCode::PARTIAL_CONTENT, Some(range)) => (true, range.total),
        _ => (false, response.content_length()),
    };

    let final_url = response.url().clone();
    let file_name = header_str(headers, header::CONTENT_DISPOSITION)
        .and_then(parse_content_disposition)
        .or_else(|| filename_from_url(&final_url))
        .or_else(|| filename_from_url(url))
        .map(|n| filename::sanitize(&n))
        .unwrap_or_else(|| filename::sanitize(""));

    let validator = header_str(headers, header::ETAG)
        .filter(|etag| !etag.starts_with("W/")) // weak ETags are not allowed in If-Range
        .or_else(|| header_str(headers, header::LAST_MODIFIED))
        .map(str::to_string);

    Ok(Probe { final_url, file_name, total_size, supports_range, validator })
}

fn header_str(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim).filter(|v| !v.is_empty())
}

fn is_html(headers: &HeaderMap) -> bool {
    header_str(headers, header::CONTENT_TYPE).is_some_and(|ct| {
        let ct = ct.to_ascii_lowercase();
        ct.starts_with("text/html") || ct.starts_with("application/xhtml")
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentRange {
    pub start: u64,
    pub end: u64,
    pub total: Option<u64>,
}

/// Parses `bytes <start>-<end>/<total|*>`.
pub fn parse_content_range(value: &str) -> Option<ContentRange> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (range, total) = rest.split_once('/')?;
    let (start, end) = range.trim().split_once('-')?;
    let start = start.trim().parse().ok()?;
    let end = end.trim().parse().ok()?;
    let total = match total.trim() {
        "*" => None,
        t => Some(t.parse().ok()?),
    };
    (end >= start).then_some(ContentRange { start, end, total })
}

/// Extracts a file name from a Content-Disposition header, preferring the
/// RFC 5987 `filename*` form. The result still needs sanitizing.
pub fn parse_content_disposition(value: &str) -> Option<String> {
    let mut plain = None;
    let mut extended = None;
    for param in split_params(value) {
        let Some((key, raw)) = param.split_once('=') else { continue };
        match key.trim().to_ascii_lowercase().as_str() {
            "filename*" => {
                let raw = raw.trim().trim_matches('"');
                let mut parts = raw.splitn(3, '\'');
                let (charset, _lang, encoded) = (parts.next(), parts.next(), parts.next());
                if let Some(encoded) = encoded {
                    let decoded = percent_decode_str(encoded);
                    let text = if charset.is_some_and(|c| c.eq_ignore_ascii_case("utf-8")) {
                        decoded.decode_utf8().ok().map(|c| c.into_owned())
                    } else {
                        Some(decoded.decode_utf8_lossy().into_owned())
                    };
                    extended = text.filter(|t| !t.trim().is_empty());
                }
            }
            "filename" => plain = Some(unquote(raw.trim())).filter(|t| !t.trim().is_empty()),
            _ => {}
        }
    }
    extended.or(plain)
}

/// Splits on `;` outside of quoted strings.
fn split_params(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut start, mut in_quotes, mut escaped) = (0, false, false);
    for (i, c) in value.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            ';' if !in_quotes => {
                parts.push(&value[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&value[start..]);
    parts
}

fn unquote(value: &str) -> String {
    let Some(inner) = value.strip_prefix('"') else { return value.to_string() };
    let mut out = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // Only `\"` and `\\` are escapes; servers often send bare Windows paths.
            '\\' if matches!(chars.peek(), Some('"' | '\\')) => out.extend(chars.next()),
            '"' => break,
            c => out.push(c),
        }
    }
    out
}

pub fn filename_from_url(url: &Url) -> Option<String> {
    let segment = url.path_segments()?.rev().find(|s| !s.is_empty())?;
    let decoded = percent_decode_str(segment).decode_utf8_lossy().into_owned();
    Some(decoded).filter(|s| !s.trim().is_empty())
}

pub fn status_error(status: StatusCode) -> AppError {
    let details = format!("HTTP {status}");
    let (code, message) = match status.as_u16() {
        401 | 407 => (
            ErrorCode::AuthRequired,
            "This file requires signing in. PS4 Downloader only downloads files that are directly accessible and can't use website logins or browser sessions.".to_string(),
        ),
        403 => (
            ErrorCode::Forbidden,
            "The server refused access to this file (HTTP 403). The link may have expired or may only work inside a browser session.".to_string(),
        ),
        404 | 410 => (
            ErrorCode::NotFound,
            "The file was not found on the server (HTTP 404). Check that the link is correct.".to_string(),
        ),
        429 => (
            ErrorCode::RateLimited,
            "The server is limiting requests right now. PS4 Downloader will wait before trying again.".to_string(),
        ),
        500..=599 => (
            ErrorCode::ServerError,
            format!("The server is temporarily unavailable (HTTP {}). Try again later.", status.as_u16()),
        ),
        code => (ErrorCode::HttpStatus, format!("The server returned an unexpected response (HTTP {code}).")),
    };
    AppError::new(code, message).with_details(details)
}

pub fn from_reqwest(err: &reqwest::Error) -> AppError {
    let details = error_chain(err);
    let (code, message) = if err.is_redirect() {
        (ErrorCode::Redirect, "The link redirected too many times or to an unsafe location, so the download was stopped.")
    } else if err.is_timeout() {
        (ErrorCode::Timeout, "The server stopped responding. Check your internet connection and try again.")
    } else if err.is_connect() {
        (ErrorCode::Network, "Unable to connect to the server.\nCheck your internet connection and try again.")
    } else if err.is_builder() {
        (ErrorCode::InvalidUrl, "This link can't be used for downloading.")
    } else {
        (ErrorCode::Network, "The connection to the server was interrupted.")
    };
    AppError::new(code, message).with_details(details)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_urls() {
        assert!(validate_url(" https://example.com/file.zip ").is_ok());
        assert!(validate_url("http://example.com").is_ok());
        assert_eq!(validate_url("ftp://example.com/a").unwrap_err().code, ErrorCode::UnsupportedProtocol);
        assert_eq!(validate_url("file:///C:/Windows/win.ini").unwrap_err().code, ErrorCode::UnsupportedProtocol);
        assert_eq!(validate_url("javascript:alert(1)").unwrap_err().code, ErrorCode::UnsupportedProtocol);
        assert_eq!(validate_url("not a url").unwrap_err().code, ErrorCode::InvalidUrl);
        assert_eq!(validate_url("").unwrap_err().code, ErrorCode::InvalidUrl);
    }

    #[test]
    fn parses_content_range() {
        assert_eq!(
            parse_content_range("bytes 0-0/12345"),
            Some(ContentRange { start: 0, end: 0, total: Some(12345) })
        );
        assert_eq!(
            parse_content_range("bytes 100-199/*"),
            Some(ContentRange { start: 100, end: 199, total: None })
        );
        assert_eq!(
            parse_content_range("bytes 0-99/10000000000").unwrap().total,
            Some(10_000_000_000)
        );
        assert_eq!(parse_content_range("bytes */1000"), None);
        assert_eq!(parse_content_range("bytes 9-1/10"), None);
        assert_eq!(parse_content_range("garbage"), None);
    }

    #[test]
    fn parses_content_disposition() {
        assert_eq!(parse_content_disposition("attachment; filename=\"Game A.zip\"").as_deref(), Some("Game A.zip"));
        assert_eq!(parse_content_disposition("attachment; filename=plain.bin").as_deref(), Some("plain.bin"));
        assert_eq!(
            parse_content_disposition("attachment; filename=\"fallback.zip\"; filename*=UTF-8''%E6%97%A5%E6%9C%AC.zip").as_deref(),
            Some("日本.zip")
        );
        assert_eq!(parse_content_disposition("attachment; filename=\"a;b \\\"c\\\".zip\"").as_deref(), Some("a;b \"c\".zip"));
        assert_eq!(parse_content_disposition("inline"), None);
    }

    #[test]
    fn traversal_in_content_disposition_is_neutralized() {
        let raw = parse_content_disposition("attachment; filename=\"..\\..\\Windows\\evil.exe\"").unwrap();
        assert_eq!(filename::sanitize(&raw), "evil.exe");
    }

    #[test]
    fn filename_from_url_decodes() {
        let url = Url::parse("https://example.com/files/My%20Game%20(EU).pkg?token=abc").unwrap();
        assert_eq!(filename_from_url(&url).as_deref(), Some("My Game (EU).pkg"));
        let url = Url::parse("https://example.com/").unwrap();
        assert_eq!(filename_from_url(&url), None);
    }
}
