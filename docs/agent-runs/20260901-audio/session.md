# Session — 音スライス（2026-09-01）

## 設計

- `kagra/audio.py`（純 Python）:
  - `tone(freq, dur, vol, wave, rate, decay)` — 16-bit PCM mono WAV bytes。
    wave: sine / square / saw / noise。指数減衰エンベロープでクリック防止。
    **ノイズは決定的 xorshift**（同じ引数 → 同じ WAV。トルネコの再現性
    方針と整合）。
  - `sound(name)` / `se(name)` — プリセット（初回合成 + キャッシュ）:
    coin（2音）、jump、hit（ノイズ）、ok、bite、cast、hurt。
  - `play_wav(wav, loop)` — winsound（SND_MEMORY|SND_ASYNC、loop は
    SND_LOOP）。Windows 以外は no-op。失敗は静かに無視。
- WAV 生成は stdlib `wave` モジュール（BytesIO）。追加依存なし。

## 躓き

- なし（大きいものは無し）。tone() に書いたノイズ分岐の冗長な死にコードを
  1 箇所掃除した程度。
- テストで実際に鳴らさない（winsound は非同期再生のため）。合成のみ検証。

## 検証

- pytest tests/test_audio.py: 8 件（WAV 形式 / 長さ / 波形 / 決定性 /
  プリセット差分 / キャッシュ / coin 2音 / play_wav が例外を出さない）。
- サンプル `examples/python_game_minimal.py` に配線: cast → se("cast")、
  bite → se("bite")。ヘッドレス実行はイベント消費をしないので鳴らない。
- `pytest tests -m "not golden"` 全パス（588 件）。
