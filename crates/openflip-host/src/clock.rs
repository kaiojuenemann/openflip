//! The clock as a host sees it: feed it input, ask it for frames.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use chrono::{NaiveDateTime, Timelike};
use openflip_core::{Config, MonitorMode, Text};
use openflip_render::overlay::OverlayLayout;
use openflip_render::{Flipper, Font, Scene};

use crate::look::{load_config, Look};
use crate::menu::{MenuAction, MenuKey, SettingsMenu};

/// Redraw slightly after the second turns over, never just before it.
const TICK_SLACK: Duration = Duration::from_millis(5);
/// Upper bound on the frame rate during a flip. Presenting normally waits
/// for vsync anyway; this keeps systems without vsync from spinning.
const MIN_FRAME_TIME: Duration = Duration::from_micros(1_000_000 / 120);
/// How often the config file is checked for changes made elsewhere, e.g.
/// by hand or by the settings of another openflip instance.
const RELOAD_INTERVAL: Duration = Duration::from_secs(1);
/// How soon to try again when a frame could not be shown.
const RETRY_DELAY: Duration = Duration::from_millis(50);

/// Keys the clock reacts to; hosts map their platform's key events to these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    /// Enter or Space.
    Activate,
    /// Delete or Backspace.
    Reset,
    /// `S`: opens or closes the settings.
    Settings,
    Escape,
    /// `Q`.
    Quit,
}

/// What the host has to do after the clock handled an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    Nothing,
    /// Draw a new frame soon.
    Redraw,
    /// Show a font picker and pass the chosen file to [`Clock::set_font`].
    ChooseFont,
    /// Close the window, or the settings sheet of a screensaver.
    Exit,
}

/// One frame to draw.
pub struct Frame {
    pub scene: Scene,
    /// Set when the font changed since the previous frame; hand it to the
    /// renderer before drawing.
    pub new_font: Option<Font>,
    /// How many flaps struck their card since the previous frame, if the
    /// settings ask for sound; play that many flap sounds.
    pub flap_sounds: usize,
}

pub struct Clock {
    look: Look,
    /// Where settings are saved; `None` if the system has no config directory.
    config_path: Option<PathBuf>,
    /// Modification time of the config file as last read or written.
    config_modified: Option<SystemTime>,
    last_reload_check: Instant,
    fixed_time: Option<NaiveDateTime>,
    flipper: Flipper,
    /// Origin of the animation clock.
    started: Instant,
    menu: Option<SettingsMenu>,
    /// Only the settings are shown; closing them ends the host.
    settings_only: bool,
    /// Error shown in the settings, e.g. when saving failed.
    message: Option<Text>,
    /// The settings panel as last drawn, for mouse hit-testing.
    overlay: Option<OverlayLayout>,
    cursor: (f32, f32),
    /// Something changed that is not on screen yet.
    dirty: bool,
    font_changed: bool,
    /// When the next frame is due without further input.
    next_due: Option<Instant>,
}

impl Clock {
    /// A clock using the config at `config_path`, or the defaults.
    pub fn new(config_path: Option<PathBuf>) -> Self {
        let config = load_config(config_path.as_deref());
        let config_modified = config_path.as_deref().and_then(modified);
        Self {
            look: Look::from_config(config),
            config_path,
            config_modified,
            last_reload_check: Instant::now(),
            fixed_time: None,
            flipper: Flipper::default(),
            started: Instant::now(),
            menu: None,
            settings_only: false,
            message: None,
            overlay: None,
            cursor: (0.0, 0.0),
            dirty: true,
            font_changed: false,
            next_due: None,
        }
    }

    /// Shows only the settings, already open; closing them ends the host.
    pub fn settings_only(mut self) -> Self {
        self.settings_only = true;
        self.menu = Some(SettingsMenu::default());
        self
    }

    /// Freezes the clock at `time`, for screenshots and tests.
    pub fn fixed_time(mut self, time: NaiveDateTime) -> Self {
        self.fixed_time = Some(time);
        self
    }

    /// Forces a new frame, e.g. after the surface was resized.
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    pub fn look(&self) -> &Look {
        &self.look
    }

