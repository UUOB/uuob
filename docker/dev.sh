#!/usr/bin/env bash
# Run a command inside the UUOB dev container (cached cargo registry, target dir, node_modules).
set -euo pipefail
# The repository this is run for: UUOB itself, or an app that has UUOB's
# repository as its `uuob` folder (a git submodule). Its app is in `app/`.
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git -C "$HERE" rev-parse --show-superproject-working-tree)"
# The build cache, one per repository (UUOB_TARGET names another): a Tauri
# plugin's build writes files next to its sources, so a cache shared with
# another checkout would take them for built.
TARGET=${UUOB_TARGET:-uuob-target${ROOT:+-$(basename "$ROOT")}}
ROOT="${ROOT:-$(git -C "$HERE" rev-parse --show-toplevel)}"
IMAGE=uuob-dev:1
docker image inspect "$IMAGE" >/dev/null 2>&1 || docker build -t "$IMAGE" "$HERE"
exec docker run --rm -i --network host \
  -v "$ROOT":/work -w /work \
  -v uuob-cargo-registry:/usr/local/cargo/registry \
  -v "$TARGET":/work/target \
  -e CARGO_TERM_COLOR=never -e CARGO_PROFILE_DEV_DEBUG -e CARGO_INCREMENTAL \
  "$IMAGE" "$@"
