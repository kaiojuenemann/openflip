//! Draws a [`Scene`] into a texture.

use ab_glyph::{Font as _, PxScale, ScaleFont as _};

use crate::atlas::{Atlas, AtlasFull};
use crate::font::Font;
use crate::scene::{Color, Fold, HAlign, Half, Item, Rect, Scene, Text};

/// Floats per instance: rect (4), atlas rect (4), colour (4), params (2),
/// fold (8).
const INSTANCE_FLOATS: usize = 22;
const MODE_RECT: f32 = 0.0;
const MODE_GLYPH: f32 = 1.0;

pub struct Painter {
    font: Font,
    /// Whether the target stores linear light and colours must be converted.
    linear_target: bool,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    globals: wgpu::Buffer,
    instances: wgpu::Buffer,
    atlas: Atlas,
    /// Scene size the atlas was filled for; glyph sizes change with it.
    atlas_scene_size: (u32, u32),
}

impl Painter {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, font: Font) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("openflip bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("openflip pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![
            0 => Float32x4,
            1 => Float32x4,
            2 => Float32x4,
            3 => Float32x2,
            4 => Float32x4,
            5 => Float32x4,
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("openflip pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: (INSTANCE_FLOATS * 4) as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("openflip atlas sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("openflip globals"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let instances = Self::instance_buffer(device, 64);
        let atlas = Atlas::new(device);
        let bind_group = Self::bind_group(device, &bind_group_layout, &globals, &atlas, &sampler);

        Self {
            font,
            linear_target: format.is_srgb(),
            pipeline,
            bind_group_layout,
            bind_group,
            sampler,
            globals,
            instances,
            atlas,
            atlas_scene_size: (0, 0),
        }
    }

    /// Clears `target` to the scene background and draws the scene on top.
    pub fn paint(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        scene: &Scene,
    ) {
        if self.atlas_scene_size != (scene.width, scene.height) {
            self.atlas.clear();
            self.atlas_scene_size = (scene.width, scene.height);
        }
        let data = loop {
            match self.build_instances(queue, scene) {
                Ok(data) => break data,
                Err(AtlasFull) => {
                    if !self.atlas.grow(device) {
                        // The glyphs do not fit even the largest texture the
                        // device supports: draw the cards without text
                        // rather than nothing.
                        break self.build_rects_only(scene);
                    }
                    self.bind_group = Self::bind_group(
                        device,
                        &self.bind_group_layout,
                        &self.globals,
                        &self.atlas,
                        &self.sampler,
                    );
                }
            }
        };

        let globals = [
            scene.width as f32,
            scene.height as f32,
            self.atlas.size as f32,
            self.atlas.size as f32,
        ];
        queue.write_buffer(&self.globals, 0, &to_bytes(&globals));
        let bytes = to_bytes(&data);
        if self.instances.size() < bytes.len() as u64 {
            self.instances = Self::instance_buffer(device, data.len() / INSTANCE_FLOATS);
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.instances, 0, &bytes);
        }

        let background = self.color(scene.background);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("openflip frame"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("openflip pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: background.r as f64,
                            g: background.g as f64,
                            b: background.b as f64,
                            a: background.a as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            let count = (data.len() / INSTANCE_FLOATS) as u32;
            if count > 0 {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.instances.slice(..));
                pass.draw(0..6, 0..count);
            }
        }
        queue.submit([encoder.finish()]);
    }

    fn build_instances(
        &mut self,
        queue: &wgpu::Queue,
        scene: &Scene,
    ) -> Result<Vec<f32>, AtlasFull> {
        let mut data = Vec::new();
        for group in &scene.groups {
            let fold = fold_params(group.fold);
            for item in &group.items {
                match item {
                    Item::Rect(rect) => self.push_rect(rect, &fold, &mut data),
                    Item::Text(text) => self.push_text(queue, text, &fold, &mut data)?,
                }
            }
        }
        Ok(data)
    }

    fn build_rects_only(&self, scene: &Scene) -> Vec<f32> {
        let mut data = Vec::new();
        for group in &scene.groups {
            let fold = fold_params(group.fold);
            for item in &group.items {
                if let Item::Rect(rect) = item {
                    self.push_rect(rect, &fold, &mut data);
                }
            }
        }
        data
    }

    fn push_rect(&self, rect: &Rect, fold: &[f32; 8], data: &mut Vec<f32>) {
        let c = self.color(rect.color);
        data.extend_from_slice(&[
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            0.0,
            0.0,
            0.0,
            0.0,
            c.r,
            c.g,
            c.b,
            c.a,
            rect.radius,
            MODE_RECT,
        ]);
        data.extend_from_slice(fold);
    }

    fn push_text(
        &mut self,
        queue: &wgpu::Queue,
        text: &Text,
        fold: &[f32; 8],
        data: &mut Vec<f32>,
    ) -> Result<(), AtlasFull> {
        let font = self.font.0.as_scaled(PxScale::from(text.size));
        let glyphs: Vec<_> = text.content.chars().map(|c| font.glyph_id(c)).collect();

        // Centre vertically on the ink of the digit zero, so every text with
        // the same size shares a baseline regardless of its content.
        let zero = font.glyph_id('0').with_scale(text.size);
        let (ink_top, ink_bottom) = font
            .outline_glyph(zero)
            .map(|o| (o.px_bounds().min.y, o.px_bounds().max.y))
            .unwrap_or((-font.ascent(), 0.0));
        let baseline = (text.y - (ink_top + ink_bottom) / 2.0).round();

        // Pen positions for each glyph.
        let mut pens = Vec::with_capacity(glyphs.len());
        let width = match text.slot_width {
            Some(slot) => {
                for (i, id) in glyphs.iter().enumerate() {
                    pens.push(i as f32 * slot + (slot - font.h_advance(*id)) / 2.0);
                }
                slot * glyphs.len() as f32
            }
            None => {
                let mut pen = 0.0;
                for (i, id) in glyphs.iter().enumerate() {
                    if i > 0 {
                        pen += font.kern(glyphs[i - 1], *id);
                    }
                    pens.push(pen);
                    pen += font.h_advance(*id);
                }
                pen
            }
        };
        let start = match text.align {
            HAlign::Left => text.x,
            HAlign::Center => text.x - width / 2.0,
        };

        let color = self.color(text.color);
        for (id, pen) in glyphs.into_iter().zip(pens) {
            let Some(glyph) = self.atlas.glyph(queue, &self.font, id, text.size)? else {
                continue;
            };
            let x = (start + pen).round() + glyph.offset_x.round();
            let y = baseline + glyph.offset_y.round();
            data.extend_from_slice(&[
                x,
                y,
                glyph.width as f32,
                glyph.height as f32,
                glyph.x as f32,
                glyph.y as f32,
                glyph.width as f32,
                glyph.height as f32,
                color.r,
                color.g,
                color.b,
                color.a,
                0.0,
                MODE_GLYPH,
            ]);
            data.extend_from_slice(fold);
        }
        Ok(())
    }

    fn color(&self, color: Color) -> Color {
        if self.linear_target {
            color.to_linear()
        } else {
            color
        }
    }

    fn instance_buffer(device: &wgpu::Device, instances: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("openflip instances"),
            size: (instances.max(64).next_power_of_two() * INSTANCE_FLOATS * 4) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        globals: &wgpu::Buffer,
        atlas: &Atlas,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("openflip bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

/// Instance data for a fold: hinge y, centre x, half (-1 top, 1 bottom,
/// 0 none), angle, eye distance (0 = not rotated), shade, shade falloff,
/// padding.
fn fold_params(fold: Option<Fold>) -> [f32; 8] {
    match fold {
        None => [0.0; 8],
        Some(f) => [
            f.hinge_y,
            f.center_x,
            match f.half {
                Half::Top => -1.0,
                Half::Bottom => 1.0,
            },
            f.angle,
            f.eye_distance,
            f.shade,
            f.shade_falloff,
            0.0,
        ],
    }
}

fn to_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
