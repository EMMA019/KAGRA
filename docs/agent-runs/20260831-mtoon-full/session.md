# Session

## 調査

- 0.19 `mtoon.rs`: MtoonGpu（shadeColor+toony / rimColor+fresnelPower /
  shift+lift+outlineWidth / outlineColor / matcap / uv_anim）。VRM 1.0
  （VRMC_materials_mtoon）と VRM 0.x（materialProperties）の両方をパース。
  `is_hair_material` / `boost_hair_rim` で髪のリムを自動強化。
- 0.30: `MtoonShade`（shade_color / toony / shift のみ）→ `InstanceRaw.mtoon`
  （location 9）→ shader3d.wgsl の Toon は half-Lambert 1 段階 + 固定フレネル。

## 実装の往復

1. `scene3d.rs` の `MtoonShade` を拡張（rim_color / rim_power / rim_lift /
   outline_color / outline_width）。GPU パック `gpu_rim` / `gpu_outline` /
   `gpu_shift_lift` を追加。
2. `gltf_load.rs` の `load_mtoon` を拡張: VRM 1.0 の parametricRim* /
   outline* と VRM 0.x の _Rim* / _Outline* をパース。
3. `InstanceRaw` に `mtoon2`（rim+power）/ `mtoon3`（outline+width）/
   `mtoon4`（shift+lift）を追加（location 10/11/12）。`GpuMesh` も同様に
   拡張し、`upload_mesh` / render_frame で受け渡し。
4. `shader3d.wgsl`: Toon に shift 反映とリム（色 + fresnel power + lift）。
   `vs_outline`（法線方向へ width 押し出し）+ `fs_outline`（単色）を追加。
5. `pipeline_outline`（カリング Front、深度 Less、HDR ターゲット）を追加し、
   render_frame の 3D パスで本体の後に outline_width > 0 の Toon メッシュを
   再描画。
6. テスト: `load_mtoon_parses_rim_and_outline_vrm1` / `_vrm0_rim_outline`
   （パース検証）、`emma_vrm_on_disk_loads_all_textured_parts` に「Emma の
   髪が rim か outline を持つ」アサートを追加。

## 最終状態

- MToon は影 2 段階（shade 色 + toony + shift）+ リム（色 + power + lift）+
  アウトライン（backface push-out）まで 0.19 相当に
- matcap / normal テクスチャは次スライス（テクスチャバインディング追加）

## 次（未実施）

- 表情プリセット（vrm_expression: smile / angry / sad / blink / aa の 1 つを
  適用する API。0.30 は blink / aa のみ）
- SpringBone 強化、matcap / normal テクスチャ
