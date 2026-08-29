# Result — Phase 6 完了（セーブ深化）

汎用エンジン化ロードマップ Phase 6（オートセーブ / スロット複数 /
バージョン付きマイグレーション）が完了した。

## 設計

バニーガーデンとトルネコがそれぞれ独自に書いていた JSON セーブを共通
`kagra/save.py` に寄せた。全て拡張非依存（テストは tmp_path で実ファイル検証）。

| API | 用途 |
|---|---|
| `save_data(path, data, *, version=1, backup=True)` | `{"version":N,"data":...}` をアトミック保存。直前内容を `.bak` に残す（オートセーブ保険） |
| `load_data(path, *, version, migrations, default)` | 読み込み + マイグレーション。旧形式（version キー無しの生 dict）は version 0 として読む |
| `migrate_data(data, from, to, migrations)` | `migrations[v]`（v→v+1）を順に適用。変換が無ければそのまま（壊れたセーブを出さない） |
| `atomic_write(path, text)` | tmp に書いて `os.replace`。途中で落ちても旧ファイル無傷 |
| `SlotStore(dir, *, count, version, migrations)` | 複数スロット（`slot_N.json`）。save/load/delete/slots/latest |

## 配線

- bunny_garden / torneko の `_save` / `_load` を `save_data` / `load_data`
  （version=1）に置換。セーブ形式は `{"version":1,"data":{...}}` に変わるが、
  旧形式のセーブは version 0 として自動ロード（後方互換）。

## verify

- pytest: 617 パス（test_save.py 11 件: 往復 / 欠損 / 破損 / 旧形式 /
  マイグレーションチェーン / 欠落ステップ停止 / アトミック / .bak /
  スロット save/load/latest / クランプ+削除 / スロットマイグレーション）
- torneko ヘッドレス: `--seed 12345 --turns 200` 2 回 → save MD5 一致
  （version 付き形式でも決定論維持）
- `gen_api_index --check` OK（Rust 変更なし）

## 次の山

Phase 7 — ローカライズ（文字列テーブル + 言語切替）。バニーガーデンと
トルネコの UI 文字列を `kagra.i18n` に寄せ、`lang` で切替。
