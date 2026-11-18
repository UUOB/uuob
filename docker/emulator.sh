#!/usr/bin/env bash
# Start a headless Android emulator in a long-lived container named uuob-emu.
#   docker/emulator.sh start          # boot (takes ~1-2 min)
#   docker exec uuob-emu adb ...      # drive it
#   docker/emulator.sh shot out.png   # screenshot
#   docker/emulator.sh stop
# A second one next to it: UUOB_EMU=uuob-emu2 UUOB_EMU_NET=bridge docker/emulator.sh start
# Short on disk: UUOB_EMU_RAM=9g keeps the phone's storage in memory instead (it needs ~7.4 GB).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git -C "$HERE" rev-parse --show-superproject-working-tree)" # an app's repository, if run from its `uuob` folder
ROOT="${ROOT:-$(git -C "$HERE" rev-parse --show-toplevel)}"
NAME=${UUOB_EMU:-uuob-emu} NET=${UUOB_EMU_NET:-host}
case "${1:-}" in
  start)
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    docker run -d --name "$NAME" --network "$NET" --device /dev/kvm ${UUOB_EMU_RAM:+--tmpfs /root/.android/avd:rw,exec,size=$UUOB_EMU_RAM} -v "$ROOT":/work -w /work uuob-android:1 sh -c '
      echo no | avdmanager create avd -n phone -k "system-images;android-36;google_apis;x86_64" -d pixel_7 >/dev/null &&
      emulator -avd phone -no-window -no-audio -no-boot-anim -gpu swiftshader_indirect -no-snapshot -memory 4096' >/dev/null
    docker exec "$NAME" sh -c 'adb wait-for-device; until [ "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d "\r")" = 1 ]; do sleep 3; done'
    docker exec "$NAME" adb shell settings put system screen_off_timeout 1800000
    echo booted ;;
  shot) docker exec "$NAME" adb exec-out screencap -p > "$2" ;;
  stop) docker rm -f "$NAME" >/dev/null ;;
  *) echo "usage: $0 start|shot <file>|stop"; exit 1 ;;
esac
