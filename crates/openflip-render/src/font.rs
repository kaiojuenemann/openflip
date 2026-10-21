//! Font loading. The bundled font is Inter Display Bold (SIL OFL 1.1, see
//! `assets/OFL.txt`).

use std::path::Path;
use std::{fmt, fs, io};

use ab_glyph::FontArc;

const BUNDLED: &[u8] = include_bytes!("../assets/InterDisplay-Bold.otf");

#[derive(Clone)]
pub struct Font(pub(crate) FontArc);

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
        Self(FontArc::try_from_slice(BUNDLED).expect("bundled font is valid"))
    }

    pub fn from_file(path: &Path) -> Result<Self, FontError> {
        let data = fs::read(path).map_err(FontError::Io)?;
        FontArc::try_from_vec(data)
            .map(Self)
            .map_err(|_| FontError::Invalid)
    }
}

impl fmt::Debug for Font {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Font")
    }
}
