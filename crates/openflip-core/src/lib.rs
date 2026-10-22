//! Platform-independent core of openflip: what the clock shows and how it is configured.
//!
//! Nothing in this crate touches windows, GPUs or the operating system's
//! screensaver APIs; those live in the renderer and the platform wrappers.

pub mod clock;
pub mod config;
pub mod i18n;
pub mod settings;

pub use clock::{ClockFace, DateLine};
pub use config::{Config, MonitorMode, TimeFormat};
pub use i18n::{Language, Text};
pub use settings::Setting;
