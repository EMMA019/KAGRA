# Result

## What landed

kagra-shared（wgpu 30）に **HDR フレーム + 閾値ブルーム** を移植。

- 3D パスは**線形 HDR**（Rgba16Float）フレームへ。`shader3d.wgsl` の fs_main から
  tone_map を外し、exposure + ACES は bloom composite が適用
- Bloom: extract（閾値 + ソフト膝）→ 半解像度ガウシアン H/V → composite
  （sharp + bloom*intensity → exposure → ACES → sRGB）。0.19 と同じ式
- HUD は composite 後に重ねる（トーン後の色を保つ）
- `Renderer::set_bloom(threshold, intensity)` 公開。intensity 0 = 絵は変わらない
- play_world と offscreen example はデフォルト bloom（0.85 / 0.35）、
  offscreen は `--no-bloom` で比較可能

## Commands

```text
cargo test -p kagra-shared --lib
# 358 passed; 0 failed

cargo test -p kagra-shared --features render --test offscreen_render
# 11 passed（new: bloom_spills_light_around_bright_quad）

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

target/debug/examples/offscreen.exe 960 540 scratch/crest_bloom.png world kagra-shared/tests/fixtures/crest_isle_world.json
# bloom あり/なしで diff 902 px（最大 20/255）＝コイン・水面ハイライトがにじむ

python -m kagra.play_world kagra-shared/tests/fixtures/crest_isle_world.json --seconds 4
# ok（回帰なし）
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json
python -m kagra.verify examples/verify_scenarios/interact_fish_smoke.json
# ok
```

## Try

```text
# 比較用 PNG（bloom あり / なし）
python -m kagra.render_world kagra-shared/tests/fixtures/crest_isle_world.json scratch/crest_bloom.png
python -m kagra.render_world kagra-shared/tests/fixtures/crest_isle_world.json scratch/crest_nobloom.png --no-bloom
```

## Files

- `kagra-shared/src/render/bloom.rs`（新規）— BloomPass（0.19 移植、wgpu 30 API）
- `kagra-shared/src/render/bloom.wgsl`（新規）— extract / blur / composite(+ACES)
- `kagra-shared/src/render/mod.rs` — 3D パスを HDR フレームへ、composite 後に HUD、
  `set_bloom` / resize
- `kagra-shared/src/render/shader3d.wgsl` — fs_main の tone_map 削除（リニア出力）
- `kagra-shared/examples/window.rs` / `offscreen.rs` — デフォルト bloom 有効
- `kagra-shared/tests/offscreen_render.rs` — bloom 統合テスト（3D 白 box）

## Stuck（= ドキュメントの穴）

- **トーン後抽出では Bloom が効かない**（ACES がハイライトを 1.0 に圧縮）。
  3D フレームは常に線形 HDR、トーンは必ず最終 composite で。
- HUD は Bloom 対象外（composite 後に重ねる）。HUD をフレームに混ぜると
  トーンされて色が変わる。
- `world_doc::compile_meshes` は heightfield 無し dump で MeshId が飛ぶ。
  `upload_compile_meshes` は dense 前提。個別メッシュは `upload_mesh`。
- **ファイル書き換えに PowerShell の Get/Set-Content を使わない**
  （日本語コメントが Shift-JIS 誤読で文字化けする）。edit ツールを使う。
