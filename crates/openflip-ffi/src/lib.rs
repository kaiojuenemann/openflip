//! C interface to openflip, for hosts written in other languages.
//!
//! Today this serves the macOS screensaver: a thin Swift `ScreenSaverView`
//! hands its `NSView` to [`openflip_new_for_view`] and forwards frames and
//! input. The declarations for C and Swift live in
//! `platform/macos/include/openflip.h`; keep both in sync.
//!
//! All functions must be called from the thread that owns the view (the
//! main thread on macOS). Panics are caught at the boundary and turned into
//! a "nothing happened" result, so a bug cannot take the host process down.

use std::ffi::{c_char, c_void, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::Instant;

use openflip_core::Config;
use openflip_host::{Clock, FlapSound, Key, Response};
use openflip_render::SurfaceRenderer;

/// Results of the input functions, telling the host what to do next.
pub const OPENFLIP_NOTHING: u32 = 0;
pub const OPENFLIP_REDRAW: u32 = 1;
/// Show a font picker and pass the chosen file to [`openflip_set_font`].
pub const OPENFLIP_CHOOSE_FONT: u32 = 2;
/// Close the settings window.
pub const OPENFLIP_EXIT: u32 = 3;

/// Key codes for [`openflip_key`].
pub const OPENFLIP_KEY_UP: u32 = 0;
pub const OPENFLIP_KEY_DOWN: u32 = 1;
pub const OPENFLIP_KEY_LEFT: u32 = 2;
pub const OPENFLIP_KEY_RIGHT: u32 = 3;
pub const OPENFLIP_KEY_ACTIVATE: u32 = 4;
pub const OPENFLIP_KEY_RESET: u32 = 5;
pub const OPENFLIP_KEY_SETTINGS: u32 = 6;
pub const OPENFLIP_KEY_ESCAPE: u32 = 7;
pub const OPENFLIP_KEY_QUIT: u32 = 8;

/// One clock drawing into one view.
pub struct OpenFlip {
    clock: Clock,
    renderer: SurfaceRenderer,
    size: (u32, u32),
    /// Whether the view is on the primary display; see [`openflip_set_primary`].
    primary: bool,
    sound: FlapSound,
}

fn response_code(response: Response) -> u32 {
    match response {
        Response::Nothing => OPENFLIP_NOTHING,
        Response::Redraw => OPENFLIP_REDRAW,
        Response::ChooseFont => OPENFLIP_CHOOSE_FONT,
        Response::Exit => OPENFLIP_EXIT,
    }
}

/// Runs `f`, turning a panic into `fallback` instead of unwinding into C.
fn guard<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| {
        eprintln!("openflip: internal error (panic) at the C interface");
        fallback
    })
}

/// Creates a clock drawing into an `NSView`, sized `width` × `height`
/// pixels. With `settings_only`, the view shows the settings panel and
/// [`OPENFLIP_EXIT`] asks the host to close it. Returns null on failure
/// (e.g. no Metal device); the reason is printed to stderr.
///
/// # Safety
///
/// `view` must point to a live `NSView` that outlives the returned handle,
/// and this must be called on the main thread.
#[no_mangle]
pub unsafe extern "C" fn openflip_new_for_view(
    view: *mut c_void,
    width: u32,
    height: u32,
    settings_only: bool,
) -> *mut OpenFlip {
    guard(std::ptr::null_mut(), || {
        // SAFETY: forwarded from the caller.
        match unsafe { new_for_view(view, (width, height), settings_only) } {
            Ok(handle) => Box::into_raw(Box::new(handle)),
            Err(e) => {
                eprintln!("openflip: cannot draw into this view: {e}");
                std::ptr::null_mut()
            }
        }
    })
}

