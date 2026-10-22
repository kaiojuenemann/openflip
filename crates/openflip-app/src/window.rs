//! The clock in a window: as an app, as a screensaver, or embedded in a
//! window that belongs to someone else (the preview in the Windows
//! screensaver settings, or XScreenSaver's window on Linux).

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

use crate::cli::{Embed, Options};
use crate::screensaver::{ExitOnInput, Input};

enum Mode {
    /// The interactive clock: settings, fullscreen toggle and so on.
    App,
    /// Fullscreen on top of everything; any input ends it.
    Screensaver(ExitOnInput),
    /// Drawn into a foreign parent window. Ignores input (the owner
    /// handles it) and ends with its parent.
    Embedded(Embed),
}

pub fn run(
    config_path: Option<PathBuf>,
    options: &Options,
) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = event_loop(options.embed.is_some())?;
    let mut clock = Clock::new(config_path);
    if options.settings {
        clock = clock.settings_only();
    }
    if let Some(time) = options.time {
        clock = clock.fixed_time(time);
    }
    let mode = match options.embed {
        Some(embed) => Mode::Embedded(embed),
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
            Mode::Embedded(_) => false,
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
                Mode::Embedded(_) => true,
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
            Mode::Embedded(embed) => embed_attributes(embed)?,
            _ => Window::default_attributes()
                .with_title("openflip")
                .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
                .with_fullscreen(self.fullscreen.then_some(Fullscreen::Borderless(None))),
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

        // Raise the screensaver above everything only now: on Windows,
        // Direct3D refused to create a swap chain for a fullscreen window
        // that was topmost from the start.
        if let Mode::Screensaver(_) = self.mode {
            window.set_window_level(WindowLevel::AlwaysOnTop);
        }

        self.window = Some(window);
        self.renderer = Some(renderer);
        self.update_cursor();
        Ok(())
    }
}

/// On Linux, embedding means X11 (XScreenSaver), so don't let winit pick
/// Wayland just because it is available.
#[cfg(all(unix, not(target_os = "macos")))]
fn event_loop(embed: bool) -> Result<EventLoop<()>, winit::error::EventLoopError> {
    use winit::platform::x11::EventLoopBuilderExtX11;
    let mut builder = EventLoop::builder();
    if embed {
        builder.with_x11();
    }
    builder.build()
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn event_loop(_embed: bool) -> Result<EventLoop<()>, winit::error::EventLoopError> {
    EventLoop::new()
}

/// A borderless child window filling `size` pixels of its parent.
#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
fn child_attributes(size: (u32, u32)) -> WindowAttributes {
    Window::default_attributes()
        .with_title("openflip")
        .with_decorations(false)
        .with_position(winit::dpi::PhysicalPosition::new(0, 0))
        .with_inner_size(winit::dpi::PhysicalSize::new(size.0.max(1), size.1.max(1)))
}

/// A child of the window Windows handed us for the screensaver preview.
#[cfg(windows)]
fn embed_attributes(embed: Embed) -> Result<WindowAttributes, String> {
    use raw_window_handle::{RawWindowHandle, Win32WindowHandle};
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;

    let Embed::Window(parent) = embed else {
        return Err("-root is for XScreenSaver on Linux; use /s on Windows".into());
    };
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
    let size = (
        (rect.right - rect.left).max(1) as u32,
        (rect.bottom - rect.top).max(1) as u32,
    );
    // SAFETY: Windows handed us this window for the preview. Our window is
    // its child, so it is destroyed with the parent and never outlives it.
    Ok(unsafe {
        child_attributes(size)
            .with_parent_window(Some(RawWindowHandle::Win32(Win32WindowHandle::new(handle))))
    })
}

/// A child of the X window XScreenSaver gave us, or of the root window.
#[cfg(all(unix, not(target_os = "macos")))]
fn embed_attributes(embed: Embed) -> Result<WindowAttributes, String> {
    use raw_window_handle::{RawWindowHandle, XlibWindowHandle};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt;

    let (connection, screen) =
        x11rb::connect(None).map_err(|e| format!("cannot connect to the X server: {e}"))?;
    let parent: u32 = match embed {
        Embed::Window(id) => {
            u32::try_from(id).map_err(|_| format!("invalid X window id {id:#x}"))?
        }
        Embed::XScreenSaverRoot => match std::env::var("XSCREENSAVER_WINDOW") {
            Ok(id) => {
                let id = crate::cli::parse_window_id(&id)?;
                u32::try_from(id).map_err(|_| format!("invalid X window id {id:#x}"))?
            }
            Err(_) => connection.setup().roots[screen].root,
        },
    };
    let geometry = connection
        .get_geometry(parent)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|_| format!("no X window {parent:#x}"))?;
    let size = (u32::from(geometry.width), u32::from(geometry.height));
    log::info!("drawing into X window {parent:#x} ({}x{})", size.0, size.1);
    // SAFETY: our window is a child of `parent`, so the X server destroys it
    // together with the parent; it never outlives it.
    Ok(unsafe {
        child_attributes(size).with_parent_window(Some(RawWindowHandle::Xlib(
            XlibWindowHandle::new(std::ffi::c_ulong::from(parent)),
        )))
    })
}

#[cfg(target_os = "macos")]
fn embed_attributes(_embed: Embed) -> Result<WindowAttributes, String> {
    Err("embedding (/p, -root, -window-id) is not available on macOS; use OpenFlip.saver".into())
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
            Mode::Embedded(_) => match event {
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
