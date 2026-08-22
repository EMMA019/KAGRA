# KAGRA ロードマップ — バズるまで、そして全自動 AI VTuber まで

最終更新: 2026-08-22（0.1.2 リリース済み、VRM ギャップ 6 項目実装済み、ライブデモ動画あり）

North Star（北極星）は 2 つ。

1. **「pip install kagra → 2 コマンドで VRM が歌って踊る」を、カテゴリの合言葉にする**
2. **人間が一切関与しない全自動 AI VTuber を、KAGRA だけで動かす**

この 2 つは別々の山ではない。1 が裾野（インストール数・認知）、2 が頂上（誰も真似できないデモ）。
1 の途中成果物がそのまま 2 の部品になるように並べてある。

---

## 現在地（正直な棚卸し）

**持っているもの**

- `pip install kagra`（約 5MB、Rust 不要、Windows / Linux wheel）→ `python -m kagra` で歌って踊る
- VRM 0.x / 1.0: GPU スキニング、MToon（matcap / UV アニメ / ノーマルマップ込み）、SpringBone + コライダー、node constraint、表情 override、一人称、視線、まばたき
- モーション: BVH / FBX（Mixamo、指対応済み）/ VRMA（指・表情・LookAt 対応）、上半身レイヤー、クロスフェード、IK
- 音: WAV リップシンク（フォルマント母音推定・ループ）、内蔵ソング合成、`sing()` / `dance()` 1 行 API
- AI: `AiCharacter`（LLM 接続、感情推定、リップシンク連動）、顔トラッキング（facetrack extra）
- README にライブデモ動画、`--loop` / `--mascot`（配信・マスコット用途）

**足りないもの（このロードマップで埋める）**

- 配信出力（仮想カメラ / NDI / RTMP）— OBS に「映す」手段がウィンドウキャプチャしかない
- ライブチャット取り込み（YouTube / Twitch）
- TTS の公式レシピ（エンジン非同梱で、コピペで動く手順）
- モーション生成の公式レシピ（text-to-vrma 連携）
- 自律ループ（話題選択 → 発話 → 歌 → 休憩 を人間なしで回す頭脳）
- macOS wheel（検証手段ができるまで凍結中）
- ブラウザデモ（kagra-shared、凍結中）

---

## Phase 0 — 0.1.3 を出す（信頼の即金化）

merged 済みの実装をユーザーの手元に届ける。バズの前提は「試した人が最初の 5 分で成功すること」。

- [ ] `assets/cute_song_trial.wav`（2.9MB）と `.vrma` を wheel に同梱するか決める
      （「5MB で全部入り」の売り文句を守るなら sdist/wheel から除外し、初回 DL に回す手もある）
- [ ] バージョン 0.1.3 バンプ + CHANGELOG 整理（コライダー / constraint / override / 一人称 / MToon / テクスチャ / VRMA リターゲット修正 / FBX 指 / リップシンク強化）
- [ ] タグ → publish ワークフロー（Windows + Linux、sdist）
- [ ] リリースノートに Before / After の GIF（コライダー効果は貫通比較が一番わかる）

**完了条件**: `pip install -U kagra` だけで新機能が全部動く。

## Phase 1 — 「一撃デモ」を磨く（最初のバズ弾）

見た人が 10 秒で理解できる 1 本に全部賭ける。長い機能紹介は要らない。

- [ ] 実 WAV + 実 VRMA + コライダー + 表情で 30〜60 秒の動画を撮り直す
      （素材: Irodori-TTS で歌声 or 自作 WAV、text-to-vrma でダンス。どちらもエンジンには同梱しない）
- [ ] 動画の最後に必ず「`pip install kagra` / `python -m kagra`」の 2 行だけ映す
- [ ] X 初投稿（日本語）。文面は「Unity なしで Python だけ。VRM が歌って踊る。5MB」系の 1 メッセージ
- [ ] 同日に Reddit r/Python・r/VirtualYoutubers、Hacker News「Show HN」（英語 README が受け皿）
- [ ] ニコニコ / YouTube にも同じ動画（ニコニ立体ちゃん使用時はクレジット必須）
- [ ] README 冒頭に比較表: KAGRA vs Unity+UniVRM vs VSeeFace vs three-vrm
      （行は「インストール」「コード量」「ライセンス」「AI 連携」だけ。盛らない）

**完了条件**: 外部の誰かが動画経由で試して、issue か star が付く。

**バズの原則（全フェーズ共通）**

- 主語は「VRM が歌って踊る」であって「ゲームエンジン」ではない。カテゴリ名で戦う
- 毎回同じ 2 行（pip install kagra / python -m kagra）で締める。刷り込み
- 嘘をつかない。「◯◯はまだ無い」を README の正直リストに残し続ける（信頼が差別化）
- 競合を貶さない。比較表は事実のみ

## Phase 2 — 試した人を離さない（コンテンツと受け皿）

- [ ] `examples/` を「コピペで完結する 20〜30 行」に統一。各例の冒頭に完成 GIF
- [ ] 公式レシピ集 `docs/recipes/`:
      - 自分の VRM で歌わせる（VRoid Studio → KAGRA）
      - Irodori-TTS で歌声 WAV を作って `av.sing("voice.wav")`
      - text-to-vrma でダンスを作って `av.dance("dance.vrma")`
      - OBS でウィンドウキャプチャして配信に載せる（現状動く手順）
      - `--mascot` でデスクトップマスコット化
- [ ] GitHub Discussions を開く（Discord はコミュニティが 100 人を超えてから）
- [ ] issue テンプレ（VRM 名 / OS / GPU / ログの 4 点）
- [ ] 「動いた VRM 報告スレ」を用意（互換性の社会的証明が積み上がる）

