# KAGRA ロードマップ — 需要の発見から「AI に体を与える標準」まで

最終更新: 2026-08-23（0.1.3 は PyPI 済み。絵作り 1a・初回体験・kairi 頭脳接続まで完了。
次は Stage 1 — 4 本の楔で需要の在り処を数字で確定する）

## 北極星

**「Python で AI に体を与えるなら KAGRA」という第一想起の座。**
JS における three-vrm のポジションの Python 版。

測るのは自分の投稿への反応（Push）ではなく Pull:

- 投稿していない日に install と「動いた」報告が来るか（オーガニック比率）
- 外部発のコンテンツ（記事・動画・配信）が毎月出るか

## 3 原則

1. **需要は完成品から逆流する。** 誰も「エンジンが欲しい」とは思っていない。
   「デスクトップに自分のアバターがいる」「寝てる間に AI が配信してる」が欲しい。
   売るのは完成品。ライブラリの需要はその裏から流れてくる。
2. **Push は検証手段であって需要ではない。** 投稿した日の star は反応。
   投稿していない日の流入が需要。
3. **楔は同時に打ち、勝者を数字で選ぶ。** どの痛みが本物かは事前に分からない。
   同じ労力で 4 本試し、引きのあった楔に全張りする。

## 何で戦うか（資産の棚卸し）

技術的な優位は「点」ではなく「縦の貫通」。

1. **統合密度 × 配布**: 依存ゼロ・約 5MB wheel で、glTF → MToon → SpringBone →
   リップシンク → TTS → OBS 仮想カメラが 1 プロセスに入る。この組み合わせは
   他に無い。競合は全部「Python → ブラウザ / Godot ブリッジ」
   （omnilimb-face、waifu-llm-vrm、AIAvatarKit は自前で 3D を描画しない）
2. **VRM 実装深度**: マルチスキン、コライダー付き SpringBone、node constraint、
   expression override、一人称レイヤー。「表示できます」系とは別物
3. **エージェント開発ループ**: `kagra.verify`（ヘッドレス検証）/ golden /
   `docs/API_INDEX.md` / MCP（`kagra_api_search` / `kagra_verify` / `kagra_render`）。
   開発ループ自体を AI エージェント用に設計したエンジンは他に無い

**弱み（正直リスト）**: 影の品質、ポスト処理の薄さ（bloom / vignette まで）、
複数アバター時の性能未検証、VRM の視錐台カリング未実装、バス係数 1、
汎用エンジン志向の名残（editor / tilemap / mapgen …）が広く浅い。

---

## Stage 0 — 現在地（完了）

- [x] 0.1.3 PyPI（Windows / Linux、約 5MB、Rust 不要）
- [x] VRM 0.x / 1.0 一式・リップシンク（WAV + VOICEVOX mora）・仮想カメラ・
      HUD・JSONL チャット・3D 物理・顔トラ
- [x] 絵作り（旧 Phase 1a）: 空・リム・showcase カメラ・接地・HUD。
      素の `python -m kagra` がデバッグ画面に見えない
- [x] 頭脳接続: `kagra.brain.KairiBrain` + `docs/recipes/ai-brain.md` +
      `examples/vrm_kairi_chat.py`（PR #25）
- [x] 初回体験: own-VRM 1 行、シャドウ警告、`kagra.cmd`、レシピ、
      issue テンプレ（bug / worked）、macOS wheel CI smoke

**機能は最初のユーザーを取るのに足りている。** ボトルネックは流通と、
「どの痛みが本物か」がまだ数字で分かっていないこと。

## 描画性能トラック（three.js と同じ種類の仕事。ブラウザ対決ではない）

北極星は変えない。Web で three-vrm と戦わない。デスクトップ wgpu で
three.js が当たり前にやっていることを一段ずつ足す。

- [x] P0: ワールド 3D の視錐台カリング（`draw_mesh_3d` / `draw_mesh_id`）。
      `kagra.render_stats()` / `set_mesh_cull()`。VRM はまだカリングしない
      （ボーン AABB 無しだとアニメでポップする）
- [ ] P1: VRM スキンのパッド付きボーン AABB
- [ ] P2: 箱 / 隕石の 3D インスタンシング（今の InstanceBatch は 2D 専用）
- [ ] P3: マテリアルソート / doubleSided のときだけ両面
- [ ] P4: 影・IBL の品質（弱みリスト）

## Stage 1 — 楔の同時検証（2 週間 × 最大 2 サイクル）

