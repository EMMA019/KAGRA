# Prompt — Phase 0-②: 空間音響（距離減衰・定位）

> 汎用エンジン化ロードマップ Phase 0「新旧レンダラー差分を埋める」の ②。
> 0.19 の `set_listener` / `play_se(x=, y=, z=)`（Crest Isle 実装）が新しい
> audio.py に無いので移植する。キャラ/アセット制作とは別トラック。

指示（実質）:

- `kagra/audio.py` に `set_listener(...)` と `play_se(name, x, y, z, volume,
  ref_distance, max_distance)` を追加。
- 距離減衰 + ステレオパンは 0.19 の `kagra/spatial.py`（純 Python）を
  そのまま使う（逆二乗減衰 + equal-power パン、HRTF なし）。
- パンはステレオ WAV に左右ゲインを焼き込んで winsound で鳴らす
  （「シェル側 = Python 側」の原則のまま）。
- テスト: 減衰 / パン方向 / ステレオ化 / リスナー設定の 4 系統。
