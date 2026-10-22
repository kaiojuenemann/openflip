//! Single-channel texture holding rasterized glyphs, packed into shelves.

use std::collections::HashMap;

use ab_glyph::{Font as _, GlyphId, PxScale, ScaleFont as _};

use crate::font::Font;

/// Empty border around each glyph so linear filtering never bleeds.
const PADDING: u32 = 1;
const INITIAL_SIZE: u32 = 1024;

/// Where a glyph sits in the atlas and how to place it relative to the pen.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GlyphEntry {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Offset of the bitmap's top-left corner from the pen position on the baseline.
    pub offset_x: f32,
    pub offset_y: f32,
}

struct Shelf {
    y: u32,
    height: u32,
    cursor: u32,
}

pub(crate) struct Atlas {
    pub size: u32,
    max_size: u32,
    shelves: Vec<Shelf>,
    /// Keyed by glyph and pixel size in 1/4 px steps. `None` for glyphs
    /// without ink, such as spaces.
    entries: HashMap<(GlyphId, u32), Option<GlyphEntry>>,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

/// Returned when a glyph does not fit; the caller grows the atlas and retries.
#[derive(Debug)]
pub(crate) struct AtlasFull;

impl Atlas {
    pub fn new(device: &wgpu::Device) -> Self {
        let max_size = device.limits().max_texture_dimension_2d;
        Self::with_size(device, INITIAL_SIZE.min(max_size), max_size)
    }

    fn with_size(device: &wgpu::Device, size: u32, max_size: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("openflip glyph atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            size,
            max_size,
            shelves: Vec::new(),
            entries: HashMap::new(),
            texture,
            view,
        }
    }

    /// Replaces the atlas with an empty one of twice the size. Returns
    /// `false` when the device cannot hold a bigger texture.
    pub fn grow(&mut self, device: &wgpu::Device) -> bool {
        if self.size >= self.max_size {
            return false;
        }
        *self = Self::with_size(device, (self.size * 2).min(self.max_size), self.max_size);
        true
    }

    /// Drops all glyphs, e.g. when the screen size and with it every font size changed.
    pub fn clear(&mut self) {
        self.shelves.clear();
        self.entries.clear();
    }

    pub fn glyph(
        &mut self,
        queue: &wgpu::Queue,
        font: &Font,
        id: GlyphId,
        size: f32,
    ) -> Result<Option<GlyphEntry>, AtlasFull> {
        let key = (id, (size * 4.0).round() as u32);
        if let Some(entry) = self.entries.get(&key) {
            return Ok(*entry);
        }
        let glyph = id.with_scale(PxScale::from(size));
        let Some(outline) = font
            .data
            .as_scaled(PxScale::from(size))
            .outline_glyph(glyph)
        else {
            self.entries.insert(key, None);
            return Ok(None);
        };
        let bounds = outline.px_bounds();
        let width = bounds.width() as u32;
        let height = bounds.height() as u32;
        if width == 0 || height == 0 {
            self.entries.insert(key, None);
            return Ok(None);
        }
        let (x, y) = self.allocate(width + 2 * PADDING, height + 2 * PADDING)?;

        let mut pixels = vec![0u8; (width * height) as usize];
        outline.draw(|px, py, coverage| {
            if px < width && py < height {
                pixels[(py * width + px) as usize] = (coverage.clamp(0.0, 1.0) * 255.0) as u8;
            }
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: x + PADDING,
                    y: y + PADDING,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let entry = GlyphEntry {
            x: x + PADDING,
            y: y + PADDING,
            width,
            height,
            offset_x: bounds.min.x,
            offset_y: bounds.min.y,
        };
        self.entries.insert(key, Some(entry));
        Ok(Some(entry))
    }

    fn allocate(&mut self, width: u32, height: u32) -> Result<(u32, u32), AtlasFull> {
        if width > self.size || height > self.size {
            return Err(AtlasFull);
        }
        if let Some(shelf) = self
            .shelves
            .iter_mut()
            .find(|s| s.height >= height && s.cursor + width <= self.size)
        {
            let x = shelf.cursor;
            shelf.cursor += width;
            return Ok((x, shelf.y));
        }
        let y = self.shelves.last().map_or(0, |s| s.y + s.height);
        if y + height > self.size {
            return Err(AtlasFull);
        }
        self.shelves.push(Shelf {
            y,
            height,
            cursor: width,
        });
        Ok((0, y))
    }
}
