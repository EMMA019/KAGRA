plugins {
    // 8.5.1+ zip-aligns uncompressed native libraries to 16 KB (zipalign -P 16).
    id("com.android.application") version "8.5.2" apply false
    id("org.jetbrains.kotlin.android") version "2.0.20" apply false
}