    pub fn is_settings_open(&self) -> bool {
        self.menu.is_some()
    }

    pub fn key(&mut self, key: Key) -> Response {
        let Some(menu) = &mut self.menu else {
            return match key {
                Key::Settings => {
                    self.menu = Some(SettingsMenu::default());
                    self.message = None;
                    self.redraw()
                }
                Key::Escape | Key::Quit => Response::Exit,
                _ => Response::Nothing,
            };
        };
        let menu_key = match key {
            Key::Up => MenuKey::Up,
            Key::Down => MenuKey::Down,
            Key::Left => MenuKey::Left,
            Key::Right => MenuKey::Right,
            Key::Activate => MenuKey::Activate,
            Key::Reset => MenuKey::Reset,
            Key::Settings | Key::Escape => return self.close_settings(),
            Key::Quit => return Response::Exit,
        };
        let mut config = self.look.config.clone();
        let action = menu.key(menu_key, &mut config);
        self.menu_action(action, config)
    }

    pub fn mouse_moved(&mut self, x: f32, y: f32) -> Response {
        self.cursor = (x, y);
        let hit = self.row_under_cursor();
        let (Some(menu), Some((row, _))) = (&mut self.menu, hit) else {
            return Response::Nothing;
        };
        let action = menu.hover(row);
        let config = self.look.config.clone();
        self.menu_action(action, config)
    }

    pub fn mouse_pressed(&mut self, x: f32, y: f32) -> Response {
        self.cursor = (x, y);
        let Some(overlay) = &self.overlay else {
            return Response::Nothing;
        };
        if !overlay.panel.contains(x, y) {
            return self.close_settings();
        }
        let hit = self.row_under_cursor();
        let (Some(menu), Some((row, back))) = (&mut self.menu, hit) else {
            return Response::Nothing;
        };
        let mut config = self.look.config.clone();
        let action = menu.click(row, back, &mut config);
        self.menu_action(action, config)
    }

    /// A click at the position of the last [`mouse_moved`](Self::mouse_moved),
    /// for platforms whose click events carry no position.
    pub fn mouse_pressed_at_cursor(&mut self) -> Response {
        let (x, y) = self.cursor;
        self.mouse_pressed(x, y)
    }

    /// Uses the font file at `path`, after checking that it loads.
    pub fn set_font(&mut self, path: PathBuf) -> Response {
        if Font::from_file(&path).is_err() {
            self.message = Some(Text::FontFailed);
            return self.redraw();
        }
        let config = Config {
            font: Some(path),
            ..self.look.config.clone()
        };
        self.apply_config(config, true);
        self.redraw()
    }

    /// Tells the clock that the last frame did not reach the screen (see
    /// `SurfaceRenderer::render`), so another one is due shortly. Otherwise
    /// the screen would stay empty until the clock face next changes.
    pub fn frame_not_shown(&mut self) {
        let retry = Instant::now() + RETRY_DELAY;
        self.next_due = Some(self.next_due.map_or(retry, |due| due.min(retry)));
    }

    /// Whether a frame should be drawn at `now`; for hosts that are called
    /// periodically, like screensavers.
    pub fn needs_frame(&self, now: Instant) -> bool {
        self.dirty || self.next_due.is_some_and(|due| now >= due)
    }

    /// When the next frame is due if no input arrives; `None` means only
    /// input can change what is shown. For hosts that sleep in between.
    pub fn next_frame_at(&self) -> Option<Instant> {
        if self.dirty {
            Some(Instant::now())
        } else {
            self.next_due
        }
    }

    /// Advances the clock and builds the frame for a surface of `size` pixels.
    pub fn frame(&mut self, size: (u32, u32)) -> Frame {
        self.frame_on(size, true)
    }

    /// Whether a display shows the clock, or only the background because
    /// the settings ask for the primary display only.
    pub fn shows_clock_on(&self, primary: bool) -> bool {
        primary || self.look.config.monitors == MonitorMode::All
    }

