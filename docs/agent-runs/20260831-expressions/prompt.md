# Prompt

ロードマップの「表情プリセット」スライス: kagra-shared（wgpu 30）に VRM の
表情プリセット（smile / angry / sad / joy / blink / aa 等）をダンプから選択
できるようにしてください。

方針:
- 0.19 `vrm_expression.rs` の override / isBinary の考え方は参考にしつつ、
  0.30 の薄い morph.rs（blink / aa のみ）を拡張する
- `walker.expression`（String）と `walker.morph`（重み）を dump に追加し、
  WorldPlay が interact / event / anim と連動して表情を切り替えられるように
- 既存の 360 lib + 12 offscreen テスト、clippy、verify をパスさせる
- play_world の Emma で表情が変わること、docs/agent-runs/ にログを残すこと
