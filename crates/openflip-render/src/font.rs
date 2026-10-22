//! Font loading. The bundled font is Inter Display Bold (SIL OFL 1.1, see
//! `assets/OFL.txt`).

use std::path::Path;
use std::{fmt, fs, io};

use ab_glyph::{Font as _, FontArc, PxScale, ScaleFont as _};

const BUNDLED: &[u8] = include_bytes!("../assets/InterDisplay-Bold.otf");

#[derive(Clone)]
pub struct Font {
    pub(crate) data: FontArc,
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

impl Font {
    pub fn bundled() -> Self {
        Self::new(FontArc::try_from_slice(BUNDLED).expect("bundled font is valid"))
    }

    pub fn from_file(path: &Path) -> Result<Self, FontError> {
        let data = fs::read(path).map_err(FontError::Io)?;
        FontArc::try_from_vec(data)
            .map(Self::new)
            .map_err(|_| FontError::Invalid)
    }

    fn new(data: FontArc) -> Self {
        let waist = find_waist(&data);
        Self { data, waist }
    }
}

/// Finds where the two bowls of the 8 meet: the ink that the glyph's
/// vertical centre line crosses between its upper and lower counters.
fn find_waist(font: &FontArc) -> Option<f32> {
    const SIZE: f32 = 512.0;
    let scaled = font.as_scaled(PxScale::from(SIZE));
    let glyph = scaled.glyph_id('8').with_scale(SIZE);
    let outline = scaled.outline_glyph(glyph)?;
    let bounds = outline.px_bounds();
    let (width, height) = (bounds.width() as u32, bounds.height() as u32);
    let mut column = vec![0.0; height as usize];
    outline.draw(|x, y, coverage| {
        if x == width / 2 {
            column[y as usize] = coverage;
        }
    });
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
    let waist = bounds.min.y + (start + end + 1) as f32 / 2.0;
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
        let scaled = font.data.as_scaled(PxScale::from(size));
        let zero = scaled
            .outline_glyph(scaled.glyph_id('0').with_scale(size))
            .unwrap()
            .px_bounds();
        let (top, bottom) = (-zero.min.y / size, -zero.max.y / size);
        let middle = (top + bottom) / 2.0;
        assert!(waist > middle, "{waist} above {middle}");
        assert!(
            waist < middle + (top - bottom) * 0.1,
            "{waist} near {middle}"
        );
    }
}
