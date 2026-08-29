# Session

## 背景

0.19 の頃は Python（`kagra.run` + Scene.update）がゲームループを所有していた。
shared（wgpu 30）への移行でループ所有が Rust（WorldPlay）に移り、「ゲーム
ロジックを Python で書く」入口が無くなった。このスライスで橋を戻す。

## 実装の往復

1. `kagra-shared/Cargo.toml`: feature "python" = ["dep:pyo3",
   "pyo3/extension-module", "render"]。pyo3 0.21（kagra-core と同じ世代）。
2. `src/py.rs`（新規）: PyO3 モジュール `kagra_shared`。
   - `WorldDoc`: from_json / to_json / player_position / props_named
   - `WorldPlay`: from_json / tick / confirm / set_input / dump / anim +
     接着 API（emit_event / take_events / start_timer / step_interact）
   - `render_world_doc`: dump JSON → RGBA8（オフスクリーン）
3. `kagra-shared/pyproject.toml`: maturin 用（module-name = kagra_shared、
   features = ["python"]）。
4. **躓き 1**: `maturin develop -m kagra-shared/Cargo.toml` はルート pyproject
   （kagra-core 用）の module-name を読む → `PyInit_kagra_core` を探して失敗。
   修正: kagra-shared ディレクトリ内で `maturin develop --release` を実行し、
   その場の pyproject.toml を使う。
5. **躓き 2**: `project.license.file = "../LICENSE"` はプロジェクト外で不正 →
   `license = { text = "MIT" }` に。
6. 実測（.venv の Python）:
   - WorldDoc: crest dump 読み書き roundtrip ✅
   - WorldPlay: set_input(W) + 10 tick → anim "walk" ✅
   - 接着 API: J + step_interact → cast イベント → start_timer(3s, bite)
     → 180 tick → bite イベント ✅
   - render_world_doc: 64x64 → 16384 bytes、全画素非ゼロ（島が描画）✅

## 次（未実施）

- `kagra/__init__.py` からの再エクスポート（`import kagra; kagra.WorldPlay`）
- Python ゲームマスターのループ（Scene + kagra.run の shared 版）
- トルネコライクを Python 1 本で組む実演
