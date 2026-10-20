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
}
