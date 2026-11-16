#!/usr/bin/env bash
# Start a headless Android emulator in a long-lived container named uuob-emu.
#   docker/emulator.sh start          # boot (takes ~1-2 min)
#   docker exec uuob-emu adb ...      # drive it
#   docker/emulator.sh shot out.png   # screenshot
#   docker/emulator.sh stop
# A second one next to it: UUOB_EMU=uuob-emu2 UUOB_EMU_NET=bridge docker/emulator.sh start
# Short on disk: UUOB_EMU_DATA=2G gives the phone a smaller storage (the default needs ~7 GB free).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NAME=${UUOB_EMU:-uuob-emu} NET=${UUOB_EMU_NET:-host}
case "${1:-}" in
  start)
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    docker run -d --name "$NAME" --network "$NET" --device /dev/kvm -e UUOB_EMU_DATA -v "$ROOT":/work -w /work uuob-android:1 sh -c '
      echo no | avdmanager create avd -n phone -k "system-images;android-36;google_apis;x86_64" -d pixel_7 >/dev/null &&
      { [ -z "$UUOB_EMU_DATA" ] || sed -i "s/^disk.dataPartition.size.*/disk.dataPartition.size=$UUOB_EMU_DATA/" /root/.android/avd/phone.avd/config.ini; } &&
      emulator -avd phone -no-window -no-audio -no-boot-anim -gpu swiftshader_indirect -no-snapshot -memory 4096' >/dev/null
    docker exec "$NAME" sh -c 'adb wait-for-device; until [ "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d "\r")" = 1 ]; do sleep 3; done'
    docker exec "$NAME" adb shell settings put system screen_off_timeout 1800000
    echo booted ;;
  shot) docker exec "$NAME" adb exec-out screencap -p > "$2" ;;
  stop) docker rm -f "$NAME" >/dev/null ;;
  *) echo "usage: $0 start|shot <file>|stop"; exit 1 ;;
esac
