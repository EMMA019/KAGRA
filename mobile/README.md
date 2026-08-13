# KAGRA Mobile & Wasm

共有コアは **`kagra-shared`**（C ABI + wasm-bindgen）。Python `kagra-core` とは別クレート。
wgpu の 3D + 2D レンダラを内蔵しているので、Android / iOS / Web で**同じコードが同じ絵を描く**。
既定のシーンは運転デモで、トラックで道を走る。

## 構成

| パス | 内容 |
|------|------|
| `kagra-shared/` | Rust 共有 lib（session / input / scene / render / FFI / wasm） |
| `kagra-shared/src/scene3d.rs` | 3D シーンの記述。カメラ・メッシュ・視錐台カリング |
| `kagra-shared/src/road.rs` | Catmull-Rom 経路・弧長・チャンク・LOD（GPU 非依存） |
| `kagra-shared/src/vehicle.rs` | 車両運動（自転車モデル）と追従カメラ |
| `kagra-shared/src/driving.rs` | 運転デモ。経路に沿った道と景物、HUD |
| `kagra-shared/src/save.rs` / `audio.rs` | セーブ JSON・設定・音声レベル（再生はシェル） |
| `kagra-shared/src/gltf_load.rs` | 最小 glTF（静的メッシュ）ローダ |
| `kagra-shared/src/scene.rs` | 2D の描画内容（HUD とタッチデモ） |
| `kagra-shared/src/render/` | wgpu（`--features render`）。3D パス → 2D HUD パス |
| `kagra-shared/examples/offscreen.rs` | 画面なしで 1 枚焼いて PNG に出す |
| `kagra-shared/www/` | ブラウザ用ページ |
| `mobile/android/` | Gradle アプリ + JNI（SurfaceView に描画） |
| `mobile/ios/` | SwiftPM（`KagraShell`）+ `App/KagraIOSApp.swift`（CAMetalLayer に描画） |
| `scripts/build_wasm.sh` | wasm32 ビルド + JS バインディング生成 |
| `scripts/build_android_native.sh` | NDK `.so` 配置 |

`scene3d.rs` / `vehicle.rs` / `driving.rs` は **GPU に触らない**。カメラ行列、視錐台
カリング、車両の挙動、ワールド生成はすべて純粋な計算なので、GPU の無い CI でも
検証できる。実際に絵が出るかは `tests/offscreen_render.rs` が受け持つ。

## まず絵が出ることを確認する（GPU のある PC で完結）

```bash
cargo run -p kagra-shared --features render --example offscreen
# → scratch/shared_offscreen.png（運転シーン）

cargo run -p kagra-shared --features render --example offscreen -- 640 360 demo.png 2d
# → 2D のタッチデモ
```

実機やブラウザを用意せずにシーンとシェーダを確認できる。同じ判定は
`cargo test -p kagra-shared --features render` にも入っている（GPU が無い環境では自動スキップ）。

## Wasm

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli   # 未導入なら（crate と同じバージョンで）
./scripts/build_wasm.sh          # features = wasm,render
python -m http.server -d kagra-shared/www 8000
# → http://localhost:8000
```

WebGPU が無いブラウザでも WebGL2 バックエンドで動く。矢印 / WASD、または画面下の
パッド（左＝ハンドル、右＝アクセルとブレーキ）で運転できる。

`wasm-pack` は使わない。0.13 系は cargo へ `--out-dir` を渡すが、cargo 側が
これを `--artifact-dir` に改名して nightly 限定にしたため、stable では通らない。

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
set_drive(steer, throttle, brake)   # steer は -1..1、他は 0..1
set_scene(kind)                     # 0 = 運転(3D)、1 = タッチデモ(2D)
request_frame → stats_json
save_json / load_json               # セーブ（ファイル I/O はシェル）
set_settings(vol, steer_sens, muted)
audio_json                          # engine/wind/brake レベル（再生はシェル）
pause / resume

attach_android_surface(ANativeWindow*) / attach_ios_view(UIView*) / attachCanvas(canvas)
attach_offscreen(w,h)        # 自己診断
render                       # request_frame の後に呼ぶ
has_renderer / detach_surface
```

座標系はどのプラットフォームでも**ピクセル**（左上原点）。シェル側は
`devicePixelRatio` や `contentsScale` を掛けてから渡すこと。`render` feature 無しで
ビルドした lib でも描画系のシンボルは存在し、`-1` と `last_error()` で理由を返す。

`set_pad` は運転にも繋がっている（左右がハンドル、上がアクセル、下がブレーキ）ので、
仮想スティックしか持たないシェルでもそのまま動く。連続値を扱えるなら `set_drive` を使う。

## 3D の約束ごと

- 右手系で **y が上**、奥行きは wgpu に合わせて **0..1**。glam の `camera::rh::proj::directx`
  を使う（`opengl` 系は -1..1 なので合わない）。
- 追従カメラは車体の後ろから前を見るので、**画面の右は -X**。`steer` の正は「運転席から
  見て右」で、これは `heading`（反時計まわりが正）が減る向き。
- インスタンスは頂点バッファで渡し、base instance は使わない。ストレージバッファも
  使わない。いずれも **WebGL2 に無い**ため。
- 3D と 2D HUD は同じレンダーパスで描く。HUD 側は深度テストを常に通し、書き込まない。

## 道路（S4）

- 経路は Catmull-Rom。弧長テーブルから「距離 s の位置と向き」を取り出す。
- ワールドは 80m チャンク。トラック前後だけを生かし、長く走ってもインスタンス数は増えない。
- LOD: 近くは道+中央線+ポール、中間は道+ポール間引き、遠くは道だけ。
- 道セグメントは非均一スケールしない（シェーダが法線を単純変換するため）。刻みを 8m 以下にして重ねる。

## 見た目とセーブ（S5 / S6）

- マテリアル種別（Solid / Road / Grass / Sky）をインスタンス属性で渡し、シェーダ側で
  手続きノイズやスカイグラデを付ける。外部テクスチャ必須にはしない。
- スカイは深度書き込み無し・カリング無しのドーム。地面影は半透明の平面インスタンス。
- 最小 glTF（POSITION + NORMAL + indices、data URI）を `gltf_load` で読める。見た目差し替え用。
- セーブは JSON のみ。シェルが localStorage / Documents / filesDir へ書く。
- 音声はレベルだけ返す。実再生は Web Audio / AudioTrack / AVAudioEngine。
