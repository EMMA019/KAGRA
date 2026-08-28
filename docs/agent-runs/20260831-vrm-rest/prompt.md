# Prompt

kagra-shared（wgpu 30）の VRM を残り 3 スライスで 0.19 相当に引き上げてください:

- **A: SpringBone 強化** — コリジョン付き Verlet（球/カプセル、複数連鎖、剛性/重力）。
  0.19 の vrm_spring.rs を薄い版（spring.rs、コリジョン無し）から強化
- **B: MToon の matcap / normal テクスチャ** — 追加バインディング + シェーダー適用
- **C: ボーン制約（VRMC_node_constraint）+ firstPerson（VRM firstPerson 注釈）**

既存の 363 lib + 12 offscreen テスト、clippy、verify をパスさせ、play_world の
Emma で髪が揺れて質感が上がること、docs/agent-runs/ にログを残してコミット・
プッシュすること。