**完了条件**: 初見が README だけで自分の VRM を歌わせられる。

## Phase 3 — AI VTuber v1「半自動」（人間は起動するだけ）

`AiCharacter` を核に、**起動したら放置で 30 分もつ**配信キャラを作る。まだ人間が起動・停止はする。

- [ ] **自律ループ**: `kagra/autopilot.py`
      - 状態機械: 雑談 → 歌 → ダンス → 休憩（アイドルモーション）→ 雑談…
      - 話題は LLM に「前の発言」「時刻」「予定表 JSON」を渡して生成
      - 沈黙が N 秒続いたら自発的に話す / 歌う
- [ ] **TTS 接続の公式化**: VOICEVOX（無料・ローカル・商用可）を第一候補に
      `AiCharacter(tts="voicevox")` を実レシピ化。Irodori-TTS は歌用
- [ ] **歌のレパートリー**: WAV + VRMA + 歌詞タイムラインの「セットリスト」フォルダ規約
      （`setlist/song01/{voice.wav, dance.vrma, meta.json}` を順に回す）
- [ ] **画面の作り込み**: ステージ・字幕（発話テキスト表示）・曲名オーバーレイを kagra の 2D API で
- [ ] **モーションの継ぎ目**: 曲間のクロスフェード、待機ループ、`relax_hands` の自動適用を点検

**完了条件**: `python -m kagra.autopilot` で 30 分、人間の操作ゼロで見ていられる。

## Phase 4 — AI VTuber v2「全自動」（人間はいない）

ここが個人的ゴール。**配信の開始から視聴者対応まで無人**。

- [ ] **配信出力**: 最重要の欠け。選択肢は
      1. 仮想カメラ（Windows: OBS Virtual Camera 互換 / pyvirtualcam 連携が最短）
      2. NDI 出力
      3. ffmpeg パイプで直接 RTMP（YouTube Live へ OBS なし送出）
      まず 1 を extra（`kagra[stream]`）で。3 ができると本当に無人になる
- [ ] **チャット取り込み**: YouTube Live Chat API / Twitch IRC を extra で
      - チャット → LLM 応答 → TTS → リップシンク の往復をレイテンシ込みで設計
      - 読み上げるコメントの選別（LLM でフィルタ + NG ワード辞書）
- [ ] **セーフティ層**: 無人だからこそ必須
      - 発話前フィルタ（NG 判定に落ちたら定型文に差し替え）
      - トピック禁止リスト、個人情報の反射的復唱の防止
      - 全発話ログ + ワンタッチ停止（キルスイッチだけは人間に残す）
- [ ] **スケジューラ**: 配信枠の自動開始 / 終了（YouTube API）、サムネ自動生成、配信タイトル生成
- [ ] **記憶**: 配信をまたぐ軽量メモリ（JSON で「昨日話したこと」を持つ。ベクタ DB はまだ要らない）

**完了条件**: 告知から配信終了まで人間の操作ゼロの枠が 1 本成立し、アーカイブが残る。
これ自体が最強のバズ動画になる（「この VTuber、中に誰もいません」）。

## Phase 5 — 裾野を広げる（エコシステム）

- [ ] macOS wheel（検証できる Mac が手に入ってから。無理に出さない）
- [ ] ブラウザデモ（kagra-shared / WebGPU）— 「インストールすら不要」の第 2 波
      ただしレンダラ統合はしない。デモ専用と割り切る
- [ ] プラグイン規約（`kagra.contracts` の拡張）: モーション / TTS / チャットソースを差し替え可能に
- [ ] テンプレートリポジトリ「my-ai-vtuber」: fork して VRM とセットリストを置くだけ
- [ ] VRoid Hub / BOOTH 文化圏への礼儀: 利用規約リンク集、クレジット自動表示ヘルパー
- [ ] WebP / KTX2 テクスチャ、VRMA 以外の表情アニメ、残りの正直リスト消化

## 計測（バズの定義を数字にする）

| 指標 | Phase 1 終了時 | Phase 3 終了時 | Phase 4 終了時 |
|---|---|---|---|
| GitHub Stars | 100 | 500 | 2,000 |
| PyPI 月間 DL | 500 | 3,000 | 10,000 |
| デモ動画再生 | 1 万 | 5 万 | （無人配信そのものが証拠） |
| 外部 issue/PR | 5 | 30 | 100 |

数字は目標であって約束ではない。ただし「動画を出したのに 2 週間で再生 1,000 未満」なら
動画の作り直しを機能追加より優先する。**バズの失敗は機能不足ではなく伝え方の失敗として扱う。**

## やらないことリスト（重要）

- torch / 重量級モデルをコア依存に入れない（5MB の pip install が最大の武器）
- text-to-vrma / Irodori-TTS のベンダリング（ライセンスと重量。レシピで繋ぐ）
- kagra-core と kagra-shared のレンダラ統合（別クレートのまま）
- Mac wheel の見切り発車
- 比較表で競合を貶すこと

## リスクと決めごと

- **アセット同梱サイズ**: `assets/` の WAV/VRMA が wheel に入ると 5MB を超える。0.1.3 で方針決定
- **モデル規約**: AvatarSample 系は pixiv 規約。デモ・動画は Alicia（クレジット付き）か自作 VRM に限定
- **無人配信の責任**: セーフティ層（Phase 4）が済むまで無人公開配信はしない。テストは限定公開で
- **API キー**: autopilot は環境変数のみ。キーをコードやログに出さない
