#!/usr/bin/env bash
# Run a command inside the UUOB dev container (cached cargo registry, target dir, node_modules).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE=uuob-dev:1
docker image inspect "$IMAGE" >/dev/null 2>&1 || docker build -t "$IMAGE" "$ROOT/docker"
exec docker run --rm -i --network host \
  -v "$ROOT":/work -w /work \
  -v uuob-cargo-registry:/usr/local/cargo/registry \
  -v "${UUOB_TARGET:-uuob-target}":/work/target \
  -e CARGO_TERM_COLOR=never -e CARGO_PROFILE_DEV_DEBUG -e CARGO_INCREMENTAL \
  "$IMAGE" "$@"
