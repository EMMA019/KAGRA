# Android shell

共有コアは `kagra-shared`（C ABI）。起動シーンは **Crest Isle**（`set_scene(2)`）。
プレイヤーは Kenney 風カプセル。**VRM ではない。** Python `kagra-core` とは別レンダラ。

```bash
# リポジトリルートから
./scripts/build_android_native.sh   # cargo-ndk + NDK r28+。libkagra_shared.so を jniLibs へ
cd mobile/android
gradle :app:assembleDebug
```

NDK は r29（`29.0.14206865`、`scripts/setup_android_sdk.sh`）。16 KB ページ端末向けに
Rust / JNI は `max-page-size=16384` でリンクし、`libc++_shared.so` は NDK のものを同梱する。
`libkagra_shared.so` の SONAME は `libkagra_shared.so`（`build.rs` の
`cargo:rustc-cdylib-link-arg`）。JNI は `-Wl,-l:libkagra_shared.so` でリンクし、
`DT_NEEDED` にビルドマシンの絶対パスを書かない。パッケージしたゲームと
`dev.kagra.shell` は同じ `app/src/main/cpp/CMakeLists.txt` を使う。デバッグ署名は
`app/debug.keystore`（`android` / `androiddebugkey`）。

`libkagra_shared.so` が無い ABI では JNI スタブで起動し、画面にビルド手順を出す。
操作: 左＝歩き、右下＝ジャンプ、右上＝アクション。タップでタイトルから開始。

ジャンル例を選んでデバッグ APK にするときは、リポジトリルートで:

```bash
python scripts/package_android.py --game examples/platformer_minimal.py
```

`games/<例>.json` の title / icon / version / entry を Gradle とアセットに焼き、
`world` があるときは WorldPlay でその dump を進める。`kagra_game.json` が無い
起動（上の素の `assembleDebug`）は今までどおり Crest Isle。リリース署名と
itch.io への手動アップロードは [docs/distribution.md](../../docs/distribution.md)。

運転デモに戻すときは `KagraNative.setScene(0)`。

## KAGRA Dev

`applicationId` は `dev.kagra.devshell`。ゲームは同梱しない。起動画面で master か
PR 番号を選び、GitHub Pages のバンドル（world / assets / manifest）を落として
WorldPlay する。再読込ボタンか、上のバーを下に引くと取り直す。デバッグ APK は
Package Android の artifact `kagra-dev-apk`。手順は
[docs/preview.md](../../docs/preview.md)。
