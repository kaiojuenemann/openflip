# Roadmap

Each milestone is tracked as a GitHub issue. Platforms are implemented in the
order macOS → Windows → Linux; CI builds all three throughout.

## v0.1

| # | Milestone | Scope |
|---|---|---|
| 0 | Scaffold | Workspace, core crate (clock face, date line, config, i18n), CI, community files ✅ |
| 1 | Renderer core | `winit` window + `wgpu` surface, card geometry, glyph rendering, built-in themes, scaling, custom fonts, fullscreen/windowed |
| 2 | 3D flip animation | Perspective fold of the top half, shading and shadow, correct timing on each tick, frame pacing |
| 3 | Settings overlay | Toggle with a key, edit all `Config` fields live, persist to `config.toml`, English/German UI |
| 4 | macOS | App bundle; `.saver` with Swift `ScreenSaverView` wrapper, preview + options sheet, install guide incl. Gatekeeper |
| 5 | Windows | `.scr` with `/s`, `/p <hwnd>`, `/c`; exit on input; install guide |
| 6 | Linux | App on X11 and Wayland; XScreenSaver hack; idle-daemon guide |
| 7 | Multi-monitor | All vs. primary display, consistent on all three platforms |
| 8 | Release v0.1 | Changelog, screenshots, tag; make repository public |

## Later

- Installer packages (`.dmg`, `.msi`, AppImage / `.deb` / Flatpak) built by CI
- macOS code signing and notarization
- Deeper KDE / GNOME screensaver integration
- More languages
