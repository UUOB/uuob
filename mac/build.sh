#!/usr/bin/env bash
# On a Mac (GitHub's Mac runner, see .github/workflows/mac.yml): build UUOB.app
# for Apple silicon and Intel, then the .dmg (first install) and the
# .app.tar.gz (what the Update button installs), in out/.
# Needs Rust with the aarch64-apple-darwin and x86_64-apple-darwin targets, and Xcode.
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/uuob-app/Cargo.toml | head -1)
build=mac/build gen=mac/build/gen app=mac/build/UUOB.app
rm -rf "$build" && mkdir -p "$gen" out

# 1. The core, for both kinds of Mac, as one library.
for t in aarch64-apple-darwin x86_64-apple-darwin; do
  MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release --lib -p uuob-app --target "$t"
done
lipo -create -output "$build/libuuob_app.dylib" target/{aarch64,x86_64}-apple-darwin/release/libuuob_app.dylib
install_name_tool -id @rpath/libuuob_app.dylib "$build/libuuob_app.dylib"

# 2. Its Swift bindings (UniFFI), from the library.
cargo run -q -p uuob-app --features bindgen --bin uniffi-bindgen -- \
  generate --library target/aarch64-apple-darwin/release/libuuob_app.dylib --language swift --out-dir "$gen"

# 3. The app, for both kinds of Mac.
for arch in arm64 x86_64; do
  swiftc -O -parse-as-library -swift-version 5 -target "$arch-apple-macos14.0" -module-name UUOB \
    -I "$gen" -Xcc -fmodule-map-file="$gen/UuobCoreFFI.modulemap" \
    mac/Sources/*.swift "$gen/UuobCore.swift" \
    -L "$build" -luuob_app -Xlinker -rpath -Xlinker @executable_path/../Frameworks \
    -o "$build/UUOB-$arch"
done

# 4. UUOB.app, signed for this Mac only (no paid Apple account: macOS asks once, "Open Anyway").
mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks" "$app/Contents/Resources"
lipo -create -output "$app/Contents/MacOS/UUOB" "$build"/UUOB-{arm64,x86_64}
cp "$build/libuuob_app.dylib" "$app/Contents/Frameworks/"
cp mac/UUOB.icns "$app/Contents/Resources/"
sed "s/VERSION/$version/g" mac/Info.plist > "$app/Contents/Info.plist"
codesign --force --deep --sign - "$app"

# 5. What gets published.
mkdir -p "$build/dmg" && cp -R "$app" "$build/dmg/" && ln -s /Applications "$build/dmg/Applications"
hdiutil create -volname UUOB -srcfolder "$build/dmg" -ov -format UDZO "out/UUOB_${version}_universal.dmg"
(cd "$build" && COPYFILE_DISABLE=1 tar -czf "../../out/UUOB.app.tar.gz" UUOB.app)
ls -la out