同じフォーマット（15〜30 行のスクリプト + 60 秒動画 + そのコミュニティの言葉で
書いた投稿文）を 4 本、別々の共同体に投げる。

| 楔 | 完成品の絵 | 投げ先 | 賭けている痛み |
|---|---|---|---|
| **A: ローカル LLM に体を** | Ollama + VRM が 15 行で喋る | r/LocalLLaMA、HN | 「俺の LLM に顔がない」 |
| **B: 無人 3D AI VTuber** | 頭脳 + 歌 + ダンスの配信画面 | X、ニコニコ、VTuber 圏 | 「Live2D でなく 3D で無人配信したい」 |
| **C: VRoid マスコット** | 自分の VRoid がデスクトップ常駐で喋る | X 日本語圏、VRoid 界隈 | 「作ったアバターの置き場がない」 |
| **D: エージェントが作るゲーム** | Cursor / Claude が API 検索 → 実装 → ヘッドレス検証 → 動くゲーム | AI コーディング界隈（X、HN） | 「エージェントは画面が見えない」 |

動画の作法（全楔共通）:

- 最初の 3 秒に一番いいカット。つかみに機能説明を置かない
- 最後は「`pip install kagra` / `python -m kagra`」の 2 行だけ
- README 先頭に 10 秒 GIF（動画リンクより先に絵を見せる）
- 楔 D は動画不要でもよい: エージェントに実際に作らせたログとリザルトが
  そのままコンテンツになる（このリポジトリ自体がエージェント開発の実証）
- [x] D-0 導線: ルート `AGENTS.md`、README エージェント節、`docs/agent-runs/`
- [x] D-1 参照実装の公開 API 化: Orb Rush から `_` 付き import を削除。
      `world_to_screen` / `set_position` / `set_yaw` / `texture_from_fn` /
      `tone` / ビルボード・床メッシュ / `save_json`
- [x] D-2 初回実証（ログ込み）: 一行プロンプト
      「3レーンでハートをキャッチ」→ `examples/vrm_heart_catch.py` +
      `docs/agent-runs/20260823-heart-catch/`。GPU verify はこの環境では
      `kagra_core` 未ビルドのため未実行（シナリオは置いた）
- [x] D-3 躓き直し: `ActionController` を公開 + `names()`、索引に
      2D/3D `world_to_screen`・`save_json`・`ensure_vrm` の注記、
      レシピ `docs/recipes/agent-game.md`
- [x] 棚分け: 推奨 examples を VRM / エージェントゲームに絞り、
      レガシー 2D は `examples/archive/`。API 索引を Front / Shelf に分割。
      kagra-shared は別の運転デモと明記（レンダラは統合しない）
- [x] 最小 3D ワールド面: `upload_mesh_3d` / `draw_mesh_id`、
      `World3D`（床 + 箱衝突）、`Camera3D.follow`
- [x] D-4 2本目のエージェント製ゲーム: 「箱部屋を歩いてスイッチを踏む」
      → `examples/vrm_switch_room.py` +
      `docs/agent-runs/20260823-switch-room/`
- [x] D-5 独立エージェントの 3 本目: 「降ってくる箱を避ける」
      → `examples/vrm_dodge_room.py` +
      `docs/agent-runs/20260823-dodge-room/`（Claude。Cursor/Grok ではない）

**判定**: 2 週間で「動いた」報告 3 件以上の楔が勝ち。全滅なら機能ではなく
**切り口の言葉**を変えて 1 回だけ再試行。全滅 × 2 サイクルで Stage 6 へ。

## Stage 2 — 勝者をコード不要の完成品に

真の需要の大半は「コードを書きたくない人」にある。対象を開発者から一段広げる。

- 勝ち楔を 1 コマンド + 設定ファイルに。VRM はドラッグ & ドロップ、
  頭脳は Ollama / kairi / OpenAI 互換を設定 1 行で切替
- **合格ライン**: README 通りに 10 分で「自分のアバターが自分の選んだ声で喋る」
- **本物のサイン**: プログラマ以外からの「動いた」報告が 1 件でも来ること

## Stage 3 — 旗艦の公開運用（動く広告）

勝ち楔の完成形を**自分で運用して見せ続ける**。デモを 24 時間営業の広告にする。

