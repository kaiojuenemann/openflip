//! Places the cards, digits and date line on screen.
//!
//! Like the classic flip clocks, every pair of digits (hours, minutes,
//! seconds) shares one card. All sizes derive from the card width, so the
//! clock looks the same at any resolution.

use std::f32::consts::{FRAC_PI_2, PI};

use openflip_core::DateLine;

use crate::animation::{flip_angle, CardContent, CardFlip};
use crate::scene::{Fold, Group, HAlign, Half, Item, Rect, Scene, Text, VAlign};
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
/// Eye distance for the flip perspective relative to the card height.
const EYE_DISTANCE: f32 = 4.0;
/// Darkest shading of the flap when it stands edge-on.
const FLAP_SHADE: f32 = 0.45;
/// Darkest shadow the falling flap casts on the lower half.
const CAST_SHADOW: f32 = 0.5;

/// Where one card sits on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CardBox {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl CardBox {
    fn hinge_y(&self) -> f32 {
        self.y + self.height / 2.0
    }

    fn center_x(&self) -> f32 {
        self.x + self.width / 2.0
    }
}

/// Builds the scene for one frame.
///
/// `scale` is the fraction of the screen the clock may fill, as in
/// [`Config::scale`](openflip_core::Config::scale).
pub fn build_scene(
    cards: &[CardFlip],
    date: Option<&DateLine>,
    theme: &Theme,
    (width, height): (u32, u32),
    scale: f32,
) -> Scene {
    let count = cards.len().max(1) as f32;
    let scale = scale.clamp(0.1, 1.0);

    let date_extra = if date.is_some() {
        DATE_GAP + DATE_SIZE
    } else {
        0.0
    };
    let by_width = scale * width as f32 / (count + (count - 1.0) * GAP);
    let by_height = scale * height as f32 / (CARD_ASPECT * (1.0 + date_extra));
    let card_w = by_width.min(by_height).floor().max(1.0);
    let card_h = (card_w * CARD_ASPECT).floor().max(1.0);
    let gap = (card_w * GAP).round();

    let block_w = count * card_w + (count - 1.0) * gap;
    let block_h = card_h * (1.0 + date_extra);
    let left = ((width as f32 - block_w) / 2.0).round();
    let top = ((height as f32 - block_h) / 2.0).round();

    let mut scene = Scene {
        width,
        height,
        background: theme.background,
        groups: Vec::new(),
    };

    for (i, flip) in cards.iter().enumerate() {
        let card = CardBox {
            x: left + i as f32 * (card_w + gap),
            y: top,
            width: card_w,
            height: card_h,
        };
        push_card(&mut scene, card, flip, theme);
    }

    if let Some(date) = date {
        let size = card_h * DATE_SIZE;
        scene.groups.push(Group {
            fold: None,
            items: vec![Item::Text(Text {
                content: date.0.clone(),
                size,
                color: theme.secondary,
                x: width as f32 / 2.0,
                y: top + card_h + card_h * DATE_GAP + size / 2.0,
                align: HAlign::Center,
                valign: VAlign::Middle,
                slot_width: None,
            })],
        });
    }

    scene
}

fn push_card(scene: &mut Scene, card: CardBox, flip: &CardFlip, theme: &Theme) {
    if !flip.is_flipping() {
        scene.groups.push(Group {
            fold: None,
            items: face(card, &flip.to, theme),
        });
        scene.groups.push(split_line(card, theme));
        return;
    }

    let angle = flip_angle(flip.progress);
    let fold = |half, angle, shade, shade_falloff| Fold {
        hinge_y: card.hinge_y(),
        center_x: card.center_x(),
        half,
        angle,
        eye_distance: card.height * EYE_DISTANCE,
        shade,
        shade_falloff,
    };

    // Behind the flap: the new value is already revealed on top, the old
    // value still shows below until the flap covers it.
    let falling = angle < FRAC_PI_2;
    let shadow = if falling {
        CAST_SHADOW * angle.sin()
    } else {
        0.0
    };
    scene.groups.push(Group {
        fold: Some(fold(Half::Top, 0.0, 0.0, 0.0)),
        items: face(card, &flip.to, theme),
    });
    scene.groups.push(Group {
        fold: Some(fold(Half::Bottom, 0.0, shadow, card.height / 2.0)),
        items: face(card, &flip.from, theme),
    });
    scene.groups.push(split_line(card, theme));

    // The flap: old top half while falling, new bottom half (its back)
    // while coming down.
    let flap_shade = FLAP_SHADE * angle.sin();
    let flap = if falling {
        fold(Half::Top, angle, flap_shade, 0.0)
    } else {
        fold(Half::Bottom, angle - PI, flap_shade, 0.0)
    };
    scene.groups.push(Group {
        fold: Some(flap),
        items: face(card, if falling { &flip.from } else { &flip.to }, theme),
    });
}

