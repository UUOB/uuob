# The core's Kotlin bindings call into Rust through JNA, which finds classes by name.
-keep class com.sun.jna.** { *; }
-keep class * extends com.sun.jna.** { *; }
-keep class app.uuob.core.** { *; }
-dontwarn java.awt.**
