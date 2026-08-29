# old/ — 旧エンジン（アーカイブ）

ここは**過去のエンジン**です。新しいゲームはここから始めないでください。
新本線（shared wgpu 30）はリポジトリルート直下（`kagra-shared/` +
`kagra/` の新モジュール + `examples/` の新ゲーム）にあります。

| 場所 | 内容 |
|---|---|
| `kagra-core/` | 旧 Rust エンジン（wgpu 0.19 / RendererV2）。pip デモ（0.1.4）の本体 |
| `examples/` | 旧 RendererV2 デモ（vrm_open_world / orb_rush / relic_run / heart_catch / sing_dance 等）+ rules + 録画（media/） |
| `examples/archive/` | 旧 2D / タイルマップ / エディタのデモ |
| `docs/`（docs-archive） | 旧ガイド・旧 63% ロードマップ |

## 動かすには（pip デモ / 旧デモ）

`import kagra` は引き続き動きます（コンパイル済み `kagra/kagra_core.pyd`
が残っているため）。旧デモはこのフォルダから実行:

```bash
python -m kagra                                  # 歌って踊る（0.19 pip デモ）
python old/examples/vrm_open_world.py            # VRM Crest Isle（RendererV2）
python old/examples/vrm_orb_rush.py              # 参照ゲーム
```

旧拡張を再ビルドする場合:

```bash
cd old/kagra-core && maturin develop --release && cd ../..
```

## 注意

- ルートの Cargo workspace は `kagra-shared` のみ（kagra-core はここで
  単体 workspace としてビルド）。
- ルート `pyproject.toml` の pip ビルドは `old/kagra-core/Cargo.toml` を
  参照します（`import kagra` を活かすため）。
- 分離計画: リポジトリルートの `docs/REORG.ja.md`。
