#!/usr/bin/env bash
# Inside the Android container: build one app's APK for one kind of phone and sign it.
#   docker/android.sh docker/apk.sh <uuob|tasks|calendar|photos> [aarch64|x86_64]
# Leaves dist/<App>-<version>-<arch>.apk and, next to it, its update signature (.sig).
# Two keys are used, both kept in the uuob-android-keys volume: Android's own
# (an update must be signed like the installed app) and the update key (the app
# checks it before installing; see crates/uuob-update).
set -euo pipefail
app=$1 arch=${2:-aarch64}
case $app in uuob) dir=app name=UUOB ;; tasks) dir=apps/tasks name=Tasks ;; calendar) dir=apps/calendar name=Calendar ;; photos) dir=apps/photos name=Photos ;; *) echo "unknown app: $app"; exit 1 ;; esac
tools=$(ls -d "$ANDROID_HOME"/build-tools/* | tail -1)

# Tauri writes TauriActivity.kt into the app only when its own build step runs,
# and with one build cache for three apps it doesn't run again for an app that
# was never built in this checkout. Make it run.
find "$dir/src-tauri/gen/android/app/src/main/java" -name TauriActivity.kt | grep -q . ||
  find target -maxdepth 4 -path '*/build/tauri-[0-9a-f]*' -prune -exec rm -rf {} + 2>/dev/null || true

# UUOB_BUILD_CONFIG (optional): extra settings for this build, e.g. another update address when testing.
(cd "$dir" && npx tauri android build --apk --target "$arch" ${UUOB_BUILD_CONFIG:+--config "$UUOB_BUILD_CONFIG"})
built=$(ls "$dir"/src-tauri/gen/android/app/build/outputs/apk/universal/release/*.apk | head -1)
version=$("$tools/aapt2" dump badging "$built" | sed -n "s/.*versionName='\([^']*\)'.*/\1/p")
out=dist/$name-$version-${arch/aarch64/arm64}.apk

mkdir -p dist
"$tools/zipalign" -f -p 4 "$built" /tmp/aligned.apk
"$tools/apksigner" sign --ks /root/.android/debug.keystore --ks-pass pass:android --key-pass pass:android \
  --ks-key-alias androiddebugkey --out "$out" /tmp/aligned.apk
rm -f "$out.idsig"
npx tauri signer sign -f /root/.android/uuob-update.key -p "" "$out" >/dev/null
chown -R 1000:1000 dist "$dir" crates/uuob-update node_modules 2>/dev/null || true
echo "$out"
