# Platform integrations

Each subdirectory will hold the screensaver packaging for one operating system.
They are implemented in this order:

1. `macos/` — `.saver` bundle: a thin Swift `ScreenSaverView` subclass hosting
   the Rust renderer (built as a static library). Unsigned until we have a
   Developer ID; the README will document the Gatekeeper exception.
2. `windows/` — `.scr`: the `openflip` binary handling `/s` (run), `/p <hwnd>`
   (preview) and `/c` (settings → opens the app with the overlay).
3. `linux/` — XScreenSaver hack for X11 (`-root` / `-window-id`); on Wayland
   the app runs fullscreen, started by an idle daemon such as `swayidle`.
