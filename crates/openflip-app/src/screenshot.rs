//! `--screenshot`: render a single frame to a PNG without opening a window.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use openflip_render::{render_offscreen, Gpu, Painter, OFFSCREEN_FORMAT};

use crate::cli::Options;
use crate::Look;

pub fn run(look: &Look, options: &Options, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let size = options.size.unwrap_or((1920, 1080));
    let time = options
        .time
        .unwrap_or_else(|| chrono::Local::now().naive_local());
    let scene = look.scene_at(time, size);

    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let gpu = pollster::block_on(Gpu::new(&instance, None))?;
    let mut painter = Painter::new(&gpu.device, OFFSCREEN_FORMAT, look.font.clone());
    let pixels = render_offscreen(&gpu, &mut painter, &scene)?;

    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), size.0, size.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    Ok(())
}