- 楔 B 勝ちの場合（旧 Phase 3/4 の技術タスクをここに吊るす）:
  - [ ] `kagra/autopilot.py` — 雑談 → 歌 → ダンス → 休憩ループ。
        話題は LLM に「前の発言」「時刻」「予定表 JSON」。沈黙 N 秒で自発
  - [ ] セットリスト規約 `setlist/song01/{voice.wav, dance.vrma, meta.json}`、
        曲間クロスフェード、待機ループ、`relax_hands` 点検
  - [ ] チャット取り込み（YouTube Live Chat / Twitch IRC を extra で。
        API キーはコアに入れない）
  - [ ] セーフティ — 発話前 NG（kairi 経由なら接地フィルタも通る）、
        トピック禁止、全発話ログ、キルスイッチだけ人間に残す
  - [ ] スケジューラ（枠の自動開始・終了）、配信をまたぐ軽量 JSON 記憶
  - [ ] NDI → ffmpeg パイプで RTMP 直送(OBS 不要化)
  - セーフティが済むまで無人の**公開**配信はしない。テストは限定公開
- 楔 C 勝ちの場合: 常駐マスコットの配布物（zip / installer）
- 楔 D 勝ちの場合: エージェント製ゲームのギャラリー + 作らせ方ガイド

**完了条件**: 旗艦（例:「この VTuber、中に誰もいません」のアーカイブ）経由の
流入が、投稿していない日にも来ること。

## Stage 4 — 需要のある場所に「体」を供給する

自前で需要を作るより、既に需要がある場所に部品として刺さる方が速い。

- LangChain / エージェントフレームワークの「体」ツール化
- Discord ボットに顔を付けるテンプレ
- AIAvatarKit のレンダラーバックエンド / Open-LLM-VTuber ユーザーの 3D 乗り換え口
- kairi は「事実を無人で喋らせるなら」の条件付き推奨として棚に並べる
- **本物のサイン**: 相手側エコシステムのドキュメントや会話に KAGRA の名前が出る

## Stage 5 — 複利(需要が需要を呼ぶ構造)

- 「動いた VRM」報告 → README ギャラリー掲載 → 投稿者が自分で拡散、のループ
- 設定・セットリストの共有形式。ユーザー作テンプレが次のユーザーの入口になる
- ブラウザデモ（kagra-shared / WebGPU）は裾野施策としてここで解禁
- **達成の定義**: オーガニック比率 50% 超、外部発の紹介コンテンツが毎月出る

## Stage 6 — 転進条件(正直な出口)

Stage 1 を 2 サイクル回して全楔で引きがなければ、「ネイティブ単独への需要は薄い」
が結論。単独プロダクトの拡大は畳み、供給側に専念する:

- AIAvatarKit の描画バックエンド / Open-LLM-VTuber の 3D フロントエンド
- エージェント向け検証基盤（verify / golden / MCP）の切り出し

Rust コアという堀はどの道でもそのまま資産になる。

## 計測(目標であって約束ではない)

| 指標 | Stage 1 終了 | Stage 3 終了 | Stage 5 |
|---|---|---|---|
| 「動いた」報告 | 3+（勝ち楔） | 10+（うち自作 VRM 5+） | 継続的 |
| GitHub Stars | 100 | 500 | 2,000 |
| PyPI 月間 DL | 500 | 3,000 | 10,000 |
| オーガニック比率 | — | 30% | 50%+ |
| 外部発コンテンツ | 1 | 3 | 毎月 |

## やらないこと

- 汎用 2D エンジンとして pygame / pyxel の土俵で競わない
- Live2D 対応(Open-LLM-VTuber の土俵に上がらない)
- Web レンダラで three-vrm と直接対決しない（ブラウザデモは Stage 5 の裾野施策のみ）
- torch / 重量級モデルをコア依存に入れない（5MB が最大の武器）
- text-to-vrma / Irodori-TTS のベンダリング（レシピで繋ぐ）
- kagra-core と kagra-shared のレンダラ統合
- YouTube / Twitch の API キーをコアやログに置くこと
- 比較表で競合を貶すこと
- デモへの高解像度テクスチャ / アセット同梱（絵作りはプロシージャル + ライトで）

## 決めごと

- **アセット**: WAV / VRMA は wheel に入れない
- **モデル規約**: デモ・動画は Alicia（クレジット付き）か自作 VRM。
  AvatarSample 系は pixiv 規約
- **無人配信**: Stage 3 のセーフティが済むまで公開しない
- **API キー**: 環境変数のみ
- **投稿の締め**: 毎回同じ 2 行（`pip install kagra` / `python -m kagra`）
- **正直さ**: 「◯◯はまだ無い」リストを README に残す。盛らない
