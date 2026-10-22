# Platform integrations

Each subdirectory will hold the screensaver packaging for one operating system.
They are implemented in this order:

1. `macos/` — `OpenFlip.saver` and `openflip.app`: a thin Swift
   `ScreenSaverView` subclass hosting the Rust renderer through the
   `openflip-ffi` static library. Ad-hoc signed until we have a Developer
   ID; see [macos/README.md](macos/README.md) for installing.
2. `windows/` — `openflip.scr`: the `openflip` binary itself, which
   understands `/s` (run), `/p <hwnd>` (preview in the settings dialog) and
   `/c` (settings); see [windows/README.md](windows/README.md).
3. `linux/` — XScreenSaver hack for X11 (`-root` / `-window-id`); on Wayland
   the app runs fullscreen, started by an idle daemon such as `swayidle`.