    /// Like [`Clock::frame`], for one of several displays sharing this
    /// clock. Secondary displays stay blank if [`Clock::shows_clock_on`]
    /// says so.
    pub fn frame_on(&mut self, size: (u32, u32), primary: bool) -> Frame {
        let frame_start = Instant::now();
        if frame_start.duration_since(self.last_reload_check) >= RELOAD_INTERVAL {
            self.last_reload_check = frame_start;
            self.reload_if_changed();
        }

        let now = self
            .fixed_time
            .unwrap_or_else(|| chrono::Local::now().naive_local());
        let t = self.started.elapsed().as_secs_f64();
        self.flipper.update(&self.look.cards_at(now), t);
        let landings = self.flipper.take_landings(t);
        let mut scene = if self.shows_clock_on(primary) {
            self.look.scene(&self.flipper.cards(t), now, size)
        } else {
            Scene {
                width: size.0,
                height: size.1,
                background: self.look.theme.background,
                groups: Vec::new(),
            }
        };

        self.overlay = self.menu.as_ref().filter(|_| primary).map(|menu| {
            let message = self.message.map(|m| self.look.language.text(m).to_string());
            self.look.overlay(menu, message, size)
        });
        if let Some(overlay) = &self.overlay {
            scene.groups.extend(overlay.groups.iter().cloned());
        }

        self.dirty = false;
        self.next_due = if self.flipper.is_animating(t) {
            Some(frame_start + MIN_FRAME_TIME)
        } else if self.fixed_time.is_none() {
            Some(frame_start + self.until_next_tick() + TICK_SLACK)
        } else {
            None
        };
        Frame {
            scene,
            new_font: std::mem::take(&mut self.font_changed).then(|| self.look.font.clone()),
            flap_sounds: if self.look.config.sound { landings } else { 0 },
        }
    }

    fn redraw(&mut self) -> Response {
        self.dirty = true;
        Response::Redraw
    }

    fn close_settings(&mut self) -> Response {
        if self.settings_only {
            return Response::Exit;
        }
        self.menu = None;
        self.overlay = None;
        self.redraw()
    }

    fn menu_action(&mut self, action: MenuAction, config: Config) -> Response {
        match action {
            MenuAction::Nothing => Response::Nothing,
            MenuAction::Redraw => self.redraw(),
            MenuAction::ConfigChanged => {
                self.apply_config(config, true);
                self.redraw()
            }
            MenuAction::ChooseFont => Response::ChooseFont,
        }
    }

    /// Uses a changed config right away and, if `save`, writes it to disk.
    fn apply_config(&mut self, config: Config, save: bool) {
        self.font_changed |= config.font != self.look.config.font;
        self.look = Look::from_config(config);
        self.dirty = true;
        if !save {
            return;
        }
        let saved = match &self.config_path {
            Some(path) => self.look.config.save(path).map_err(|e| e.to_string()),
            None => Err("no config directory".into()),
        };
        self.message = match saved {
            Ok(()) => None,
            Err(e) => {
                eprintln!("openflip: could not save settings: {e}");
                Some(Text::SaveFailed)
            }
        };
        self.config_modified = self.config_path.as_deref().and_then(modified);
    }

    /// Picks up changes another program made to the config file.
    fn reload_if_changed(&mut self) {
        let Some(path) = &self.config_path else {
            return;
        };
        let current = modified(path);
        if current.is_none() || current == self.config_modified {
            return;
        }
        self.config_modified = current;
        let config = load_config(Some(path));
        if config != self.look.config {
            self.apply_config(config, false);
        }
    }

    /// Time until the clock face next changes.
    fn until_next_tick(&self) -> Duration {
        let now = chrono::Local::now().naive_local();
        let millis = u64::from(now.nanosecond() / 1_000_000).min(999);
        let to_next_second = Duration::from_millis(1000 - millis);
        if self.look.config.show_seconds {
            to_next_second
        } else {
            to_next_second + Duration::from_secs(59 - u64::from(now.second().min(59)))
        }
    }

