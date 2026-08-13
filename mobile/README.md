# KAGRA Mobile & Wasm

共有コアは **`kagra-shared`**（C ABI + wasm-bindgen）。Python `kagra-core` とは別クレート。
wgpu の 2D レンダラを内蔵しているので、Android / iOS / Web で**同じコードが同じ絵を描く**。

## 構成

| パス | 内容 |
|------|------|
| `kagra-shared/` | Rust 共有 lib（session / input / scene / render / FFI / wasm） |
| `kagra-shared/src/scene.rs` | 画面内容の記述。GPU 非依存なので単体テストできる |
| `kagra-shared/src/render/` | wgpu 2D（`--features render`） |
| `kagra-shared/examples/offscreen.rs` | 画面なしで 1 枚焼いて PNG に出す |
| `kagra-shared/www/` | ブラウザ用ページ |
| `mobile/android/` | Gradle アプリ + JNI（SurfaceView に描画） |
| `mobile/ios/` | SwiftPM（`KagraShell`）+ `App/KagraIOSApp.swift`（CAMetalLayer に描画） |
| `scripts/build_wasm.sh` | wasm32 ビルド |
| `scripts/build_android_native.sh` | NDK `.so` 配置 |

## まず絵が出ることを確認する（GPU のある PC で完結）

```bash
cargo run -p kagra-shared --features render --example offscreen
# → scratch/shared_offscreen.png
```

実機やブラウザを用意せずにシーンとシェーダを確認できる。同じ判定は
`cargo test -p kagra-shared --features render` にも入っている（GPU が無い環境では自動スキップ）。

## Wasm

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack          # 未導入なら
./scripts/build_wasm.sh          # features = wasm,render
python -m http.server -d kagra-shared/www 8000
# → http://localhost:8000
```

WebGPU が無いブラウザでも WebGL2 バックエンドで動く。ドラッグと WASD で四角が動く。

## Android

```bash
# 1) native（--features render 付きで .so を作る）
./scripts/build_android_native.sh   # needs cargo-ndk + NDK

# 2) APK
cd mobile/android
gradle :app:assembleDebug           # Android Studio / SDK が必要
```

`libkagra_shared.so` が無い ABI では JNI スタブで起動し、描画は諦めて手順を画面に出す。
描画経路は `SurfaceView` → `ANativeWindow` → `kagra_shared_attach_android_surface`。

## iOS

```bash
cd mobile/ios
swift build          # CLI / ライブラリ（stub.c でリンク）
swift test
```

`stub.c` は描画できないので、`swift build` はあくまで API の整合性チェック。
実機で絵を出すには:

1. `cargo build -p kagra-shared --release --features render --target aarch64-apple-ios`
2. できた `libkagra_shared.a` を Xcode の App ターゲットにリンク（`stub.c` は外す）
3. `App/KagraIOSApp.swift` を App ターゲットに追加

描画経路は `UIView`(CAMetalLayer) → `kagra_shared_attach_ios_view`。

## wgpu のバージョン

`kagra-shared` は **wgpu 30**、Python 拡張の `kagra-core` は **wgpu 0.19** を使う。
別クレートなので混在しても問題ない。共有コアを新しい世代にしているのは、
古い wgpu の WebGPU バックエンドが今のブラウザが削除した limit を要求して
device 作成に失敗するため（実測で確認済み）。

共有コアでは **dx12 バックエンドを無効化**している。wgpu-hal 30 が要求する
gpu-allocator 0.28 が windows 0.62.2 でコンパイルできないため。Windows では
Vulkan が使われる。共有コアの実行先は Android / iOS / Web で、Windows は
オフスクリーン検証専用なので実害はない。

## シェル共通 API

```text
create / destroy
create_surface(w,h)
set_asset_root(path)
push_pointer / set_pad
request_frame → stats_json
pause / resume

attach_android_surface(ANativeWindow*) / attach_ios_view(UIView*) / attachCanvas(canvas)
attach_offscreen(w,h)        # 自己診断
render                       # request_frame の後に呼ぶ
has_renderer / detach_surface
```

座標系はどのプラットフォームでも**ピクセル**（左上原点）。シェル側は
`devicePixelRatio` や `contentsScale` を掛けてから渡すこと。`render` feature 無しで
ビルドした lib でも描画系のシンボルは存在し、`-1` と `last_error()` で理由を返す。
