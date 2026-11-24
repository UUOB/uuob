#!/usr/bin/env bash
# Build a repository's Mac app on a Mac over SSH, instead of on GitHub.
#   mac/remote-build.sh [repo-dir]      (default: this repository)
#   UUOB_SDK=../UUOB-sdk mac/remote-build.sh ../UUOB-calendar   (with an unpushed kit)
# The Mac is the SSH host "uuob-mac" (an entry in ~/.ssh/config), or $UUOB_MAC.
# Everything it puts on the Mac stays in one folder, ~/uuob-build: the source,
# the build files, the Rust download cache. ~/uuob-build/uninstall.sh removes it all.
# The Mac needs Rust, Apple's Command Line Tools, and Xcode: either installed, or
# unpacked at ~/uuob-build/Xcode.app, which the build then uses without changing the system.
# The .dmg and .app.tar.gz come back to <repo>/dist/mac/.
set -euo pipefail
mac=${UUOB_MAC:-uuob-mac}
repo=$(cd "${1:-$(dirname "$0")/..}" && pwd)
name=$(basename "$repo")
dir=uuob-build
result=$repo/dist/mac

mkdir -p "$result" 2>/dev/null && [ -w "$result" ] || {
  echo "Can't write to $result (docker/apk.sh leaves dist/ owned by root). Fix: sudo chown -R \$USER $repo/dist" >&2; exit 1; }

ssh -o BatchMode=yes "$mac" true 2>/dev/null || {
  echo "Can't reach the Mac ($mac). Is it awake, on Tailscale, with Remote Login on?" >&2; exit 1; }

# 1. The one folder, and the way to remove it.
ssh "$mac" "mkdir -p $dir/src $dir/cargo $dir/tmp && cat > $dir/uninstall.sh && chmod +x $dir/uninstall.sh" <<'EOF'
#!/bin/sh
# Removes everything the UUOB builds put on this Mac: ~/uuob-build and the build key.
sed -i '' '/ uuob-mac-build$/d' ~/.ssh/authorized_keys 2>/dev/null
rm -rf ~/uuob-build
echo "Removed. You can also turn off Remote Login: System Settings > General > Sharing."
EOF

# 2. The working tree as it is now (unpushed changes included): the files git tracks or
#    would track, submodules whole. The Mac's old copy goes first, except its build files.
(cd "$repo" && git ls-files -co --exclude-standard | grep -v '^android/' \
  | tar -c --ignore-failed-read --exclude .git --exclude target --exclude node_modules -T -) \
  | ssh "$mac" "mkdir -p $dir/src/$name && cd $dir/src/$name \
      && find . -mindepth 1 -maxdepth 1 ! -name target ! -name out -exec rm -rf {} + && tar -x"
# With UUOB_SDK=<a UUOB-sdk checkout>, that checkout (uncommitted changes included) is
# built as sdk/ instead of the submodule, without touching the repository's own sdk/.
[ -z "${UUOB_SDK:-}" ] || (cd "$UUOB_SDK" && git ls-files -co --exclude-standard \
  | tar -c --ignore-failed-read --exclude .git --exclude target -T -) \
  | ssh "$mac" "cd $dir/src/$name && rm -rf sdk && mkdir sdk && cd sdk && tar -x"

# 3. Build, with every cache pointed into the folder; caffeinate keeps the Mac awake meanwhile.
ssh "$mac" "cd $dir/src/$name && { [ ! -d \$HOME/$dir/Xcode.app ] || export DEVELOPER_DIR=\$HOME/$dir/Xcode.app/Contents/Developer; } \
  && export PATH=\$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:\$PATH \
  CARGO_HOME=\$HOME/$dir/cargo TMPDIR=\$HOME/$dir/tmp CLANG_MODULE_CACHE_PATH=\$HOME/$dir/tmp/clang \
  && rm -rf out && caffeinate -i mac/build.sh"

# 4. Bring the result back.
ssh "$mac" "cd $dir/src/$name/out && COPYFILE_DISABLE=1 tar --no-mac-metadata --no-xattrs -c *" | tar -x -C "$result"
ls -la "$result"
