//! Backend-independent description of one frame, in physical pixels.

/// A colour in sRGB space with straight (non-premultiplied) alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// Builds an opaque colour from a `0xRRGGBB` value.
    pub fn hex(rgb: u32) -> Self {
        let channel = |shift: u32| ((rgb >> shift) & 0xff) as f32 / 255.0;
        Self {
            r: channel(16),
            g: channel(8),
            b: channel(0),
            a: 1.0,
        }
    }

    /// Converts to linear light, for render targets with an sRGB format.
    pub fn to_linear(self) -> Self {
        let convert = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        Self {
            r: convert(self.r),
            g: convert(self.g),
            b: convert(self.b),
            a: self.a,
        }
    }
}

/// A filled rectangle with rounded corners.
#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub radius: f32,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
}

/// A run of text, vertically centred on `y` using the height of the digits.
#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    pub content: String,
    /// Font size in pixels (one em).
    pub size: f32,
    pub color: Color,
    pub x: f32,
    pub y: f32,
    pub align: HAlign,
    /// When set, every character is centred in a slot of this width, so
    /// digits do not shift sideways as they change.
    pub slot_width: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub width: u32,
    pub height: u32,
    pub background: Color,
    /// Drawn in order, before any text.
    pub rects: Vec<Rect>,
    pub texts: Vec<Text>,
    /// Drawn in order, on top of the text (e.g. the split line through the digits).
    pub overlays: Vec<Rect>,
}
