# Session — 空間音響（2026-09-01）

## 設計

- `kagra/spatial.py`（0.19 から既存の純 Python）の `spatial_mix` を
  `kagra/audio.py` から import（再発明しない）。
- `set_listener(x, y, z, fx, fy, fz, ux, uy, uz)` — 聞き手の位置と向き。
- `play_se(name, x, y, z, volume, ref_distance, max_distance)`:
  1. `spatial_mix` で gain / pan / left / right を計算（距離 0 → 無視で 2D）。
  2. `_spatialize` でモノ WAV をステレオ WAV にし、左右ゲインを焼き込む。
  3. `play_wav(stereo)` で winsound 再生。
- 2D の `se(name)` は従来どおりモノ。

## 検証

- pytest tests/test_audio.py: 12 件（距離減衰: 近=1.0 / 遠=0、
  パン方向: +X で右スピーカー / -X で左、ステレオ化: 左ゲイン 1.0・右 0
  なら右チャンネル無音、リスナー設定 + play_se が例外なし）。
