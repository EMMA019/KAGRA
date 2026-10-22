# 20261022 genre audio + flow

master は PR #128（`36cd2cb`、Windows / Android 配布）まで進んでいた。作業ブランチは `cursor/genre-audio-flow-0cb5`。共有ファイルは追加だけにして、WorldPlay に新しいジャンルモジュールは足していない。

## 決めたこと

- 音の状態機械は `kagra-shared/src/mixer.rs`（純ロジック）。デスクトップ再生は `playback` feature の rodio 0.20（`features = ["wav"]`。rodio に `playback` feature は無い）。`python` feature が playback を有効にする。既定の `cargo test` / clippy は rodio をコンパイルしない（shared-lib CI に ALSA が無い）。
- wasm と Android は rodio を外す。Android は `cues_json` を JNI `takeAudioCues` で取り、`AudioBus`（SoundPool + ループ MediaPlayer）が `filesDir/kagra_root/assets/audio` を鳴らす。短いバッファの先読みでキューを消さないよう、1 回目は `hold_cues` に残す。
- デバイスが無い、`KAGRA_AUDIO=0`、`cargo test` ではスピーカーを開かない。`try_default` の失敗は無音。
- クリップは `assets/audio/{jump,fire,card,bgm}.wav`。自作の短い合成音で、`LICENSE.txt` は CC0。第三者の録音は入れない。
- フローは `kagra-shared/src/flow.rs`。Title / Play / Pause / Result。音量とハイスコアは JSON version 1。Windows は `%APPDATA%/kagra`、Android は Activity が `filesDir/kagra` を `set_save_root` する。それ以外は `$XDG_DATA_HOME/kagra` か `~/.local/share/kagra`。`KAGRA_SAVE_DIR` が優先。`cfg(test)` ではルートを渡さない限り書かない。
- フローを載せるのは `box_hop` だけ（`uses_flow`）。他ジャンルの HUD とテストは変えない。`start()` は自動で Play にしない（既存テストが `start()` のあと tick する）。`confirm` / Retry が Play と BGM に入る。
- 効果音はジャンプ（接地中の jump 入力）、FPS の弾を消費した発射、カードのプレイ成功。
- Python `kagra.audio` は従来の合成 API を残し、BGM / SFX / グループ / ミュートを足した。テストは拡張を import しない。`kagra.flow.save_dir` は同じパス規則の純 Python。
- パッケージの既定は Box Hop。`games/platformer_minimal.json` と fps / card に `bundle_assets: ["assets/audio"]`。
- タイトル文字が旧バナーのボタンと重なったので、フローがタイトルの帯と START を描き、Play 以外ではジャンル側の帯を消す。
- `attach_renderer` は運転メッシュを先に載せる。World 描画がそのリストのまま glTF を `update_mesh` すると id 16 で落ちた。World の初回描画でメッシュを捨てて `compile_meshes` を載せる。
- Python のオフスクリーンはレンダラをプロセス内に残す。別の dump を続けるとメッシュ数が合わず同じエラーになる。`WorldDoc::gpu_mesh_key` が変わったときだけ載せ直す。

## 躓き

- ローカル master は #128 より古かった。`origin/master` を取ってから分岐した。
- rodio 0.20 に `playback` feature は無い。`wav` だけにした。
- `std::env::var` を `Fn(&str) -> Option<String>` に渡すと HRTB で落ちる。クロージャにした。
- タイトルの「SPACE / TAP START」とボタンラベル START が同じ y で重なった。ヒントをボタンの下に移した。
