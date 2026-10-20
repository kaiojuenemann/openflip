//! Command-line options.

use std::path::PathBuf;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

pub const USAGE: &str = "\
Usage: openflip [OPTIONS]

Options:
  --windowed            Start in a window instead of fullscreen
  --screenshot <FILE>   Render one frame to a PNG file and exit
  --size <WxH>          Image size for --screenshot (default 1920x1080)
  --time <TIME>         Show a fixed time, e.g. 10:42 or 2026-10-20T10:42:07
  -h, --help            Print this help

Keys: F or F11 toggles fullscreen, Esc or Q quits.";

#[derive(Debug, Default, PartialEq)]
pub struct Options {
    pub windowed: bool,
    pub screenshot: Option<PathBuf>,
    pub size: Option<(u32, u32)>,
    pub time: Option<NaiveDateTime>,
    pub help: bool,
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--windowed" => options.windowed = true,
            "--screenshot" => options.screenshot = Some(value("--screenshot")?.into()),
            "--size" => options.size = Some(parse_size(&value("--size")?)?),
            "--time" => options.time = Some(parse_time(&value("--time")?)?),
            "-h" | "--help" => options.help = true,
            other => return Err(format!("unknown option: {other}")),
        }
    }
    Ok(options)
}

fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let invalid = || format!("invalid size '{text}', expected e.g. 1920x1080");
    let (w, h) = text.split_once('x').ok_or_else(invalid)?;
    let w: u32 = w.parse().map_err(|_| invalid())?;
    let h: u32 = h.parse().map_err(|_| invalid())?;
    if w == 0 || h == 0 || w > 16384 || h > 16384 {
        return Err(invalid());
    }
    Ok((w, h))
}

fn parse_time(text: &str) -> Result<NaiveDateTime, String> {
    if let Ok(full) = NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S") {
        return Ok(full);
    }
    let time = NaiveTime::parse_from_str(text, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(text, "%H:%M"))
        .map_err(|_| {
            format!("invalid time '{text}', expected e.g. 10:42 or 2026-10-20T10:42:07")
        })?;
    let today: NaiveDate = chrono::Local::now().date_naive();
    Ok(today.and_time(time))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_screenshot_options() {
        let options = parse(args(&[
            "--screenshot",
            "out.png",
            "--size",
            "800x600",
            "--time",
            "2026-10-20T10:42:07",
        ]))
        .unwrap();
        assert_eq!(options.screenshot, Some(PathBuf::from("out.png")));
        assert_eq!(options.size, Some((800, 600)));
        assert_eq!(options.time.unwrap().to_string(), "2026-10-20 10:42:07");
    }

    #[test]
    fn accepts_short_times() {
        let options = parse(args(&["--time", "07:05"])).unwrap();
        let time = options.time.unwrap();
        assert_eq!((time.hour(), time.minute(), time.second()), (7, 5, 0));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(args(&["--size", "big"])).is_err());
        assert!(parse(args(&["--size", "0x10"])).is_err());
        assert!(parse(args(&["--time", "25:00"])).is_err());
        assert!(parse(args(&["--screenshot"])).is_err());
        assert!(parse(args(&["--frobnicate"])).is_err());
    }
}
