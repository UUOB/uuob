#!/usr/bin/env bash
# Run a command in the container for the Linux window (GTK 4, Libadwaita 1.5,
# Relm4): UUOB's own (`linux/`) or an app's that has UUOB's repository as its
# `uuob` folder. E.g. `docker/linux.sh cargo build -p uuob-linux`.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git -C "$HERE" rev-parse --show-superproject-working-tree)"
# The build cache, one per repository (UUOB_TARGET names another).
TARGET=${UUOB_TARGET:-uuob-target-linux${ROOT:+-$(basename "$ROOT")}}
ROOT="${ROOT:-$(git -C "$HERE" rev-parse --show-toplevel)}"
IMAGE=uuob-linux:1
docker image inspect "$IMAGE" >/dev/null 2>&1 || docker build -t "$IMAGE" -f "$HERE/linux.Dockerfile" "$HERE"
exec docker run --rm -i --network host \
  -v "$ROOT":/work -w /work \
  -v uuob-cargo-registry:/usr/local/cargo/registry \
  -v "$TARGET":/work/target \
  -e CARGO_TERM_COLOR=never \
  "$IMAGE" "$@"
