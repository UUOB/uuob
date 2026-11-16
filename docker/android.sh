#!/usr/bin/env bash
# Run a command inside the UUOB Android container (KVM for the emulator).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE=uuob-android:1
docker image inspect "$IMAGE" >/dev/null 2>&1 || docker build -t "$IMAGE" -f "$ROOT/docker/android.Dockerfile" "$ROOT/docker"
exec docker run --rm -i --network host --device /dev/kvm \
  -v "$ROOT":/work -w /work \
  -v uuob-cargo-registry:/usr/local/cargo/registry \
  -v "${UUOB_TARGET:-uuob-target}":/work/target \
  -v uuob-gradle:/root/.gradle \
  -v uuob-android-keys:/root/.android \
  -e CARGO_TERM_COLOR=never -e CARGO_PROFILE_DEV_DEBUG -e CARGO_INCREMENTAL -e UUOB_BUILD_CONFIG \
  "$IMAGE" "$@"
