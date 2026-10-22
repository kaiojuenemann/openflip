//! The settings a user can change in the app, independent of how they are
//! presented: each [`Setting`] knows its label, how to show its current
//! value and how to step through its choices.

use crate::config::{Config, MonitorMode, TimeFormat, SCALE_RANGE, THEMES};
use crate::i18n::{Language, Text};

const SCALE_STEP: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    TimeFormat,
    Seconds,
    Date,
    Sound,
    Theme,
    Size,
    Font,
    Monitors,
    Language,
}

impl Setting {
    /// All settings, in the order they are listed.
    pub const ALL: [Setting; 9] = [
        Setting::TimeFormat,
        Setting::Seconds,
        Setting::Date,
        Setting::Sound,
        Setting::Theme,
        Setting::Size,
        Setting::Font,
        Setting::Monitors,
        Setting::Language,
    ];

    pub fn label(self, language: Language) -> &'static str {
        language.text(match self {
            Self::TimeFormat => Text::TimeFormat,
            Self::Seconds => Text::Seconds,
            Self::Date => Text::Date,
            Self::Sound => Text::Sound,
            Self::Theme => Text::Theme,
            Self::Size => Text::Size,
            Self::Font => Text::Font,
            Self::Monitors => Text::Monitors,
            Self::Language => Text::Language,
        })
    }

    /// The current value as shown to the user. `system` is the language the
    /// operating system uses, shown next to the "System" choice.
    pub fn value(self, config: &Config, language: Language, system: Language) -> String {
        let on_off = |on: bool| {
            language
                .text(if on { Text::On } else { Text::Off })
                .to_string()
        };
        match self {
            Self::TimeFormat => language
                .text(match config.time_format {
                    TimeFormat::H24 => Text::Hours24,
                    TimeFormat::H12 => Text::Hours12,
                })
                .to_string(),
            Self::Seconds => on_off(config.show_seconds),
            Self::Date => on_off(config.show_date),
            Self::Sound => on_off(config.sound),
            Self::Theme => language.theme_name(&config.theme),
            Self::Size => format!("{} %", (config.scale * 100.0).round()),
            Self::Font => match &config.font {
                None => language.text(Text::BundledFont).to_string(),
                Some(path) => path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string()),
            },
            Self::Monitors => language
                .text(match config.monitors {
                    MonitorMode::All => Text::AllMonitors,
                    MonitorMode::Primary => Text::PrimaryMonitor,
                })
                .to_string(),
            Self::Language => match config.language {
                None => format!(
                    "{} ({})",
                    language.text(Text::SystemLanguage),
                    system.native_name()
                ),
                Some(chosen) => chosen.native_name().to_string(),
            },
        }
    }

    /// Moves to the next (or previous) choice, wrapping around. Returns
    /// `false` for settings that cannot be stepped, such as the font, which
    /// is picked from a file.
    pub fn step(self, config: &mut Config, forward: bool) -> bool {
        match self {
            Self::TimeFormat => {
                config.time_format = match config.time_format {
                    TimeFormat::H24 => TimeFormat::H12,
                    TimeFormat::H12 => TimeFormat::H24,
                }
            }
            Self::Seconds => config.show_seconds = !config.show_seconds,
            Self::Date => config.show_date = !config.show_date,
            Self::Sound => config.sound = !config.sound,
            Self::Theme => {
                let current = THEMES.iter().position(|t| *t == config.theme);
                config.theme = cycle(&THEMES, current, forward).to_string();
            }
            Self::Size => {
                let (min, max) = SCALE_RANGE;
                let steps = ((max - min) / SCALE_STEP).round() as i32;
                let current = ((config.scale.clamp(min, max) - min) / SCALE_STEP).round() as i32;
                let next = (current + if forward { 1 } else { -1 }).rem_euclid(steps + 1);
                config.scale = min + next as f32 * SCALE_STEP;
            }
            Self::Font => return false,
            Self::Monitors => {
                config.monitors = match config.monitors {
                    MonitorMode::All => MonitorMode::Primary,
                    MonitorMode::Primary => MonitorMode::All,
                }
            }
            Self::Language => {
                let choices = [None, Some(Language::English), Some(Language::German)];
                let current = choices.iter().position(|c| *c == config.language);
                config.language = cycle(&choices, current, forward);
            }
        }
        true
    }
}

