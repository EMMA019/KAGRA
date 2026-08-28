# Result

## What landed

kagra-shared（wgpu 30）に **FXAA** を追加。Bloom composite の出力（sRGB）に
輝度エッジ検出 + 勾配方向ブレンドを適用してから最終ターゲットへ。HUD は
FXAA 後に重ねるので UI の色は変わらない。

- `Renderer::set_fxaa(bool)` 公開（デフォルト有効）
- offscreen example は `--no-fxaa` で比較可能
- crest の FXAA あり/なしで diff 12014 px（エッジが滑らかに）

## Commands

```text
cargo test -p kagra-shared --lib
# 358 passed; 0 failed

cargo test -p kagra-shared --features render --test offscreen_render
# 12 passed（new: fxaa_smooths_edges_with_intermediate_colors）

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

target/debug/examples/offscreen.exe 960 540 scratch/crest_fxaa.png world kagra-shared/tests/fixtures/crest_isle_world.json
# FXAA あり/なしで diff 12014 px / max 263

python -m kagra.play_world kagra-shared/tests/fixtures/crest_isle_world.json --seconds 4
# ok（回帰なし）
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json
# ok
```

## Try

```text
# FXAA あり / なし の比較 PNG
python -m kagra.render_world kagra-shared/tests/fixtures/crest_isle_world.json scratch/crest_fxaa.png
python -m kagra.render_world kagra-shared/tests/fixtures/crest_isle_world.json scratch/crest_nofxaa.png --no-fxaa
```

## Files

- `kagra-shared/src/render/fxaa.wgsl`（新規）— FXAA 3.11 風簡易版
- `kagra-shared/src/render/bloom.rs` — composite_tex + FXAA パス + set_fxaa
- `kagra-shared/src/render/mod.rs` — apply 呼び出し / set_fxaa / COPY_DST
- `kagra-shared/examples/offscreen.rs` — `--no-fxaa`
- `kagra-shared/tests/offscreen_render.rs` — FXAA 統合テスト

## Stuck（= ドキュメントの穴）

- **エッジ方向は「コントラスト強度」ではなく「勾配ベクトル」で選ぶ**。
  強度比較は縦/横エッジの 1px 横断で同値になり、エッジに沿う方向へブレンド
  して効かない。
- **GPU を直接触るテストは `GPU()` ミューテックスを取る**（並列実行で
  デッドロックする）。
- ACES 後の白は ~232。エッジ中間色の判定範囲は 50..220 など「白を除外」する。
