//! Renders real frames on a GPU. Ignored by default because CI machines
//! usually have no GPU; run with `cargo test -- --ignored` where one (or a
//! software Vulkan driver such as lavapipe) is available.

use openflip_core::{ClockFace, Config, DateLine, Language, TimeFormat};
use openflip_render::{
    layout, render_offscreen, CardContent, CardFlip, Color, Font, Gpu, Item, Painter, Rect, Scene,
    Theme, OFFSCREEN_FORMAT,
};

fn render(scene: &Scene) -> Vec<u8> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = pollster::block_on(Gpu::new(&instance, None)).expect("a GPU adapter");
    let mut painter = Painter::new(&gpu.device, OFFSCREEN_FORMAT, Font::bundled());
    render_offscreen(&gpu, &mut painter, scene).expect("rendering succeeds")
}

fn pixel(image: &[u8], width: u32, x: f32, y: f32) -> [u8; 4] {
    let i = ((y as u32 * width + x as u32) * 4) as usize;
    image[i..i + 4].try_into().unwrap()
}

fn rgb8(c: Color) -> [u8; 4] {
    let q = |v: f32| (v * 255.0).round() as u8;
    [q(c.r), q(c.g), q(c.b), 255]
}

fn at_rest(face: &ClockFace) -> Vec<CardFlip> {
    CardContent::from_face(face)
        .into_iter()
        .map(CardFlip::at_rest)
        .collect()
}

/// Card backgrounds and split lines, each in paint order.
fn rects(scene: &Scene) -> (Vec<&Rect>, Vec<&Rect>) {
    scene
        .items()
        .filter_map(|item| match item {
            Item::Rect(r) => Some(r),
            _ => None,
        })
        .partition(|r| r.radius > 0.0)
}

#[test]
#[ignore = "needs a GPU"]
fn draws_background_cards_digits_and_split_line() {
    let theme = Theme::classic();
    let config = Config {
        time_format: TimeFormat::H12,
        show_seconds: true,
        ..Config::default()
    };
    let time = chrono::NaiveDate::from_ymd_opt(2026, 10, 20)
        .unwrap()
        .and_hms_opt(20, 8, 8)
        .unwrap();
    let date = DateLine::at(time, Language::German);
    let cards = at_rest(&ClockFace::at(time, &config));
    let scene = layout::build_scene(&cards, Some(&date), &theme, (800, 450), 0.9);
    let image = render(&scene);
    assert_eq!(image.len(), 800 * 450 * 4);

    assert_eq!(pixel(&image, 800, 2.0, 2.0), rgb8(theme.background));

    let (cards, splits) = rects(&scene);
    let card = cards[0];
    // Near the card's top edge, away from the rounded corners and the digits.
    assert_eq!(
        pixel(&image, 800, card.x + card.width * 0.5, card.y + 4.0),
        rgb8(theme.card)
    );

    let split = splits[0];
    assert_eq!(
        pixel(
            &image,
            800,
            split.x + split.width * 0.5,
            split.y + split.height * 0.5
        ),
        rgb8(theme.background)
    );

    let digits = rgb8(theme.digits);
    let mut found = false;
    for y in card.y as u32..(card.y + card.height) as u32 {
        for x in card.x as u32..(card.x + card.width) as u32 {
            if pixel(&image, 800, x as f32, y as f32) == digits {
                found = true;
            }
        }
    }
    assert!(found, "digits are drawn");
}

#[test]
#[ignore = "needs a GPU"]
fn large_screens_grow_the_glyph_atlas() {
    let face = ClockFace {
        hours: ['0', '8'],
        minutes: ['4', '9'],
        seconds: Some(['2', '3']),
        pm: None,
    };
    let scene = layout::build_scene(&at_rest(&face), None, &Theme::classic(), (5120, 2880), 1.0);
    let image = render(&scene);
    let digits = rgb8(Theme::classic().digits);
    assert!(image.chunks(4).any(|p| p == digits));
}

/// A scene where the minutes card flips from 42 to 43, plus that card's
/// rectangle at rest.
fn mid_flip(progress: f32) -> (Scene, Rect) {
    let content = |digits: &str| CardContent {
        digits: digits.into(),
        marker: None,
    };
    let flip = CardFlip {
        from: content("42"),
        to: content("43"),
        progress,
    };
    let cards = [CardFlip::at_rest(content("10")), flip];
    let scene = layout::build_scene(&cards, None, &Theme::classic(), (800, 450), 0.9);
    let card = (*rects(&scene).0.last().unwrap()).clone();
    (scene, card)
}

#[test]
#[ignore = "needs a GPU"]
fn the_falling_flap_is_drawn_in_perspective() {
    let (scene, card) = mid_flip(0.4);
    let image = render(&scene);
    // Coming towards the viewer, the flap grows wider than the card: some
    // pixel just right of the card, above the hinge, is no longer background.
    let background = rgb8(Theme::classic().background);
    let x = card.x + card.width + 2.0;
    let hinge = card.y + card.height / 2.0;
    let widened = (2..(card.height / 2.0) as u32)
        .any(|dy| pixel(&image, 800, x, hinge - dy as f32) != background);
    assert!(widened, "flap extends beyond the card edge");
}

#[test]
#[ignore = "needs a GPU"]
fn the_falling_flap_casts_a_shadow_on_the_lower_half() {
    let (scene, card) = mid_flip(0.45);
    let image = render(&scene);
    let card_color = rgb8(Theme::classic().card);
    // Near the left edge, just below the split line, away from the digits.
    let shadowed = pixel(&image, 800, card.x + 6.0, card.y + card.height * 0.55);
    assert!(
        shadowed[0] < card_color[0],
        "{shadowed:?} darker than {card_color:?}"
    );
    // Far from the hinge the shadow fades out.
    let lit = pixel(&image, 800, card.x + 6.0, card.y + card.height - 8.0);
    assert!(
        lit[0].abs_diff(card_color[0]) <= 1,
        "{lit:?} close to {card_color:?}"
    );
    assert!(lit[0] > shadowed[0]);
}
