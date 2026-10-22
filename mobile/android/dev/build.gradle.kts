import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// CI writes this. A public Pages URL is not a secret. Empty falls back to this repo.
val pagesProps = Properties()
val pagesFile = rootProject.file("kagra.pages.properties")
if (pagesFile.exists()) {
    pagesFile.reader(Charsets.UTF_8).use { pagesProps.load(it) }
}
val pagesBase = (pagesProps.getProperty("pages_base") ?: "https://emma019.github.io/KAGRA")
    .trim()
    .replace("\\", "")
    .replace("\"", "")

android {
    namespace = "dev.kagra.shell"
    compileSdk = 35
    // app モジュールと同じ NDK r29。libc++_shared.so を 16 KB アラインで同梱する。
    ndkVersion = System.getenv("KAGRA_ANDROID_NDK_VERSION") ?: "29.0.14206865"

    defaultConfig {
        applicationId = "dev.kagra.devshell"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        buildConfigField("String", "PAGES_BASE", "\"$pagesBase\"")

        externalNativeBuild {
            cmake {
                cppFlags += "-std=c++17"
                arguments += listOf("-DANDROID_STL=c++_shared")
            }
        }
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    buildFeatures {
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/cpp/CMakeLists.txt")
            version = "3.22.1"
        }
    }

    // 非圧縮の .so を 16 KB zip 境界に置く。ゲーム APK と同じ。
    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
    }

    sourceSets {
        getByName("main") {
            // The import of java.util makes a bare `java` the package, not this source set.
            with(this.java) {
                srcDir("../app/src/main/java")
                // AGP's Kotlin DSL source set has no exclude(); it lives on filter.
                filter.exclude("**/MainActivity.kt")
            }
            jniLibs.srcDir("../app/src/main/jniLibs")
        }
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.activity:activity-ktx:1.9.2")
}
