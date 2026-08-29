# Result — 空間音響スライス

## 成果物

- `kagra/audio.py` — `set_listener` / `play_se(x, y, z, volume,
  ref_distance, max_distance)` / `_spatialize`（モノ→ステレオ WAV + 左右
  ゲイン）。`kagra.spatial.spatial_mix`（0.19 の純 Python 実装）を使用。
- `tests/test_audio.py` — 12 件（減衰・パン・ステレオ・リスナー）。

## verify

- pytest tests/test_audio.py 12 件パス。
- 例: `set_listener(0,0,0, 0,0,1)` のとき `play_se("coin", x=5, y=0, z=0)`
  は右スピーカー寄り、`x=100` は無音（max_distance=48）。

## 使い方（ゲーム側）

```python
from kagra.audio import set_listener, play_se

set_listener(px, py, pz, 0, 0, 1)   # プレイヤー位置 + 前向き
play_se("coin", x=src_x, y=src_y, z=src_z)  # 距離減衰 + パン
```

## Phase 0 の進捗（このスライスまで）

- ⑦ master マージ: **解決済み**（master == origin/master、README 実態一致）
- ④ VRMC_node_constraint: **実は適用済み**（apply_constraints が
  sample_locals に配線済み。ロードマップの「解析のみ」は古い情報）
- ② 空間音響: **本スライスで実装** ✅
- 残り: ① 袖ヘルパーボーン（kagra-core vrm.rs ensure_sleeve_cloth の移植）
  ⑤ firstPerson 適用 ⑥ カプセル上端の坂浮き ③ TTS 音素同期 ⑧ Crest VRM 統合
