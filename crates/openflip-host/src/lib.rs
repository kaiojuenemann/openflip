//! What every openflip host has in common.
//!
//! The window app, the macOS screensaver and later the Windows and Linux
//! screensavers differ only in how they get a drawing surface and input
//! events. Everything else — which time to show, when to draw the next
//! frame, the settings panel, loading and saving the config — lives here,
//! behind [`Clock`].

mod clock;
mod look;
pub mod menu;

pub use clock::{Clock, Frame, Key, Response};
pub use look::{load_config, Look};
