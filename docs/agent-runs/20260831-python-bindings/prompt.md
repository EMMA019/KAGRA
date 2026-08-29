# Prompt

「0.30 を Python に」の第一歩: kagra-shared（wgpu 30）に PyO3 バインディングを
追加して、**Rust の機能を全部 Python から呼べる**ようにしてください。

方針:
- feature "python"（pyo3 + render）で分離。wasm / Android / iOS に影響させない
- 公開対象: WorldDoc（dump 読み書き）、WorldPlay（tick / 入力 / dump / 接着 API）、
  render_world_doc（dump → RGBA オフスクリーン）
- Python がゲームループを所有できるように（Python ゲームマスター）
- maturin で kagra-shared を Python 拡張としてビルドし、import して実測確認
- 既存の lib 370 / offscreen 12 テスト、clippy、wasm ビルドを壊さない
