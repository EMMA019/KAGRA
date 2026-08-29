# Prompt — 共通コア移植スライズ③: 音

> 監査で「音（bgm/se/play_se/3Dパン/tone/sound）は 0.19 にしか無い」と
> 判明。3ジャンル（特にバニーガーデン）は SE / BGM が雰囲気に効く。

指示（実質）:

- `kagra/audio.py` に 0.19 の tone() / sound() を移植（純 Python 合成、
  WAV bytes を返す）。拡張非依存でテスト可能に。
- 再生は「シェル側 = Python 側」の原則: デスクトップは標準ライブラリ
  winsound（Windows）、他は no-op。wasm / mobile は各シェルが担当。
- プリセット SE: coin / jump / hit / ok / bite / cast / hurt 程度。
  合成は決定的（同じ引数 → 同じ WAV。トルネコの再現性と整合）。
- サンプルゲームに配線: cast 時 se("cast")、bite 時 se("bite")。
- 各スライスはコミット + テスト + ログを閉じる。
