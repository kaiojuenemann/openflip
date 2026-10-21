# openflip on macOS

Two bundles, both universal (Apple silicon and Intel), macOS 12 or later:

- **OpenFlip.saver** — the screensaver
- **openflip.app** — the clock as an app (fullscreen; `S` for settings)

## Install

Download `openflip-macos.zip` from the latest CI run or release and unzip it.

openflip is not yet signed with an Apple Developer ID, so macOS warns that
it "cannot be checked for malicious software". To allow it once:

### Screensaver

1. Double-click **OpenFlip.saver** and choose to install it for this user.
2. If macOS refuses to open it, open **System Settings → Privacy & Security**,
   scroll down and click **Open Anyway** next to the message about
   OpenFlip.saver, then double-click it again.
3. Choose **openflip** in **System Settings → Screen Saver**. **Options…**
   opens the settings.

If System Settings still shows "can't be opened", remove the quarantine flag
in Terminal and reselect the screensaver:

```sh
xattr -dr com.apple.quarantine ~/Library/Screen\ Savers/OpenFlip.saver
```

### App

Move **openflip.app** to *Applications*, then right-click it, choose
**Open** and confirm. After the first start it opens normally.

## Settings

The app and the screensaver keep **separate settings**: macOS runs
screensavers in a sandbox that cannot read the app's files. Change the
screensaver's look with **Options…** in System Settings, the app's with
`S` while it runs.

| | Settings file |
|---|---|
| App | `~/Library/Application Support/org.openflip.openflip/config.toml` |
| Screensaver | `~/Library/Containers/com.apple.ScreenSaver.Engine.legacyScreenSaver/Data/Library/Application Support/org.openflip.openflip/config.toml` |

Fonts chosen in the screensaver's options are copied next to its settings,
so they keep working inside the sandbox.

## Build

Requires the Xcode command line tools and Rust.

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
platform/macos/build.sh                      # → target/macos/
OPENFLIP_LOG=1 swift platform/macos/smoke-test.swift target/macos/OpenFlip.saver
```

## How it fits together

The screensaver is a small Swift `ScreenSaverView` subclass
(`Sources/OpenFlipView.swift`) linked against the Rust static library
`openflip-ffi` (C interface in `include/openflip.h`). Rust creates a Metal
surface on the view and does all drawing, timing and settings; Swift only
forwards frames, sizes and, in the Options sheet, keyboard and mouse input.

Since macOS 14 the system keeps screensaver views alive after the
screensaver is dismissed; the view listens for
`com.apple.screensaver.willstop` and releases its GPU resources then.
