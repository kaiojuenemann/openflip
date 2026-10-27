//! Font loading. The bundled font is Inter Display Bold (SIL OFL 1.1, see
//! `assets/OFL.txt`).

use std::path::Path;
use std::sync::Arc;
use std::{fmt, fs, io};

use ab_glyph_rasterizer::{point, Point, Rasterizer};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};

pub(crate) use skrifa::GlyphId;

const BUNDLED: &[u8] = include_bytes!("../assets/InterDisplay-Bold.otf");

/// A font, read with skrifa. Sizes are in pixels from the lowest descender
/// to the highest ascender (as with the ab_glyph crate used before), not per
/// em, so a size of `h` makes text of that overall height.
#[derive(Clone)]
pub struct Font {
    data: Arc<[u8]>,
    /// Face within a font collection (`.ttc`); 0 for single fonts.
    index: u32,
    /// Ems per pixel of size: `units_per_em / (ascent - descent)`.
    em_per_size: f32,
    /// Ascent as a share of the size.
    ascent: f32,
    /// Height of the digit 8's waist, where its two bowls meet, above the
    /// baseline in ems; `None` if the font has no recognisable 8.
    pub(crate) waist: Option<f32>,
}

#[derive(Debug)]
pub enum FontError {
    Io(io::Error),
    Invalid,
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "could not read font: {e}"),
            Self::Invalid => write!(f, "not a valid TrueType/OpenType font"),
        }
    }
}

impl std::error::Error for FontError {}

/// A glyph rasterized to coverage values (0 to 1), one per pixel, rows top
/// to bottom.
pub(crate) struct Raster {
    /// Position of the top-left pixel relative to the pen on the baseline;
    /// y grows downwards.
    pub left: f32,
    pub top: f32,
    pub width: u32,
    pub height: u32,
    pub coverage: Vec<f32>,
}

impl Font {
    pub fn bundled() -> Self {
        Self::new(Arc::from(BUNDLED)).expect("bundled font is valid")
    }

    /// Reads a TrueType or OpenType font; of a collection, the first face.
    pub fn from_file(path: &Path) -> Result<Self, FontError> {
        let data = fs::read(path).map_err(FontError::Io)?;
        Self::new(data.into()).ok_or(FontError::Invalid)
    }

    fn new(data: Arc<[u8]>) -> Option<Self> {
        let index = 0;
        let font = FontRef::from_index(&data, index).ok()?;
        let metrics = font.metrics(Size::unscaled(), LocationRef::default());
        let units_per_em = f32::from(metrics.units_per_em);
        let height = metrics.ascent - metrics.descent;
        if !(height > 0.0 && units_per_em > 0.0) {
            return None;
        }
        let mut font = Self {
            data,
            index,
            em_per_size: units_per_em / height,
            ascent: metrics.ascent / height,
            waist: None,
        };
        font.waist = find_waist(&font);
        Some(font)
    }

    fn face(&self) -> FontRef<'_> {
        FontRef::from_index(&self.data, self.index).expect("checked when loaded")
    }

    fn ppem(&self, size: f32) -> Size {
        Size::new(size * self.em_per_size)
    }

    /// The glyph for `c`; the font's "missing glyph" if it has none.
    pub(crate) fn glyph_id(&self, c: char) -> GlyphId {
        self.face().charmap().map(c).unwrap_or(GlyphId::NOTDEF)
    }

    /// Distance from the baseline to the top of the font, in pixels.
    pub(crate) fn ascent(&self, size: f32) -> f32 {
        self.ascent * size
    }

    /// How far the pen moves after the glyph, in pixels.
    pub(crate) fn advance(&self, id: GlyphId, size: f32) -> f32 {
        self.face()
            .glyph_metrics(self.ppem(size), LocationRef::default())
            .advance_width(id)
            .unwrap_or(0.0)
    }

    /// Top and bottom of the glyph's ink relative to the baseline, in whole
    /// pixels, y growing downwards; `None` for glyphs without ink.
    pub(crate) fn ink_top_bottom(&self, id: GlyphId, size: f32) -> Option<(f32, f32)> {
        let bounds = self.bounds(id, size)?;
        Some(((-bounds.y_max).floor(), (-bounds.y_min).ceil()))
    }

    /// Rasterizes the glyph at `size`; `None` for glyphs without ink.
    pub(crate) fn rasterize(&self, id: GlyphId, size: f32) -> Option<Raster> {
        let bounds = self.bounds(id, size)?;
        let left = bounds.x_min.floor();
        let top = (-bounds.y_max).floor();
        let width = (bounds.x_max.ceil() - left) as u32;
        let height = ((-bounds.y_min).ceil() - top) as u32;
        if width == 0 || height == 0 {
            return None;
        }
        let mut pen = RasterPen {
            rasterizer: Rasterizer::new(width as usize, height as usize),
            origin: (left, top),
            start: point(0.0, 0.0),
            last: point(0.0, 0.0),
        };
        let glyph = self.face().outline_glyphs().get(id)?;
        glyph
            .draw(
                DrawSettings::unhinted(self.ppem(size), LocationRef::default()),
                &mut pen,
            )
            .ok()?;
        let mut coverage = vec![0.0; (width * height) as usize];
        pen.rasterizer.for_each_pixel_2d(|x, y, c| {
            coverage[(y * width + x) as usize] = c.clamp(0.0, 1.0);
        });
        Some(Raster {
            left,
            top,
            width,
            height,
            coverage,
        })
    }

    /// The glyph outline's bounding box in pixels, y growing upwards.
    fn bounds(&self, id: GlyphId, size: f32) -> Option<Bounds> {
        let glyph = self.face().outline_glyphs().get(id)?;
        let mut bounds = BoundsPen(None);
        glyph
            .draw(
                DrawSettings::unhinted(self.ppem(size), LocationRef::default()),
                &mut bounds,
            )
            .ok()?;
        bounds.0
    }
}

