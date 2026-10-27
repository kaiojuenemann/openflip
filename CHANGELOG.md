# Changelog

All notable changes to openflip are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Fonts are now read with skrifa (fontations) instead of ab_glyph, whose
  parser ttf-parser is no longer maintained (RUSTSEC-2026-0192). Digits look
  the same. Kerning between characters is no longer applied; the bundled font
  has none, but a custom font that relies on it may set the date line
  slightly wider.
- On Wayland, window decorations no longer draw a title, so ttf-parser is no
  longer pulled in through winit either.

## [0.1.0] - 2026-10-22

The first release: a flip clock app and screensaver for macOS, Windows and
Linux.

### Added

- Flip clock with a real 3D card fold, rendered on the GPU (wgpu: Metal,
  Direct3D 12, Vulkan or OpenGL), with shading, a cast shadow and a small
  rebound when the flap lands.
- 12-hour or 24-hour time, optional seconds, optional weekday and date.
- Themes (classic, light, amber, midnight), size, custom fonts; cards with a
  subtle felt texture lit softly from the front.
- Optional flap sound, cut from a CC0 recording of a split-flap display.
- Settings as an in-app overlay (keyboard and mouse), stored as
  `config.toml`; English and German interface, following the system language.
- Clock on every display, or on the primary display only.
- macOS: `openflip.app` and `OpenFlip.saver` (universal, unsigned) with
  preview and options sheet.
- Windows: `openflip.scr` with fullscreen, preview (`/p`) and settings (`/c`).
- Linux: app on X11 and Wayland, XScreenSaver hack, setup for swayidle and
  hypridle.
- CI on all three platforms with smoke tests of the packages and GPU render
  tests.

[Unreleased]: https://github.com/kaiojuenemann/openflip/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/kaiojuenemann/openflip/releases/tag/v0.1.0
