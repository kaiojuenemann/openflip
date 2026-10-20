//! Turns a point in time into the digits shown on the flip cards.

use chrono::{Datelike, NaiveDateTime, Timelike};

use crate::config::{Config, TimeFormat};
use crate::i18n::Language;

/// The digit pairs shown on the cards at a given moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockFace {
    pub hours: [char; 2],
    pub minutes: [char; 2],
    pub seconds: Option<[char; 2]>,
    /// `Some(true)` for PM, `Some(false)` for AM, `None` in 24-hour mode.
    pub pm: Option<bool>,
}

impl ClockFace {
    pub fn at(time: NaiveDateTime, config: &Config) -> Self {
        let (hour, pm) = match config.time_format {
            TimeFormat::H24 => (time.hour(), None),
            TimeFormat::H12 => {
                let (is_pm, hour) = time.hour12();
                (hour, Some(is_pm))
            }
        };
        Self {
            hours: two_digits(hour),
            minutes: two_digits(time.minute()),
            seconds: config.show_seconds.then(|| two_digits(time.second())),
            pm,
        }
    }
}

/// The optional line with weekday and date below the clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateLine(pub String);

impl DateLine {
    pub fn at(time: NaiveDateTime, language: Language) -> Self {
        let weekday = language.weekday(time.weekday());
        let month = language.month(time.month());
        let text = match language {
            Language::English => format!("{weekday}, {month} {}", time.day()),
            Language::German => format!("{weekday}, {}. {month}", time.day()),
        };
        Self(text)
    }
}

fn two_digits(value: u32) -> [char; 2] {
    let tens = char::from_digit(value / 10, 10).expect("value below 100");
    let ones = char::from_digit(value % 10, 10).expect("digit below 10");
    [tens, ones]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn time(h: u32, m: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 20)
            .unwrap()
            .and_hms_opt(h, m, s)
            .unwrap()
    }

    #[test]
    fn shows_24_hour_time_without_seconds_by_default() {
        let face = ClockFace::at(time(21, 7, 5), &Config::default());
        assert_eq!(face.hours, ['2', '1']);
        assert_eq!(face.minutes, ['0', '7']);
        assert_eq!(face.seconds, None);
        assert_eq!(face.pm, None);
    }

    #[test]
    fn shows_12_hour_time_with_seconds() {
        let config = Config {
            time_format: TimeFormat::H12,
            show_seconds: true,
            ..Config::default()
        };
        let face = ClockFace::at(time(0, 30, 9), &config);
        assert_eq!(face.hours, ['1', '2']);
        assert_eq!(face.seconds, Some(['0', '9']));
        assert_eq!(face.pm, Some(false));

        let face = ClockFace::at(time(13, 0, 0), &config);
        assert_eq!(face.hours, ['0', '1']);
        assert_eq!(face.pm, Some(true));
    }

    #[test]
    fn formats_the_date_line_per_language() {
        let t = time(9, 0, 0);
        assert_eq!(DateLine::at(t, Language::English).0, "Tuesday, October 20");
        assert_eq!(DateLine::at(t, Language::German).0, "Dienstag, 20. Oktober");
    }
}
