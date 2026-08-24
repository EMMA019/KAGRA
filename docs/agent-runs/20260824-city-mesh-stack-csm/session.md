# Session — 2026-08-24 city / mesh / stack / CSM

## 決断

- OSM は入れない。箱の街 JSON（`docs/schemas/city.json`）だけ。
- Rapier は入れない。三角形は静的。積み木は反復 + スリープ。
- CSM は 2 段。既定 1 段のまま（Prop Garden の画素を守る）。フィルム級の多段ではない。

## 実装

- `kagra/city.py` + `World3D.load_city`
- `Physics3D.add_trimesh` / `Prop(..., mesh_hit=True)` / `ramp_mesh`
- `add_box(..., is_static=False)` + `solver_iters` / sleep
- `set_shadow_cascades(2)` — depth array 2 層。近は視点、遠は今までの和

## 躓き

- Walk が毎フレーム vx を書く件は前回済み。積み木はカプセルを眠らせない。
- スキニングの vs_shadow と color の group 3 が同じモジュール。書き込みは 256 バイト ShadowU の vp0。
- 既定 1 段なら sample は layer 0 だけ。室内の絵は変えないつもり。
