import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val gameProps = Properties()
val gamePropsFile = rootProject.file("kagra.game.properties")
if (gamePropsFile.exists()) {
    gamePropsFile.reader(Charsets.UTF_8).use { gameProps.load(it) }
}

fun gameProp(key: String, default: String): String {
    val raw = gameProps.getProperty(key)
    return if (raw.isNullOrBlank()) default else raw.trim()
}

val releaseKeystore = System.getenv("ANDROID_KEYSTORE_FILE")

android {
    namespace = "dev.kagra.shell"
    compileSdk = 35
    // NDK r28+（ここでは r29）は arm64-v8a / x86_64 を 16 KB アラインで出す。
    // libc++_shared.so もこの NDK のプリビルドを APK に入れる。
    // バージョンは scripts/setup_android_sdk.sh の NDK_VERSION と揃える。
    // CI は KAGRA_ANDROID_NDK_VERSION を渡す。
    ndkVersion = System.getenv("KAGRA_ANDROID_NDK_VERSION") ?: "29.0.14206865"

    defaultConfig {
        applicationId = gameProp("application_id", "dev.kagra.shell")
        minSdk = 26
        targetSdk = 35
        versionCode = gameProp("version_code", "1").toInt()
        versionName = gameProp("version_name", "0.1.0")
        manifestPlaceholders["appLabel"] = gameProp("title", "KAGRA Shell")
        manifestPlaceholders["appIcon"] = gameProp(
            "icon",
            "@android:drawable/sym_def_app_icon",
        )
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        externalNativeBuild {
            cmake {
                cppFlags += "-std=c++17"
                // c++_shared: NDK r28+ の libc++_shared.so（16 KB アライン）を同梱する。
                // c++_static にすると libc++_shared.so は消えるが、NDK を上げれば足りる。
                arguments += listOf("-DANDROID_STL=c++_shared")
            }
        }
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    if (!releaseKeystore.isNullOrBlank()) {
        signingConfigs {
            create("release") {
                storeFile = file(releaseKeystore)
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEY_ALIAS")
                keyPassword = System.getenv("ANDROID_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            if (!releaseKeystore.isNullOrBlank()) {
                signingConfig = signingConfigs.getByName("release")
            }
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
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

    // 非圧縮の .so を 16 KB zip 境界に置く。AGP 8.5.1+ が zipalign -P 16 する。
    // useLegacyPackaging true は圧縮してしまい、16 KB 端末で mmap できない。
    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.activity:activity-ktx:1.9.2")
}
