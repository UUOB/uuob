#!/bin/sh
# Renders the logo into the app's icon files (desktop, tray, Android). Runs in Docker.
# Usage: sh brand/export.sh   (from the repository root)
set -e
cd "$(dirname "$0")/.."
python3 brand/build.py
docker run --rm -v "$PWD":/w -w /w -u 0 alpine:latest sh -c '
  set -e
  apk add --no-cache rsvg-convert python3 >/dev/null 2>&1
  I=app/src-tauri/icons
  R=app/src-tauri/gen/android/app/src/main/res
  cp brand/logo-icon.svg $I/icon.svg
  rsvg-convert -w 32   -h 32   brand/logo-icon.svg -o $I/32x32.png
  rsvg-convert -w 128  -h 128  brand/logo-icon.svg -o $I/128x128.png
  rsvg-convert -w 256  -h 256  brand/logo-icon.svg -o $I/128x128@2x.png
  rsvg-convert -w 512  -h 512  brand/logo-icon.svg -o $I/icon.png
  T=$(mktemp -d)
  for s in 16 32 64 128 256 512 1024; do rsvg-convert -w $s -h $s brand/logo-icon.svg -o $T/$s.png; done
  python3 - "$T" "$I/icon.icns" <<"EOF"
import struct, sys
t, out = sys.argv[1], sys.argv[2]
types = [("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128), ("ic08", 256), ("ic09", 512), ("ic10", 1024),
         ("ic11", 32), ("ic12", 64), ("ic13", 256), ("ic14", 512)]
body = b"".join(k.encode() + struct.pack(">I", 8 + len(d)) + d
                for k, s in types for d in [open(f"{t}/{s}.png", "rb").read()])
open(out, "wb").write(b"icns" + struct.pack(">I", 8 + len(body)) + body)
EOF
  for d in mdpi:48 hdpi:72 xhdpi:96 xxhdpi:144 xxxhdpi:192; do
    n=${d%%:*}; s=${d##*:}; f=$((s * 9 / 4))
    rsvg-convert -w $s -h $s brand/logo-icon.svg       -o $R/mipmap-$n/ic_launcher.png
    rsvg-convert -w $s -h $s brand/logo-round.svg      -o $R/mipmap-$n/ic_launcher_round.png
    rsvg-convert -w $f -h $f brand/logo-foreground.svg -o $R/mipmap-$n/ic_launcher_foreground.png
  done
  chown -R '"$(id -u):$(id -g)"' $I $R
'
echo "Icons updated."
