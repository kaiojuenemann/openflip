//! openflip application entry point.

mod cli;
mod screenshot;
mod window;

use std::process::ExitCode;

use chrono::NaiveDateTime;
use openflip_core::{ClockFace, Config, DateLine, Language};
use openflip_render::{layout, CardContent, CardFlip, Font, Scene, Theme};

fn main() -> ExitCode {
    let options = match cli::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("openflip: {e}\n\n{}", cli::USAGE);
            return ExitCode::FAILURE;
        }
    };
    if options.help {
        println!("{}", cli::USAGE);
        return ExitCode::SUCCESS;
    }

    let look = Look::from_config(load_config());
    let result = match &options.screenshot {
        Some(path) => screenshot::run(&look, &options, path),
        None => window::run(look, &options),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("openflip: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A broken config file should not keep the clock from showing.
fn load_config() -> Config {
    match Config::default_path().and_then(|path| Config::load(&path)) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("openflip: {e}; using default settings");
            Config::default()
        }
    }
}

/// Everything derived from the config that is needed to draw a frame.
pub struct Look {
    pub config: Config,
    pub theme: Theme,
    pub language: Language,
    pub font: Font,
}

impl Look {
    fn from_config(config: Config) -> Self {
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
        let language = config.language.unwrap_or_else(system_language);
        Self {
            config,
            theme,
            language,
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
}

fn system_language() -> Language {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|var| std::env::var(var).ok().filter(|v| !v.is_empty()))
        .map(|tag| Language::from_locale(&tag))
        .unwrap_or_default()
}
