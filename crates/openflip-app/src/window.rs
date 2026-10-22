//! The clock in a window: as an app, as a screensaver (one window per
//! display), or embedded in a window that belongs to someone else (the
//! preview in the Windows screensaver settings, or XScreenSaver's window on
//! Linux).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use openflip_host::{Clock, Key, Response};
use openflip_render::{Gpu, SurfaceRenderer};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowAttributes, WindowId, WindowLevel};

use crate::cli::{Embed, Options};
use crate::screensaver::{ExitOnInput, Input};

enum Mode {
    /// The interactive clock: settings, fullscreen toggle and so on.
    App,
    /// Fullscreen on every display, on top of everything; any input ends it.
    Screensaver,
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
        None if options.screensaver => Mode::Screensaver,
        None => Mode::App,
    };
    let mut app = App {
        display: event_loop.owned_display_handle(),
        clock,
        // The settings window, opened by the OS screensaver settings, is
        // never fullscreen.
        fullscreen: match mode {
            Mode::App => !options.windowed && !options.settings,
            Mode::Screensaver => true,
            Mode::Embedded(_) => false,
        },
        mode,
        views: Vec::new(),
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
    /// One clock for all windows, so they flip in step.
    clock: Clock,
    mode: Mode,
    fullscreen: bool,
    /// The app and embedded modes have one window; the screensaver has one
    /// per display.
    views: Vec<View>,
    error: Option<String>,
}

/// A window and what draws into it.
struct View {
    window: Arc<Window>,
    renderer: SurfaceRenderer,
    /// Whether this is the primary display, which shows the clock even if
    /// the settings ask to blank the others.
    primary: bool,
    /// Screensaver only. Mouse positions are relative to each window, so
    /// every window measures movement on its own.
    exit: Option<ExitOnInput>,
}

