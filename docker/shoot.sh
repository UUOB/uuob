#!/usr/bin/env bash
# In the Linux image (docker/linux.Dockerfile): run UUOB's window headless and take screenshots.
#   docker/shoot.sh <data-dir> <out-prefix> [WxH] [dark|light] [click-x,click-y ...]
# Each click is followed by a screenshot <out-prefix>-N.png.
set -euo pipefail
DATA=$1; OUT=$2; SIZE=${3:-1280x820}; THEME=${4:-dark}; shift 4 || true
export DISPLAY=:99 NO_AT_BRIDGE=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 LIBGL_ALWAYS_SOFTWARE=1
# Libadwaita picks light or dark itself (GTK_THEME would replace its styling).
[ "$THEME" = dark ] && export ADW_DEBUG_COLOR_SCHEME=prefer-dark || export ADW_DEBUG_COLOR_SCHEME=prefer-light
Xvfb :99 -screen 0 ${SIZE}x24 >/dev/null 2>&1 &
sleep 1
W=${SIZE%x*}; H=${SIZE#*x}
UUOB_DATA_DIR=$DATA ./target/debug/uuob-window >/tmp/app.log 2>&1 &
APP=$!
for i in $(seq 1 60); do WID=$(xdotool search --name '^UUOB$' 2>/dev/null | head -1 || true); [ -n "$WID" ] && break; sleep 0.5; done
xdotool windowmove "$WID" 0 0 windowsize "$WID" "$W" "$H"
sleep 4
import -window root "$OUT-0.png"
n=1
for c in "$@"; do
  if [[ "$c" == key:* ]]; then xdotool type --delay 40 "${c#key:}"; 
  elif [[ "$c" == press:* ]]; then xdotool key "${c#press:}";
  else xdotool mousemove ${c%,*} ${c#*,} click 1; fi
  sleep 1.5
  import -window root "$OUT-$n.png"; n=$((n+1))
done
kill $APP; tail -5 /tmp/app.log
