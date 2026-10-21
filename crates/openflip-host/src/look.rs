//! Everything derived from the config that is needed to draw a frame.

use std::path::Path;

use chrono::NaiveDateTime;
use openflip_core::{ClockFace, Config, DateLine, Language};
use openflip_render::overlay::{build_overlay, OverlayLayout};
use openflip_render::{layout, CardContent, CardFlip, Font, Scene, Theme};

use crate::menu::SettingsMenu;

/// Loads the config at `path`. A missing or broken file should not keep the
/// clock from showing, so errors fall back to the defaults.
pub fn load_config(path: Option<&Path>) -> Config {
    let Some(path) = path else {
        return Config::default();
    };
    Config::load(path).unwrap_or_else(|e| {
        eprintln!("openflip: {e}; using default settings");
        Config::default()
    })
}

pub struct Look {
    pub config: Config,
    pub theme: Theme,
    /// Language of the date and the settings.
    pub language: Language,
    /// The operating system's language, shown next to the "System" choice.
    pub system_language: Language,
    pub font: Font,
}

impl Look {
    pub fn from_config(config: Config) -> Self {
        let theme = Theme::by_name(&config.theme).unwrap_or_else(|| {
            eprintln!(
                "openflip: unknown theme '{}', using 'classic' (available: {})",
                config.theme,
                Theme::NAMES.join(", ")
            );
            Theme::classic()
        });
        let font = match &config.font {
            Some(path) => Font::from_file(path).unwrap_or_else(|e| {
                eprintln!("openflip: {}: {e}; using the bundled font", path.display());
                Font::bundled()
            }),
            None => Font::bundled(),
        };
        let system_language = system_language();
        Self {
            language: config.language.unwrap_or(system_language),
            config,
            theme,
            system_language,
            font,
        }
    }

    /// The cards the clock shows at `time`.
    pub fn cards_at(&self, time: NaiveDateTime) -> Vec<CardContent> {
        CardContent::from_face(&ClockFace::at(time, &self.config))
    }

    pub fn scene(&self, cards: &[CardFlip], time: NaiveDateTime, size: (u32, u32)) -> Scene {
        let date = self
            .config
            .show_date
            .then(|| DateLine::at(time, self.language));
        layout::build_scene(cards, date.as_ref(), &self.theme, size, self.config.scale)
    }

    /// The settings panel, to be painted over a scene of the same size.
    pub fn overlay(
        &self,
        menu: &SettingsMenu,
        message: Option<String>,
        size: (u32, u32),
    ) -> OverlayLayout {
        let view = menu.view(&self.config, self.language, self.system_language, message);
        build_overlay(&view, &self.theme, size)
    }
}

/// The user's preferred language as the operating system reports it. GUI
/// apps and screensavers on macOS usually have no `LANG`, so this asks the
/// system rather than the environment.
fn system_language() -> Language {
    sys_locale::get_locale()
        .map(|tag| Language::from_locale(&tag))
        .unwrap_or_default()
}
