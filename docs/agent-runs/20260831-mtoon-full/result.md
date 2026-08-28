# Result

## What landed

kagra-shared（wgpu 30）の MToon を **0.19 相当の完全版**に移植。

- **影の 2 段階**: shadeColor + shadingToony + shadingShift（VRM 1.0 /
  VRM 0.x からパース）
- **リムライト**: rimColor + parametricRimFresnelPower + rimLift
- **アウトライン**: outlineColor + outlineWidth。backface push-out
  （`vs_outline` で法線方向へ押し出し、カリング Front の `pipeline_outline`）
- `MtoonShade` 拡張 + `InstanceRaw.mtoon2/3/4`（location 10/11/12）で GPU に
  受け渡し
- matcap / normal テクスチャは次スライス

## Commands

```text
cargo test -p kagra-shared --lib
# 360 passed; 0 failed
#   new: load_mtoon_parses_rim_and_outline_vrm1 / _vrm0_rim_outline
#   emma_vrm_on_disk… に「Emma 髪が rim or outline を持つ」アサート追加

cargo test -p kagra-shared --features render --test offscreen_render
# 12 passed

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json --seconds 4
# ok（回帰なし）
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json
# ok
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json
# VRoid 主人公: 影の境界がシャープ + 髪のリム + シルエットのアウトライン
```

## Files

- `kagra-shared/src/scene3d.rs` — MtoonShade 拡張 + gpu_* パック
- `kagra-shared/src/gltf_load.rs` — load_mtoon 拡張（VRM1/VRM0 の rim/outline）
- `kagra-shared/src/render/mod.rs` — InstanceRaw.mtoon2/3/4、pipeline_outline、
  render_frame のアウトライン draw
- `kagra-shared/src/render/shader3d.wgsl` — Toon 強化（shift/rim）、
  vs_outline / fs_outline

## Stuck（= ドキュメントの穴）

- インスタンス属性は location 9..12 まで拡張した（mtoon/mtoon2/mtoon3/mtoon4）。
  これ以上増やすなら UBO 化を検討。
- アウトラインは「法線方向の押し出し」なので、スキン変形後の法線に対して
  適用される（CPU skinning 済み Vertex3）。スカート等の薄いメッシュは
  押し出しで破綻しやすい → outlineWidth は VRM 側の値をクランプして使う。