#[derive(Clone, Copy)]
struct Bounds {
    x_min: f32,
    y_min: f32,
    x_max: f32,
    y_max: f32,
}

/// Collects the box around all points of an outline, control points
/// included (fonts put on-curve points at the extremes, so this is tight).
struct BoundsPen(Option<Bounds>);

impl BoundsPen {
    fn add(&mut self, x: f32, y: f32) {
        let b = self.0.get_or_insert(Bounds {
            x_min: x,
            y_min: y,
            x_max: x,
            y_max: y,
        });
        b.x_min = b.x_min.min(x);
        b.y_min = b.y_min.min(y);
        b.x_max = b.x_max.max(x);
        b.y_max = b.y_max.max(y);
    }
}

impl OutlinePen for BoundsPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.add(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.add(x, y);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.add(cx, cy);
        self.add(x, y);
    }
    fn curve_to(&mut self, c0x: f32, c0y: f32, c1x: f32, c1y: f32, x: f32, y: f32) {
        self.add(c0x, c0y);
        self.add(c1x, c1y);
        self.add(x, y);
    }
    fn close(&mut self) {}
}

/// Draws an outline into a coverage rasterizer whose top-left pixel sits at
/// `origin` (pixels, y down) relative to the glyph's origin.
struct RasterPen {
    rasterizer: Rasterizer,
    origin: (f32, f32),
    start: Point,
    last: Point,
}

impl RasterPen {
    /// Font coordinates (y up) to rasterizer coordinates (y down).
    fn to_pixel(&self, x: f32, y: f32) -> Point {
        point(x - self.origin.0, -y - self.origin.1)
    }
}

impl OutlinePen for RasterPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.last = self.to_pixel(x, y);
        self.start = self.last;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.to_pixel(x, y);
        self.rasterizer.draw_line(self.last, p);
        self.last = p;
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (c, p) = (self.to_pixel(cx, cy), self.to_pixel(x, y));
        self.rasterizer.draw_quad(self.last, c, p);
        self.last = p;
    }
    fn curve_to(&mut self, c0x: f32, c0y: f32, c1x: f32, c1y: f32, x: f32, y: f32) {
        let (c0, c1, p) = (
            self.to_pixel(c0x, c0y),
            self.to_pixel(c1x, c1y),
            self.to_pixel(x, y),
        );
        self.rasterizer.draw_cubic(self.last, c0, c1, p);
        self.last = p;
    }
    fn close(&mut self) {
        if self.last != self.start {
            self.rasterizer.draw_line(self.last, self.start);
            self.last = self.start;
        }
    }
}

/// Finds where the two bowls of the 8 meet: the ink that the glyph's
/// vertical centre line crosses between its upper and lower counters.
fn find_waist(font: &Font) -> Option<f32> {
    const SIZE: f32 = 512.0;
    let raster = font.rasterize(font.glyph_id('8'), SIZE)?;
    let (width, height) = (raster.width, raster.height);
    let column: Vec<f32> = (0..height)
        .map(|y| raster.coverage[(y * width + width / 2) as usize])
        .collect();
    // Rows with ink on the centre line, in the middle of the glyph; the
    // run closest to the middle is the waist.
    let middle = height as f32 / 2.0;
    let rows = (height * 3 / 10)..(height * 7 / 10);
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for y in rows.filter(|&y| column[y as usize] >= 0.5) {
        match runs.last_mut() {
            Some((_, end)) if *end + 1 == y => *end = y,
            _ => runs.push((y, y)),
        }
    }
    let (start, end) = runs.into_iter().min_by(|a, b| {
        let distance = |(s, e): &(u32, u32)| ((s + e) as f32 / 2.0 - middle).abs();
        distance(a).total_cmp(&distance(b))
    })?;
    let waist = raster.top + (start + end + 1) as f32 / 2.0;
    Some(-waist / SIZE)
}

impl fmt::Debug for Font {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Font")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_waist_of_the_bundled_8_is_just_above_the_middle_of_the_digits() {
        let font = Font::bundled();
        let waist = font.waist.expect("the bundled font has an 8");
        let size = 1000.0;
        let (ink_top, ink_bottom) = font.ink_top_bottom(font.glyph_id('0'), size).unwrap();
        let (top, bottom) = (-ink_top / size, -ink_bottom / size);
        let middle = (top + bottom) / 2.0;
        assert!(waist > middle, "{waist} above {middle}");
        assert!(
            waist < middle + (top - bottom) * 0.1,
            "{waist} near {middle}"
        );
    }
}
