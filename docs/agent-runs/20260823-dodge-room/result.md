# Result — Meteor Dodge（独立エージェントによる初回検証）

## 成果物

| パス | 役割 |
|---|---|
| `examples/vrm_dodge_room.py` | ゲーム本体（公開 API のみ） |
| `examples/dodge_room_rules.py` | 落下・当たり判定・難易度カーブ（GPU 不要） |
| `examples/verify_scenarios/dodge_room_smoke.json` | ヘッドレス検証シナリオ |
| `tests/test_dodge_room.py` | ルール8件 + 私用 import 禁止テスト |

## テスト

- `pytest tests/test_dodge_room.py` — 8/8 パス（1回失敗→修正→再実行、詳細は
  `session.md` 躓き1）
- `pytest tests -m "not golden"` — リポジトリ全体で166件パス（既存分は
  無傷）
- `python tools/gen_api_index.py --check` — ドリフトなし（新規APIを
  発明していない証拠）

## verify

**この環境では未実行**。`kagra_core`のRustツールチェーン自体が無く、
`maturin develop`まで到達できない。前2回（heart-catch, switch-room）と
同じ壁だが、今回はビルドツールチェーンの不在まで確認できた分、原因の
切り分けがより明確。シナリオは置いたので、GPU環境で
`python -m kagra.verify examples/verify_scenarios/dodge_room_smoke.json`
を実行すれば閉じられるはず。

## 往復数

1。プロンプト → 既存2本のパターン調査 → API検索 → 実装 → テスト失敗 →
修正 → 全体テスト → ドリフトチェック。GPU verifyは次の往復（ローカル環境）。

## この結果が示すこと（Wedge D の外部検証として）

- あなたが指示していないエージェント（このchat自身、Cursor/Grok不使用）が、
  `AGENTS.md`だけを読んで、既存の躓きログを参考にしつつ、新しい躓きも
  出しながら、公開APIのみでpytestが通る新規ゲームを1本作れた。
- ただし「誰の指示も受けずに」ではない — heart-catch/switch-roomの
  セッションログを読める状態だった点は、真っさらな第三者の再現性とは
  厳密には異なる。真の外部検証には、これらのログを読んでいない人物/
  エージェントによる試行が別途必要。

## セッション後に issue 化すべき穴

1. 難易度定数（`fall_speed`, `spawn_gap`の係数）は未調整。実プレイでの
   バランス確認が要る。
2. `test_*_uses_only_public_imports`系のテストを毎回コピペで書くのではなく、
   「そのファイルが実際にimportしている公開API名を自動抽出して検証する」
   共通ヘルパーにできると、今回のような差し替えミスが構造的に防げる。
3. `Physics3D.on_collide`を使う版のMeteor Dodgeを別途作れば、
   「ルール関数で判定」と「物理エンジンのコールバックで判定」の
   2スタイルの比較ができる（将来のエージェント向けドキュメントの材料になる）。
