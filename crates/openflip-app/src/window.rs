//! Interactive mode: a fullscreen or windowed clock driven by winit.

use std::path::PathBuf;
use std::sync::Arc;

use openflip_host::{Clock, Key, Response};
use openflip_render::SurfaceRenderer;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::cli::Options;

pub fn run(
    config_path: Option<PathBuf>,
    options: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut clock = Clock::new(config_path);
    if options.settings {
        clock = clock.settings_only();
    }
    if let Some(time) = options.time {
        clock = clock.fixed_time(time);
    }
    let mut app = App {
        display: event_loop.owned_display_handle(),
        clock,
        // The settings window, opened by the OS screensaver settings, is
        // never fullscreen.
        fullscreen: !options.windowed && !options.settings,
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
    clock: Clock,
    fullscreen: bool,
    window: Option<Arc<Window>>,
    renderer: Option<SurfaceRenderer>,
    error: Option<String>,
}

impl App {
    /// The cursor is hidden in fullscreen, except while the settings are open.
    fn update_cursor(&self) {
        if let Some(window) = &self.window {
            window.set_cursor_visible(!self.fullscreen || self.clock.is_settings_open());
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        if let Some(window) = &self.window {
            window.set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
        }
        self.update_cursor();
    }

    fn respond(&mut self, event_loop: &ActiveEventLoop, response: Response) {
        match response {
            Response::Nothing => {}
            Response::Redraw => {
                self.update_cursor();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            Response::ChooseFont => {
                let picked = rfd::FileDialog::new()
                    .add_filter("Fonts", &["ttf", "otf", "ttc"])
                    .pick_file();
                if let Some(path) = picked {
                    let response = self.clock.set_font(path);
                    self.respond(event_loop, response);
                }
            }
            Response::Exit => event_loop.exit(),
        }
    }

    fn draw(&mut self) {
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let frame = self.clock.frame(renderer.size());
        if let Some(font) = frame.new_font {
            renderer.set_font(font);
        }
        renderer.render(&frame.scene);
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
            self.clock.look().font.clone(),
        ))
        .map_err(|e| e.to_string())?;

        self.window = Some(window);
        self.renderer = Some(renderer);
        self.update_cursor();
        Ok(())
    }
}

/// Maps a key press to the clock's keys. Arrow keys repeat while held;
/// everything else reacts to the first press only.
fn clock_key(key: WinitKey<&str>, repeat: bool) -> Option<Key> {
    let key = match key {
        WinitKey::Named(NamedKey::ArrowUp) => Key::Up,
        WinitKey::Named(NamedKey::ArrowDown) => Key::Down,
        WinitKey::Named(NamedKey::ArrowLeft) => Key::Left,
        WinitKey::Named(NamedKey::ArrowRight) => Key::Right,
        _ if repeat => return None,
        WinitKey::Named(NamedKey::Enter | NamedKey::Space) => Key::Activate,
        WinitKey::Named(NamedKey::Delete | NamedKey::Backspace) => Key::Reset,
        WinitKey::Named(NamedKey::Escape) => Key::Escape,
        WinitKey::Character(c) if c.eq_ignore_ascii_case("s") => Key::Settings,
        WinitKey::Character(c) if c.eq_ignore_ascii_case("q") => Key::Quit,
        _ => return None,
    };
    Some(key)
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
                self.respond(event_loop, Response::Redraw);
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        repeat,
                        ..
                    },
                ..
            } => {
                // Fullscreen is the window's business, not the clock's.
                let fullscreen_key = matches!(logical_key.as_ref(), WinitKey::Named(NamedKey::F11))
                    || matches!(logical_key.as_ref(), WinitKey::Character(c) if c.eq_ignore_ascii_case("f"));
                if fullscreen_key && !repeat && !self.clock.is_settings_open() {
                    self.set_fullscreen(!self.fullscreen);
                } else if let Some(key) = clock_key(logical_key.as_ref(), repeat) {
                    let response = self.clock.key(key);
                    self.respond(event_loop, response);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let response = self.clock.mouse_moved(position.x as f32, position.y as f32);
                self.respond(event_loop, response);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                // The position comes from the last CursorMoved event.
                let response = self.clock.mouse_pressed_at_cursor();
                self.respond(event_loop, response);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(match self.clock.next_frame_at() {
            Some(at) => ControlFlow::WaitUntil(at),
            None => ControlFlow::Wait,
        });
    }
}
