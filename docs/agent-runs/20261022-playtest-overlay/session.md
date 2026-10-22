# 直してここ

## 決めたこと

- オーバーレイの実体は Rust `Playtest`（Android / Wasm の `SharedSession`）。Windows の Python ゲームは同じ定数・同じ zlib+base64 の `kagra/playtest.py`。`kagra/__init__.py` には出さない（API index を動かさない）。
- 既定はオフ。Wasm プレビューはコンストラクタでオン。Android は `BuildConfig.DEBUG` のとき、世界を読む前にオン。Windows zip は `frozen_launch` が `KAGRA_PLAYTEST=1` を既定にする。`0` を明示したリリースはオフ。
- 再現は「いまの dump に過去を足す」ではない。約 10 秒以内で一番古い WorldDoc チェックポイントを `from_json` し、`dump_t < t <= captured_t` の walk / confirm だけ、記録された `dt` で tick する。隙間は埋めない（長押しの 0.6 秒はイベントが無い）。
- 日本語の変換はビットマップフォントだけでは足りない。ブラウザは textarea、Android は EditText、Windows は tk の Text。Rust 側のパネルは lavapipe 用で、IME があるホストは `external chrome`。
- スクリーンショットはパネルを描く前のフレーム。URL には画像を入れない。6000 字を超える状態は `KAGRA-PLAYTEST-1` ブロックを貼る。
- トークンも有料サービスも使わない。issue は `template=playtest.yml&labels=playtest`。

## つまずき

- `"Z" * 40000` は zlib でほぼ消えて URL に入ってしまう。繰り返しの無い `k{i:08x}-` でオーバーフローを確認した。
- 有効化が `load_game_json` より先だと、t=0 のスナップが空の stats になる。同じ時刻のスナップは世界の dump で置き換える。
- serde_json は日本語を `\uXXXX` にする。Python は UTF-8 のまま圧縮する。両方とも相手のブロックを復号できればよく、バイト一致は要求しない。
