//! Which display an X window is on, for XScreenSaver: it runs one copy of
//! the screensaver per display, and only the primary one shows the clock
//! when the settings ask for that.

use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{ConnectionExt as _, Window};

/// A rectangle in root window coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Area {
    /// Whether the centre of `self` lies within `display`.
    pub fn centred_in(&self, display: &Area) -> bool {
        let cx = i64::from(self.x) + i64::from(self.width) / 2;
        let cy = i64::from(self.y) + i64::from(self.height) / 2;
        let (dx, dy) = (i64::from(display.x), i64::from(display.y));
        (dx..dx + i64::from(display.width)).contains(&cx)
            && (dy..dy + i64::from(display.height)).contains(&cy)
    }
}

/// Whether `window`, `size` pixels large, is on the primary display. If
/// no monitor is marked primary, the first one counts, as on Wayland. If
/// the X server cannot tell at all (no RandR), every window counts as
/// primary, so no display stays blank by mistake.
pub fn on_primary_output(
    connection: &impl Connection,
    root: Window,
    window: Window,
    size: (u32, u32),
) -> bool {
    let (Some(display), Some(position)) = (
        primary_output_area(connection, root),
        window_position(connection, root, window),
    ) else {
        return true;
    };
    let area = Area {
        x: position.0,
        y: position.1,
        width: size.0,
        height: size.1,
    };
    log::info!("window at {area:?}, primary display at {display:?}");
    area.centred_in(&display)
}

/// The primary monitor, or else the first, as RandR 1.5 reports them: the
/// same list xrandr and XScreenSaver use.
fn primary_output_area(connection: &impl Connection, root: Window) -> Option<Area> {
    let monitors = connection
        .randr_get_monitors(root, true)
        .ok()?
        .reply()
        .ok()?
        .monitors;
    let primary = monitors
        .iter()
        .find(|m| m.primary)
        .or_else(|| monitors.first())?;
    Some(Area {
        x: i32::from(primary.x),
        y: i32::from(primary.y),
        width: u32::from(primary.width),
        height: u32::from(primary.height),
    })
}

fn window_position(
    connection: &impl Connection,
    root: Window,
    window: Window,
) -> Option<(i32, i32)> {
    let reply = connection
        .translate_coordinates(window, root, 0, 0)
        .ok()?
        .reply()
        .ok()?;
    Some((i32::from(reply.dst_x), i32::from(reply.dst_y)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT: Area = Area {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    const RIGHT: Area = Area {
        x: 1920,
        y: 0,
        width: 2560,
        height: 1440,
    };

    #[test]
    fn a_window_belongs_to_the_display_its_centre_is_on() {
        assert!(LEFT.centred_in(&LEFT));
        assert!(!LEFT.centred_in(&RIGHT));
        assert!(RIGHT.centred_in(&RIGHT));
        assert!(!RIGHT.centred_in(&LEFT));
    }

    #[test]
    fn displays_left_of_or_above_the_origin_count_too() {
        let above = Area {
            x: -400,
            y: -1080,
            width: 1920,
            height: 1080,
        };
        assert!(above.centred_in(&above));
        assert!(!above.centred_in(&LEFT));
    }
}
