# Prompt

ロードマップのレンダリングスライス: **FXAA（ジャギー除去、シェーダー1つ）** を
kagra-shared（wgpu 30）に追加してください。

方針:
- Bloom composite の出力（sRGB）を中間テクスチャに書き、FXAA（輝度エッジ検出 +
  方向ブレンド）で最終ターゲットへ。HUD は FXAA の後に重ねる（色を保つ）
- `Renderer::set_fxaa(bool)` を公開。デフォルト有効
- 既存の play_world / Crest / emma walker / offscreen を壊さない
- オフスクリーンテストで「エッジに中間色が現れる」ことを確認し、
  cargo test / clippy / verify をパスさせ、docs/agent-runs/ にログを残す
