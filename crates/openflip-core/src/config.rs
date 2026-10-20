//! User settings, stored as TOML in the operating system's config directory.

use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

use serde::{Deserialize, Serialize};

use crate::i18n::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeFormat {
    H12,
    #[default]
    H24,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MonitorMode {
    /// Show the clock on every connected display.
    #[default]
    All,
    /// Show the clock on the primary display and blank the others.
    Primary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub time_format: TimeFormat,
    pub show_seconds: bool,
    pub show_date: bool,
    /// Name of a built-in theme, e.g. `classic`.
    pub theme: String,
    /// Size of the clock relative to the screen; `1.0` fills the width.
    pub scale: f32,
    /// Optional path to a TTF/OTF font replacing the bundled one.
    pub font: Option<PathBuf>,
    pub monitors: MonitorMode,
    /// `None` follows the operating system's language.
    pub language: Option<Language>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            time_format: TimeFormat::H24,
            show_seconds: false,
            show_date: false,
            theme: "classic".into(),
            scale: 0.9,
            font: None,
            monitors: MonitorMode::All,
            language: None,
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Parse(toml::de::Error),
    Serialize(toml::ser::Error),
    NoConfigDir,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "could not access config file: {e}"),
            Self::Parse(e) => write!(f, "invalid config file: {e}"),
            Self::Serialize(e) => write!(f, "could not write config: {e}"),
            Self::NoConfigDir => write!(f, "no config directory found for this user"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// `~/Library/Application Support/org.openflip.openflip/config.toml` on macOS,
    /// `%APPDATA%\openflip\openflip\config\config.toml` on Windows,
    /// `$XDG_CONFIG_HOME/openflip/config.toml` on Linux.
    pub fn default_path() -> Result<PathBuf, ConfigError> {
        directories::ProjectDirs::from("org", "openflip", "openflip")
            .map(|dirs| dirs.config_dir().join("config.toml"))
            .ok_or(ConfigError::NoConfigDir)
    }

    /// Loads the config, falling back to defaults when the file does not exist yet.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(ConfigError::Parse),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(ConfigError::Io(e)),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let text = toml::to_string_pretty(self).map_err(ConfigError::Serialize)?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(ConfigError::Io)?;
        }
        fs::write(path, text).map_err(ConfigError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let config: Config = toml::from_str("time_format = \"h12\"").unwrap();
        assert_eq!(config.time_format, TimeFormat::H12);
        assert_eq!(config.theme, "classic");
    }

    #[test]
    fn round_trips_through_a_file() {
        let dir = std::env::temp_dir().join(format!("openflip-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let config = Config {
            show_date: true,
            language: Some(Language::German),
            monitors: MonitorMode::Primary,
            ..Config::default()
        };
        config.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), config);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_file_yields_defaults() {
        let path = Path::new("/definitely/not/there/config.toml");
        assert_eq!(Config::load(path).unwrap(), Config::default());
    }
}
