plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.bolt.app"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.bolt.app"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // Debug-signed so the APK installs locally; real distribution needs a release keystore.
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { compose = true }
    sourceSets["main"].jniLibs.srcDir(layout.buildDirectory.dir("rustJniLibs"))
    packaging { jniLibs.useLegacyPackaging = false }
}

// Build the Rust core (crates/bolt-android) for the Android ABIs with cargo-ndk.
// Skip with -PskipRust when the .so files are already built.
val rustRoot = rootProject.projectDir.resolve("../..")
val buildRust by tasks.registering(Exec::class) {
    // Always optimised: an unoptimised QUIC/crypto build is far too slow to be representative.
    val profile = "--release"
    workingDir = rustRoot
    val sdk = System.getenv("ANDROID_HOME") ?: System.getenv("ANDROID_SDK_ROOT")
        ?: "${System.getenv("LOCALAPPDATA")}/Android/Sdk"
    val ndkDir = file("$sdk/ndk").listFiles()?.filter { it.isDirectory }?.maxByOrNull { it.name }
    environment("ANDROID_NDK_HOME", ndkDir?.absolutePath ?: "")
    commandLine(
        "cargo", "ndk", "-t", "arm64-v8a", "-t", "x86_64", "--platform", "29",
        "-o", layout.buildDirectory.dir("rustJniLibs").get().asFile.absolutePath,
        "build", profile, "-p", "bolt-android",
    )
    onlyIf { !project.hasProperty("skipRust") }
}
tasks.matching { it.name == "preBuild" }.configureEach { dependsOn(buildRust) }

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2025.06.01")
    implementation(composeBom)
    implementation("androidx.core:core-ktx:1.16.0")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.1")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-core")
    implementation("androidx.compose.animation:animation")
    implementation("androidx.documentfile:documentfile:1.1.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
    implementation("com.google.zxing:core:3.5.3")
    val camerax = "1.4.2"
    implementation("androidx.camera:camera-core:$camerax")
    implementation("androidx.camera:camera-camera2:$camerax")
    implementation("androidx.camera:camera-lifecycle:$camerax")
    implementation("androidx.camera:camera-view:$camerax")
    debugImplementation("androidx.compose.ui:ui-tooling")
}
