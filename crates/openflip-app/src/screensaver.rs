//! Screensaver behaviour: end as soon as the user does something.

use std::time::{Duration, Instant};

/// How far the mouse may move before the screensaver ends, in pixels. The
/// window receives a position when it appears and the mouse may jitter, so
/// zero would end the screensaver right away.
const MOUSE_THRESHOLD: f64 = 10.0;

/// Input right after the start is ignored: the click on "Preview" in the
/// Windows dialog, the key that started it from a terminal, or stray events
/// the system sends to a new window would otherwise end it at once.
const GRACE_PERIOD: Duration = Duration::from_secs(1);

/// What happened, as far as ending the screensaver is concerned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    KeyPressed,
    ButtonPressed,
    Wheel,
    MouseAt(f64, f64),
    /// The window moved or resized, e.g. when it became fullscreen.
    WindowChanged,
    /// Anything else, including key and button releases.
    Other,
}

#[derive(Debug)]
pub struct ExitOnInput {
    started: Instant,
    /// The mouse position movement is measured from.
    origin: Option<(f64, f64)>,
}

impl ExitOnInput {
    pub fn new(now: Instant) -> Self {
        Self {
            started: now,
            origin: None,
        }
    }

    /// Whether `input`, arriving at `now`, should end the screensaver.
    pub fn ends(&mut self, input: Input, now: Instant) -> bool {
        let settled = now.saturating_duration_since(self.started) >= GRACE_PERIOD;
        match input {
            Input::KeyPressed | Input::ButtonPressed | Input::Wheel => settled,
            Input::MouseAt(x, y) if !settled => {
                // Follow the mouse until the grace period is over.
                self.origin = Some((x, y));
                false
            }
            Input::MouseAt(x, y) => {
                let (ox, oy) = *self.origin.get_or_insert((x, y));
                (x - ox).hypot(y - oy) > MOUSE_THRESHOLD
            }
            // Positions before and after are relative to different window
            // corners; the jump is not a mouse movement.
            Input::WindowChanged => {
                self.origin = None;
                false
            }
            Input::Other => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started() -> (ExitOnInput, Instant) {
        let t0 = Instant::now();
        (ExitOnInput::new(t0), t0)
    }

    fn later(t0: Instant) -> Instant {
        t0 + GRACE_PERIOD + Duration::from_millis(10)
    }

    #[test]
    fn input_during_the_grace_period_is_ignored() {
        let (mut exit, t0) = started();
        let soon = t0 + Duration::from_millis(200);
        assert!(!exit.ends(Input::KeyPressed, soon));
        assert!(!exit.ends(Input::ButtonPressed, soon));
        assert!(!exit.ends(Input::MouseAt(0.0, 0.0), soon));
        assert!(!exit.ends(Input::MouseAt(500.0, 500.0), soon));
    }

    #[test]
    fn keys_clicks_and_scrolling_end_it_afterwards() {
        let (mut exit, t0) = started();
        assert!(exit.ends(Input::KeyPressed, later(t0)));
        assert!(exit.ends(Input::ButtonPressed, later(t0)));
        assert!(exit.ends(Input::Wheel, later(t0)));
    }

    #[test]
    fn releases_never_end_it() {
        // E.g. letting go of Enter after starting the screensaver from a terminal.
        let (mut exit, t0) = started();
        assert!(!exit.ends(Input::Other, later(t0)));
    }

    #[test]
    fn small_jitter_is_ignored_and_real_movement_ends_it() {
        let (mut exit, t0) = started();
        let t = later(t0);
        assert!(
            !exit.ends(Input::MouseAt(500.0, 300.0), t),
            "sets the origin"
        );
        assert!(!exit.ends(Input::MouseAt(503.0, 304.0), t));
        assert!(!exit.ends(Input::MouseAt(497.0, 296.0), t));
        assert!(exit.ends(Input::MouseAt(520.0, 300.0), t));
    }

    #[test]
    fn movement_is_measured_from_where_the_mouse_was_when_the_grace_period_ended() {
        let (mut exit, t0) = started();
        exit.ends(Input::MouseAt(100.0, 100.0), t0);
        exit.ends(
            Input::MouseAt(400.0, 300.0),
            t0 + Duration::from_millis(900),
        );
        assert!(!exit.ends(Input::MouseAt(402.0, 301.0), later(t0)));
        assert!(exit.ends(Input::MouseAt(450.0, 300.0), later(t0)));
    }

    #[test]
    fn going_fullscreen_is_not_a_mouse_movement() {
        let (mut exit, t0) = started();
        let t = later(t0);
        // The same mouse position, first relative to a window, then to the screen.
        exit.ends(Input::MouseAt(640.0, 353.0), t);
        exit.ends(Input::WindowChanged, t);
        assert!(!exit.ends(Input::MouseAt(803.0, 452.0), t));
        assert!(!exit.ends(Input::MouseAt(805.0, 450.0), t));
        assert!(exit.ends(Input::MouseAt(860.0, 452.0), t));
    }
}
