# Prompt

ロードマップのレンダリングスライス: **HDR + Bloom** を kagra-shared（wgpu 30）に
移植してください。kagra-core（wgpu 0.19）の `renderer/bloom.rs` と `BLOOM_SHADER`
を参考に、同じ「閾値ブルーム」を現行レンダラに載せます。

方針:
- 3D フレームは**線形 HDR**（Rgba16Float、トーン前）に描き、Bloom はトーン前に
  抽出・加算し、最後に exposure + ACES トーンマップ + sRGB エンコードを適用する
  （トーン後に抽出するとハイライトが 1.0 に圧縮されて Bloom が効かない）
- HUD は Bloom 合成後に重ねる（トーン後の色を変えない）
- 既存の play_world / Crest / emma walker / offscreen を壊さない
- `set_bloom(threshold, intensity)` を公開。intensity 0 = 見た目が変わらない
- オフスクリーンテストで「明るい物の周囲に光がにじむ」ことを確認し、
  cargo test / clippy / verify をパスさせ、docs/agent-runs/ にログを残す