impl App {
    /// The cursor is hidden in fullscreen, except while the settings are open.
    fn update_cursor(&self) {
        let visible = match self.mode {
            Mode::App => !self.fullscreen || self.clock.is_settings_open(),
            Mode::Screensaver => false,
            Mode::Embedded(_) => true,
        };
        for view in &self.views {
            view.window.set_cursor_visible(visible);
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        for view in &self.views {
            view.window
                .set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
        }
        self.update_cursor();
    }

    fn request_redraw(&self) {
        for view in &self.views {
            view.window.request_redraw();
        }
    }

    fn respond(&mut self, event_loop: &ActiveEventLoop, response: Response) {
        match response {
            Response::Nothing => {}
            Response::Redraw => {
                self.update_cursor();
                self.request_redraw();
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

    fn draw(&mut self, id: WindowId) {
        let Some(index) = self.views.iter().position(|v| v.window.id() == id) else {
            return;
        };
        let view = &self.views[index];
        let frame = self.clock.frame_on(view.renderer.size(), view.primary);
        // The font changed for all windows, but only this frame says so.
        if let Some(font) = frame.new_font {
            for view in &mut self.views {
                view.renderer.set_font(font.clone());
            }
        }
        if !self.views[index].renderer.render(&frame.scene) {
            self.clock.frame_not_shown();
        }
    }

    fn create_windows(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        // (window attributes, primary display)
        let windows: Vec<(WindowAttributes, bool)> = match self.mode {
            Mode::Embedded(embed) => vec![embed_attributes(embed)?],
            Mode::Screensaver => screensaver_windows(event_loop),
            Mode::App => vec![(
                Window::default_attributes()
                    .with_title("openflip")
                    .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
                    .with_fullscreen(self.fullscreen.then_some(Fullscreen::Borderless(None))),
                true,
            )],
        };

        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(self.display.clone()),
            ));
        let mut gpu: Option<Gpu> = None;
        for (attributes, primary) in windows {
            let window = Arc::new(
                event_loop
                    .create_window(attributes)
                    .map_err(|e| e.to_string())?,
            );
            let surface = instance
                .create_surface(window.clone())
                .map_err(|e| e.to_string())?;
            let size = window.inner_size();
            let size = (size.width, size.height);
            let font = self.clock.look().font.clone();
            let renderer = match &gpu {
                Some(gpu) => SurfaceRenderer::with_gpu(gpu.clone(), surface, size, font),
                None => pollster::block_on(SurfaceRenderer::new(&instance, surface, size, font)),
            }
            .map_err(|e| e.to_string())?;
            gpu.get_or_insert_with(|| renderer.gpu().clone());

            // Raise the screensaver above everything only now: on Windows,
            // Direct3D refused to create a swap chain for a fullscreen
            // window that was topmost from the start.
            let exit = match self.mode {
                Mode::Screensaver => {
                    window.set_window_level(WindowLevel::AlwaysOnTop);
                    Some(ExitOnInput::new(Instant::now()))
                }
                _ => None,
            };
            self.views.push(View {
                window,
                renderer,
                primary,
                exit,
            });
        }
        self.update_cursor();
        Ok(())
    }
}

/// A fullscreen window on every display. Wayland does not say which display
/// is the primary one; the first then takes its place.
fn screensaver_windows(event_loop: &ActiveEventLoop) -> Vec<(WindowAttributes, bool)> {
    let monitors: Vec<MonitorHandle> = event_loop.available_monitors().collect();
    let primary = event_loop
        .primary_monitor()
        .or_else(|| monitors.first().cloned());
    log::info!("screensaver on {} display(s)", monitors.len().max(1));
    let window = |monitor: Option<MonitorHandle>| {
        Window::default_attributes()
            .with_title("openflip")
            .with_fullscreen(Some(Fullscreen::Borderless(monitor)))
    };
    if monitors.is_empty() {
        return vec![(window(None), true)];
    }
    monitors
        .into_iter()
        .map(|monitor| {
            let is_primary = Some(&monitor) == primary.as_ref();
            (window(Some(monitor)), is_primary)
        })
        .collect()
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

/// A child of the window Windows handed us for the screensaver preview,
/// which counts as the primary display.
#[cfg(windows)]
fn embed_attributes(embed: Embed) -> Result<(WindowAttributes, bool), String> {
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
    let attributes = unsafe {
        child_attributes(size)
            .with_parent_window(Some(RawWindowHandle::Win32(Win32WindowHandle::new(handle))))
    };
    Ok((attributes, true))
}

/// A child of the X window XScreenSaver gave us, or of the root window, and
/// whether that window is on the primary display. XScreenSaver runs one
/// copy per display, each in its own window.
#[cfg(all(unix, not(target_os = "macos")))]
fn embed_attributes(embed: Embed) -> Result<(WindowAttributes, bool), String> {
    use raw_window_handle::{RawWindowHandle, XlibWindowHandle};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt;

    let (connection, screen) =
        x11rb::connect(None).map_err(|e| format!("cannot connect to the X server: {e}"))?;
    let root = connection.setup().roots[screen].root;
    let parent: u32 = match embed {
        Embed::Window(id) => {
            u32::try_from(id).map_err(|_| format!("invalid X window id {id:#x}"))?
        }
        Embed::XScreenSaverRoot => match std::env::var("XSCREENSAVER_WINDOW") {
            Ok(id) => {
                let id = crate::cli::parse_window_id(&id)?;
                u32::try_from(id).map_err(|_| format!("invalid X window id {id:#x}"))?
            }
            Err(_) => root,
        },
    };
    let geometry = connection
        .get_geometry(parent)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|_| format!("no X window {parent:#x}"))?;
    let size = (u32::from(geometry.width), u32::from(geometry.height));
    // The root window spans all displays; it gets the clock regardless.
    let primary = parent == root || crate::x11::on_primary_output(&connection, root, parent, size);
    log::info!(
        "drawing into X window {parent:#x} ({}x{}, {})",
        size.0,
        size.1,
        if primary { "primary" } else { "secondary" }
    );
    // SAFETY: our window is a child of `parent`, so the X server destroys it
    // together with the parent; it never outlives it.
    let attributes = unsafe {
        child_attributes(size).with_parent_window(Some(RawWindowHandle::Xlib(
            XlibWindowHandle::new(std::ffi::c_ulong::from(parent)),
        )))
    };
    Ok((attributes, primary))
}

#[cfg(target_os = "macos")]
fn embed_attributes(_embed: Embed) -> Result<(WindowAttributes, bool), String> {
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
        if !self.views.is_empty() {
            return;
        }
        if let Err(e) = self.create_windows(event_loop) {
            self.error = Some(e);
            event_loop.exit();
        }
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause {
            self.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(view) = self.views.iter_mut().find(|v| v.window.id() == id) else {
            return;
        };
        match (&self.mode, &mut view.exit) {
            // Any key, click, scroll or real mouse movement ends the screensaver.
            (Mode::Screensaver, Some(exit)) => {
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
            (Mode::Embedded(_), _) => match event {
                WindowEvent::KeyboardInput { .. }
                | WindowEvent::MouseInput { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::CursorMoved { .. } => return,
                _ => {}
            },
            _ => {}
        }
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => event_loop.exit(),
            WindowEvent::Resized(size) => {
                view.renderer.resize((size.width, size.height));
                self.clock.invalidate();
                self.respond(event_loop, Response::Redraw);
            }
            WindowEvent::RedrawRequested => self.draw(id),
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
