//! Built-in colour themes.

use crate::scene::Color;

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    pub background: Color,
    pub card: Color,
    pub digits: Color,
    /// AM/PM marker and date line.
    pub secondary: Color,
}

impl Theme {
    /// Names accepted in `config.toml`, in the order the settings show them.
    pub const NAMES: [&'static str; 4] = openflip_core::config::THEMES;

    pub fn by_name(name: &str) -> Option<Self> {
        let (background, card, digits, secondary) = match name {
            "classic" => (0x000000, 0x1a1a1a, 0xb8b8b8, 0x8c8c8c),
            "light" => (0xdadada, 0xf7f7f7, 0x262626, 0x5c5c5c),
            "amber" => (0x000000, 0x17130e, 0xffb000, 0xb87f00),
            "midnight" => (0x0a0f1e, 0x18213b, 0xc9d4f6, 0x7f8cb5),
            _ => return None,
        };
        let name = Self::NAMES.into_iter().find(|n| *n == name)?;
        Some(Self {
            name,
            background: Color::hex(background),
            card: Color::hex(card),
            digits: Color::hex(digits),
            secondary: Color::hex(secondary),
        })
    }

    pub fn classic() -> Self {
        Self::by_name("classic").expect("classic theme exists")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_theme_exists() {
        for name in Theme::NAMES {
            assert_eq!(Theme::by_name(name).unwrap().name, name);
        }
        assert!(Theme::by_name("neon").is_none());
    }
}
