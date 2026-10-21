//! The clock in a window: as an app, as a screensaver, or as the small
//! preview inside the Windows screensaver settings.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use openflip_host::{Clock, Key, Response};
use openflip_render::SurfaceRenderer;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{Fullscreen, Window, WindowAttributes, WindowId, WindowLevel};

use crate::cli::Options;
use crate::screensaver::{ExitOnInput, Input};

enum Mode {
    /// The interactive clock: settings, fullscreen toggle and so on.
    App,
    /// Fullscreen on top of everything; any input ends it.
    Screensaver(ExitOnInput),
    /// Drawn into a native parent window, e.g. the preview in the Windows
    /// screensaver settings. Ignores input and ends with its parent.
    Preview(u64),
}

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
    let mode = match options.preview {
        Some(parent) => Mode::Preview(parent),
        None if options.screensaver => Mode::Screensaver(ExitOnInput::new(Instant::now())),
        None => Mode::App,
    };
    let mut app = App {
        display: event_loop.owned_display_handle(),
        clock,
        // The settings window, opened by the OS screensaver settings, is
        // never fullscreen.
        fullscreen: match mode {
            Mode::App => !options.windowed && !options.settings,
            Mode::Screensaver(_) => true,
            Mode::Preview(_) => false,
        },
        mode,
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
    mode: Mode,
    fullscreen: bool,
    window: Option<Arc<Window>>,
    renderer: Option<SurfaceRenderer>,
    error: Option<String>,
}

impl App {
    /// The cursor is hidden in fullscreen, except while the settings are open.
    fn update_cursor(&self) {
        if let Some(window) = &self.window {
            let visible = match self.mode {
                Mode::App => !self.fullscreen || self.clock.is_settings_open(),
                Mode::Screensaver(_) => false,
                Mode::Preview(_) => true,
            };
            window.set_cursor_visible(visible);
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
        let attributes = match self.mode {
            Mode::Preview(parent) => preview_attributes(parent)?,
            _ => {
                let attributes = Window::default_attributes()
                    .with_title("openflip")
                    .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
                    .with_fullscreen(self.fullscreen.then_some(Fullscreen::Borderless(None)));
                if let Mode::Screensaver(_) = self.mode {
                    attributes.with_window_level(WindowLevel::AlwaysOnTop)
                } else {
                    attributes
                }
            }
        };
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

/// A borderless child window filling the parent's client area.
#[cfg(windows)]
fn preview_attributes(parent: u64) -> Result<WindowAttributes, String> {
    use raw_window_handle::{RawWindowHandle, Win32WindowHandle};
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;

    let handle = std::num::NonZeroIsize::new(parent as isize).ok_or("invalid window handle")?;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: GetClientRect only reads the window; an invalid handle fails.
    if unsafe { GetClientRect(parent as usize as HWND, &mut rect) } == 0 {
        return Err(format!("no window with handle {parent}"));
    }
    let size = winit::dpi::PhysicalSize::new(
        (rect.right - rect.left).max(1) as u32,
        (rect.bottom - rect.top).max(1) as u32,
    );
    let attributes = Window::default_attributes()
        .with_title("openflip preview")
        .with_decorations(false)
        .with_position(winit::dpi::PhysicalPosition::new(0, 0))
        .with_inner_size(size);
    // SAFETY: Windows handed us this window for the preview. Our window is
    // its child, so it is destroyed with the parent and never outlives it.
    Ok(unsafe {
        attributes.with_parent_window(Some(RawWindowHandle::Win32(Win32WindowHandle::new(handle))))
    })
}

#[cfg(not(windows))]
fn preview_attributes(_parent: u64) -> Result<WindowAttributes, String> {
    Err("the screensaver preview (/p) is only available on Windows".into())
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
        match &mut self.mode {
            Mode::App => {}
            // Any key, click, scroll or real mouse movement ends the screensaver.
            Mode::Screensaver(exit) => {
                let input = match &event {
                    WindowEvent::KeyboardInput { event, .. }
                        if event.state == ElementState::Pressed =>
                    {
                        Input::KeyPressed
                    }
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        ..
                    } => Input::ButtonPressed,
                    WindowEvent::MouseWheel { .. } => Input::Wheel,
                    WindowEvent::CursorMoved { position, .. } => {
                        Input::MouseAt(position.x, position.y)
                    }
                    WindowEvent::Resized(_) | WindowEvent::Moved(_) => Input::WindowChanged,
                    _ => Input::Other,
                };
                if exit.ends(input, Instant::now()) {
                    log::info!("screensaver ends on {event:?}");
                    return event_loop.exit();
                }
            }
            Mode::Preview(_) => match event {
                WindowEvent::KeyboardInput { .. }
                | WindowEvent::MouseInput { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::CursorMoved { .. } => return,
                _ => {}
            },
        }
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => event_loop.exit(),
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
