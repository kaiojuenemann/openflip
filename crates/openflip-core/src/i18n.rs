//! User-facing strings. English is the fallback for every language.

use chrono::Weekday;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    English,
    German,
}

impl Language {
    /// Picks a language from a locale tag such as `de-DE` or `en_US.UTF-8`.
    pub fn from_locale(tag: &str) -> Self {
        match tag.get(..2).map(str::to_ascii_lowercase).as_deref() {
            Some("de") => Self::German,
            _ => Self::English,
        }
    }

    pub fn weekday(self, day: Weekday) -> &'static str {
        let index = day.num_days_from_monday() as usize;
        match self {
            Self::English => [
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
                "Sunday",
            ][index],
            Self::German => [
                "Montag",
                "Dienstag",
                "Mittwoch",
                "Donnerstag",
                "Freitag",
                "Samstag",
                "Sonntag",
            ][index],
        }
    }

    /// `month` is 1-based, as returned by `chrono::Datelike::month`.
    pub fn month(self, month: u32) -> &'static str {
        let index = (month - 1) as usize;
        match self {
            Self::English => [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ][index],
            Self::German => [
                "Januar",
                "Februar",
                "März",
                "April",
                "Mai",
                "Juni",
                "Juli",
                "August",
                "September",
                "Oktober",
                "November",
                "Dezember",
            ][index],
        }
    }
}

/// Fixed user-interface strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    SettingsTitle,
    TimeFormat,
    Seconds,
    Date,
    Sound,
    Theme,
    Size,
    Font,
    Monitors,
    Language,
    On,
    Off,
    Hours24,
    Hours12,
    AllMonitors,
    PrimaryMonitor,
    SystemLanguage,
    BundledFont,
    MenuHint,
    FontHint,
    SaveFailed,
    FontFailed,
}

impl Language {
    /// The language's own name, as shown in the language setting.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::German => "Deutsch",
        }
    }

    pub fn text(self, text: Text) -> &'static str {
        use Text::*;
        match self {
            Self::English => match text {
                SettingsTitle => "Settings",
                TimeFormat => "Time format",
                Seconds => "Seconds",
                Date => "Date",
                Sound => "Sound",
                Theme => "Theme",
                Size => "Size",
                Font => "Font",
                Monitors => "Displays",
                Language => "Language",
                On => "On",
                Off => "Off",
                Hours24 => "24-hour",
                Hours12 => "12-hour",
                AllMonitors => "All",
                PrimaryMonitor => "Main display only",
                SystemLanguage => "System",
                BundledFont => "Inter (built-in)",
                MenuHint => "↑↓ select  ·  ←→ change  ·  S close",
                FontHint => "Enter: choose a font file  ·  Del: built-in font",
                SaveFailed => "Could not save settings",
                FontFailed => "Could not load this font",
            },
            Self::German => match text {
                SettingsTitle => "Einstellungen",
                TimeFormat => "Zeitformat",
                Seconds => "Sekunden",
                Date => "Datum",
                Sound => "Ton",
                Theme => "Farbschema",
                Size => "Größe",
                Font => "Schrift",
                Monitors => "Bildschirme",
                Language => "Sprache",
                On => "An",
                Off => "Aus",
                Hours24 => "24 Stunden",
                Hours12 => "12 Stunden",
                AllMonitors => "Alle",
                PrimaryMonitor => "Nur Hauptbildschirm",
                SystemLanguage => "System",
                BundledFont => "Inter (mitgeliefert)",
                MenuHint => "↑↓ auswählen  ·  ←→ ändern  ·  S schließen",
                FontHint => "Enter: Schriftdatei wählen  ·  Entf: mitgelieferte Schrift",
                SaveFailed => "Einstellungen konnten nicht gespeichert werden",
                FontFailed => "Diese Schrift konnte nicht geladen werden",
            },
        }
    }

    /// Display name of a built-in theme; unknown names are shown as they are.
    pub fn theme_name(self, theme: &str) -> String {
        let name = match (self, theme) {
            (Self::English, "classic") => "Classic",
            (Self::English, "light") => "Light",
            (Self::English, "amber") => "Amber",
            (Self::English, "midnight") => "Midnight",
            (Self::German, "classic") => "Klassisch",
            (Self::German, "light") => "Hell",
            (Self::German, "amber") => "Bernstein",
            (Self::German, "midnight") => "Mitternacht",
            _ => theme,
        };
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_language_from_locale_tags() {
        assert_eq!(Language::from_locale("de-DE"), Language::German);
        assert_eq!(Language::from_locale("DE_at.UTF-8"), Language::German);
        assert_eq!(Language::from_locale("en_US.UTF-8"), Language::English);
        assert_eq!(Language::from_locale("fr-FR"), Language::English);
        assert_eq!(Language::from_locale(""), Language::English);
    }

    #[test]
    fn every_built_in_theme_has_a_translated_name() {
        for theme in crate::config::THEMES {
            for language in [Language::English, Language::German] {
                assert_ne!(language.theme_name(theme), theme, "{theme} in {language:?}");
            }
        }
    }
}
