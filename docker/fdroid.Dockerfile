# The Android image plus fdroidserver, to write the F-Droid repo index (docker/fdroid.sh).
# From pip: Debian's version can't read APKs made by current Android build tools.
FROM uuob-android:1
RUN apt-get update && apt-get install -y --no-install-recommends python3-pip \
 && pip install --break-system-packages --no-cache-dir fdroidserver \
 && rm -rf /var/lib/apt/lists/*
