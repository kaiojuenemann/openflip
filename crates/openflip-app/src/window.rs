//! Interactive mode: a fullscreen or windowed clock driven by winit.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{NaiveDateTime, Timelike};
use openflip_core::{Config, Text};
use openflip_render::overlay::OverlayLayout;
use openflip_render::{Flipper, Font, SurfaceRenderer};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

use crate::cli::Options;
use crate::menu::{MenuAction, MenuKey, SettingsMenu};
use crate::Look;

/// Redraw slightly after the second turns over, never just before it.
const TICK_SLACK: Duration = Duration::from_millis(5);
/// Upper bound on the frame rate during a flip. Presenting normally waits
/// for vsync anyway; this keeps systems without vsync from spinning.
const MIN_FRAME_TIME: Duration = Duration::from_micros(1_000_000 / 120);

pub fn run(look: Look, options: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let config_path = Config::default_path().ok();
    let mut app = App {
        display: event_loop.owned_display_handle(),
        look,
        config_path,
        // The settings window, opened by the OS screensaver settings, is
        // never fullscreen.
        fullscreen: !options.windowed && !options.settings,
        fixed_time: options.time,
        flipper: Flipper::default(),
        started: Instant::now(),
        next_frame: None,
        menu: options.settings.then(SettingsMenu::default),
        settings_only: options.settings,
        message: None,
        overlay: None,
        cursor: (0.0, 0.0),
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
    /// Where settings are saved; `None` if the system has no config directory.
    config_path: Option<PathBuf>,
    fullscreen: bool,
    fixed_time: Option<NaiveDateTime>,
    flipper: Flipper,
    /// Origin of the animation clock.
    started: Instant,
    /// When the next animation frame is due, while a flap is moving.
    next_frame: Option<Instant>,
    /// The settings overlay, while it is open.
    menu: Option<SettingsMenu>,
    /// Started with `--settings`: closing the overlay quits.
    settings_only: bool,
    /// Error shown in the overlay, e.g. when saving failed.
    message: Option<Text>,
    /// The overlay as last drawn, for mouse hit-testing.
    overlay: Option<OverlayLayout>,
    cursor: (f32, f32),
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

    fn redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// The cursor is hidden in fullscreen, except while the settings are open.
    fn update_cursor(&self) {
        if let Some(window) = &self.window {
            window.set_cursor_visible(!self.fullscreen || self.menu.is_some());
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        if let Some(window) = &self.window {
            window.set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
        }
        self.update_cursor();
    }

    fn open_menu(&mut self) {
        self.menu = Some(SettingsMenu::default());
        self.message = None;
        self.update_cursor();
        self.redraw();
    }

    fn close_menu(&mut self, event_loop: &ActiveEventLoop) {
        if self.settings_only {
            event_loop.exit();
            return;
        }
        self.menu = None;
        self.overlay = None;
        self.update_cursor();
        self.redraw();
    }

    fn handle_menu_action(&mut self, action: MenuAction, config: Config) {
        match action {
            MenuAction::Nothing => return,
            MenuAction::Redraw => {}
            MenuAction::ConfigChanged => self.apply_config(config),
            MenuAction::ChooseFont => self.choose_font(),
        }
        self.redraw();
    }

    /// Lets the user pick a font file with the system's file dialog.
    fn choose_font(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Fonts", &["ttf", "otf", "ttc"])
            .pick_file()
        else {
            return;
        };
        if Font::from_file(&path).is_err() {
            self.message = Some(Text::FontFailed);
            return;
        }
        let config = Config {
            font: Some(path),
            ..self.look.config.clone()
        };
        self.apply_config(config);
    }

    /// Uses a changed config right away and saves it.
    fn apply_config(&mut self, config: Config) {
        let font_changed = config.font != self.look.config.font;
        self.look = Look::from_config(config);
        if font_changed {
            if let Some(renderer) = &mut self.renderer {
                renderer.set_font(self.look.font.clone());
            }
        }
        let saved = match &self.config_path {
            Some(path) => self.look.config.save(path).map_err(|e| e.to_string()),
            None => Err("no config directory".into()),
        };
        self.message = match saved {
            Ok(()) => None,
            Err(e) => {
                eprintln!("openflip: could not save settings: {e}");
                Some(Text::SaveFailed)
            }
        };
    }

    /// Handles a key while the settings are open.
    fn menu_key(&mut self, event_loop: &ActiveEventLoop, key: Key<&str>, repeat: bool) {
        let menu_key = match key {
            Key::Named(NamedKey::ArrowUp) => MenuKey::Up,
            Key::Named(NamedKey::ArrowDown) => MenuKey::Down,
            Key::Named(NamedKey::ArrowLeft) => MenuKey::Left,
            Key::Named(NamedKey::ArrowRight) => MenuKey::Right,
            _ if repeat => return,
            Key::Named(NamedKey::Enter | NamedKey::Space) => MenuKey::Activate,
            Key::Named(NamedKey::Delete | NamedKey::Backspace) => MenuKey::Reset,
            Key::Named(NamedKey::Escape) => return self.close_menu(event_loop),
            Key::Character(c) if c.eq_ignore_ascii_case("s") => return self.close_menu(event_loop),
            Key::Character(c) if c.eq_ignore_ascii_case("q") => return event_loop.exit(),
            _ => return,
        };
        let mut config = self.look.config.clone();
        if let Some(menu) = &mut self.menu {
            let action = menu.key(menu_key, &mut config);
            self.handle_menu_action(action, config);
        }
    }

    /// Handles a key while only the clock is shown.
    fn clock_key(&mut self, event_loop: &ActiveEventLoop, key: Key<&str>) {
        match key {
            Key::Named(NamedKey::Escape) => event_loop.exit(),
            Key::Named(NamedKey::F11) => self.set_fullscreen(!self.fullscreen),
            Key::Character(c) if c.eq_ignore_ascii_case("q") => event_loop.exit(),
            Key::Character(c) if c.eq_ignore_ascii_case("f") => {
                self.set_fullscreen(!self.fullscreen)
            }
            Key::Character(c) if c.eq_ignore_ascii_case("s") => self.open_menu(),
            _ => {}
        }
    }

    /// The overlay row under the cursor and whether it hit the "back" arrow.
    fn row_under_cursor(&self) -> Option<(usize, bool)> {
        let (x, y) = self.cursor;
        let overlay = self.overlay.as_ref()?;
        overlay
            .rows
            .iter()
            .position(|row| row.contains(x, y))
            .map(|i| (i, overlay.rows[i].is_back(x)))
    }

    fn mouse_moved(&mut self) {
        let Some((row, _)) = self.row_under_cursor() else {
            return;
        };
        if let Some(menu) = &mut self.menu {
            let action = menu.hover(row);
            let config = self.look.config.clone();
            self.handle_menu_action(action, config);
        }
    }

    fn mouse_clicked(&mut self, event_loop: &ActiveEventLoop) {
        let Some(overlay) = &self.overlay else {
            return;
        };
        let (x, y) = self.cursor;
        if !overlay.panel.contains(x, y) {
            return self.close_menu(event_loop);
        }
        let Some((row, back)) = self.row_under_cursor() else {
            return;
        };
        let mut config = self.look.config.clone();
        if let Some(menu) = &mut self.menu {
            let action = menu.click(row, back, &mut config);
            self.handle_menu_action(action, config);
        }
    }

    fn draw(&mut self) {
        let frame_start = Instant::now();
        let now = self.now();
        let t = self.started.elapsed().as_secs_f64();
        self.flipper.update(&self.look.cards_at(now), t);
        if let Some(renderer) = &mut self.renderer {
            let size = renderer.size();
            let mut scene = self.look.scene(&self.flipper.cards(t), now, size);
            self.overlay = self.menu.as_ref().map(|menu| {
                let message = self.message.map(|m| self.look.language.text(m).to_string());
                self.look.overlay(menu, message, size)
            });
            if let Some(overlay) = &self.overlay {
                scene.groups.extend(overlay.groups.iter().cloned());
            }
            renderer.render(&scene);
        }
        // Keep drawing while a flap moves.
        self.next_frame = self
            .flipper
            .is_animating(t)
            .then(|| frame_start + MIN_FRAME_TIME);
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
            self.look.font.clone(),
        ))
        .map_err(|e| e.to_string())?;

        self.window = Some(window);
        self.renderer = Some(renderer);
        self.update_cursor();
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
            self.redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize((size.width, size.height));
                }
                self.redraw();
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
                if self.menu.is_some() {
                    self.menu_key(event_loop, logical_key.as_ref(), repeat);
                } else if !repeat {
                    self.clock_key(event_loop, logical_key.as_ref());
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
                self.mouse_moved();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.mouse_clicked(event_loop),
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
