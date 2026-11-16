#!/usr/bin/env bash
# Publish the current version of one or more apps:
#   docker/release.sh tasks calendar
#
# Before running it, raise "version" in each app's src-tauri/tauri.conf.json,
# commit and push: the Mac build runs on GitHub from the pushed commit.
# For each app this then
#   1. builds and signs the Android APK here (and, with UUOB, the `uuob`
#      program for Linux servers and the Raspberry Pi),
#   2. builds the Mac app on GitHub Actions and downloads it,
#   3. signs the files with the update key and uploads them as a release to
#      the public downloads repo,
#   4. points the app's feed there (<app>.json) and refreshes the downloads page,
#      the Homebrew casks and the F-Droid repo (docker/fdroid.sh).
# Installed apps then show "Update to <version>" (see crates/uuob-update).
#
# Needs docker, gh (logged in) and python3; the keys are in the
# uuob-android-keys Docker volume (see docker/apk.sh).
set -euo pipefail
cd "$(dirname "$0")/.."
REPO=${UUOB_RELEASES:-jachymri/uuob-releases}
[ $# -gt 0 ] || { echo "usage: $0 <uuob|tasks|calendar|photos> ..."; exit 1; }

conf() { case $1 in uuob) echo app/src-tauri/tauri.conf.json ;; tasks | calendar | photos) echo "apps/$1/src-tauri/tauri.conf.json" ;; *) echo "unknown app: $1" >&2; exit 1 ;; esac; }
field() { python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])' "$(conf "$1")" "$2"; }

[ -z "$(git status --porcelain)" ] || { echo "Commit your changes first."; exit 1; }
git fetch -q origin
branch=$(git rev-parse --abbrev-ref HEAD) head=$(git rev-parse HEAD)
[ "$head" = "$(git rev-parse "origin/$branch" 2>/dev/null)" ] || { echo "Push $branch first: the Mac build runs on GitHub."; exit 1; }
for app in "$@"; do
  tag=$app-v$(field "$app" version)
  ! gh release view "$tag" --repo "$REPO" >/dev/null 2>&1 || { echo "$tag is already published: raise the version of $app."; exit 1; }
done

# UUOB_MAC_RUN (optional): take the Mac apps from that finished or running
# GitHub run instead of starting one, e.g. when publishing again after a failure.
run=${UUOB_MAC_RUN:-}
if [ -z "$run" ]; then
  echo "== Mac: building on GitHub"
  started=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  gh workflow run mac.yml --ref "$branch" -f apps="$*"
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
for app in "$@"; do docker/android.sh docker/apk.sh "$app" aarch64 >"dist/$app.log" 2>&1 || { tail -30 "dist/$app.log"; exit 1; }; done

# UUOB without a window (crates/uuob-cli), for ARM and Intel, goes out with UUOB.
linux=()
case " $* " in *" uuob "*)
  echo "== Linux: building the uuob program here"
  for arch in arm64 amd64; do
    docker buildx build -q -f docker/server.Dockerfile --platform "linux/$arch" --target program -o "type=local,dest=dist/linux-$arch" . >/dev/null
    mv "dist/linux-$arch/uuob" "dist/uuob-$(field uuob version)-linux-$arch"
    linux+=("dist/uuob-$(field uuob version)-linux-$arch")
  done ;;
esac

echo "== Mac: waiting for GitHub (run $run)"
gh run watch "$run" --exit-status >/dev/null
gh run download "$run" -n mac -D dist/mac
for app in "$@"; do
  name=$(field "$app" productName) version=$(field "$app" version)
  mv dist/mac/"$name".app.tar.gz "dist/$name-$version-mac.app.tar.gz"
  mv dist/mac/"$name"_*.dmg "dist/$name-$version-mac.dmg"
  docker/android.sh sh -c "npx tauri signer sign -f /root/.android/uuob-update.key -p '' 'dist/$name-$version-mac.app.tar.gz' >/dev/null && chown -R 1000:1000 dist"
done

echo "== Publishing to $REPO"
rm -rf dist/releases && gh repo clone "$REPO" dist/releases -- -q
for app in "$@"; do
  name=$(field "$app" productName) version=$(field "$app" version)
  extra=()
  [ "$app" != uuob ] || extra=("${linux[@]}")
  gh release create "$app-v$version" --repo "$REPO" --title "$name $version" --notes "" \
    "dist/$name-$version-arm64.apk" "dist/$name-$version-mac.dmg" "dist/$name-$version-mac.app.tar.gz" ${extra[@]+"${extra[@]}"} >/dev/null
  python3 docker/feed.py "$REPO" "$app" "$name" "$version" dist
done
python3 docker/feed.py "$REPO" --page dist/releases
cp docker/install.sh dist/releases/install.sh # the one-line installer for servers
git -C dist/releases add -A
git -C dist/releases -c user.name="$(git config user.name)" -c user.email="$(git config user.email)" \
  commit -q -m "$(for app in "$@"; do printf '%s %s, ' "$(field "$app" productName)" "$(field "$app" version)"; done | sed 's/, $//')${UUOB_COMMIT_TRAILER:+$'\n\n'$UUOB_COMMIT_TRAILER}"
git -C dist/releases push -q
docker/fdroid.sh
echo "Published: https://github.com/$REPO"
