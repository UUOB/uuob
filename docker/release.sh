#!/usr/bin/env bash
# Publish the current version of this repository's app (the one in `app/`):
#   docker/release.sh             in UUOB's repository
#   uuob/docker/release.sh        in an app's repository (UUOB's is its `uuob` folder)
#
# Before running it, raise the version (UUOB: crates/uuob-app/Cargo.toml; an
# app: app/src-tauri/tauri.conf.json), commit and push: the Mac build runs on
# GitHub from the pushed commit.
# This then
#   1. builds and signs the Android APK here (and, for UUOB, the `uuob`
#      program for Linux servers and the Raspberry Pi),
#   2. builds the Mac app on GitHub Actions and downloads it,
#   3. signs the files with the update key and uploads them as a release to
#      the public downloads repo,
#   4. points the app's feed there (<app>.json) and refreshes its row on the
#      downloads page, its Homebrew cask and the F-Droid repo (fdroid.sh).
#      UUOB's release also rewrites the rest of the downloads page and the
#      installer for servers.
# Installed apps then show "Update to <version>" (see crates/uuob-update).
#
# Needs docker, gh (logged in) and python3; the keys are in the
# uuob-android-keys Docker volume (see apk.sh).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git -C "$HERE" rev-parse --show-superproject-working-tree)"
ROOT="${ROOT:-$(git -C "$HERE" rev-parse --show-toplevel)}"
cd "$ROOT"
D=${HERE#"$ROOT"/} # these scripts as the containers see them: docker, or uuob/docker
REPO=${UUOB_RELEASES:-jachymri/uuob-releases}

if [ -x android/build.sh ]; then
  # UUOB's native app: its name in android/settings.gradle.kts, its version in crates/uuob-app/Cargo.toml.
  name=$(sed -n 's/^rootProject.name = "\(.*\)"/\1/p' android/settings.gradle.kts)
  version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/uuob-app/Cargo.toml | head -1)
else
  field() { python3 -c 'import json,sys; print(json.load(open("app/src-tauri/tauri.conf.json"))[sys.argv[1]])' "$1"; }
  name=$(field productName) version=$(field version)
fi
app=$(echo "$name" | tr '[:upper:]' '[:lower:]') # uuob, tasks, …: its feed and its release tags
tag=$app-v$version

[ -z "$(git status --porcelain)" ] || { echo "Commit your changes first."; exit 1; }
git fetch -q origin
branch=$(git rev-parse --abbrev-ref HEAD) head=$(git rev-parse HEAD)
[ "$head" = "$(git rev-parse "origin/$branch" 2>/dev/null)" ] || { echo "Push $branch first: the Mac build runs on GitHub."; exit 1; }
! gh release view "$tag" --repo "$REPO" >/dev/null 2>&1 || { echo "$tag is already published: raise the version."; exit 1; }

# UUOB_MAC_RUN (optional): take the Mac app from that finished or running
# GitHub run instead of starting one, e.g. when publishing again after a failure.
run=${UUOB_MAC_RUN:-}
if [ -z "$run" ]; then
  echo "== Mac: building on GitHub"
  started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  gh workflow run mac.yml --ref "$branch"
  for _ in $(seq 30); do
    sleep 4
    run=$(gh run list --workflow mac.yml --limit 5 --json databaseId,headSha,createdAt,event \
      --jq "[.[] | select(.event == \"workflow_dispatch\" and .headSha == \"$head\" and .createdAt >= \"$started\")][0].databaseId // empty")
    [ -z "$run" ] || break
  done
  [ -n "$run" ] || { echo "The Mac build did not start."; exit 1; }
fi

echo "== Android: building here"
mkdir -p dist && rm -rf dist/mac dist/releases
"$HERE/android.sh" "$D/apk.sh" aarch64 >"dist/$app.log" 2>&1 || { tail -30 "dist/$app.log"; exit 1; }

# UUOB without a window (crates/uuob-cli), for ARM and Intel, goes out with UUOB.
extra=()
if [ "$app" = uuob ]; then
  echo "== Linux: building the uuob program here"
  for arch in arm64 amd64; do
    docker buildx build -q -f "$HERE/server.Dockerfile" --platform "linux/$arch" --target program -o "type=local,dest=dist/linux-$arch" . >/dev/null
    mv "dist/linux-$arch/uuob" "dist/uuob-$version-linux-$arch"
    extra+=("dist/uuob-$version-linux-$arch")
  done
fi

echo "== Mac: waiting for GitHub (run $run)"
gh run watch "$run" --exit-status >/dev/null
gh run download "$run" -n mac -D dist/mac
mv dist/mac/"$name".app.tar.gz "dist/$name-$version-mac.app.tar.gz"
mv dist/mac/"$name"_*.dmg "dist/$name-$version-mac.dmg"
"$HERE/android.sh" sh -c "npx tauri signer sign -f /root/.android/uuob-update.key -p '' 'dist/$name-$version-mac.app.tar.gz' >/dev/null && chown -R 1000:1000 dist"

echo "== Publishing to $REPO"
rm -rf dist/releases && gh repo clone "$REPO" dist/releases -- -q
gh release create "$tag" --repo "$REPO" --title "$name $version" --notes "" \
  "dist/$name-$version-arm64.apk" "dist/$name-$version-mac.dmg" "dist/$name-$version-mac.app.tar.gz" ${extra[@]+"${extra[@]}"} >/dev/null
python3 "$HERE/feed.py" "$REPO" "$app" "$name" "$version" dist
if [ "$app" = uuob ]; then
  # The page's text and the installer for servers belong to UUOB's repository;
  # an app's copy of them may be older.
  python3 "$HERE/feed.py" "$REPO" --page dist/releases
  cp "$HERE/install.sh" dist/releases/install.sh
else
  python3 "$HERE/feed.py" "$REPO" --rows dist/releases
fi
git -C dist/releases add -A
git -C dist/releases -c user.name="$(git config user.name)" -c user.email="$(git config user.email)" \
  commit -q -m "$name $version${UUOB_COMMIT_TRAILER:+$'\n\n'$UUOB_COMMIT_TRAILER}"
git -C dist/releases push -q
"$HERE/fdroid.sh"
echo "Published: https://github.com/$REPO"
