# KAGRA ロードマップ — バズるまで、そして全自動 AI VTuber まで

最終更新: 2026-08-23（0.1.3 は PyPI 済み。次は Phase 1 の一撃デモ）

North Star（北極星）は 2 つ。

1. **「pip install kagra → 2 コマンドで VRM が歌って踊る」を、カテゴリの合言葉にする**
2. **人間が一切関与しない全自動 AI VTuber を、KAGRA だけで動かす**

1 が裾野（認知・インストール数）、2 が頂上。途中の成果物が全部 2 の部品になるように並べてある。別々の山ではない。

| Phase | 目標 | 主な内容 | 完了条件 |
|---|---|---|---|
| **0** | 0.1.3 を出す | 仮想カメラ・HUD・VOICEVOX・物理など配信 V の不足 | `pip install -U kagra` で歌って踊れる（**完了**） |
| **1** | 一撃デモでバズる | 30〜60 秒の動画 → X / Reddit / HN / ニコニコ | 外部の誰かが試して star か issue が付く |
| **2** | 試した人を離さない | レシピ集・examples の簡略化・報告スレ | README だけで自分の VRM を歌わせられる |
| **3** | 半自動 AI VTuber | `kagra.autopilot`：雑談→歌→ダンス→休憩を 30 分 | 起動したら人間操作ゼロで 30 分見ていられる |
| **4** | **全自動 AI VTuber** | RTMP 直送、チャット取り込み、セーフティ、スケジューラ、記憶 | 告知から終了まで人間ゼロの枠が 1 本残り、アーカイブがある |
| **5** | エコシステム | macOS wheel、ブラウザデモ、プラグイン、テンプレ | 裾野を広げる |

---

## 現在地（0.1.3）

**もうあるもの**

- `pip install kagra`（約 5MB、Rust 不要、Windows / Linux wheel）→ `python -m kagra`
- VRM 0.x / 1.0: GPU スキニング、MToon（matcap / UV アニメ / ノーマルマップ）、SpringBone + コライダー、node constraint、表情 override、一人称、視線、まばたき
- `sing()` / `dance()` / BVH / FBX / VRMA / リップシンク（WAV + VOICEVOX mora）
- 仮想カメラ（`kagra[stream]`）→ OBS が普通のカメラとして拾える
- `StreamHud` / `ChatInbox`（JSONL）
- VOICEVOX 公式レシピ / マイク extra（`kagra[mic]`）
- `AiCharacter`（LLM 接続の土台）、顔トラ（`kagra[facetrack]`）
- `--loop` / `--stream` / `--mascot`

**まだないもの（全自動に必要な穴）**

- NDI / RTMP 直送（仮想カメラと窓キャプチャは 0.1.3 で足りる。OBS 不要は RTMP）
- YouTube / Twitch の公式チャット取り込み（JSONL 受け口はある。API キーはコアに入れない）
- 自律ループ（話題選択 → 発話 → 歌 → 休憩）
- セーフティ層
- スケジューラ・記憶
- モーション生成の公式レシピ（text-to-vrma。Phase 2）
- macOS wheel / ブラウザデモ（Phase 5。凍結中）

---

## Phase 0 — 0.1.3（完了）

- [x] WAV / `.vrma` は wheel に入れない（約 5MB。サンプル VRM は初回 DL）
- [x] バージョン 0.1.3 + CHANGELOG
- [x] 仮想カメラ extra・HUD・JSONL チャット・VOICEVOX レシピ・マイク extra
- [x] タグ `v0.1.3` → PyPI（Windows + Linux + sdist）
- [ ] リリースノート用 Before / After GIF（コライダー貫通比較。Phase 1 の動画に回してよい）

**完了条件**: `pip install -U kagra` で歌って踊れる。配信は `kagra[stream]` を足す。

## Phase 1 — 一撃デモ（いまここ）

見た人が 10 秒で理解できる 1 本に全部賭ける。長い機能紹介は要らない。

- [ ] 実 WAV + 実 VRMA + コライダー + 表情で 30〜60 秒
      （Irodori-TTS / text-to-vrma はエンジンに同梱しない）
- [ ] 動画の最後は「`pip install kagra` / `python -m kagra`」の 2 行だけ
- [ ] X（日本語）→ 同日 Reddit r/Python・r/VirtualYoutubers、HN Show HN
- [ ] ニコニコ / YouTube（Alicia 使用時はクレジット必須）
- [ ] README 比較表: KAGRA vs Unity+UniVRM vs VSeeFace vs three-vrm
      （行は「インストール」「コード量」「ライセンス」「AI 連携」だけ。盛らない）

**完了条件**: 外部の誰かが動画経由で試して、issue か star が付く。

**バズの原則（全フェーズ）**

