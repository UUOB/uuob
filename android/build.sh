#!/usr/bin/env bash
# Inside the Android container: build UUOB for Android, unsigned.
#   docker/android.sh android/build.sh [aarch64|x86_64]     (x86_64: for the emulator)
# 1. the core (crates/uuob-app) as a library for that kind of phone,
# 2. its Kotlin bindings (UniFFI), written from that library,
# 3. the app (Gradle): android/app/build/outputs/apk/release/app-release-unsigned.apk.
# docker/apk.sh then signs it.
set -euo pipefail
arch=${1:-aarch64}
cd "$(dirname "$0")/.."
case $arch in
  aarch64) triple=aarch64-linux-android abi=arm64-v8a ;;
  x86_64) triple=x86_64-linux-android abi=x86_64 ;;
  *) echo "aarch64 or x86_64"; exit 1 ;;
esac
tools=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin
var=$(echo "$triple" | tr 'a-z-' 'A-Z_')
export "CARGO_TARGET_${var}_LINKER=$tools/${triple}26-clang" "CC_${triple//-/_}=$tools/${triple}26-clang" "AR_${triple//-/_}=$tools/llvm-ar"

cargo build --release --lib -p uuob-app --target "$triple"
lib=target/$triple/release/libuuob_app.so
rm -rf android/app/src/main/jniLibs && mkdir -p "android/app/src/main/jniLibs/$abi"
# Stripped for the phone; the bindings below are read from the unstripped one.
"$tools/llvm-strip" -o "android/app/src/main/jniLibs/$abi/libuuob_app.so" "$lib"

rm -rf android/app/src/main/java/app/uuob/core
cargo run -q -p uuob-app --features bindgen --bin uniffi-bindgen -- \
  generate --library "$lib" --language kotlin --out-dir android/app/src/main/java --no-format

cd android && ./gradlew --no-daemon -q assembleRelease
