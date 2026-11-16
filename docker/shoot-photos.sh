#!/usr/bin/env bash
# Inside the dev container: demo data with pictures, UUOB and Photos side by
# side, Photos connected, and screenshots of the grid and of one photo.
#   docker/dev.sh docker/shoot-photos.sh <out-prefix>
# Photos is built from its own repository. Build it there into this
# repository's cache, so that its program is at target/debug/uuob-photos here:
#   UUOB_TARGET=uuob-target uuob/docker/dev.sh cargo build
set -euo pipefail
OUT=$1
DATA=target/demo-photos; PICS=target/demo-pictures
rm -rf "$DATA" "$PICS"; mkdir -p "$PICS"
# A dozen made-up pictures: soft colour fields, each different.
i=0
for c in "#ff9f0a-#ff375f" "#0a84ff-#30d158" "#bf5af2-#0a84ff" "#ffd60a-#ff9f0a" "#40c8e0-#5e5ce6" "#30d158-#ffd60a" \
         "#ff375f-#bf5af2" "#64d2ff-#0a84ff" "#ff9f0a-#ffd60a" "#5e5ce6-#ff375f" "#30d158-#40c8e0" "#ff453a-#ff9f0a"; do
  i=$((i+1)); f=$(printf '%s/IMG_%04d.jpg' "$PICS" $((4000+i)))
  convert -size 1600x1200 gradient:"${c%-*}"-"${c#*-}" -swirl $((i*37)) \( -size 1600x1200 plasma:fractal -blur 0x12 -colorspace Gray \) \
    -compose SoftLight -composite -quality 88 "$f"
  convert "$f" -resize 320x240 -quality 60 "${f%.jpg}.thumb.jpg"
done
UUOB_SEED_PHOTOS=$PICS cargo run -q -p uuob-core --example seed -- "$DATA" >/dev/null

export DISPLAY=:99 NO_AT_BRIDGE=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 LIBGL_ALWAYS_SOFTWARE=1 GTK_THEME=Adwaita:dark
Xvfb :99 -screen 0 2300x900x24 >/dev/null 2>&1 &
sleep 1
UUOB_DATA_DIR=$DATA ./target/debug/uuob-app >/tmp/uuob.log 2>&1 &
win() { for _ in $(seq 1 80); do w=$(xdotool search --name "^$1\$" 2>/dev/null | head -1 || true); [ -n "$w" ] && { echo "$w"; return; }; sleep 0.5; done; }
U=$(win UUOB); xdotool windowmove "$U" 0 0 windowsize "$U" 1000 900
sleep 3
XDG_DATA_HOME=/tmp/photos-data ./target/debug/uuob-photos >/tmp/photos.log 2>&1 &
P=$(win Photos); xdotool windowmove "$P" 1000 0 windowsize "$P" 1300 900
sleep 4
n=0; shot() { import -window root "$OUT-$n.png"; n=$((n+1)); }
click() { xdotool mousemove "$1" "$2" click 1; sleep "${3:-2}"; }
shot
for c in "$@"; do
  shift
  if [ "$c" = shot ]; then shot; else click "${c%,*}" "${c#*,}"; fi
done
kill %2 %3 2>/dev/null || true
tail -3 /tmp/photos.log
