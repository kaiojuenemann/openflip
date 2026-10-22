//! Device setup and the two ways to get pixels out: a window surface or an image.

use std::fmt;
use std::sync::Arc;

use crate::font::Font;
use crate::painter::Painter;
use crate::scene::Scene;

#[derive(Debug)]
pub enum RenderError {
    NoAdapter(wgpu::RequestAdapterError),
    NoDevice(wgpu::RequestDeviceError),
    UnsupportedSurface,
    Readback(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter(e) => write!(f, "no suitable graphics adapter: {e}"),
            Self::NoDevice(e) => write!(f, "could not open graphics device: {e}"),
            Self::UnsupportedSurface => write!(
                f,
                "the window surface is not supported by the graphics adapter"
            ),
            Self::Readback(e) => write!(f, "could not read back the rendered image: {e}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// An open graphics device. Cloning is cheap and shares the device, so
/// several windows can draw with one.
#[derive(Clone)]
pub struct Gpu {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    /// Opens a device. Pass the surface it will present to, if any, so the
    /// adapter is guaranteed to support it.
    pub async fn new(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, RenderError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                // A clock does not need the discrete GPU; spare the battery.
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: surface,
                ..Default::default()
            })
            .await
            .map_err(RenderError::NoAdapter)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("openflip device"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(RenderError::NoDevice)?;
        // wgpu panics on unhandled errors by default. A clock, and above all
        // a screensaver, should keep running and report them instead.
        device.on_uncaptured_error(Arc::new(|error| {
            eprintln!("openflip: graphics error: {error}");
        }));
        Ok(Self {
            adapter,
            device,
            queue,
        })
    }
}

/// Renders into a window (or any other presentable surface).
pub struct SurfaceRenderer {
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// Whether the surface accepted `config`; drawing waits until it does.
    configured: bool,
    /// Whether the current run of configuration failures was reported.
    failure_reported: bool,
    painter: Painter,
}

impl SurfaceRenderer {
    pub async fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        (width, height): (u32, u32),
        font: Font,
    ) -> Result<Self, RenderError> {
        let gpu = Gpu::new(instance, Some(&surface)).await?;
        Self::with_gpu(gpu, surface, (width, height), font)
    }

    /// Like [`SurfaceRenderer::new`], but draws with a device that is
    /// already open, e.g. one shared with the window on another display.
    pub fn with_gpu(
        gpu: Gpu,
        surface: wgpu::Surface<'static>,
        (width, height): (u32, u32),
        font: Font,
    ) -> Result<Self, RenderError> {
        let caps = surface.get_capabilities(&gpu.adapter);
        // Prefer a non-sRGB format: colours are authored in sRGB and the
        // glyph coverage then blends the way text usually looks on screen.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or(RenderError::UnsupportedSurface)?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        let painter = Painter::new(&gpu.device, format, font);
        let mut renderer = Self {
            gpu,
            surface,
            config,
            configured: false,
            failure_reported: false,
            painter,
        };
        renderer.configure();
        Ok(renderer)
    }

    /// Applies `config` to the surface. This can fail, e.g. while a window
    /// is being minimised or on a desktop without a compositor; the next
    /// frame then tries again.
    fn configure(&mut self) -> bool {
        let scope = self
            .gpu
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        self.surface.configure(&self.gpu.device, &self.config);
        let error = pollster::block_on(scope.pop());
        match &error {
            // Report the first failure only; retries happen every frame.
            Some(error) if !self.failure_reported => {
                eprintln!("openflip: cannot draw into the window: {error}");
                self.failure_reported = true;
            }
            Some(_) => {}
            None => self.failure_reported = false,
        }
        self.configured = error.is_none();
        self.configured
    }

    pub fn set_font(&mut self, font: Font) {
        self.painter.set_font(font);
    }

    /// The device this renderer draws with.
    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// Name of the graphics adapter, for diagnostics.
    pub fn adapter_name(&self) -> String {
        self.gpu.adapter.get_info().name
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn resize(&mut self, (width, height): (u32, u32)) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.configure();
    }

    /// Draws one frame. Frames that cannot be presented right now (window
    /// hidden, surface being resized) are skipped silently.
    pub fn render(&mut self, scene: &Scene) {
        if !self.configured && !self.configure() {
            return;
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.configure();
                return;
            }
            _ => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.painter
            .paint(&self.gpu.device, &self.gpu.queue, &view, scene);
        self.gpu.queue.present(frame);
    }
}

/// Renders a scene into an RGBA8 image (`width * height * 4` bytes, rows top
/// to bottom). Used for screenshots and tests.
pub fn render_offscreen(
    gpu: &Gpu,
    painter: &mut Painter,
    scene: &Scene,
) -> Result<Vec<u8>, RenderError> {
    let (width, height) = (scene.width.max(1), scene.height.max(1));
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("openflip offscreen"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    painter.paint(&gpu.device, &gpu.queue, &view, scene);

    let row_bytes = width * 4;
    let padded_row =
        row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("openflip readback"),
        size: (padded_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(height),
            },
        },
        extent,
    );
    gpu.queue.submit([encoder.finish()]);

    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| RenderError::Readback(e.to_string()))?;
    receiver
        .recv()
        .map_err(|e| RenderError::Readback(e.to_string()))?
        .map_err(|e| RenderError::Readback(e.to_string()))?;

    let mapped = slice
        .get_mapped_range()
        .map_err(|e| RenderError::Readback(e.to_string()))?;
    let mut pixels = Vec::with_capacity((row_bytes * height) as usize);
    for row in mapped.chunks(padded_row as usize) {
        pixels.extend_from_slice(&row[..row_bytes as usize]);
    }
    drop(mapped);
    buffer.unmap();
    Ok(pixels)
}

/// Format of [`render_offscreen`] images; create the painter with it.
pub const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
