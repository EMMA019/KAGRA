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
