# Result — 旧エンジン分離（old/）

## 実施内容

- `kagra-core/`（wgpu 0.19 Rust クレート）→ `old/kagra-core/`（単体 workspace 化。
  pip ビルドは `old/kagra-core/Cargo.toml` を参照）
- 旧 RendererV2 デモ 28 本 + rules + 録画 → `old/examples/`（media/ 含む）
- 旧デモのアセット（open_world / relic_run）・旧 2D アーカイブ → `old/examples/`
- 旧 RendererV2 smoke シナリオ 11 件 → `old/examples/verify_scenarios/`
- `docs/archive/` → `old/docs-archive/`、`app/`・`template/` → `old/`
- 旧デモ検証テスト 6 本 → `old/tests/`
- 削除: `map/ models/ cache/ .vision-pm`（未追跡ゴミ）
- ルート workspace は kagra-shared のみ

## 橋渡し（`import kagra` を活かす）

- `kagra/` パッケージと `.pyd` は残置（0.19 API + 新モジュール同居）
- テストの `examples/vrm_*` / `kagra-core/src/*` / smoke シナリオ参照を
  `old/` パスへ（19 + 4 + 2 箇所）。conftest が `old/examples` を sys.path に
- AGENTS.md / .github/workflows/ci.yml / docs/PUBLISHING.md / README 更新

## verify

- pytest tests -m "not golden" 全パス（旧デモ検証 6 本は old/tests/ へ）
- `import kagra` OK / バニーガーデン・トルネコのヘッドレス実行 OK
- cargo test kagra-shared（render）383 + 12 パス / 旧 kagra-core 単体ビルド OK
- `gen_api_index.py --check` OK