/// The card background with its digits and marker.
fn face(card: CardBox, content: &CardContent, theme: &Theme) -> Vec<Item> {
    let mut items = vec![
        Item::Rect(Rect {
            x: card.x,
            y: card.y,
            width: card.width,
            height: card.height,
            radius: card.width * CORNER_RADIUS,
            color: theme.card,
            texture: theme.card_texture,
        }),
        Item::Text(Text {
            content: content.digits.clone(),
            size: card.height * DIGIT_SIZE,
            color: theme.digits,
            x: card.center_x(),
            y: card.hinge_y(),
            align: HAlign::Center,
            valign: VAlign::Waist,
            slot_width: Some(card.width * DIGIT_SLOT),
        }),
    ];
    if let Some(marker) = content.marker {
        let inset = card.width * 0.07;
        let size = card.height * MARKER_SIZE;
        items.push(Item::Text(Text {
            content: marker.into(),
            size,
            color: theme.secondary,
            x: card.x + inset,
            y: card.y + inset + size / 2.0,
            align: HAlign::Left,
            valign: VAlign::Middle,
            slot_width: None,
        }));
    }
    items
}

/// The gap between the two halves, drawn through the digits.
fn split_line(card: CardBox, theme: &Theme) -> Group {
    let split = (card.height * SPLIT).max(1.0).round();
    Group {
        fold: None,
        items: vec![Item::Rect(Rect {
            x: card.x,
            y: (card.y + (card.height - split) / 2.0).round(),
            width: card.width,
            height: split,
            radius: 0.0,
            color: theme.background,
            texture: 0.0,
        })],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(digits: &str) -> CardContent {
        CardContent {
            digits: digits.into(),
            marker: None,
        }
    }

    fn resting(list: &[&str]) -> Vec<CardFlip> {
        list.iter().map(|d| CardFlip::at_rest(content(d))).collect()
    }

    /// Card backgrounds, i.e. rects with rounded corners.
    fn cards(scene: &Scene) -> Vec<&Rect> {
        scene
            .items()
            .filter_map(|item| match item {
                Item::Rect(r) if r.radius > 0.0 => Some(r),
                _ => None,
            })
            .collect()
    }

    fn texts(scene: &Scene) -> Vec<&Text> {
        scene
            .items()
            .filter_map(|item| match item {
                Item::Text(t) => Some(t),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn two_cards_are_centred_and_fit_the_screen() {
        let scene = build_scene(
            &resting(&["12", "34"]),
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
        let texts = texts(&scene);
        assert_eq!(texts[0].content, "12");
        assert_eq!(texts[1].content, "34");
    }

    #[test]
    fn seconds_add_a_third_card() {
        let scene = build_scene(
            &resting(&["12", "34", "56"]),
            None,
            &Theme::classic(),
            (1920, 1080),
            0.9,
        );
        assert_eq!(cards(&scene).len(), 3);
        assert_eq!(texts(&scene)[2].content, "56");
    }

    #[test]
    fn portrait_screens_are_limited_by_width() {
        let scene = build_scene(
            &resting(&["12", "34", "56"]),
            None,
            &Theme::classic(),
            (600, 1600),
            1.0,
        );
        let cards = cards(&scene);
        let right = cards[2].x + cards[2].width;
        assert!(cards[0].x >= 0.0 && right <= 600.0);
    }

    #[test]
    fn date_line_and_marker_stay_on_screen() {
        let date = DateLine("Tuesday, October 20".into());
        let hours = CardFlip::at_rest(CardContent {
            digits: "09".into(),
            marker: Some("PM"),
        });
        let flips = [hours, CardFlip::at_rest(content("41"))];
        let scene = build_scene(&flips, Some(&date), &Theme::classic(), (1280, 720), 1.0);
        let card = cards(&scene)[0].clone();
        let texts = texts(&scene);
        let marker = texts.iter().find(|t| t.content == "PM").unwrap();
        let date = texts
            .iter()
            .find(|t| t.content.starts_with("Tuesday"))
            .unwrap();
        assert!(date.y + date.size / 2.0 <= 720.0);
        assert!(date.y > card.y + card.height);
        assert!(marker.x > card.x);
    }

    #[test]
    fn tiny_windows_do_not_panic() {
        let scene = build_scene(
            &resting(&["12", "34", "56"]),
            None,
            &Theme::classic(),
            (1, 1),
            0.9,
        );
        assert!(cards(&scene).iter().all(|r| r.width >= 1.0));
    }

    #[test]
    fn resting_cards_are_not_folded_and_split_in_the_middle() {
        let scene = build_scene(
            &resting(&["12", "34"]),
            None,
            &Theme::classic(),
            (1920, 1080),
            0.9,
        );
        assert!(scene.groups.iter().all(|g| g.fold.is_none()));
        let splits: Vec<_> = scene
            .items()
            .filter_map(|item| match item {
                Item::Rect(r) if r.radius == 0.0 => Some(r),
                _ => None,
            })
            .collect();
        for (card, split) in cards(&scene).iter().zip(splits) {
            let middle = split.y + split.height / 2.0;
            assert!((middle - card.hinge_y_for_test()).abs() <= 1.0);
        }
    }

    fn flipping(progress: f32) -> Scene {
        let flip = CardFlip {
            from: content("34"),
            to: content("35"),
            progress,
        };
        build_scene(
            &[CardFlip::at_rest(content("12")), flip],
            None,
            &Theme::classic(),
            (1920, 1080),
            0.9,
        )
    }

    fn folds(scene: &Scene) -> Vec<(Fold, String)> {
        scene
            .groups
            .iter()
            .filter_map(|g| {
                let digits = g.items.iter().find_map(|i| match i {
                    Item::Text(t) => Some(t.content.clone()),
                    _ => None,
                })?;
                Some((g.fold?, digits))
            })
            .collect()
    }

    #[test]
    fn falling_flap_shows_the_old_top_over_new_top_and_old_bottom() {
        let scene = flipping(0.25);
        let folds = folds(&scene);
        assert_eq!(folds.len(), 3);
        let (top, top_digits) = &folds[0];
        let (bottom, bottom_digits) = &folds[1];
        let (flap, flap_digits) = &folds[2];
        assert_eq!(
            (top.half, top_digits.as_str(), top.angle),
            (Half::Top, "35", 0.0)
        );
        assert_eq!((bottom.half, bottom_digits.as_str()), (Half::Bottom, "34"));
        assert!(bottom.shade > 0.0, "flap casts a shadow on the lower half");
        assert_eq!((flap.half, flap_digits.as_str()), (Half::Top, "34"));
        assert!(flap.angle > 0.0 && flap.angle < FRAC_PI_2);
    }

    #[test]
    fn landing_flap_shows_the_new_bottom() {
        let scene = flipping(0.8);
        let (flap, digits) = folds(&scene).pop().unwrap();
        assert_eq!((flap.half, digits.as_str()), (Half::Bottom, "35"));
        assert!(flap.angle < 0.0 && flap.angle > -FRAC_PI_2);
    }

    #[test]
    fn the_flap_is_painted_last() {
        let scene = flipping(0.25);
        let last = scene.groups.last().unwrap();
        assert!(last.fold.is_some_and(|f| f.angle != 0.0));
    }

    impl Rect {
        fn hinge_y_for_test(&self) -> f32 {
            self.y + self.height / 2.0
        }
    }
}
