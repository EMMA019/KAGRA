# KAGRA 全体レビュー — 2026-08-24

0.1.3（PyPI）+ ソース `master`（#52〜#62）。
最終目標と次の手は [ROADMAP.ja.md](ROADMAP.ja.md)。ここは能力とギャップ。

## 1 行

**最終目標は「Python で AI に体を与えるなら KAGRA」という第一想起。**
エンジン到達点は **80%**（今 **約 47%**）。100% は Python だけで three-vrm + three.js + Ursina を置き換えて困らないこと。
体は約 80% で足りる。絵は約 50% で本体。室内スポット・法線・局所 4 の画素は CI 通過（#61、#62）。
ACES と金属差は CI で出た。這いの画素が次（pairwise `outdoor_crawl` は CI 待ち）。エージェントループは他に無い。頭脳の面は `kagra.brain("kairi")`（既定 https://kairi.onrender.com。モデルは wheel に無い）。出せるゲームが jam の箱ならループだけでは人は来ない。

## 何と比べるか

| ものさし | 戦う場所 | KAGRA の位置 |
|---|---|---|
| **three-vrm** | デスクトップ Python。Web では戦わない | 体は約 80%。配布はこちらが厚い。頭脳は [kairi.onrender.com](https://kairi.onrender.com)（同梱しない） |
| **three.js** | 同じ種類の仕事（ライト・影・IBL・マテリアル）。ブラウザ対決ではない | 絵は約 50%。室内影・法線・局所 4 は CI 通過。80% には這い。ACES / 金属差は維持 |
| **Ursina** | 「短い Python でゲームを書く」 | API は約 60%（親子 4 段は載った）。80% には 3 見本の 30 秒。機能欠落より見本 |
| **Unity + UniVRM** | インストールと「歌って踊るまで」 | 5MB wheel で勝つ。エディタと量産では負ける（戦わない） |
| **pygame / pyxel** | 2D エンジン | 棚に下げた。戻さない |

## 強い（残す）

1. **縦の貫通。** glTF → MToon → SpringBone（コライダー付き）→ node constraint → 表情 override → リップシンク（WAV / VOICEVOX mora）→ TTS レシピ → OBS 仮想カメラが 1 プロセス。競合はだいたいブラウザか Godot に描画を投げている。
2. **VRM 実装深度。** マルチスキン修正、VRM 0.x / 1.0、VRMA、LookAt、IK、一人称レイヤー、顔トラ extra。README の「表示できます」系との差は本当。
3. **エージェントループ。** `docs/API_INDEX.md`（AST 生成）/ `kagra.verify` / MCP / `docs/agent-runs/`。Heart Catch・Switch Room・Dodge Room までログ付き。この形のエンジンは他に無い。**ループの価値は、見本が初日を超えてから効く。**
4. **配布。** `pip install kagra` 約 5MB、Rust 不要。torch をコアに入れない判断は正しい。ソース `master` が PyPI より先なのは今の弱み。
5. **棚分け。** Front（VRM / 3D / エージェント）と Shelf（2D ECS / tilemap / エディタ / boids）。`kagra-shared` + `mobile/` は運転デモで、レンダラを混ぜない。
6. **自制。** 今 47% を 80% と呼ばない。箱部屋 4 本目を D-6 と呼ばない。北極星を機能数にしない。

## 弱い（正直）

### 頭脳（面はある。サーバーは外）

`kagra.brain("kairi")` が公式面。本命は [kairi.onrender.com](https://kairi.onrender.com)。`/api/ping` は公開、`/api/chat` は `KAIRI_API_TOKEN`。
wheel には入れない（VOICEVOX と同じ関係）。Ollama / OpenAI 互換も同じ `ask`。
エンジン 80% の重みは薄い。楔 A/B のゲート。

### 絵（three.js。今 50% → 80% で 85%）

| 項目 | 今 | 80% で要る | 80% の外 |
|---|---|---|---|
| ライト | 平行光 1 + 局所 4（`slot=`。#62 CI 通過） | 維持。キーは影 | 多数ディレクショナル、RectArea |
| IBL | HDRI + irradiance + 4 mip。金属差は CI 通過 | 維持 | 映画のフル LOD |
| 影 | 2048×2。室内スポットは CI 通過（#61） | 這わない | 4 段 CSM、SSAO |
| 汎用メッシュ | baseColor + 金属/粗さ + 法線（#61 CI 通過） | 維持 | ディスプレースメント、transmission |
| ポスト | bloom / vignette / fog / ACES（CI 通過） | 維持 | アウトライン（VRM 以外）、ボリュメトリック |
| シーングラフ | `Prop` 4 段 | 4 段（玄孫） | 無限 Object3D |
| カメラ | follow / 一人称 + ロック API | FPS として使える | 直交 3D エディタ |

絵の残りはこの順: **這い。** 見本の 30 秒は書き味。
golden / smoke スクショで閉じる。**ばらして 80% と呼ばない。**

### 体（three-vrm。既に約 80%）

体そのものは近い。80% のボトルネックではない。空きでよい。

- 複数アバターの性能が未計測（バス係数 1）
- ダンス + ジェスチャのレイヤ（VRMA の上に手だけ）
- Web / XR はやらない（決めごとどおり）

### 書き味（Ursina。API 約 60% → 80%）

`kagra/play.py` の方向は正しい。**短いスクリプトで「置いて・歩いて・使って・動かす」。**
Walk の strafe は画面右（#55）。

| Ursina | KAGRA 今 | 80% で要る | 80% の外 |
|---|---|---|---|
| `Entity(..., parent=…)` | 4 段 | 4 段 | 無限の木 |
| `FirstPersonController` | `Walk(first_person=True)` + ロック API | FPS として使える | より複雑なコントローラ |
| `mouse` クリック | `clicked_prop` / `carry` | 使って持つ（見本で） | フルインベントリ UI |
| `e.animate` | `animate` / Sequence | 手書き `p.x +=` をやめる | Tween カタログ全部 |
| `Button` / `Text` | `Label` / `Button` | エージェントが `fill`+`text` を書かない | Dear ImGui |
| `Audio` | `sound("coin")` | 一行 | DAW |
| `Entity(model='*.glb')` | 静的に畳む。`mesh_hit` | 法線が画素で見える | スキン Prop |
| ゲームパッド | `inject_pad` + gilrs | 実機で歩く | 全ベンダ抽象 |

2D の `Entity` / Tk エディタ / tilemap は Front に戻さない。
剛体は AABB で落ちる・積む・乗る（世界 55%。Rapier クレートは 5MB のため入れない）。OSM は入らない。

### 見本の天井

エージェント製は箱部屋 3 本。ループの証明であって楔 D の天井ではない。
Pretty Room / Overworld / Prop Garden は play-surface で、30 秒テストは未了。
D-6 は箱の焼き直し禁止 + 30 秒以上 + スコアかゴール。
見本無しでエンジン 80% とは呼ばない。

### ドキュメント

ルートの `KAGRA_ENGINE_GUIDE.md` はスタブ。本文は `docs/archive/`。
現行は README / `docs/API_INDEX.md` / このレビュー / ロードマップ。

### 流通

スター / フォークはほぼゼロ。Stage 1 の楔が本当に必要。
今はソース `master` が PyPI 0.1.3 より先。ソースを次の wheel に載せる。

## 構造（触るな / 畳め）

```
Python ゲーム  ──► kagra/  ──► kagra_core (wgpu, VRM, 2D/3D)
kagra-shared + mobile/     ──► 別の運転デモ（道路・トラック・Wasm）
```

- **公開 API は Front に寄せる。** Shelf の `Entity` / 2D `world_to_screen` をエージェントに踏ませない。
- **物理はキャラコン + 静的三角形 + AABB 剛体（落ちる・積む・乗る）+ 持つ。** OSM は外。Rapier クレートは wheel に入れない。
- **Windows は EventLoop 1 本。** GPU は必ず subprocess（`kagra.verify`）。
- **kagra-shared のレンダラは今混ぜない。** 統合は 80% の外。

## 楔との関係

能力を足す理由は「エンジンが欲しいから」ではない。楔が詰むから足す。
80% の絵は楔 D が詰むから足す。SSAO は楔が詰まない。

| 楔 | 今足りる絵 | 詰み | 先にやること |
|---|---|---|---|
| A ローカル LLM に体を | `avatar` + `kagra.brain("kairi")` | `KAIRI_API_TOKEN` | トークンを入れて 15 行 |
| B 無人 3D VTuber | 歌・ダンス・HUD・仮想カメラ | 頭脳 + のちオートパイロット | 頭脳のあと。旗艦は Stage 3 |
| C VRoid マスコット | 透明窓・常駐 | 配布物 / ワンライナー | エンジン不足ではない。優先は下 |
| D エージェントがゲームを | 箱部屋 3 本 + Garden / Pretty Room / Overworld（API は載った。室内影の画素は失敗） | 見知らぬ人に見せられない | **室内影の画素。3 見本。そのあと D-6** |

Stage 1 の「打てる楔を数字で選ぶ」は残す。D を箱部屋で投げて「需要が無い」と読むのは禁止。
見本が初日のままだと需要より見本を疑う。頭脳を飛ばして A/B が黙ったら、面を疑う。

## 製品の決めごと / 80% の外

**決めごと:** pygame / 2D ECS を Front に戻す、Tk を 3D エディタに育てる、
Web で three-vrm と戦う、torch をコア依存、箱部屋 4 本目を D-6、
`KAGRA_ENGINE_GUIDE.md` を現行仕様として増やす。

**80% の外（禁止ではない）:** 4 段 CSM / SSAO / ボリュメトリック / OSM / ボクセル /
カリング P9、kagra-core と kagra-shared のレンダラ統合。Rapier クレートは 5MB のため 80% の外（AABB で世界は足りる）。

次の手はロードマップの番号付きリスト。
**今約 47%。80% の本体は這いの画素と 3 見本。室内影・法線・局所 4 は閉じた。剛体は AABB。**