#[cfg(target_os = "macos")]
unsafe fn new_for_view(
    view: *mut c_void,
    size: (u32, u32),
    settings_only: bool,
) -> Result<OpenFlip, String> {
    use raw_window_handle::{
        AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
    };

    let view = std::ptr::NonNull::new(view).ok_or("no view")?;
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    // SAFETY: the caller guarantees the view outlives the handle, which owns
    // the surface.
    let surface = unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::AppKit(AppKitDisplayHandle::new())),
            raw_window_handle: RawWindowHandle::AppKit(AppKitWindowHandle::new(view)),
        })
    }
    .map_err(|e| e.to_string())?;

    let mut clock = Clock::new(Config::default_path().ok());
    if settings_only {
        clock = clock.settings_only();
    }
    let renderer = pollster::block_on(SurfaceRenderer::new(
        &instance,
        surface,
        size,
        clock.look().font.clone(),
    ))
    .map_err(|e| e.to_string())?;
    if std::env::var_os("OPENFLIP_LOG").is_some() {
        eprintln!(
            "openflip: drawing {}x{} with {}",
            size.0,
            size.1,
            renderer.adapter_name()
        );
    }
    Ok(OpenFlip {
        clock,
        renderer,
        size,
        primary: true,
        sound: FlapSound::new(),
    })
}

#[cfg(not(target_os = "macos"))]
unsafe fn new_for_view(
    _view: *mut c_void,
    _size: (u32, u32),
    _settings_only: bool,
) -> Result<OpenFlip, String> {
    Err("views are only supported on macOS".into())
}

/// Frees a handle from [`openflip_new_for_view`]. Null is ignored.
///
/// # Safety
///
/// `handle` must come from [`openflip_new_for_view`] and not be used again.
#[no_mangle]
pub unsafe extern "C" fn openflip_free(handle: *mut OpenFlip) {
    if !handle.is_null() {
        // SAFETY: the caller hands back ownership.
        guard((), || drop(unsafe { Box::from_raw(handle) }));
    }
}

/// Runs `f` on the handle, or returns `fallback` for null.
///
/// # Safety
///
/// `handle` must be null or a live handle from [`openflip_new_for_view`].
unsafe fn with<T>(handle: *mut OpenFlip, fallback: T, f: impl FnOnce(&mut OpenFlip) -> T) -> T {
    // SAFETY: forwarded from the caller.
    match unsafe { handle.as_mut() } {
        Some(handle) => guard(fallback, || f(handle)),
        None => fallback,
    }
}

/// Tells the clock the view's new size in pixels.
///
/// # Safety
///
/// `handle` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn openflip_resize(handle: *mut OpenFlip, width: u32, height: u32) {
    // SAFETY: forwarded from the caller.
    unsafe {
        with(handle, (), |h| {
            if h.size != (width, height) && width > 0 && height > 0 {
                h.size = (width, height);
                h.renderer.resize(h.size);
                h.clock.invalidate();
            }
        })
    }
}

/// Tells the clock whether the view is on the primary display. Views on
/// other displays show only the background when the settings ask for the
/// primary display only. New handles start as primary.
///
/// # Safety
///
/// `handle` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn openflip_set_primary(handle: *mut OpenFlip, primary: bool) {
    // SAFETY: forwarded from the caller.
    unsafe {
        with(handle, (), |h| {
            if h.primary != primary {
                h.primary = primary;
                h.clock.invalidate();
            }
        })
    }
}

/// Draws a frame if one is due. Call it as often as convenient (e.g. at
/// 60 Hz); it returns quickly when nothing changed. Returns whether a frame
/// reached the screen; one that did not is retried on a later call.
///
/// # Safety
///
/// `handle` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn openflip_draw(handle: *mut OpenFlip) -> bool {
    // SAFETY: forwarded from the caller.
    unsafe {
        with(handle, false, |h| {
            if !h.clock.needs_frame(Instant::now()) {
                return false;
            }
            let frame = h.clock.frame_on(h.size, h.primary);
            if let Some(font) = frame.new_font {
                h.renderer.set_font(font);
            }
            let shown = h.renderer.render(&frame.scene);
            if !shown {
                h.clock.frame_not_shown();
            }
            // macOS runs one screensaver view per display; only the one on
            // the primary display makes a sound.
            let audible = h.clock.look().config.sound && h.primary;
            h.sound.update(audible, frame.flap_sounds);
            shown
        })
    }
}

/// Handles a key, one of the `OPENFLIP_KEY_*` codes. Returns an
/// `OPENFLIP_*` result code.
///
/// # Safety
///
/// `handle` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn openflip_key(handle: *mut OpenFlip, key: u32) -> u32 {
    let key = match key {
        OPENFLIP_KEY_UP => Key::Up,
        OPENFLIP_KEY_DOWN => Key::Down,
        OPENFLIP_KEY_LEFT => Key::Left,
        OPENFLIP_KEY_RIGHT => Key::Right,
        OPENFLIP_KEY_ACTIVATE => Key::Activate,
        OPENFLIP_KEY_RESET => Key::Reset,
        OPENFLIP_KEY_SETTINGS => Key::Settings,
        OPENFLIP_KEY_ESCAPE => Key::Escape,
        OPENFLIP_KEY_QUIT => Key::Quit,
        _ => return OPENFLIP_NOTHING,
    };
    // SAFETY: forwarded from the caller.
    unsafe {
        with(handle, OPENFLIP_NOTHING, |h| {
            response_code(h.clock.key(key))
        })
    }
}

