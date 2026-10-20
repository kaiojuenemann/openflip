//! GPU renderer for openflip.
//!
//! The renderer is split into three layers so each can be tested on its own:
//!
//! - [`layout`] turns a [`ClockFace`](openflip_core::ClockFace) into a [`Scene`]
//!   of rectangles and text, in pixels, without touching the GPU.
//! - [`Painter`] draws a [`Scene`] into any `wgpu` texture.
//! - [`SurfaceRenderer`] and [`render_offscreen`] wire a painter to a window
//!   surface or to an in-memory image.
//!
//! Nothing here depends on a windowing library: the app hands in a
//! `wgpu::Surface`, which keeps the door open for the macOS and Windows
//! screensaver hosts.

mod atlas;
mod font;
mod gpu;
pub mod layout;
mod painter;
mod scene;
mod theme;

pub use font::{Font, FontError};
pub use gpu::{render_offscreen, Gpu, RenderError, SurfaceRenderer, OFFSCREEN_FORMAT};
pub use painter::Painter;
pub use scene::{Color, HAlign, Rect, Scene, Text};
pub use theme::Theme;
