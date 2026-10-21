//! Screensaver behaviour: end as soon as the user does something.

/// How far the mouse may move before the screensaver ends, in pixels. The
/// window receives a position when it appears and the mouse may jitter, so
/// zero would end the screensaver right away.
const MOUSE_THRESHOLD: f64 = 10.0;

#[derive(Debug, Default)]
pub struct ExitOnInput {
    /// The first mouse position the window saw.
    origin: Option<(f64, f64)>,
}

impl ExitOnInput {
    /// Forgets the origin after the window moved or resized, e.g. when it
    /// became fullscreen: positions before and after are relative to
    /// different window corners, and the jump is not a mouse movement.
    pub fn window_changed(&mut self) {
        self.origin = None;
    }

    /// Returns `true` when the mouse moved far enough to end the screensaver.
    pub fn mouse_moved(&mut self, x: f64, y: f64) -> bool {
        let (ox, oy) = *self.origin.get_or_insert((x, y));
        (x - ox).hypot(y - oy) > MOUSE_THRESHOLD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_position_only_sets_the_origin() {
        let mut exit = ExitOnInput::default();
        assert!(!exit.mouse_moved(500.0, 300.0));
    }

    #[test]
    fn small_jitter_is_ignored_and_real_movement_ends_it() {
        let mut exit = ExitOnInput::default();
        exit.mouse_moved(500.0, 300.0);
        assert!(!exit.mouse_moved(503.0, 304.0));
        assert!(!exit.mouse_moved(497.0, 296.0));
        assert!(exit.mouse_moved(520.0, 300.0));
    }

    #[test]
    fn going_fullscreen_is_not_a_mouse_movement() {
        let mut exit = ExitOnInput::default();
        // The same mouse position, first relative to a window, then to the screen.
        exit.mouse_moved(640.0, 353.0);
        exit.window_changed();
        assert!(!exit.mouse_moved(803.0, 452.0));
        assert!(!exit.mouse_moved(805.0, 450.0));
        assert!(exit.mouse_moved(860.0, 452.0));
    }
}
