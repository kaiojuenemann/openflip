//! Command-line options.

use std::path::PathBuf;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

pub const USAGE: &str = "\
Usage: openflip [OPTIONS]

Options:
  --fullscreen          Start fullscreen (the default)
  --windowed            Start in a window instead of fullscreen
  --settings            Open the settings in a window; closing them quits
  --screenshot <FILE>   Render one frame to a PNG file and exit
  --size <WxH>          Image size for --screenshot (default 1920x1080)
  --time <TIME>         Show a fixed time, e.g. 10:42 or 2026-10-20T10:42:07
  --progress <0..1>     For --screenshot: freeze the flip into --time at
                        this point (0 = start, 0.5 = flap edge-on)
  --screensaver         Run as screensaver: fullscreen, quit on any input
  -h, --help            Print this help

As a Windows screensaver (openflip.scr), it also understands:
  /s                    Run the screensaver
  /p <HWND>             Draw the preview into the settings dialog's window
  /c                    Open the settings

As an XScreenSaver hack on Linux (X11):
  -root                 Draw into XScreenSaver's window ($XSCREENSAVER_WINDOW)
                        or, without it, onto the root window
  -window-id <ID>       Draw into the X window ID (decimal or 0x hex)

Keys: S opens the settings, F or F11 toggles fullscreen, Esc or Q quits.";

/// A foreign window the clock draws into, filling it with a child window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Embed {
    /// A native window handle: a Windows `HWND` or an X11 window ID.
    Window(u64),
    /// XScreenSaver's window from `$XSCREENSAVER_WINDOW`, or the X root window.
    XScreenSaverRoot,
}

#[derive(Debug, Default, PartialEq)]
pub struct Options {
    pub windowed: bool,
    pub settings: bool,
    /// Run as a screensaver: fullscreen, quit on any input.
    pub screensaver: bool,
    /// Draw into a window that belongs to someone else.
    pub embed: Option<Embed>,
    pub screenshot: Option<PathBuf>,
    pub size: Option<(u32, u32)>,
    pub time: Option<NaiveDateTime>,
    pub progress: Option<f32>,
    pub help: bool,
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let args: Vec<String> = args.into_iter().collect();
    if let Some(options) = parse_screensaver_args(&args) {
        return options;
    }
    let mut options = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--fullscreen" => options.windowed = false,
            "--windowed" => options.windowed = true,
            "--screensaver" => options.screensaver = true,
            "-root" | "--root" => options.embed = Some(Embed::XScreenSaverRoot),
            "-window-id" | "--window-id" => {
                options.embed = Some(Embed::Window(parse_window_id(&value("-window-id")?)?))
            }
            "--settings" => options.settings = true,
            "--screenshot" => options.screenshot = Some(value("--screenshot")?.into()),
            "--size" => options.size = Some(parse_size(&value("--size")?)?),
            "--time" => options.time = Some(parse_time(&value("--time")?)?),
            "--progress" => options.progress = Some(parse_progress(&value("--progress")?)?),
            "-h" | "--help" => options.help = true,
            other => return Err(format!("unknown option: {other}")),
        }
    }
    Ok(options)
}

/// The arguments Windows passes to screensavers: `/s`, `/p <HWND>` and
/// `/c[:<HWND>]`, in either case, with `/` or `-`, and the window handle
/// after a space or a colon. `None` if the arguments are not of that form.
fn parse_screensaver_args(args: &[String]) -> Option<Result<Options, String>> {
    let first = args.first()?;
    let rest = first
        .strip_prefix('/')
        .or_else(|| first.strip_prefix('-').filter(|r| !r.starts_with('-')))?;
    let mut chars = rest.chars();
    let flag = chars.next()?.to_ascii_lowercase();
    let tail = chars.as_str();
    let inline = tail.strip_prefix(':');
    if !tail.is_empty() && inline.is_none() {
        return None;
    }
    let options = match flag {
        's' => Options {
            screensaver: true,
            ..Options::default()
        },
        // The handle after /c is the dialog that opened the settings; a
        // separate window works just as well.
        'c' => Options {
            settings: true,
            ..Options::default()
        },
        'p' => {
            let handle = inline.or_else(|| args.get(1).map(String::as_str));
            let handle = handle
                .and_then(|h| h.trim().parse::<u64>().ok())
                .filter(|h| *h != 0);
            match handle {
                Some(handle) => Options {
                    embed: Some(Embed::Window(handle)),
                    ..Options::default()
                },
                None => return Some(Err(format!("{first} needs a window handle"))),
            }
        }
        _ => return None,
    };
    Some(Ok(options))
}

