# KAGRA Mobile & Wasm

共有コアは **`kagra-shared`**（C ABI + wasm-bindgen）。Python `kagra-core` とは別クレート。

## 構成

| パス | 内容 |
|------|------|
| `kagra-shared/` | Rust 共有 lib（session / input / assets / FFI / wasm） |
| `kagra-shared/www/` | ブラウザ煙テスト HTML |
| `mobile/android/` | Gradle アプリ + JNI |
| `mobile/ios/` | SwiftPM（`KagraShell`）+ `App/KagraIOSApp.swift` |
| `scripts/build_wasm.sh` | wasm32 ビルド |
| `scripts/build_android_native.sh` | NDK `.so` 配置 |

## Wasm

```bash
rustup target add wasm32-unknown-unknown
./scripts/build_wasm.sh
# 任意: wasm-pack → kagra-shared/www/pkg を静的サーバで開く
```

## Android

```bash
# 1) native
./scripts/build_android_native.sh   # needs cargo-ndk + NDK

# 2) APK
cd mobile/android
./gradlew :app:assembleDebug       # Android Studio / SDK が必要
```

`libkagra_shared.so` が無い ABI では JNI スタブで起動し、画面に手順を表示します。

## iOS

```bash
cd mobile/ios
swift build          # CLI / ライブラリ（stub.c でリンク）
swift test
```

Xcode で iOS App を作る場合は `App/KagraIOSApp.swift` を App ターゲットに追加し、
本番では `stub.c` の代わりに `cargo build --target aarch64-apple-ios` の `libkagra_shared.a` をリンクします。

## 最小 API（シェル共通）

```text
create / destroy
create_surface(w,h)
set_asset_root(path)
push_pointer / set_pad
request_frame → stats_json
pause / resume
```
