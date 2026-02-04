# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.kts.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# Keep SSH-related classes
-keep class net.schmizz.** { *; }
-keep class com.hierynomus.** { *; }
-dontwarn net.schmizz.**
-dontwarn com.hierynomus.**

# Keep BouncyCastle
-keep class org.bouncycastle.** { *; }
-dontwarn org.bouncycastle.**

# Keep SLF4J
-keep class org.slf4j.** { *; }
-dontwarn org.slf4j.**

# EdDSA and SSHJ platform specific references
# Tell R8 to ignore missing classes from desktop Java packages
-dontwarn net.i2p.crypto.eddsa.**
-dontwarn sun.security.**
-dontwarn com.sun.jna.**

# Add this to ignore missing classes in general if they are from internal sun packages
-ignorewarnings

# Keep Kotlin coroutines
-keepnames class kotlinx.coroutines.internal.MainDispatcherFactory {}
-keepnames class kotlinx.coroutines.CoroutineExceptionHandler {}
-keepclassmembernames class kotlinx.** {
    volatile <fields>;
}

# Keep ViewBinding classes
-keep class * implements androidx.viewbinding.ViewBinding {
    *;
}

# Keep Zelland app classes
-keep class com.zelland.** { *; }

# Optimization is safe - these are just warnings
-dontwarn javax.annotation.**
-dontwarn javax.naming.**
-dontwarn com.jcraft.jzlib.**
-dontwarn org.ietf.jgss.**
