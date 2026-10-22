//! The settings panel drawn over the clock.
//!
//! Like [`layout`](crate::layout), this only computes geometry: it turns an
//! [`OverlayView`] into scene groups and reports where each row sits, so the
//! app can hit-test mouse clicks.

use crate::scene::{Color, Group, HAlign, Item, Rect, Text, VAlign};
use crate::theme::Theme;

/// What the panel shows; the app fills it from its settings.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayView {
    pub title: String,
    pub rows: Vec<OverlayRow>,
    pub selected: usize,
    /// Help line at the bottom.
    pub hint: String,
    /// Shown above the hint, e.g. when saving failed.
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverlayRow {
    pub label: String,
    pub value: String,
}

/// Screen areas of one row, for mouse input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowArea {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Left edge of the value column; clicks left of its centre go back.
    pub value_x: f32,
    pub value_width: f32,
}

impl RowArea {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// Whether a click at `x` should step to the previous choice.
    pub fn is_back(&self, x: f32) -> bool {
        x >= self.value_x && x < self.value_x + self.value_width * 0.25
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverlayLayout {
    pub groups: Vec<Group>,
    pub panel: RowArea,
    pub rows: Vec<RowArea>,
}

/// Row height relative to the screen height, before limits.
const ROW_HEIGHT: f32 = 0.065;
const MIN_ROW_HEIGHT: f32 = 20.0;
/// Panel width in row heights.
const PANEL_WIDTH: f32 = 15.0;
/// Text size relative to the row height.
const TEXT_SIZE: f32 = 0.42;
const TITLE_SIZE: f32 = 0.6;
const HINT_SIZE: f32 = 0.34;
const BACKDROP_ALPHA: f32 = 0.55;

pub fn build_overlay(
    view: &OverlayView,
    theme: &Theme,
    (width, height): (u32, u32),
) -> OverlayLayout {
    let (w, h) = (width as f32, height as f32);
    // Title, rows, message and hint, plus padding.
    let lines = view.rows.len() as f32 + 3.6;
    let row_h = (h * ROW_HEIGHT)
        .min(h * 0.92 / lines)
        .min(w * 0.94 / PANEL_WIDTH)
        .max(MIN_ROW_HEIGHT.min(h / lines))
        .floor();
    let panel_w = (row_h * PANEL_WIDTH).min(w).floor();
    let panel_h = (row_h * lines).floor();
    let panel_x = ((w - panel_w) / 2.0).round();
    let panel_y = ((h - panel_h) / 2.0).round();
    let pad = row_h * 0.6;

    let mut items = vec![
        Item::Rect(Rect {
            x: 0.0,
            y: 0.0,
            width: w,
            height: h,
            radius: 0.0,
            color: with_alpha(theme.background, BACKDROP_ALPHA),
        }),
        Item::Rect(Rect {
            x: panel_x,
            y: panel_y,
            width: panel_w,
            height: panel_h,
            radius: row_h * 0.35,
            color: theme.card,
        }),
        Item::Text(Text {
            content: view.title.clone(),
            size: row_h * TITLE_SIZE,
            color: theme.digits,
            x: panel_x + pad,
            y: panel_y + row_h * 0.8,
            align: HAlign::Left,
            valign: VAlign::Middle,
            slot_width: None,
        }),
    ];

    let inner_x = panel_x + pad * 0.5;
    let inner_w = panel_w - pad;
    let value_w = (inner_w * 0.5).floor();
    let value_x = inner_x + inner_w - value_w;
    let rows_top = panel_y + row_h * 1.6;
    let mut rows = Vec::with_capacity(view.rows.len());
    for (i, row) in view.rows.iter().enumerate() {
        let y = rows_top + i as f32 * row_h;
        let area = RowArea {
            x: inner_x,
            y,
            width: inner_w,
            height: row_h,
            value_x,
            value_width: value_w,
        };
        let middle = y + row_h / 2.0;
        let size = row_h * TEXT_SIZE;
        if i == view.selected {
            items.push(Item::Rect(Rect {
                x: area.x,
                y: area.y + row_h * 0.06,
                width: area.width,
                height: row_h * 0.88,
                radius: row_h * 0.2,
                color: with_alpha(theme.digits, 0.12),
            }));
        }
        items.push(Item::Text(Text {
            content: row.label.clone(),
            size,
            color: theme.secondary,
            x: inner_x + pad * 0.5,
            y: middle,
            align: HAlign::Left,
            valign: VAlign::Middle,
            slot_width: None,
        }));
        let arrow_color = if i == view.selected {
            theme.digits
        } else {
            with_alpha(theme.secondary, 0.5)
        };
        for (arrow, x) in [
            ("‹", value_x + row_h * 0.3),
            ("›", value_x + value_w - row_h * 0.3),
        ] {
            items.push(Item::Text(Text {
                content: arrow.into(),
                size,
                color: arrow_color,
                x,
                y: middle,
                align: HAlign::Center,
                valign: VAlign::Middle,
                slot_width: None,
            }));
        }
        items.push(Item::Text(Text {
            content: row.value.clone(),
            size,
            color: theme.digits,
            x: value_x + value_w / 2.0,
            y: middle,
            align: HAlign::Center,
            valign: VAlign::Middle,
            slot_width: None,
        }));
        rows.push(area);
    }

    let hint_y = panel_y + panel_h - row_h * 0.6;
    if let Some(message) = &view.message {
        items.push(Item::Text(Text {
            content: message.clone(),
            size: row_h * HINT_SIZE,
            color: theme.digits,
            x: panel_x + panel_w / 2.0,
            y: hint_y - row_h * 0.5,
            align: HAlign::Center,
            valign: VAlign::Middle,
            slot_width: None,
        }));
    }
    items.push(Item::Text(Text {
        content: view.hint.clone(),
        size: row_h * HINT_SIZE,
        color: theme.secondary,
        x: panel_x + panel_w / 2.0,
        y: hint_y,
        align: HAlign::Center,
        valign: VAlign::Middle,
        slot_width: None,
    }));

    OverlayLayout {
        groups: vec![Group { fold: None, items }],
        panel: RowArea {
            x: panel_x,
            y: panel_y,
            width: panel_w,
            height: panel_h,
            value_x,
            value_width: value_w,
        },
        rows,
    }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(rows: usize) -> OverlayView {
        OverlayView {
            title: "Settings".into(),
            rows: (0..rows)
                .map(|i| OverlayRow {
                    label: format!("Setting {i}"),
                    value: "On".into(),
                })
                .collect(),
            selected: 2,
            hint: "hint".into(),
            message: Some("oops".into()),
        }
    }

    #[test]
    fn panel_and_rows_fit_on_screen() {
        for size in [(1920, 1080), (800, 600), (600, 1600), (320, 200)] {
            let layout = build_overlay(&view(8), &Theme::classic(), size);
            let (w, h) = (size.0 as f32, size.1 as f32);
            let p = layout.panel;
            assert!(p.x >= 0.0 && p.y >= 0.0, "{size:?}");
            assert!(p.x + p.width <= w && p.y + p.height <= h, "{size:?}");
            for row in &layout.rows {
                assert!(
                    row.y >= p.y && row.y + row.height <= p.y + p.height,
                    "{size:?}"
                );
            }
        }
    }

    #[test]
    fn rows_do_not_overlap_and_are_hit_tested() {
        let layout = build_overlay(&view(8), &Theme::classic(), (1280, 720));
        for pair in layout.rows.windows(2) {
            assert!(pair[0].y + pair[0].height <= pair[1].y + 0.01);
        }
        let row = layout.rows[3];
        assert!(row.contains(row.x + 1.0, row.y + 1.0));
        assert!(!row.contains(row.x - 1.0, row.y + 1.0));
        assert!(row.is_back(row.value_x + 2.0));
        assert!(!row.is_back(row.value_x + row.value_width - 2.0));
    }

    #[test]
    fn only_the_selected_row_is_highlighted() {
        let layout = build_overlay(&view(8), &Theme::classic(), (1280, 720));
        let highlights: Vec<_> = layout.groups[0]
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Rect(r) if (r.color.a - 0.12).abs() < 1e-6 => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(highlights.len(), 1);
        assert!(layout.rows[2].contains(highlights[0].x + 1.0, highlights[0].y + 1.0));
    }
}
