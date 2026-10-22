#!/usr/bin/env bash
# Runs openflip in a virtual X server the ways Linux uses it: as an app, as
# a screensaver that ends on input, embedded into another program's window
# (-window-id) that it must outlive no longer than that window, with
# --root, and under a real XScreenSaver. Every run must be able to draw.
#
#   platform/linux/smoke-test.sh target/release/openflip
#
# Needs Xvfb, xdotool and (for the last check, as a non-root user)
# xscreensaver; a Vulkan or OpenGL driver such as mesa-vulkan-drivers.
set -uo pipefail

BIN="$(realpath "$1")"
LOGS="$(mktemp -d)"
export DISPLAY=:99 RUST_LOG=warn,openflip=info
unset WAYLAND_DISPLAY
failures=()

Xvfb :99 -screen 0 1280x720x24 >/dev/null 2>&1 &
XVFB=$!
cleanup() {
    pkill -P $$ 2>/dev/null
    kill "$XVFB" 2>/dev/null
}
trap cleanup EXIT
sleep 2

fail() {
    echo "FAILED: $1"
    failures+=("$1")
}

alive() { kill -0 "$1" 2>/dev/null; }

expect_running() {
    if alive "$1"; then
        echo "$2 is running"
    else
        fail "$2 exited early"
    fi
}

# Prints a run's output and fails if it could not draw.
finish() {
    local name="$1" what="$2"
    echo "--- $name output ---"
    grep -v '^\s*$' "$LOGS/$name.log" | grep -v 'DRI3' || true
    if grep -q 'cannot draw into the window' "$LOGS/$name.log"; then
        fail "$what could not draw into its window"
    fi
}

# 1. The app in a window.
"$BIN" --windowed >"$LOGS/app.log" 2>&1 &
APP=$!
sleep 4
expect_running $APP "app (windowed)"

# 2. Embedded into the app's window, ending when that window goes away.
WINDOW="$(xdotool search --pid $APP | tail -1)"
"$BIN" -window-id "$WINDOW" >"$LOGS/embedded.log" 2>&1 &
EMBEDDED=$!
sleep 4
expect_running $EMBEDDED "embedded (-window-id $WINDOW)"
kill $APP
sleep 3
if alive $EMBEDDED; then
    fail "embedded kept running after its window closed"
    kill $EMBEDDED
else
    echo "embedded ended with its window"
fi
finish app "app (windowed)"
finish embedded "embedded (-window-id)"

# 3. Screensaver: survives the start, ends on real mouse movement.
xdotool mousemove 640 360
"$BIN" --screensaver >"$LOGS/saver.log" 2>&1 &
SAVER=$!
sleep 4
expect_running $SAVER "screensaver"
xdotool mousemove 740 360
sleep 2
if alive $SAVER; then
    fail "screensaver kept running after the mouse moved"
    kill $SAVER
else
    echo "screensaver ended on mouse movement"
fi
finish saver "screensaver"

# 4. --root without XScreenSaver: the root window.
"$BIN" --root >"$LOGS/root.log" 2>&1 &
ROOT=$!
sleep 4
expect_running $ROOT "--root"
kill $ROOT 2>/dev/null
finish root "--root"

# 5. A real XScreenSaver, which refuses to run hacks as root.
if [ "$(id -u)" = 0 ]; then
    echo "skipping XScreenSaver: it does not run hacks as root"
elif ! command -v xscreensaver >/dev/null; then
    echo "skipping XScreenSaver: not installed"
else
    HOME_XSS="$(mktemp -d)"
    cat >"$HOME_XSS/.xscreensaver" <<CONFIG
mode:		one
selected:	0
lock:		False
fade:		False
unfade:		False
splash:		False
timeout:	1:00:00
programs:	\\
  "openflip"  $BIN --root  \\n\\

CONFIG
    HOME="$HOME_XSS" xscreensaver -no-splash >"$LOGS/xscreensaver.log" 2>&1 &
    sleep 3
    HOME="$HOME_XSS" xscreensaver-command -activate
    sleep 6
    if pgrep -x openflip >/dev/null; then
        echo "XScreenSaver runs openflip"
    else
        fail "XScreenSaver did not start openflip"
    fi
    HOME="$HOME_XSS" xscreensaver-command -deactivate
    sleep 3
    if pgrep -x openflip >/dev/null; then
        fail "openflip kept running after XScreenSaver deactivated"
    else
        echo "openflip ended with XScreenSaver"
    fi
    HOME="$HOME_XSS" xscreensaver-command -exit
    finish xscreensaver "XScreenSaver hack"
fi

if [ ${#failures[@]} -gt 0 ]; then
    printf 'smoke test failed: %s\n' "${failures[@]}" >&2
    exit 1
fi
echo OK
