# KAGRA 全体レビュー — 2026-08-24

0.1.3（PyPI）+ 未リリースの描画 P0–P8 + 部屋 v1（`Prop` / `Walk` / 島）を読んだ時点。
対象は「three.js の普段の仕事」と「Ursina でゲームを書く週」。
数値と次の手は [ROADMAP.ja.md](ROADMAP.ja.md)。ここは能力の話だけ。

## 1 行

**VRM の体は three-vrm 級に近い。3D の絵は three.js の初日。ゲームの書き味は Ursina の初日。エージェント開発ループは他に無いが、出せるゲームが jam の箱なのでループだけでは人は来ない。**

北極星は正しい。足りないのは楔の投稿文ではなく、見知らぬ人に 30 秒見せられる見本。
P0–P8 と部屋 v1 を [x] にしたのは「API が生えた」であって、「three.js 級」「Ursina 級」ではない。
級という言葉は、使える週を通るまで使わない。

## 何と比べるか

| ものさし | 戦う場所 | KAGRA の位置 |
|---|---|---|
| **three-vrm** | デスクトップ Python。Web では戦わない | 体（MToon / Spring / VRMA / 表情 / 一人称）は対抗できる。配布はこちらが厚い。頭脳の公式面は無い |
| **three.js** | 同じ種類の仕事（ライト・影・IBL・マテリアル）。ブラウザ対決ではない | **初日。** カリング / インスタンス / 平行光の 2 段影 / 点またはスポット 1（影無し） / HDRI + irradiance（スペキュラ LOD 無し） / 金属・粗さ（法線無し） / トーンマップ無し |
| **Ursina** | 「短い Python でゲームを書く」 | **初日。** `Prop` / `Walk` / `sky` / `room` + 一人称・ホバー・destroy・1 段の親子・glTF 部品・パッド inject。ロック・クリック・`animate`・Button は無い |
| **Unity + UniVRM** | インストールと「歌って踊るまで」 | 5MB wheel で勝つ。エディタと量産では負ける（戦わない） |
| **pygame / pyxel** | 2D エンジン | 棚に下げた。戻さない |

## 強い（残す）

1. **縦の貫通。** glTF → MToon → SpringBone（コライダー付き）→ node constraint → 表情 override → リップシンク（WAV / VOICEVOX mora）→ TTS レシピ → OBS 仮想カメラが 1 プロセス。競合はだいたいブラウザか Godot に描画を投げている。
2. **VRM 実装深度。** マルチスキン修正、VRM 0.x / 1.0、VRMA、LookAt、IK、一人称レイヤー、顔トラ extra。README の「表示できます」系との差は本当。
3. **エージェントループ。** `docs/API_INDEX.md`（AST 生成）/ `kagra.verify` / MCP / `docs/agent-runs/`。Heart Catch・Switch Room・Dodge Room までログ付き。この形のエンジンは他に無い。**ループの価値は、見本が初日を超えてから効く。**
4. **配布。** `pip install kagra` 約 5MB、Rust 不要。torch をコアに入れない判断は正しい。
5. **棚分け。** Front（VRM / 3D / エージェント）と Shelf（2D ECS / tilemap / エディタ / boids）。`kagra-shared` + `mobile/` は運転デモで、レンダラを混ぜない。

## 弱い（正直）

### 絵（three.js の普段との差）

| 項目 | 今（初日） | 普段の仕事（使える週の到達） | やらない |
|---|---|---|---|
| ライト | 平行光 1 + 点 **または** スポット 1（影無し）+ 半球 | 室内はスポットか点が影を落とす | 多数のディレクショナル |
| IBL | HDRI + 小さな irradiance。露出あり | スペキュラ LOD（PMREM 相当。小さくてよい） | 映画のフル LOD |
| 影 | 2048×2 層。既定 1。屋外 2 段はまだ這う | 2 段が這わない。室内にも影 | 4 段フィルム CSM、SSAO |
| 汎用メッシュ | baseColor + 金属/粗さ。法線無し | `normalTexture` | ディスプレースメント |
| ポスト | bloom / vignette / fog | ACES / filmic + sRGB | アウトライン（VRM 以外） |
| シーングラフ | `Prop` は 1 段 | 孫まで（2 段） | 深い Object3D |
| カメラ | orbit / showcase / follow / 一人称（ロック無し） | ポインタロック | 直交 3D エディタ |

カリングの続きは詰まっていない。次はトーンマップ / スペキュラ IBL / 法線 / 室内の影 / 屋外 2 段の安定。**ばらして P9 だけ出荷しない。**

### 体（three-vrm との差）

体そのものは近い。使える週のあいだは触らない。

