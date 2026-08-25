# Android shell

共有コアは `kagra-shared`（C ABI）。起動シーンは **Crest Isle**（`set_scene(2)`）。
プレイヤーは Kenney 風カプセル。**VRM ではない。** Python `kagra-core` とは別レンダラ。

```bash
# リポジトリルートから
./scripts/build_android_native.sh   # cargo-ndk + NDK。libkagra_shared.so を jniLibs へ
cd mobile/android
gradle :app:assembleDebug
```

`libkagra_shared.so` が無い ABI では JNI スタブで起動し、画面にビルド手順を出す。
操作: 左＝歩き、右下＝ジャンプ。タップでタイトルから開始。

運転デモに戻すときは `KagraNative.setScene(0)`。