- 主語は「VRM が歌って踊る」であって「ゲームエンジン」ではない
- 毎回同じ 2 行で締める
- 「◯◯はまだ無い」を README の正直リストに残す
- 競合を貶さない。比較表は事実のみ
- 動画を出して 2 週間で再生 1,000 未満なら、機能追加より撮り直しを優先する

## Phase 2 — 試した人を離さない

- [ ] `examples/` を 20〜30 行のコピペに統一。各例の冒頭に完成 GIF
- [ ] 公式レシピ `docs/recipes/`:
      - 自分の VRM（VRoid Studio → KAGRA）
      - Irodori-TTS → `av.sing("voice.wav")`
      - text-to-vrma → `av.dance("dance.vrma")`
      - [x] OBS / 仮想カメラ（`docs/recipes/stream.md`）
      - [x] VOICEVOX（`docs/recipes/voicevox.md`）
      - `--mascot`
- [ ] GitHub Discussions（Discord は 100 人超えてから）
- [ ] issue テンプレ（VRM 名 / OS / GPU / ログ）
- [ ] 「動いた VRM 報告スレ」

**完了条件**: 初見が README だけで自分の VRM を歌わせられる。

## Phase 3 — 半自動（人間は起動するだけ）

`AiCharacter` を核に、**起動したら放置で 30 分もつ**。開始・停止はまだ人間。

- [ ] `kagra/autopilot.py` — 雑談 → 歌 → ダンス → 休憩 → 雑談…
      話題は LLM に「前の発言」「時刻」「予定表 JSON」
      沈黙 N 秒で自発的に話す / 歌う
- [x] TTS: `avatar.speak_voicevox`。Irodori-TTS は歌用。エンジン非同梱
- [ ] セットリスト規約 `setlist/song01/{voice.wav, dance.vrma, meta.json}`
- [x] `StreamHud` / `kagra.stage`
- [ ] 曲間クロスフェード、待機ループ、`relax_hands` の点検

**完了条件**: `python -m kagra.autopilot` で 30 分、操作ゼロで見ていられる。

## Phase 4 — 全自動（人間はいない）

ここが頂上。「この VTuber、中に誰もいません」のアーカイブが 1 本残ること。

1. **配信出力** — 0.1.3 で仮想カメラは入った。次は NDI、最終的に ffmpeg パイプで RTMP（OBS 不要）
2. **チャット取り込み** — JSONL 受け口はある。YouTube Live Chat / Twitch IRC を extra で。チャット → LLM → TTS → リップシンクをレイテンシ込みで。選別（LLM + NG 辞書）
3. **セーフティ** — 発話前 NG、トピック禁止、個人情報の反射的復唱防止、全発話ログ、キルスイッチだけ人間に残す
4. **スケジューラ** — 枠の自動開始・終了、タイトル・サムネ生成
5. **記憶** — 配信をまたぐ軽量 JSON（「昨日話したこと」。ベクタ DB はまだ不要）

**完了条件**: 告知から終了まで人間ゼロの枠が 1 本成立し、アーカイブが残る。

セーフティが済むまで無人の**公開**配信はしない。テストは限定公開。

## Phase 5 — エコシステム

- [ ] macOS wheel（検証できる Mac ができてから）
- [ ] ブラウザデモ（kagra-shared / WebGPU）。レンダラ統合はしない
- [ ] プラグイン規約（`kagra.contracts`）: モーション / TTS / チャットソース
- [ ] テンプレ「my-ai-vtuber」: fork して VRM とセットリストを置くだけ
- [ ] VRoid Hub / BOOTH の礼儀: 規約リンク、クレジット自動表示
- [ ] WebP / KTX2、VRMA 以外の表情アニメ

## 計測（目標であって約束ではない）

| 指標 | Phase 1 終了時 | Phase 3 終了時 | Phase 4 終了時 |
|---|---|---|---|
| GitHub Stars | 100 | 500 | 2,000 |
| PyPI 月間 DL | 500 | 3,000 | 10,000 |
| デモ動画再生 | 1 万 | 5 万 | （無人配信そのものが証拠） |
| 外部 issue/PR | 5 | 30 | 100 |

## やらないこと

- torch / 重量級モデルをコア依存に入れない（5MB が最大の武器）
- text-to-vrma / Irodori-TTS のベンダリング（レシピで繋ぐ）
- kagra-core と kagra-shared のレンダラ統合
- Mac wheel の見切り発車
- 比較表で競合を貶すこと
- YouTube / Twitch の API キーをコアやログに置くこと

## 決めごと

- **アセット**: WAV / VRMA は wheel に入れない
- **モデル規約**: デモ・動画は Alicia（クレジット付き）か自作 VRM。AvatarSample 系は pixiv 規約
- **無人配信**: Phase 4 のセーフティが済むまで公開しない
- **API キー**: 環境変数のみ
