# Prompt

ロードマップの「完全 MToon 移植」スライス: kagra-shared（wgpu 30）の thin MToon
を 0.19 の完全 MToon 相当に上げてください。

方針:
- kagra-core `mtoon.rs`（MtoonGpu）+ shaders.rs の MToon シェーディングを参考に、
  shader3d.wgsl の Toon マテリアルへ「影の 2 段階（shade 色 + toony + shift）、
  リムライト（色 + fresnel power + lift）、アウトライン（backface push-out）」
  を統合する
- VRM 1.0（VRMC_materials_mtoon）/ VRM 0.x（materialProperties）から shade /
  rim / outline パラメータを読み取って反映する
- matcap / normal テクスチャは次スライス（テクスチャの追加バインディングが要る）
- 既存の 358 lib + 12 offscreen テスト、clippy、verify をパスさせる
- play_world の VRM 主人公（Emma）の見た目が上がること、docs/agent-runs/ に
  ログを残すこと
