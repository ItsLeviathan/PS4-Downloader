use crate::error::{AppError, AppResult, ErrorCode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_STORAGE_ROOT: &str = r"F:\PS4 Games";
pub const ALLOWED_CONNECTIONS: [u8; 4] = [1, 2, 4, 8];
pub const MAX_SIMULTANEOUS: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub storage_root: PathBuf,
    pub max_simultaneous: u8,
    pub connections: u8,
    /// Resume interrupted and queued downloads automatically when the app starts.
    pub auto_start: bool,
    pub notify_completed: bool,
    pub notify_failed: bool,
    pub notify_storage: bool,
    pub theme: Theme,
    /// Key for the RAWG game database used by game search.
    pub rawg_api_key: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            storage_root: PathBuf::from(DEFAULT_STORAGE_ROOT),
            max_simultaneous: 2,
            connections: 4,
            auto_start: false,
            notify_completed: true,
            notify_failed: true,
            notify_storage: true,
            theme: Theme::Dark,
            rawg_api_key: String::new(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> AppResult<()> {
        if !(1..=MAX_SIMULTANEOUS).contains(&self.max_simultaneous) {
            return Err(AppError::new(
                ErrorCode::InvalidSettings,
                format!("Simultaneous downloads must be between 1 and {MAX_SIMULTANEOUS}."),
            ));
        }
        if !ALLOWED_CONNECTIONS.contains(&self.connections) {
            return Err(AppError::new(
                ErrorCode::InvalidSettings,
                "Connections per download must be 1, 2, 4 or 8.",
            ));
        }
        if !self.storage_root.is_absolute() {
            return Err(AppError::new(
                ErrorCode::InvalidSettings,
                "The download location must be a full folder path, for example F:\\PS4 Games.",
            ));
        }
        Ok(())
    }

    /// Repairs values that may have been hand-edited or come from an older version.
    pub fn sanitized(mut self) -> Self {
        let defaults = Self::default();
        if !(1..=MAX_SIMULTANEOUS).contains(&self.max_simultaneous) {
            self.max_simultaneous = defaults.max_simultaneous;
        }
        if !ALLOWED_CONNECTIONS.contains(&self.connections) {
            self.connections = defaults.connections;
        }
        if !self.storage_root.is_absolute() {
            self.storage_root = defaults.storage_root;
        }
        self.rawg_api_key = self.rawg_api_key.trim().to_string();
        self
    }
}
