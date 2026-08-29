# Result — 音スライス

## 成果物

- `kagra/audio.py` — `tone` / `sound` / `se` / `play_wav` / `preset_names`
  （純 Python 合成、決定的、winsound 再生、他プラットフォームは no-op）
- `tests/test_audio.py` — 8 件
- `examples/python_game_minimal.py` — cast / bite に SE 配線

## verify

- pytest 全パス / 合成は決定的（同じ引数 → 同じ WAV）
- サンプルはヘッドレスで正常（音は鳴らさないが、経路は配線済み）

## 使い方

```python
from kagra.audio import se, tone, play_wav

se("coin")                                  # プリセット
play_wav(tone(880, 0.08, wave="sine"))      # 任意トーン
```

## 次の山

④ 入力拡張（マウス位置 / クリック等）→ ⑤ 1 本目ジャンル（バニーガーデン系）。
