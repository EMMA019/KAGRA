# Result

## What landed

kagra-shared（wgpu 30）に PyO3 バインディングを追加し、**Python から Rust の
機能を呼べる**ようにした。Python ゲームマスター（Python がループを所有）の橋。

- `WorldDoc`: dump JSON の読み書き（from_json / to_json / player_position /
  props_named）
- `WorldPlay`: tick / set_input / dump / anim + 接着 API（emit_event /
  take_events / start_timer / step_interact）
- `render_world_doc`: dump → RGBA8 オフスクリーン描画

## Commands

```text
# ビルド（kagra-shared ディレクトリ内で）
cd kagra-shared && maturin develop --release

# Python から
.venv/Scripts/python.exe -c "import kagra_shared as ks; print(ks.__version__)"

# 検証
cargo test -p kagra-shared --lib                    # 370 passed
cargo test -p kagra-shared --features render --test offscreen_render  # 12 passed
cargo clippy -p kagra-shared --all-targets --features python -- -D warnings  # ok
cargo check -p kagra-shared --target wasm32-unknown-unknown --features wasm,render  # ok
```

## Try

```python
import kagra_shared as ks

play = ks.WorldPlay.from_json(open("kagra-shared/tests/fixtures/interact_fish_world.json").read())
play.set_input(0.0, 1.0, False, False, False)   # W 前進
play.tick(1/60)
print(play.anim())                                # "walk"

play.set_input(0.0, 0.0, False, True, False)     # J
play.step_interact()                              # 水辺 → cast イベント
print(play.take_events("cast"))
play.start_timer("cast", 3.0, "bite")             # 3 秒タイマー
for _ in range(180): play.tick(1/60)
print(play.take_events("bite"))                   # ["bite"]

rgba = ks.render_world_doc(dump_json, 64, 64)     # オフスクリーン RGBA
```

## Files

- `kagra-shared/src/py.rs`（新規）— PyO3 モジュール
- `kagra-shared/Cargo.toml` — feature "python"
- `kagra-shared/pyproject.toml`（新規）— maturin 設定
- `kagra-shared/src/lib.rs` — `#[cfg(feature = "python")] pub mod py`

## Stuck（= ドキュメントの穴）

- `maturin develop -m <Cargo.toml>` はルート pyproject の module-name を読む。
  **kagra-shared ディレクトリ内で `maturin develop` を実行**すること。
- pyo3 は native のみ。wasm ビルドには入らない（feature 分離済み）。
