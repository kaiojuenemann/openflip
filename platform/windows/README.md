# openflip on Windows

`openflip-windows.zip` contains:

- **openflip.scr** — the screensaver
- **openflip.exe** — the clock as an app (fullscreen; `S` for settings)

Both are the same program; Windows treats files ending in `.scr` as
screensavers and starts them with `/s` (run), `/p` (preview) or `/c`
(settings).

## Install

openflip is not yet signed with a code-signing certificate, so Windows may
warn about it.

1. Unzip the download. If Windows marked the files as downloaded from the
   internet, right-click **openflip.scr** → **Properties** → tick
   **Unblock** → **OK** (otherwise *Install* may be missing or fail).
2. Right-click **openflip.scr** → **Install**. The screensaver settings open
   with openflip selected; **Settings…** opens openflip's settings, and the
   small preview shows the running clock.
3. If SmartScreen shows "Windows protected your PC", click **More info** →
   **Run anyway**.

To install it for every user, copy **openflip.scr** to `C:\Windows\System32`
and choose it in **Settings → Personalization → Lock screen → Screen saver**.

## Settings

App and screensaver share one settings file:
`%APPDATA%\openflip\openflip\config\config.toml`. Change it with `S` in the
app or with **Settings…** in the screensaver dialog.

For now the screensaver shows the clock on the main display; all-display
support follows with multi-monitor support (#7).

## Build

```bat
cargo build --release
copy target\release\openflip.exe target\release\openflip.scr
powershell -File platform\windows\smoke-test.ps1 target\release\openflip.scr
```