/// The element after (or before) `current`, wrapping around. An unknown
/// current value starts from the first element.
fn cycle<T: Copy>(choices: &[T], current: Option<usize>, forward: bool) -> T {
    let len = choices.len();
    let index = match (current, forward) {
        (None, _) => 0,
        (Some(i), true) => (i + 1) % len,
        (Some(i), false) => (i + len - 1) % len,
    };
    choices[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_has_a_label_in_both_languages() {
        for setting in Setting::ALL {
            assert!(!setting.label(Language::English).is_empty());
            assert!(!setting.label(Language::German).is_empty());
        }
    }

    #[test]
    fn toggles_and_cycles_wrap_around() {
        let mut config = Config::default();
        assert!(Setting::Seconds.step(&mut config, true));
        assert!(config.show_seconds);
        Setting::Seconds.step(&mut config, false);
        assert!(!config.show_seconds);

        for _ in 0..THEMES.len() {
            Setting::Theme.step(&mut config, true);
        }
        assert_eq!(config.theme, "classic");
        Setting::Theme.step(&mut config, false);
        assert_eq!(config.theme, "midnight");
    }

    #[test]
    fn unknown_theme_restarts_at_the_first() {
        let mut config = Config {
            theme: "neon".into(),
            ..Config::default()
        };
        Setting::Theme.step(&mut config, false);
        assert_eq!(config.theme, "classic");
    }

    #[test]
    fn size_moves_in_five_percent_steps_and_wraps() {
        let mut config = Config::default();
        assert_eq!(config.scale, 0.9);
        Setting::Size.step(&mut config, true);
        assert!((config.scale - 0.95).abs() < 1e-6);
        Setting::Size.step(&mut config, true);
        Setting::Size.step(&mut config, true);
        assert!((config.scale - 0.5).abs() < 1e-6, "wraps to the minimum");
        Setting::Size.step(&mut config, false);
        assert!(
            (config.scale - 1.0).abs() < 1e-6,
            "wraps back to the maximum"
        );
    }

    #[test]
    fn odd_sizes_snap_to_the_grid() {
        let mut config = Config {
            scale: 0.73,
            ..Config::default()
        };
        Setting::Size.step(&mut config, true);
        assert!((config.scale - 0.8).abs() < 1e-6);
    }

    #[test]
    fn language_cycles_through_system_english_german() {
        let mut config = Config::default();
        Setting::Language.step(&mut config, true);
        assert_eq!(config.language, Some(Language::English));
        Setting::Language.step(&mut config, true);
        assert_eq!(config.language, Some(Language::German));
        Setting::Language.step(&mut config, true);
        assert_eq!(config.language, None);
    }

    #[test]
    fn font_is_not_stepped() {
        let mut config = Config::default();
        assert!(!Setting::Font.step(&mut config, true));
        assert_eq!(config, Config::default());
    }

    #[test]
    fn values_are_shown_in_the_ui_language() {
        let config = Config::default();
        let (en, de) = (Language::English, Language::German);
        assert_eq!(Setting::Seconds.value(&config, de, de), "Aus");
        assert_eq!(Setting::Size.value(&config, en, en), "90 %");
        assert_eq!(Setting::Theme.value(&config, de, de), "Klassisch");
        assert_eq!(Setting::Language.value(&config, de, en), "System (English)");
        let custom = Config {
            font: Some("/fonts/Clock-Bold.ttf".into()),
            ..Config::default()
        };
        assert_eq!(Setting::Font.value(&custom, en, en), "Clock-Bold.ttf");
    }
}