- 複数アバターの性能が未計測（バス係数 1）
- ダンス + ジェスチャのレイヤ（VRMA の上に手だけ）
- Web / XR はやらない（決めごとどおり）

### 書き味（Ursina でゲームを書く週との差）

`kagra/play.py` の方向は正しい。到達点を「置いて歩いて触る」から上げる。
**短いスクリプトで「置いて・歩いて・使って・動かす」。** 初日のホバーで止めない。

| Ursina | KAGRA 今 | 使える週 | やらない |
|---|---|---|---|
| `Entity(..., parent=…)` | `Prop`、親子 1 段 | 孫まで | 深い木 |
| `FirstPersonController` | `Walk(first_person=True)`、ロック無し | ポインタロック | マリオ 64 |
| `mouse` クリック | ホバーのみ | 押下して使う / 持つ | フルインベントリ UI |
| `e.animate` | `p.x` / `vx` | `animate` / Sequence | Tween カタログ全部 |
| `Button` / `Text` | 毎フレーム `fill` + `text` | `Label` / `Button` | Dear ImGui |
| `Audio` | `tone` を手書き | 一行の `sound` | DAW |
| `Entity(model='*.glb')` | 静的に畳む。`mesh_hit` で三角形 | そのまま（PBR は法線まで） | スキン Prop |
| ゲームパッド | `inject_pad` | USB/XInput | 全ベンダ抽象 |

2D の `Entity` / Tk エディタ / tilemap は使わない。Rapier は入れない。

### 頭脳（ロードマップとの食い違い）

`kagra.brain.KairiBrain` と `docs/recipes/ai-brain.md` は **リポジトリに無い。**
あるのは棚の `AiCharacter`。楔 A の前に面を作る。D のゲートにはしない
（VTuber の絵は足りている。ゲームの絵が足りていない）。

### ドキュメントの二枚看板

`KAGRA_ENGINE_GUIDE.md` は「v3 / Phase 6」、`pip install` 不可、VRM 1.0 のみ、テスト 2 本、と書いてある。**現行仕様ではない。** エージェントがこれを読むと API を発明する。現行は README / `docs/API_INDEX.md` / このレビュー / ロードマップ。

## 構造（触るな / 畳め）

```
Python ゲーム  ──► kagra/  ──► kagra_core (wgpu, VRM, 2D/3D)
kagra-shared + mobile/     ──► 別の運転デモ（道路・トラック・Wasm）
```

- **レンダラは統合しない。** 共有コアの絵は運転デモ用。VRM スタックに足さない。
- **公開 API は Front に寄せる。** Shelf の `Entity` / 2D `world_to_screen` をエージェントに踏ませない。Front を厚くする方が、Shelf を消すより先。
- **物理は Rapier を入れない。** キャラコン + 静的三角形 + AABB 積み木 + 持つ。任意スキンメッシュや安定した巨大スタックは後回し。
- **Windows は EventLoop 1 本。** GPU は必ず subprocess（`kagra.verify`）。この制約を壊さない。

## 楔との関係

能力を足す理由は「エンジンが欲しいから」ではない。楔が詰むから足す。
**今詰まっている楔は D で、詰みの中身は API 不足ではなく見本が初日なこと。**

| 楔 | 今足りる絵 | 詰み | 先にやること |
|---|---|---|---|
| A ローカル LLM に体を | `avatar` + `sing` / `speak` | 頭脳の公式面が無い | `kagra.brain`（使える週と並行可） |
| B 無人 3D VTuber | 歌・ダンス・HUD・仮想カメラ | オートパイロット / セーフティ | Stage 3。今はやらない |
| C VRoid マスコット | 透明窓・常駐 | 配布物 | エンジン不足ではない |
| D エージェントがゲームを | 箱部屋 3 本 + Garden / Pretty Room / Overworld（どれも初日） | 見知らぬ人に見せられない | **使える週の 3 見本。そのあと D-6** |

Stage 1 の「4 楔を数字で選ぶ」は残す。D を箱部屋で投げて「需要が無い」と読むのは禁止。
使える週を飛ばして全滅したら、需要より見本を疑う。

## 今やらない（レビュー側の釘）

- pygame / 2D ECS / tilemap を Front に戻す
- Tk エディタを 3D エディタに育てる
- Web で three-vrm と戦う
- kagra-core と kagra-shared のレンダラ統合
- Rapier / ボクセル / OSM（箱の街 JSON は別）
- 4 段フィルム CSM / SSAO / ボリュメトリック
- torch をコア依存にする
- カリング P9
- 箱部屋 4 本目を D-6 と呼ぶ
- `KAGRA_ENGINE_GUIDE.md` を現行仕様として増やす（履歴にする）

次のチェックリストはロードマップの **使える週**。P0–P8 の続きではない。
