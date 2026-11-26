#!/usr/bin/env bash
# Back up or restore the release keys, which live only in the uuob-android-keys Docker volume:
#   docker/keys.sh backup [file]    one file, encrypted with a passphrase you choose
#   docker/keys.sh restore <file>   puts the keys back into the volume
# The keys are Android's signing key (debug.keystore: every update must be signed with it),
# the update key (uuob-update.key: installed apps check it before they update) and the F-Droid
# repo's key (fdroid/). Without them no installed copy can be updated again, so keep the backup
# off this computer (a USB stick, a password manager) and the passphrase somewhere else.
set -euo pipefail
VOLUME=uuob-android-keys
FILES="debug.keystore uuob-update.key uuob-update.key.pub fdroid/keystore.p12 fdroid/config.yml"
CIPHER=(-aes-256-cbc -pbkdf2 -iter 600000)

case "${1:-}" in
  backup)
    out=${2:-uuob-keys-$(date +%Y-%m-%d).tar.gz.enc}
    [ ! -e "$out" ] || { echo "$out already exists." >&2; exit 1; }
    (umask 077 && docker run --rm -v "$VOLUME":/k:ro alpine tar -C /k -cz $FILES | openssl enc "${CIPHER[@]}" -salt -out "$out")
    echo "Saved $out. Copy it off this computer, and keep the passphrase somewhere else."
    ;;
  restore)
    in=${2:?"which backup file?"}
    openssl enc -d "${CIPHER[@]}" -in "$in" | docker run --rm -i -v "$VOLUME":/k alpine tar -C /k -xz
    echo "The keys are back in the $VOLUME volume."
    ;;
  *)
    sed -n '2,4p' "$0" >&2
    exit 1
    ;;
esac
