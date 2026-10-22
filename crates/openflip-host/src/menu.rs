//! Navigation in the settings overlay, independent of windows and events.

use openflip_core::{Config, Language, Setting, Text};
use openflip_render::overlay::{OverlayRow, OverlayView};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    Left,
    Right,
    /// Enter or Space.
    Activate,
    /// Delete or Backspace.
    Reset,
}

/// What the app has to do after the menu handled an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Nothing,
    /// Only the menu's own state changed.
    Redraw,
    /// The config changed and must be applied and saved.
    ConfigChanged,
    /// The user wants to pick a font file.
    ChooseFont,
}

#[derive(Debug, Default)]
pub struct SettingsMenu {
    selected: usize,
}

impl SettingsMenu {
    fn setting(&self) -> Setting {
        Setting::ALL[self.selected]
    }

    pub fn key(&mut self, key: MenuKey, config: &mut Config) -> MenuAction {
        let count = Setting::ALL.len();
        match key {
            MenuKey::Up => {
                self.selected = (self.selected + count - 1) % count;
                MenuAction::Redraw
            }
            MenuKey::Down => {
                self.selected = (self.selected + 1) % count;
                MenuAction::Redraw
            }
            MenuKey::Left => self.step(config, false),
            MenuKey::Right | MenuKey::Activate => self.step(config, true),
            MenuKey::Reset if self.setting() == Setting::Font && config.font.is_some() => {
                config.font = None;
                MenuAction::ConfigChanged
            }
            MenuKey::Reset => MenuAction::Nothing,
        }
    }

    /// Moves the selection to the row under the mouse.
    pub fn hover(&mut self, row: usize) -> MenuAction {
        if row == self.selected || row >= Setting::ALL.len() {
            return MenuAction::Nothing;
        }
        self.selected = row;
        MenuAction::Redraw
    }

    /// A click on a row; `back` when it hit the left part of the value.
    pub fn click(&mut self, row: usize, back: bool, config: &mut Config) -> MenuAction {
        if row >= Setting::ALL.len() {
            return MenuAction::Nothing;
        }
        self.selected = row;
        self.step(config, !back)
    }

    fn step(&mut self, config: &mut Config, forward: bool) -> MenuAction {
        if self.setting().step(config, forward) {
            MenuAction::ConfigChanged
        } else {
            MenuAction::ChooseFont
        }
    }

    pub fn view(
        &self,
        config: &Config,
        language: Language,
        system: Language,
        message: Option<String>,
    ) -> OverlayView {
        let hint = if self.setting() == Setting::Font {
            Text::FontHint
        } else {
            Text::MenuHint
        };
        OverlayView {
            title: language.text(Text::SettingsTitle).into(),
            rows: Setting::ALL
                .iter()
                .map(|s| OverlayRow {
                    label: s.label(language).into(),
                    value: s.value(config, language, system),
                })
                .collect(),
            selected: self.selected,
            hint: language.text(hint).into(),
            message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn select(menu: &mut SettingsMenu, setting: Setting) {
        let mut config = Config::default();
        while menu.setting() != setting {
            menu.key(MenuKey::Down, &mut config);
        }
    }

    #[test]
    fn up_and_down_wrap_around() {
        let mut menu = SettingsMenu::default();
        let mut config = Config::default();
        assert_eq!(menu.key(MenuKey::Up, &mut config), MenuAction::Redraw);
        assert_eq!(menu.setting(), Setting::Language);
        menu.key(MenuKey::Down, &mut config);
        assert_eq!(menu.setting(), Setting::TimeFormat);
    }

    #[test]
    fn left_right_and_enter_change_the_selected_setting() {
        let mut menu = SettingsMenu::default();
        select(&mut menu, Setting::Seconds);
        let mut config = Config::default();
        assert_eq!(
            menu.key(MenuKey::Right, &mut config),
            MenuAction::ConfigChanged
        );
        assert!(config.show_seconds);
        menu.key(MenuKey::Left, &mut config);
        assert!(!config.show_seconds);
        menu.key(MenuKey::Activate, &mut config);
        assert!(config.show_seconds);
    }

    #[test]
    fn the_font_row_opens_a_file_picker_and_resets() {
        let mut menu = SettingsMenu::default();
        select(&mut menu, Setting::Font);
        let mut config = Config::default();
        assert_eq!(
            menu.key(MenuKey::Activate, &mut config),
            MenuAction::ChooseFont
        );
        assert_eq!(menu.key(MenuKey::Reset, &mut config), MenuAction::Nothing);
        config.font = Some("/x.ttf".into());
        assert_eq!(
            menu.key(MenuKey::Reset, &mut config),
            MenuAction::ConfigChanged
        );
        assert_eq!(config.font, None);
    }

    #[test]
    fn clicks_select_and_step() {
        let mut menu = SettingsMenu::default();
        let mut config = Config::default();
        let theme = Setting::ALL
            .iter()
            .position(|s| *s == Setting::Theme)
            .unwrap();
        assert_eq!(
            menu.click(theme, false, &mut config),
            MenuAction::ConfigChanged
        );
        assert_eq!(menu.setting(), Setting::Theme);
        assert_eq!(config.theme, "light");
        menu.click(theme, true, &mut config);
        assert_eq!(config.theme, "classic");
        assert_eq!(menu.click(99, false, &mut config), MenuAction::Nothing);
    }

    #[test]
    fn hover_only_redraws_on_change() {
        let mut menu = SettingsMenu::default();
        assert_eq!(menu.hover(0), MenuAction::Nothing);
        assert_eq!(menu.hover(4), MenuAction::Redraw);
        assert_eq!(menu.hover(4), MenuAction::Nothing);
    }

    #[test]
    fn view_lists_every_setting_and_switches_the_hint_on_the_font_row() {
        let mut menu = SettingsMenu::default();
        let config = Config::default();
        let view = menu.view(&config, Language::German, Language::German, None);
        assert_eq!(view.title, "Einstellungen");
        assert_eq!(view.rows.len(), Setting::ALL.len());
        let normal_hint = view.hint.clone();
        select(&mut menu, Setting::Font);
        let view = menu.view(&config, Language::German, Language::German, None);
        assert_ne!(view.hint, normal_hint);
    }
}
