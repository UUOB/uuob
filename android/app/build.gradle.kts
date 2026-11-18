// UUOB for Android: Jetpack Compose screens over the Rust core.
// android/build.sh first builds the core (src/main/jniLibs) and its Kotlin
// bindings (src/main/java/app/uuob/core), then runs Gradle.

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

// UUOB's version lives in one place, crates/uuob-app/Cargo.toml.
val uuobVersion = Regex("^version = \"(.+)\"", RegexOption.MULTILINE)
    .find(rootProject.file("../crates/uuob-app/Cargo.toml").readText())!!.groupValues[1]
val (major, minor, patch) = uuobVersion.split(".").map { it.toInt() }

android {
    namespace = "app.uuob"
    compileSdk = 37
    defaultConfig {
        applicationId = "app.uuob"
        minSdk = 26
        targetSdk = 36
        // As the Tauri app numbered them (0.3.3 was 3003), so updates install over it.
        versionCode = major * 1_000_000 + minor * 1_000 + patch
        versionName = uuobVersion
    }
    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    buildFeatures { compose = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    // Only the core's library for the phone it is built for (build.sh).
    packaging { jniLibs { useLegacyPackaging = false } }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2026.09.00"))
    // 1.5: Material 3 Expressive (its theme and motion) is public from this version.
    implementation("androidx.compose.material3:material3:1.5.0-rc01")
    implementation("androidx.compose.material3:material3-adaptive-navigation-suite")
    implementation("androidx.compose.material:material-icons-extended:1.7.8")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
    // The QR scanner: the camera (CameraX) and ZXing. No Google services.
    implementation("androidx.camera:camera-camera2:1.7.0")
    implementation("androidx.camera:camera-lifecycle:1.7.0")
    implementation("androidx.camera:camera-view:1.7.0")
    implementation("com.google.zxing:core:3.5.4")
    // The Kotlin bindings (UniFFI) reach the core through JNA.
    implementation("net.java.dev.jna:jna:5.19.1@aar")
}
