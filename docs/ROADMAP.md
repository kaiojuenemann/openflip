# Roadmap

Each milestone is tracked as a GitHub issue. Platforms are implemented in the
order macOS → Windows → Linux; CI builds all three throughout. Items without
an issue number are proposals; they get an issue once they are scheduled.

## v0.1 — released 2026-10-22

| # | Milestone | Scope |
|---|---|---|
| 0 | Scaffold | Workspace, core crate (clock face, date line, config, i18n), CI, community files ✅ |
| 1 | Renderer core | `winit` window + `wgpu` surface, card geometry, glyph rendering, built-in themes, scaling, custom fonts, fullscreen/windowed ✅ |
| 2 | 3D flip animation | Perspective fold of the top half, shading and shadow, correct timing on each tick, frame pacing ✅ |
| 3 | Settings overlay | Toggle with a key, edit all `Config` fields live, persist to `config.toml`, English/German UI ✅ |
| 4 | macOS | App bundle; `.saver` with Swift `ScreenSaverView` wrapper, preview + options sheet, install guide incl. Gatekeeper ✅ |
| 5 | Windows | `.scr` with `/s`, `/p <hwnd>`, `/c`; exit on input; install guide ✅ |
| 6 | Linux | App on X11 and Wayland; XScreenSaver hack; idle-daemon guide ✅ |
| 7 | Multi-monitor | All vs. primary display, consistent on all three platforms ✅ |
| – | Flip sound | Authentic flap sound, toggleable in the settings (#23) ✅ |
| 8 | Release v0.1 | Changelog, README animation, release automation ✅; make repository public (#8) |

Also in v0.1, beyond the original plan: digits aligned to the split line,
felt texture with a soft frontal light, release drafts from "Run workflow"
with checksums, weekly dependency audit, pinned CI actions, `SECURITY.md`.

## v0.1.x — before telling the world

| Item | Scope |
|---|---|
| Go public | Make the repository public; enable private vulnerability reporting and Dependabot alerts; protect `main` (PR and green CI required) (#8) |
| Real-hardware check | Two displays on macOS and Windows (all / primary only), flip sound in the macOS `.saver` sandbox and the Windows `.scr`, no white window after launch on macOS |
| Maintained font parser | Read fonts with `skrifa` instead of the unmaintained `ttf-parser` (RUSTSEC-2026-0192) (#33) |
| Fixes from that check | Whatever the check turns up; all of v0.1.x is released as 0.1.1 |

## v0.2 — easier to install, more choice

| Item | Scope |
|---|---|
| Installer packages | `.dmg` for macOS; winget manifest and an installer that registers the `.scr` on Windows; AppImage, `.deb` and Flatpak on Linux; all built and attached by the release job |
| ARM builds | Windows on ARM (`aarch64-pc-windows-msvc`) and Linux `aarch64` packages next to the x86-64 ones (macOS is already universal) |
| Code signing | macOS Developer ID signing and notarization, Windows Authenticode, so no Gatekeeper/SmartScreen workarounds are needed. Needs paid certificates: owner decision |
| More settings | Sound volume; reduced motion (cards change without the fold, also following the system setting); custom card and digit colours |
| More languages | Move the UI texts into one translation file per language so contributors can add languages without touching code |

## v0.3 — in the browser

| Item | Scope |
|---|---|
| Web version | WebAssembly build for desktop and mobile browsers (iPhone, iPad, Android): WebGPU with WebGL2 fallback, touch settings, wake lock, installable web app, hosted on GitHub Pages. Plan, steps W1–W8 and open decisions in #25 |

## Ideas, not scheduled

- Deeper KDE Plasma / GNOME integration (neither runs third-party screensavers today)
- Additional clock faces, e.g. a world clock row or a date-only card
- A "night" mode that dims the clock on a schedule
