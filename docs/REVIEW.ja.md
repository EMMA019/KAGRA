# KAGRA 全体レビュー — 2026-08-24

0.1.3（PyPI）+ ソース `master`（#52 使える週 API、#53 スナップテスト、#54 ロードマップ整理）。
最終目標と次の手は [ROADMAP.ja.md](ROADMAP.ja.md)。ここは能力とギャップ。

## 1 行

**最終目標は「Python で AI に体を与えるなら KAGRA」という第一想起。**
体は three-vrm 級に近い。絵と書き味は usable-week の API まで来た（画素未確認。法線と USB パッドは未着手）。エージェントループは他に無い。頭脳の公式面は `kagra.brain("kairi")`（既定 https://kairi.onrender.com。モデルは wheel に無い）。出せるゲームが jam の箱ならループだけでは人は来ない。

「three.js の普段 + Ursina で書く週」は **今のバー** であって最終目標ではない。
Rapier / OSM / 4 段 CSM などはバーの外。禁止ではない。

## 何と比べるか

| ものさし | 戦う場所 | KAGRA の位置 |
|---|---|---|
| **three-vrm** | デスクトップ Python。Web では戦わない | 体（MToon / Spring / VRMA / 表情 / 一人称）は対抗できる。配布はこちらが厚い。頭脳は [kairi.onrender.com](https://kairi.onrender.com)（同梱しない） |
| **three.js** | 同じ種類の仕事（ライト・影・IBL・マテリアル）。ブラウザ対決ではない | **API は普段の仕事に近づいた。画素未確認。** トーンマップ opt-in、スペキュラ mip、スポット透視影、2 段スナップ。法線無し |
| **Ursina** | 「短い Python でゲームを書く」 | **API は書く週に近づいた。** ロック / クリック / `animate` / `Label` / 持つ / 孫。USB パッド無し。30 秒テストは未実施 |
| **Unity + UniVRM** | インストールと「歌って踊るまで」 | 5MB wheel で勝つ。エディタと量産では負ける（戦わない） |
| **pygame / pyxel** | 2D エンジン | 棚に下げた。戻さない |

## 強い（残す）

1. **縦の貫通。** glTF → MToon → SpringBone（コライダー付き）→ node constraint → 表情 override → リップシンク（WAV / VOICEVOX mora）→ TTS レシピ → OBS 仮想カメラが 1 プロセス。競合はだいたいブラウザか Godot に描画を投げている。
2. **VRM 実装深度。** マルチスキン修正、VRM 0.x / 1.0、VRMA、LookAt、IK、一人称レイヤー、顔トラ extra。README の「表示できます」系との差は本当。
3. **エージェントループ。** `docs/API_INDEX.md`（AST 生成）/ `kagra.verify` / MCP / `docs/agent-runs/`。Heart Catch・Switch Room・Dodge Room までログ付き。この形のエンジンは他に無い。**ループの価値は、見本が初日を超えてから効く。**
4. **配布。** `pip install kagra` 約 5MB、Rust 不要。torch をコアに入れない判断は正しい。ソース `master` が PyPI より先なのは今の弱み。
5. **棚分け。** Front（VRM / 3D / エージェント）と Shelf（2D ECS / tilemap / エディタ / boids）。`kagra-shared` + `mobile/` は運転デモで、レンダラを混ぜない。
6. **自制。** API があることを級と呼ばない。箱部屋 4 本目を D-6 と呼ばない。北極星を機能数にしない。

## 弱い（正直）

### 頭脳（面はある。サーバーは外）

`kagra.brain("kairi")` が公式面。本命は [kairi.onrender.com](https://kairi.onrender.com)。`/api/ping` は公開、`/api/chat` は `KAIRI_API_TOKEN`。
wheel には入れない（VOICEVOX と同じ関係）。Ollama / OpenAI 互換も同じ `ask`。

### 絵（three.js の普段との差）

| 項目 | 今 | 今のバー（使える週） | バーの外（後回し。禁止ではない） |
|---|---|---|---|
| ライト | 平行光 1 + 点またはスポット 1 + 半球。スポットは透視影 | 室内のスポット影が画素で見える | 多数のディレクショナル |
| IBL | HDRI + irradiance + 4 mip スペキュラ | 金属がプラスチックに見えない | 映画のフル LOD |
| 影 | 2048×2 層。既定 1。スナップあり。這いの画素未確認 | 2 段が這わない。室内にも影 | 4 段 CSM、SSAO |
| 汎用メッシュ | baseColor + 金属/粗さ。法線無し | `normalTexture` | ディスプレースメント |
| ポスト | bloom / vignette / fog / ACES opt-in | 真っ白・泥が止まる | アウトライン（VRM 以外） |
| シーングラフ | `Prop` 孫まで | 孫で足りる | 深い Object3D |
| カメラ | follow / 一人称 + ロック API | FPS として使える | 直交 3D エディタ |

絵の残りはこの順: **法線マップ → 室内影の画素 → トーンマップの画素。**
golden / smoke スクショで閉じる。**ばらして「API があるから級」と呼ばない。**

### 体（three-vrm との差）

体そのものは近い。0.5 と使える週のあいだは後回しにしてよい。

- 複数アバターの性能が未計測（バス係数 1）
- ダンス + ジェスチャのレイヤ（VRMA の上に手だけ）
- Web / XR はやらない（決めごとどおり）

### 書き味（Ursina でゲームを書く週との差）

`kagra/play.py` の方向は正しい。**短いスクリプトで「置いて・歩いて・使って・動かす」。**
Walk の strafe が画面左右と逆なら #55（カメラ right = `forward × up`）。

| Ursina | KAGRA 今 | 今のバー | バーの外 |
|---|---|---|---|
| `Entity(..., parent=…)` | 孫まで | 孫で足りる | 深い木 |
| `FirstPersonController` | `Walk(first_person=True)` + ロック API | FPS として使える | より複雑なコントローラ |
| `mouse` クリック | `clicked_prop` / `carry` | 使って持つ | フルインベントリ UI |
| `e.animate` | `animate` / Sequence | 手書き `p.x +=` をやめる | Tween カタログ全部 |
| `Button` / `Text` | `Label` / `Button` | エージェントが `fill`+`text` を書かない | Dear ImGui |
| `Audio` | `sound("coin")` | 一行 | DAW |
| `Entity(model='*.glb')` | 静的に畳む。`mesh_hit` | 法線まで | スキン Prop |
| ゲームパッド | `inject_pad` | USB/XInput | 全ベンダ抽象 |

2D の `Entity` / Tk エディタ / tilemap は Front に戻さない。Rapier は今無い（後回し）。

### 見本の天井

エージェント製は箱部屋 3 本。ループの証明であって楔 D の天井ではない。
Pretty Room / Overworld / Prop Garden は play-surface で、30 秒テストは未了。
D-6 は箱の焼き直し禁止 + 30 秒以上 + スコアかゴール。

### ドキュメント

ルートの `KAGRA_ENGINE_GUIDE.md` はスタブ。本文は `docs/archive/`。
現行は README / `docs/API_INDEX.md` / このレビュー / ロードマップ。

### 流通

スター / フォークはほぼゼロ。Stage 1 の楔が本当に必要。
今はソース `master` が PyPI 0.1.3 より先。usable-week を次の wheel に載せる。

## 構造（触るな / 畳め）

```
Python ゲーム  ──► kagra/  ──► kagra_core (wgpu, VRM, 2D/3D)
kagra-shared + mobile/     ──► 別の運転デモ（道路・トラック・Wasm）
```

- **公開 API は Front に寄せる。** Shelf の `Entity` / 2D `world_to_screen` をエージェントに踏ませない。
- **物理は今キャラコン + 静的三角形 + AABB 積み木 + 持つ。** Rapier は後回し。禁止ではない。
- **Windows は EventLoop 1 本。** GPU は必ず subprocess（`kagra.verify`）。
- **kagra-shared のレンダラは今混ぜない。** 統合はバーの外。

## 楔との関係

能力を足す理由は「エンジンが欲しいから」ではない。楔が詰むから足す。

| 楔 | 今足りる絵 | 詰み | 先にやること |
|---|---|---|---|
| A ローカル LLM に体を | `avatar` + `kagra.brain("kairi")` | `KAIRI_API_TOKEN` | トークンを入れて 15 行 |
| B 無人 3D VTuber | 歌・ダンス・HUD・仮想カメラ | 頭脳 + のちオートパイロット | 頭脳のあと。旗艦は Stage 3 |
| C VRoid マスコット | 透明窓・常駐 | 配布物 / ワンライナー | エンジン不足ではない。優先は下 |
| D エージェントがゲームを | 箱部屋 3 本 + Garden / Pretty Room / Overworld（API は載った。画素未確認） | 見知らぬ人に見せられない | **3 見本の画素。そのあと D-6** |

Stage 1 の「打てる楔を数字で選ぶ」は残す。D を箱部屋で投げて「需要が無い」と読むのは禁止。
使える週を飛ばして全滅したら、需要より見本を疑う。頭脳を飛ばして A/B が黙ったら、面を疑う。

## 製品の決めごと / 今のバーの外

**決めごと:** pygame / 2D ECS を Front に戻す、Tk を 3D エディタに育てる、
Web で three-vrm と戦う、torch をコア依存、箱部屋 4 本目を D-6、
`KAGRA_ENGINE_GUIDE.md` を現行仕様として増やす。

**今のバーの外（禁止ではない）:** Rapier / ボクセル / OSM、4 段 CSM / SSAO /
ボリュメトリック、カリング P9、kagra-core と kagra-shared のレンダラ統合。

次の手はロードマップの番号付きリスト。
**頭脳の面は載った。残りは 2 法線と画素 → 3 PyPI → 4 D-6 → 5 楔 C ワンライナー → 6 Stage 1。**
