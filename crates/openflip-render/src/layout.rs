//! Places the cards, digits and date line on screen.
//!
//! Like the classic flip clocks, every pair of digits (hours, minutes,
//! seconds) shares one card. All sizes derive from the card width, so the
//! clock looks the same at any resolution.

use openflip_core::{ClockFace, DateLine};

use crate::scene::{HAlign, Rect, Scene, Text};
use crate::theme::Theme;

/// Card height relative to its width.
const CARD_ASPECT: f32 = 0.95;
/// Gap between cards relative to the card width.
const GAP: f32 = 0.06;
const CORNER_RADIUS: f32 = 0.055;
/// Thickness of the horizontal split line relative to the card height.
const SPLIT: f32 = 0.012;
/// Digit font size relative to the card height.
const DIGIT_SIZE: f32 = 0.74;
/// Width of one digit slot relative to the card width.
const DIGIT_SLOT: f32 = 0.43;
const MARKER_SIZE: f32 = 0.09;
const DATE_SIZE: f32 = 0.11;
/// Space between the cards and the date line relative to the card height.
const DATE_GAP: f32 = 0.08;

/// Builds the scene for one moment.
///
/// `scale` is the fraction of the screen the clock may fill, as in
/// [`Config::scale`](openflip_core::Config::scale).
pub fn build_scene(
    face: &ClockFace,
    date: Option<&DateLine>,
    theme: &Theme,
    (width, height): (u32, u32),
    scale: f32,
) -> Scene {
    let mut pairs = vec![face.hours, face.minutes];
    pairs.extend(face.seconds);
    let cards = pairs.len() as f32;
    let scale = scale.clamp(0.1, 1.0);

    let date_extra = if date.is_some() {
        DATE_GAP + DATE_SIZE
    } else {
        0.0
    };
    let by_width = scale * width as f32 / (cards + (cards - 1.0) * GAP);
    let by_height = scale * height as f32 / (CARD_ASPECT * (1.0 + date_extra));
    let card_w = by_width.min(by_height).floor().max(1.0);
    let card_h = (card_w * CARD_ASPECT).floor();
    let gap = (card_w * GAP).round();

    let block_w = cards * card_w + (cards - 1.0) * gap;
    let block_h = card_h * (1.0 + date_extra);
    let left = ((width as f32 - block_w) / 2.0).round();
    let top = ((height as f32 - block_h) / 2.0).round();

    let mut scene = Scene {
        width,
        height,
        background: theme.background,
        rects: Vec::new(),
        texts: Vec::new(),
        overlays: Vec::new(),
    };

    let split = (card_h * SPLIT).max(1.0).round();
    for (i, pair) in pairs.iter().enumerate() {
        let x = left + i as f32 * (card_w + gap);
        scene.rects.push(Rect {
            x,
            y: top,
            width: card_w,
            height: card_h,
            radius: card_w * CORNER_RADIUS,
            color: theme.card,
        });
        scene.overlays.push(Rect {
            x,
            y: (top + (card_h - split) / 2.0).round(),
            width: card_w,
            height: split,
            radius: 0.0,
            color: theme.background,
        });
        scene.texts.push(Text {
            content: pair.iter().collect(),
            size: card_h * DIGIT_SIZE,
            color: theme.digits,
            x: x + card_w / 2.0,
            y: top + card_h / 2.0,
            align: HAlign::Center,
            slot_width: Some(card_w * DIGIT_SLOT),
        });
    }

    if let Some(pm) = face.pm {
        let inset = card_w * 0.07;
        let size = card_h * MARKER_SIZE;
        scene.texts.push(Text {
            content: if pm { "PM" } else { "AM" }.into(),
            size,
            color: theme.secondary,
            x: left + inset,
            y: top + inset + size / 2.0,
            align: HAlign::Left,
            slot_width: None,
        });
    }

    if let Some(date) = date {
        let size = card_h * DATE_SIZE;
        scene.texts.push(Text {
            content: date.0.clone(),
            size,
            color: theme.secondary,
            x: width as f32 / 2.0,
            y: top + card_h + card_h * DATE_GAP + size / 2.0,
            align: HAlign::Center,
            slot_width: None,
        });
    }

    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(seconds: bool, pm: Option<bool>) -> ClockFace {
        ClockFace {
            hours: ['1', '2'],
            minutes: ['3', '4'],
            seconds: seconds.then_some(['5', '6']),
            pm,
        }
    }

    fn cards(scene: &Scene) -> Vec<&Rect> {
        scene.rects.iter().collect()
    }

    #[test]
    fn two_cards_are_centred_and_fit_the_screen() {
        let scene = build_scene(
            &face(false, None),
            None,
            &Theme::classic(),
            (1920, 1080),
            0.9,
        );
        let cards = cards(&scene);
        assert_eq!(cards.len(), 2);
        let left = cards[0].x;
        let right = cards[1].x + cards[1].width;
        assert!(
            (left - (1920.0 - right)).abs() <= 1.0,
            "horizontally centred"
        );
        assert!(right - left <= 1920.0 * 0.9 + 1.0, "respects scale");
        assert!(cards[0].height <= 1080.0 * 0.9);
        assert_eq!(scene.texts[0].content, "12");
        assert_eq!(scene.texts[1].content, "34");
    }

    #[test]
    fn seconds_add_a_third_card() {
        let scene = build_scene(
            &face(true, None),
            None,
            &Theme::classic(),
            (1920, 1080),
            0.9,
        );
        assert_eq!(cards(&scene).len(), 3);
        assert_eq!(scene.texts[2].content, "56");
    }

    #[test]
    fn portrait_screens_are_limited_by_width() {
        let scene = build_scene(&face(true, None), None, &Theme::classic(), (600, 1600), 1.0);
        let cards = cards(&scene);
        let right = cards[2].x + cards[2].width;
        assert!(cards[0].x >= 0.0 && right <= 600.0);
    }

    #[test]
    fn date_line_and_marker_stay_on_screen() {
        let date = DateLine("Tuesday, October 20".into());
        let scene = build_scene(
            &face(false, Some(true)),
            Some(&date),
            &Theme::classic(),
            (1280, 720),
            1.0,
        );
        let marker = scene.texts.iter().find(|t| t.content == "PM").unwrap();
        let date = scene
            .texts
            .iter()
            .find(|t| t.content.starts_with("Tuesday"))
            .unwrap();
        assert!(date.y + date.size / 2.0 <= 720.0);
        assert!(date.y > cards(&scene)[0].y + cards(&scene)[0].height);
        assert!(marker.x > cards(&scene)[0].x);
    }

    #[test]
    fn tiny_windows_do_not_panic() {
        let scene = build_scene(
            &face(true, Some(false)),
            None,
            &Theme::classic(),
            (1, 1),
            0.9,
        );
        assert!(scene.rects.iter().all(|r| r.width >= 1.0));
    }

    #[test]
    fn split_lines_cross_the_middle_of_each_card() {
        let scene = build_scene(
            &face(true, None),
            None,
            &Theme::classic(),
            (1920, 1080),
            0.9,
        );
        assert_eq!(scene.overlays.len(), 3);
        for (card, split) in scene.rects.iter().zip(&scene.overlays) {
            assert_eq!(split.x, card.x);
            let middle = split.y + split.height / 2.0;
            assert!((middle - (card.y + card.height / 2.0)).abs() <= 1.0);
        }
    }
}
