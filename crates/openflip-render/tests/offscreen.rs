//! Renders real frames on a GPU. Ignored by default because CI machines
//! usually have no GPU; run with `cargo test -- --ignored` where one (or a
//! software Vulkan driver such as lavapipe) is available.

use openflip_core::{ClockFace, Config, DateLine, Language, TimeFormat};
use openflip_render::{
    layout, render_offscreen, Font, Gpu, Painter, Scene, Theme, OFFSCREEN_FORMAT,
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

fn rgb8(c: openflip_render::Color) -> [u8; 4] {
    let q = |v: f32| (v * 255.0).round() as u8;
    [q(c.r), q(c.g), q(c.b), 255]
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
    let scene = layout::build_scene(
        &ClockFace::at(time, &config),
        Some(&date),
        &theme,
        (800, 450),
        0.9,
    );
    let image = render(&scene);
    assert_eq!(image.len(), 800 * 450 * 4);

    assert_eq!(pixel(&image, 800, 2.0, 2.0), rgb8(theme.background));

    let card = &scene.rects[0];
    // Just inside the card's corner, away from the rounded edge and the digits.
    assert_eq!(
        pixel(&image, 800, card.x + card.width * 0.5, card.y + 4.0),
        rgb8(theme.card)
    );

    let split = &scene.overlays[0];
    assert_eq!(
        pixel(
            &image,
            800,
            split.x + split.width * 0.5,
            split.y + split.height * 0.5
        ),
        rgb8(theme.background)
    );

    // Somewhere on the card there must be digit-coloured ink.
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
    let scene = layout::build_scene(&face, None, &Theme::classic(), (5120, 2880), 1.0);
    let image = render(&scene);
    let digits = rgb8(Theme::classic().digits);
    assert!(image.chunks(4).any(|p| p == digits));
}
