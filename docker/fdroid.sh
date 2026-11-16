#!/usr/bin/env bash
# Refresh the F-Droid repo of the public downloads repo, so the apps can also be
# installed and updated with an F-Droid app (docker/release.sh runs this last):
#   docker/fdroid.sh            (uuob/docker/fdroid.sh in an app's repository)
# It takes the Android version each app's feed points to, writes a signed
# F-Droid index for them and puts both on the gh-pages branch, which GitHub
# serves at https://<user>.github.io/<repo>/fdroid/repo. The branch is replaced
# every time, so old APKs don't pile up.
# The repo's signing key is created on first use and kept, like the other keys,
# only in the uuob-android-keys Docker volume (/root/.android/fdroid).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git -C "$HERE" rev-parse --show-superproject-working-tree)"
cd "${ROOT:-$(git -C "$HERE" rev-parse --show-toplevel)}"
REPO=${UUOB_RELEASES:-jachymri/uuob-releases}
URL=https://${REPO%%/*}.github.io/${REPO##*/}/fdroid/repo
IMAGE=uuob-fdroid:1
docker image inspect "$IMAGE" >/dev/null 2>&1 || docker build -t "$IMAGE" -f "$HERE/fdroid.Dockerfile" "$HERE"

rm -rf dist/pages dist/fdroid && mkdir -p dist/fdroid/repo
gh repo clone "$REPO" dist/pages -- -q
for feed in dist/pages/*.json; do
  apk=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["platforms"]["android-aarch64"]["url"])' "$feed")
  [ -f "dist/${apk##*/}" ] && cp "dist/${apk##*/}" dist/fdroid/repo/ || curl -fsSL -o "dist/fdroid/repo/${apk##*/}" "$apk"
done

docker run --rm -i -v "$PWD/dist/fdroid":/out -v uuob-android-keys:/root/.android -e URL="$URL" "$IMAGE" bash -s <<'EOF'
set -euo pipefail
mkdir -p /root/.android/fdroid && cd /root/.android/fdroid
[ -f keystore.p12 ] || keytool -genkeypair -keystore keystore.p12 -storetype PKCS12 -alias uuob -keyalg RSA -keysize 4096 \
  -validity 10000 -storepass android -keypass android -dname "CN=UUOB" 2>/dev/null
cat > config.yml <<YML
repo_url: $URL
repo_name: UUOB
repo_description: UUOB, your own backend, and the apps that use it. Each app is built and signed by its author.
archive_older: 0
keystore: keystore.p12
repo_keyalias: uuob
keystorepass: android
keypass: android
keydname: CN=UUOB
YML
chmod 600 config.yml
rm -rf repo metadata && mkdir -p metadata && cp -r /out/repo repo
app() { printf 'Name: "%s"\nSummary: "%s"\nAuthorName: UUOB\nLicense: AGPL-3.0-or-later\nCategories:\n  - %s\n' "$2" "$3" "$4" > "metadata/$1.yml"; }
app app.uuob UUOB "Your own backend: links your devices and keeps your apps' data on them" System
app app.uuob.tasks Tasks "Tasks, kept in your own UUOB" Writing
app app.uuob.calendar Calendar "Calendar, kept in your own UUOB" Time
app app.uuob.photos Photos "Photos and videos, kept in your own UUOB" Graphics
fdroid update >/out/update.log 2>&1 || { tail -20 /out/update.log; exit 1; }
rm -rf /out/repo && cp -r repo /out/repo
keytool -list -v -keystore keystore.p12 -storepass android -alias uuob 2>/dev/null | sed -n 's/.*SHA256: //p' | tr -d ':' > /out/fingerprint
chown -R 1000:1000 /out
EOF

cd dist/pages
git checkout -q --orphan gh-pages && git rm -rfq . && mkdir fdroid && cp -r ../fdroid/repo fdroid/repo && touch .nojekyll
git add -A
git -c user.name="$(git -C ../.. config user.name)" -c user.email="$(git -C ../.. config user.email)" \
  commit -q -m "F-Droid repo${UUOB_COMMIT_TRAILER:+$'\n\n'$UUOB_COMMIT_TRAILER}"
git push -qf origin gh-pages
echo "F-Droid repo: $URL?fingerprint=$(cat ../fdroid/fingerprint)"