/// The mouse moved to (`x`, `y`) in pixels from the view's top-left corner.
///
/// # Safety
///
/// `handle` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn openflip_mouse_moved(handle: *mut OpenFlip, x: f32, y: f32) -> u32 {
    // SAFETY: forwarded from the caller.
    unsafe {
        with(handle, OPENFLIP_NOTHING, |h| {
            response_code(h.clock.mouse_moved(x, y))
        })
    }
}

/// The left mouse button went down at (`x`, `y`) in pixels from the
/// view's top-left corner.
///
/// # Safety
///
/// `handle` must be null or a live handle.
#[no_mangle]
pub unsafe extern "C" fn openflip_mouse_pressed(handle: *mut OpenFlip, x: f32, y: f32) -> u32 {
    // SAFETY: forwarded from the caller.
    unsafe {
        with(handle, OPENFLIP_NOTHING, |h| {
            response_code(h.clock.mouse_pressed(x, y))
        })
    }
}

/// Uses the font file at `path` (UTF-8). The file is copied next to the
/// config first: a sandboxed screensaver may lose access to the original.
///
/// # Safety
///
/// `handle` must be null or a live handle, and `path` a valid C string.
#[no_mangle]
pub unsafe extern "C" fn openflip_set_font(handle: *mut OpenFlip, path: *const c_char) -> u32 {
    if path.is_null() {
        return OPENFLIP_NOTHING;
    }
    // SAFETY: the caller guarantees a valid C string.
    let Ok(path) = unsafe { CStr::from_ptr(path) }.to_str() else {
        return OPENFLIP_NOTHING;
    };
    let path = keep_copy(Path::new(path));
    // SAFETY: forwarded from the caller.
    unsafe {
        with(handle, OPENFLIP_NOTHING, |h| {
            response_code(h.clock.set_font(path))
        })
    }
}

/// Copies a font into `fonts/` next to the config and returns the copy, or
/// the original path if copying fails.
fn keep_copy(original: &Path) -> PathBuf {
    let copy = Config::default_path().ok().and_then(|config| {
        let dir = config.parent()?.join("fonts");
        let target = dir.join(original.file_name()?);
        std::fs::create_dir_all(&dir).ok()?;
        std::fs::copy(original, &target).ok()?;
        Some(target)
    });
    copy.unwrap_or_else(|| original.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_handles_are_ignored() {
        // SAFETY: null is explicitly allowed.
        unsafe {
            assert!(!openflip_draw(std::ptr::null_mut()));
            assert_eq!(
                openflip_key(std::ptr::null_mut(), OPENFLIP_KEY_UP),
                OPENFLIP_NOTHING
            );
            assert_eq!(
                openflip_mouse_pressed(std::ptr::null_mut(), 1.0, 1.0),
                OPENFLIP_NOTHING
            );
            assert_eq!(
                openflip_set_font(std::ptr::null_mut(), std::ptr::null()),
                OPENFLIP_NOTHING
            );
            openflip_resize(std::ptr::null_mut(), 10, 10);
            openflip_free(std::ptr::null_mut());
        }
    }

    #[test]
    fn unknown_key_codes_do_nothing() {
        // SAFETY: null is explicitly allowed.
        assert_eq!(
            unsafe { openflip_key(std::ptr::null_mut(), 999) },
            OPENFLIP_NOTHING
        );
    }

    #[test]
    fn panics_do_not_cross_the_boundary() {
        assert_eq!(guard(7, || panic!("boom")), 7);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn views_are_rejected_outside_macos() {
        let mut dummy = 0u8;
        // SAFETY: the pointer is never dereferenced off macOS.
        let handle =
            unsafe { openflip_new_for_view(std::ptr::addr_of_mut!(dummy).cast(), 10, 10, false) };
        assert!(handle.is_null());
    }
}
