use serde::{Deserialize, Serialize};
use std::fmt;

/// Machine-readable error category. The frontend uses it to pick icons and
/// actions; the user-facing text lives in `AppError::message`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    InvalidUrl,
    UnsupportedProtocol,
    Network,
    Timeout,
    NotFound,
    Forbidden,
    AuthRequired,
    RateLimited,
    ServerError,
    HttpStatus,
    NotAFile,
    Redirect,
    InsufficientSpace,
    StorageUnavailable,
    Io,
    ChecksumMismatch,
    SizeMismatch,
    InvalidChecksum,
    InvalidSettings,
    Duplicate,
    InvalidState,
    UnknownDownload,
    RangeNotHonored,
    ApiKey,
    Internal,
}

/// An error that is safe to show to the user. `message` is plain language;
/// `details` carries the technical cause for the "View details" toggle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default)]
    pub details: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), details: None }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Transient failures that are worth retrying automatically.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.code,
            ErrorCode::Network | ErrorCode::Timeout | ErrorCode::ServerError | ErrorCode::RateLimited
        )
    }

    /// A filesystem failure while doing `action` (e.g. "write to the download file").
    pub fn io(action: &str, err: &std::io::Error) -> Self {
        Self::new(ErrorCode::Io, format!("Could not {action}.")).with_details(err.to_string())
    }

    pub fn storage_unavailable(root: &std::path::Path) -> Self {
        Self::new(
            ErrorCode::StorageUnavailable,
            "Storage drive unavailable. Reconnect the drive to continue, or choose another folder in Settings.",
        )
        .with_details(format!("{} is not accessible", root.display()))
    }

    pub fn internal(details: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, "Something went wrong inside PS4 Downloader.").with_details(details)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.details {
            Some(d) => write!(f, "{} ({d})", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for AppError {}

/// Flattens an error and its sources into one line for `details`.
pub fn error_chain(err: &dyn std::error::Error) -> String {
    let mut out = err.to_string();
    let mut source = err.source();
    while let Some(e) = source {
        let text = e.to_string();
        if !out.contains(&text) {
            out.push_str(": ");
            out.push_str(&text);
        }
        source = e.source();
    }
    out
}
