# UUOB Android build/test image: dev image + JDK + Android SDK/NDK + emulator.
FROM uuob-dev:1
RUN apt-get update && apt-get install -y --no-install-recommends openjdk-17-jdk-headless unzip \
      libpulse0 libnss3 libxcomposite1 libxcursor1 libxdamage1 libxi6 libxtst6 libasound2 \
    && rm -rf /var/lib/apt/lists/*
ENV ANDROID_HOME=/opt/android NDK_HOME=/opt/android/ndk/28.2.13676358 JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64
ENV PATH=$PATH:/opt/android/cmdline-tools/latest/bin:/opt/android/platform-tools:/opt/android/emulator
RUN mkdir -p /opt/android/cmdline-tools && cd /tmp \
 && curl -sSLo t.zip https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip \
 && unzip -q t.zip && mv cmdline-tools /opt/android/cmdline-tools/latest && rm t.zip \
 && yes | sdkmanager --licenses >/dev/null \
 && sdkmanager "platform-tools" "platforms;android-36" "build-tools;36.0.0" "ndk;28.2.13676358" \
      "emulator" "system-images;android-36;google_apis;x86_64" >/dev/null
RUN rustup target add aarch64-linux-android x86_64-linux-android
# The newest AndroidX libraries (UUOB's Compose app) compile against Android 37.
RUN yes | sdkmanager "platforms;android-37.0" >/dev/null
