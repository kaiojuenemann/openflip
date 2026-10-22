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
pub enum Item {
    Rect(Rect),
    Text(Text),
}

/// Which half of a card a [`Fold`] keeps; the other half is cut away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    Top,
    Bottom,
}

/// Cuts a group's content at a horizontal hinge, keeps one half and
/// optionally rotates it about the hinge in 3D.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fold {
    /// The hinge line, usually the middle of a card.
    pub hinge_y: f32,
    /// Horizontal centre of the card; the perspective converges here.
    pub center_x: f32,
    pub half: Half,
    /// Rotation about the hinge in radians. Positive angles lift the top
    /// half towards the viewer; negative angles do the same for the bottom
    /// half. `0` lies flat on the screen.
    pub angle: f32,
    /// Distance of the eye from the screen in pixels; smaller values give
    /// stronger perspective.
    pub eye_distance: f32,
    /// Darkening from `0` (none) to `1` (black).
    pub shade: f32,
    /// When positive, the darkening fades out linearly over this many
    /// pixels away from the hinge, like a shadow cast by a flap above it.
    pub shade_falloff: f32,
}

/// Items drawn together, optionally cut and rotated by a [`Fold`].
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub fold: Option<Fold>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub width: u32,
    pub height: u32,
    pub background: Color,
    /// Painted in order; later groups cover earlier ones.
    pub groups: Vec<Group>,
}

impl Scene {
    /// All items in paint order, ignoring folds. Handy for tests.
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.groups.iter().flat_map(|g| &g.items)
    }
}
