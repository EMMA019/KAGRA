# Result — Phase 4 完了（音 DSP）

汎用エンジン化ロードマップ Phase 4（リバーブ / ダッキング / クロスフェード）が
完了した。

## 設計

再生（winsound 等）は 1 音ずつしか鳴らせない。そこで「混ぜる・加工する」は
**WAV bytes → WAV bytes の純 Python DSP**（`kagra/dsp.py`）として実装し、
ゲームが合成してから 1 本で鳴らす。全て決定論的（乱数なし）。

| API | 用途 |
|---|---|
| `mix(*wavs, gains=...)` | 複数モノ WAV を重ねて 1 本に（BGM+SE を先に合成）。長さは最長に合わせる |
| `reverb(wav, *, roomsize, damping, wet)` | Schroeder リバーブ（コムフィルタ 4 + オールパス 2）。wet=0 はバイパス |
| `crossfade(a, b, seconds)` | BGM 切替の等パワークロスフェード（末尾と先頭を cos/sin で混合） |
| `duck(bgm, *, at, dur, amount, attack, release)` | SE が鳴る間 BGM を下げ、release で戻す |

## verify

- pytest: 598 パス（test_dsp.py 16 件: mix の和/gains/パディング/決定論、
  reverb の tail/wet=0 バイパス、crossfade の長さと混合領域、duck の
  減衰・回復・amount=0 バイパス）
- `gen_api_index --check` OK（Rust 変更なし）

## 次の山

Phase 5 — UI 成熟（メッセージウィンドウの自動改行 / スクロール、
選択肢のページ送り、バー/メーターの共通化）。バニーガーデンとトルネコの
HUD が `kagra.ui2d` に寄る。
