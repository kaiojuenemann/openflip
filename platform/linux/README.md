# openflip on Linux

`openflip-linux-x86_64.tar.gz` contains:

- **openflip** — the clock (app, screensaver and XScreenSaver hack in one)
- **openflip.xml** — the description shown in XScreenSaver's settings
- **INSTALL.md** — this guide

openflip needs a Vulkan or OpenGL driver and, on X11, `libxkbcommon-x11`;
every desktop installation has them.

## Install

```sh
tar xzf openflip-linux-x86_64.tar.gz
install -Dm755 openflip ~/.local/bin/openflip   # or /usr/local/bin for everyone
```

Make sure `~/.local/bin` is on your `PATH`. Run `openflip` for the
fullscreen clock (`S` for settings, `Esc` to quit) or `openflip --windowed`.

## As a screensaver

Linux has no single screensaver system; pick the one your desktop uses.

### XScreenSaver (X11)

1. Add openflip to the `programs:` list in `~/.xscreensaver`, e.g. right
   after the line `programs:` add

   ```
     "openflip"  openflip --root  \n\
   ```

2. Optionally copy `openflip.xml` to `/usr/share/xscreensaver/config/` so
   the settings dialog shows its description.
3. Restart XScreenSaver (`xscreensaver-command -restart`) and choose
   **openflip** in `xscreensaver-settings`.

XScreenSaver starts openflip with `--root` and ends it when you return; the
preview in `xscreensaver-settings` uses `-window-id`. Change the clock's
look with `openflip --settings`.

With several monitors, XScreenSaver runs one openflip per monitor. With
**Displays: Primary** in the settings, only the primary monitor
(`xrandr --output … --primary`, otherwise the first one) shows the clock.

### Sway, Hyprland and other wlroots compositors (Wayland)

Start openflip from your idle daemon. It runs fullscreen on every output and
ends on any key, click or mouse movement; the `resume` command is a safety
net. Wayland does not tell apps which output is the primary one, so with
**Displays: Primary** the clock shows on the first output the compositor
lists.

**swayidle** (e.g. in your Sway config):

```
exec swayidle -w \
    timeout 300 'openflip --screensaver' \
    resume 'pkill -x openflip'
```

**hypridle** (`~/.config/hypr/hypridle.conf`):

```
listener {
    timeout = 300
    on-timeout = openflip --screensaver
    on-resume = pkill -x openflip
}
```

### GNOME and KDE Plasma

Neither runs third-party screensavers. You can still start the clock by
hand (`openflip`), e.g. from a keyboard shortcut.

## Settings

App and screensaver share one settings file:
`$XDG_CONFIG_HOME/openflip/config.toml` (usually `~/.config/openflip/`).

## Test

`platform/linux/smoke-test.sh path/to/openflip` runs every mode in a
virtual X server (needs `Xvfb`, `xdotool` and, as a non-root user,
`xscreensaver`).