    /// The settings row under the cursor and whether it hit the "back" arrow.
    fn row_under_cursor(&self) -> Option<(usize, bool)> {
        let (x, y) = self.cursor;
        let overlay = self.overlay.as_ref()?;
        let row = overlay.rows.iter().position(|row| row.contains(x, y))?;
        Some((row, overlay.rows[row].is_back(x)))
    }
}

fn modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openflip_core::Setting;
    use std::path::Path;

    const SIZE: (u32, u32) = (1280, 720);

    /// A fresh config directory per test.
    fn temp_config(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openflip-host-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("config.toml")
    }

    fn time() -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 21)
            .unwrap()
            .and_hms_opt(10, 42, 0)
            .unwrap()
    }

    fn bundled_font_file() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../openflip-render/assets/InterDisplay-Bold.otf")
    }

    #[test]
    fn a_frame_that_was_not_shown_is_retried_soon() {
        let mut clock = Clock::new(None).fixed_time(time());
        clock.frame(SIZE);
        assert_eq!(
            clock.next_frame_at(),
            None,
            "a frozen clock waits for input"
        );

        clock.frame_not_shown();
        let retry = clock.next_frame_at().expect("a retry is scheduled");
        assert!(retry <= Instant::now() + RETRY_DELAY);
        assert!(!clock.needs_frame(Instant::now()));
        assert!(clock.needs_frame(retry));
    }

    #[test]
    fn secondary_displays_are_blank_when_only_the_primary_shows_the_clock() {
        let mut clock = Clock::new(None).fixed_time(time());
        assert!(!clock.frame_on(SIZE, false).scene.groups.is_empty());

        clock.apply_config(
            Config {
                monitors: MonitorMode::Primary,
                ..Config::default()
            },
            false,
        );
        assert!(!clock.frame_on(SIZE, true).scene.groups.is_empty());
        let blank = clock.frame_on(SIZE, false).scene;
        assert!(blank.groups.is_empty());
        assert_eq!(blank.background, clock.look().theme.background);
        assert_eq!((blank.width, blank.height), SIZE);
    }

    /// Frames of a clock that flips from 10:42 to 10:43, until the flap
    /// has landed; returns the flap sounds of each frame.
    fn flap_sounds_of_one_flip(sound: bool) -> Vec<usize> {
        let mut clock = Clock::new(None).fixed_time(time());
        clock.apply_config(
            Config {
                sound,
                ..Config::default()
            },
            false,
        );
        let mut sounds = vec![clock.frame(SIZE).flap_sounds];
        clock.fixed_time = Some(time() + chrono::Duration::minutes(1));
        sounds.push(clock.frame(SIZE).flap_sounds);
        // Let the flip finish without waiting for it.
        clock.started -= Duration::from_secs(1);
        sounds.push(clock.frame(SIZE).flap_sounds);
        sounds.push(clock.frame(SIZE).flap_sounds);
        sounds
    }

    #[test]
    fn a_flip_is_heard_once_when_sound_is_on() {
        assert_eq!(flap_sounds_of_one_flip(true), [0, 0, 1, 0]);
        assert_eq!(flap_sounds_of_one_flip(false), [0, 0, 0, 0]);
    }

    #[test]
    fn s_opens_and_closes_the_settings_and_escape_exits_only_when_closed() {
        let mut clock = Clock::new(None).fixed_time(time());
        assert_eq!(clock.key(Key::Escape), Response::Exit);
        assert_eq!(clock.key(Key::Settings), Response::Redraw);
        assert!(clock.is_settings_open());
        assert_eq!(clock.key(Key::Escape), Response::Redraw);
        assert!(!clock.is_settings_open());
        assert_eq!(clock.key(Key::Up), Response::Nothing);
    }

    #[test]
    fn closing_settings_only_mode_exits() {
        let mut clock = Clock::new(None).settings_only();
        assert!(clock.is_settings_open());
        assert_eq!(clock.key(Key::Settings), Response::Exit);
    }

    #[test]
    fn changes_are_applied_and_saved() {
        let path = temp_config("save");
        let mut clock = Clock::new(Some(path.clone())).fixed_time(time());
        clock.key(Key::Settings);
        clock.key(Key::Down);
        assert_eq!(clock.key(Key::Right), Response::Redraw);
        assert!(clock.look().config.show_seconds);
        assert!(Config::load(&path).unwrap().show_seconds);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_failed_save_is_reported_in_the_settings() {
        let mut clock = Clock::new(None).fixed_time(time());
        clock.key(Key::Settings);
        clock.key(Key::Right);
        assert_eq!(clock.message, Some(Text::SaveFailed));
    }

    #[test]
    fn edits_to_the_file_are_picked_up() {
        let path = temp_config("reload");
        Config::default().save(&path).unwrap();
        let mut clock = Clock::new(Some(path.clone())).fixed_time(time());
        // Make sure the file's modification time actually changes.
        std::thread::sleep(Duration::from_millis(20));
        let edited = Config {
            theme: "amber".into(),
            ..Config::default()
        };
        edited.save(&path).unwrap();
        clock.reload_if_changed();
        assert_eq!(clock.look().config.theme, "amber");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn mouse_clicks_change_rows_and_a_click_outside_closes() {
        let mut clock = Clock::new(None).fixed_time(time());
        clock.key(Key::Settings);
        clock.frame(SIZE);
        let overlay = clock.overlay.clone().unwrap();
        let date_row = overlay.rows[2];
        let (x, y) = (
            date_row.value_x + date_row.value_width * 0.75,
            date_row.y + date_row.height / 2.0,
        );
        assert_eq!(clock.mouse_moved(x, y), Response::Redraw);
        assert_eq!(clock.mouse_pressed(x, y), Response::Redraw);
        assert!(clock.look().config.show_date);
        assert_eq!(clock.mouse_pressed(1.0, 1.0), Response::Redraw);
        assert!(!clock.is_settings_open());
    }

    #[test]
    fn the_font_row_asks_the_host_for_a_file() {
        let mut clock = Clock::new(None).fixed_time(time());
        clock.key(Key::Settings);
        let font = Setting::ALL
            .iter()
            .position(|s| *s == Setting::Font)
            .unwrap();
        for _ in 0..font {
            clock.key(Key::Down);
        }
        assert_eq!(clock.key(Key::Activate), Response::ChooseFont);
    }

    #[test]
    fn a_new_font_reaches_the_next_frame_once() {
        let mut clock = Clock::new(None).fixed_time(time());
        assert!(clock.frame(SIZE).new_font.is_none());
        assert_eq!(clock.set_font(bundled_font_file()), Response::Redraw);
        assert!(clock.frame(SIZE).new_font.is_some());
        assert!(clock.frame(SIZE).new_font.is_none());
    }

    #[test]
    fn a_broken_font_is_rejected() {
        let mut clock = Clock::new(None).fixed_time(time());
        clock.set_font(PathBuf::from("/no/such/font.ttf"));
        assert_eq!(clock.look().config.font, None);
        assert_eq!(clock.message, Some(Text::FontFailed));
    }

    #[test]
    fn frames_are_due_only_when_something_changes() {
        let mut clock = Clock::new(None).fixed_time(time());
        assert!(clock.needs_frame(Instant::now()), "first frame");
        clock.frame(SIZE);
        assert!(!clock.needs_frame(Instant::now()));
        assert_eq!(
            clock.next_frame_at(),
            None,
            "a frozen clock waits for input"
        );
        clock.key(Key::Settings);
        assert!(clock.needs_frame(Instant::now()));
    }

    #[test]
    fn a_running_clock_is_due_at_the_next_tick() {
        let mut clock = Clock::new(None);
        clock.frame(SIZE);
        let due = clock.next_frame_at().unwrap();
        assert!(due > Instant::now());
        assert!(due <= Instant::now() + Duration::from_secs(61));
    }

    #[test]
    fn the_settings_are_painted_over_the_clock() {
        let mut clock = Clock::new(None).fixed_time(time());
        let plain = clock.frame(SIZE).scene.groups.len();
        clock.key(Key::Settings);
        assert!(clock.frame(SIZE).scene.groups.len() > plain);
    }
}
