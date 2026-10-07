plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "id.ducad.studio"
    compileSdk = 35

    defaultConfig {
        applicationId = "id.ducad.studio"
        minSdk = 26
        targetSdk = 35
        versionCode = (project.findProperty("ducadVersionCode") as String?)?.toInt() ?: 1
        versionName = (project.findProperty("ducadVersionName") as String?) ?: "0.3.0"
        ndk { abiFilters += listOf("arm64-v8a") }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    // libducad_android.so + libc++_shared.so ditulis `cargo ndk -o` ke sini.
    sourceSets["main"].jniLibs.srcDirs("src/main/jniLibs")
    packaging { jniLibs.useLegacyPackaging = false }
}

dependencies {
    implementation("androidx.games:games-activity:3.0.5")
    implementation("androidx.core:core-ktx:1.15.0")
    // GameActivity 3.x mewarisi AppCompatActivity.
    implementation("androidx.appcompat:appcompat:1.7.0")
}