/// An X11 window ID as XScreenSaver writes it: `0x1a00007` or decimal.
pub fn parse_window_id(text: &str) -> Result<u64, String> {
    let text = text.trim();
    let id = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => text.parse(),
    };
    id.ok()
        .filter(|id| *id != 0)
        .ok_or_else(|| format!("invalid window id '{text}'"))
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

fn parse_progress(text: &str) -> Result<f32, String> {
    text.parse::<f32>()
        .ok()
        .filter(|p| (0.0..=1.0).contains(p))
        .ok_or_else(|| format!("invalid progress '{text}', expected a number from 0 to 1"))
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
        let options = parse(args(&["--progress", "0.25"])).unwrap();
        assert_eq!(options.progress, Some(0.25));
    }

    #[test]
    fn accepts_short_times() {
        let options = parse(args(&["--time", "07:05"])).unwrap();
        let time = options.time.unwrap();
        assert_eq!((time.hour(), time.minute(), time.second()), (7, 5, 0));
    }

    #[test]
    fn understands_windows_screensaver_arguments() {
        let saver = parse(args(&["/s"])).unwrap();
        assert!(saver.screensaver);
        assert!(parse(args(&["/S"])).unwrap().screensaver);
        assert!(parse(args(&["-s"])).unwrap().screensaver);

        assert_eq!(
            parse(args(&["/p", "1234"])).unwrap().embed,
            Some(Embed::Window(1234))
        );
        assert_eq!(
            parse(args(&["/P:5678"])).unwrap().embed,
            Some(Embed::Window(5678))
        );

        assert!(parse(args(&["/c"])).unwrap().settings);
        assert!(parse(args(&["/c:4321"])).unwrap().settings);
        assert!(parse(args(&["-C"])).unwrap().settings);
    }

    #[test]
    fn the_last_of_fullscreen_and_windowed_wins() {
        assert!(!parse(args(&["--fullscreen"])).unwrap().windowed);
        assert!(
            parse(args(&["--fullscreen", "--windowed"]))
                .unwrap()
                .windowed
        );
        assert!(
            !parse(args(&["--windowed", "--fullscreen"]))
                .unwrap()
                .windowed
        );
    }

    #[test]
    fn understands_xscreensaver_arguments() {
        assert_eq!(
            parse(args(&["-root"])).unwrap().embed,
            Some(Embed::XScreenSaverRoot)
        );
        assert_eq!(
            parse(args(&["-window-id", "0x1a00007"])).unwrap().embed,
            Some(Embed::Window(0x1a00007))
        );
        assert_eq!(
            parse(args(&["--window-id", "27262983"])).unwrap().embed,
            Some(Embed::Window(27262983))
        );
        assert!(parse(args(&["-window-id", "0x"])).is_err());
        assert!(parse(args(&["-window-id"])).is_err());
        assert!(parse(args(&["--screensaver"])).unwrap().screensaver);
    }

    #[test]
    fn rejects_a_preview_without_window() {
        assert!(parse(args(&["/p"])).is_err());
        assert!(parse(args(&["/p", "zero"])).is_err());
        assert!(parse(args(&["/p:0"])).is_err());
    }

    #[test]
    fn regular_options_are_not_mistaken_for_screensaver_ones() {
        assert!(parse(args(&["-h"])).unwrap().help);
        assert!(parse(args(&["--settings"])).unwrap().settings);
        assert!(parse(args(&["/x"])).is_err());
        assert!(parse(args(&["/save"])).is_err());
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(args(&["--size", "big"])).is_err());
        assert!(parse(args(&["--size", "0x10"])).is_err());
        assert!(parse(args(&["--time", "25:00"])).is_err());
        assert!(parse(args(&["--screenshot"])).is_err());
        assert!(parse(args(&["--frobnicate"])).is_err());
        assert!(parse(args(&["--progress", "1.5"])).is_err());
        assert!(parse(args(&["--progress", "NaN"])).is_err());
    }
}
