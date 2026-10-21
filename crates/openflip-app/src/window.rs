//! Interactive mode: a fullscreen or windowed clock driven by winit.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{NaiveDateTime, Timelike};
use openflip_render::{Flipper, SurfaceRenderer};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::cli::Options;
use crate::Look;

/// Redraw slightly after the second turns over, never just before it.
const TICK_SLACK: Duration = Duration::from_millis(5);
/// Upper bound on the frame rate during a flip. Presenting normally waits
/// for vsync anyway; this keeps systems without vsync from spinning.
const MIN_FRAME_TIME: Duration = Duration::from_micros(1_000_000 / 120);

pub fn run(look: Look, options: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = App {
        display: event_loop.owned_display_handle(),
        look,
        fullscreen: !options.windowed,
        fixed_time: options.time,
        flipper: Flipper::default(),
        started: Instant::now(),
        next_frame: None,
        window: None,
        renderer: None,
        error: None,
    };
    event_loop.run_app(&mut app)?;
    match app.error {
        Some(e) => Err(e.into()),
        None => Ok(()),
    }
}

struct App {
    display: OwnedDisplayHandle,
    look: Look,
    fullscreen: bool,
    fixed_time: Option<NaiveDateTime>,
    flipper: Flipper,
    /// Origin of the animation clock.
    started: Instant,
    /// When the next animation frame is due, while a flap is moving.
    next_frame: Option<Instant>,
    window: Option<Arc<Window>>,
    renderer: Option<SurfaceRenderer>,
    error: Option<String>,
}

impl App {
    fn now(&self) -> NaiveDateTime {
        self.fixed_time
            .unwrap_or_else(|| chrono::Local::now().naive_local())
    }

    /// Time until the clock face next changes.
    fn until_next_tick(&self) -> Duration {
        let now = chrono::Local::now().naive_local();
        let millis = u64::from(now.nanosecond() / 1_000_000).min(999);
        let to_next_second = Duration::from_millis(1000 - millis);
        if self.look.config.show_seconds {
            to_next_second
        } else {
            to_next_second + Duration::from_secs(59 - u64::from(now.second().min(59)))
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        if let Some(window) = &self.window {
            window.set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
            window.set_cursor_visible(!fullscreen);
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let attributes = Window::default_attributes()
            .with_title("openflip")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .with_fullscreen(self.fullscreen.then_some(Fullscreen::Borderless(None)));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|e| e.to_string())?,
        );
        window.set_cursor_visible(!self.fullscreen);

        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(self.display.clone()),
            ));
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;
        let size = window.inner_size();
        let renderer = pollster::block_on(SurfaceRenderer::new(
            &instance,
            surface,
            (size.width, size.height),
            self.look.font.clone(),
        ))
        .map_err(|e| e.to_string())?;

        self.window = Some(window);
        self.renderer = Some(renderer);
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        if let Err(e) = self.create_window(event_loop) {
            self.error = Some(e);
            event_loop.exit();
        }
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize((size.width, size.height));
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();
                let now = self.now();
                let t = self.started.elapsed().as_secs_f64();
                self.flipper.update(&self.look.cards_at(now), t);
                if let Some(renderer) = &mut self.renderer {
                    let scene = self
                        .look
                        .scene(&self.flipper.cards(t), now, renderer.size());
                    renderer.render(&scene);
                }
                // Keep drawing while a flap moves.
                self.next_frame = self
                    .flipper
                    .is_animating(t)
                    .then(|| frame_start + MIN_FRAME_TIME);
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => match logical_key.as_ref() {
                Key::Named(NamedKey::Escape) => event_loop.exit(),
                Key::Character(c) if c.eq_ignore_ascii_case("q") => event_loop.exit(),
                Key::Named(NamedKey::F11) => self.set_fullscreen(!self.fullscreen),
                Key::Character(c) if c.eq_ignore_ascii_case("f") => {
                    self.set_fullscreen(!self.fullscreen)
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let next_tick = self
            .fixed_time
            .is_none()
            .then(|| Instant::now() + self.until_next_tick() + TICK_SLACK);
        let wake = match (self.next_frame, next_tick) {
            (Some(frame), Some(tick)) => Some(frame.min(tick)),
            (frame, tick) => frame.or(tick),
        };
        event_loop.set_control_flow(match wake {
            Some(at) => ControlFlow::WaitUntil(at),
            None => ControlFlow::Wait,
        });
    }
}
