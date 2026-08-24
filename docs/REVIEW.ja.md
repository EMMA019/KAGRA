# KAGRA 全体レビュー — 2026-08-23

0.1.3（PyPI）+ 未リリースの描画性能 P0–P8 + `Prop` / `Walk` / `sky` / `room` を読んだ時点の棚卸し。
対象は「three.js / three.vrm 級の 3D」と「Ursina 級のゲーム面」。
数値目標や投稿計画は [ROADMAP.ja.md](ROADMAP.ja.md)。ここは能力の話だけ。

## 1 行

**VRM の体は three-vrm 級に近い。3D の絵は three.js の簡易フォワード（点/スポット 1 + HDRI + irradiance キューブ、フル PMREM / CSM 無し）。ゲームの書き味は Ursina の最初の週 + 閉じた部屋。エージェント開発ループは他に無い。**

「Python で AI に体を与える」北極星は正しい。エンジンとして Ursina / three.js を名指しするなら、能力トラックを需要トラックと並べて走らせないと、楔 D（エージェントがゲームを書く）が箱部屋で頭打ちになる。

## 何と比べるか

| ものさし | 戦う場所 | KAGRA の位置 |
|---|---|---|
| **three-vrm** | デスクトップ Python。Web では戦わない | 体（MToon / Spring / VRMA / 表情 / 一人称）は対抗できる。配布と AI 接続はこちらが厚い |
| **three.js** | 同じ種類の仕事（ライト・影・IBL・マテリアル・カリング）。ブラウザ対決ではない | カリング / インスタンス / 1 本影（ワールド含む） / 点またはスポット 1（影無し） / HDRI + irradiance キューブ（フル PMREM 無し） / 汎用金属・粗さ。CSM は無い |
| **Ursina** | 「短い Python で部屋を置いて歩く」 | `Prop` / `Walk` / `sky` / `room` + 一人称・ホバー・destroy・テクスチャ・1 段の親子・glTF 部品・パッド |
| **Unity + UniVRM** | インストールと「歌って踊るまで」 | 5MB wheel で勝つ。エディタと量産パイプラインでは負ける（戦わない） |
| **pygame / pyxel** | 2D エンジン | 棚に下げた。戻さない |

## 強い（残す）

1. **縦の貫通。** glTF → MToon → SpringBone（コライダー付き）→ node constraint → 表情 override → リップシンク（WAV / VOICEVOX mora）→ TTS レシピ → OBS 仮想カメラが 1 プロセス。競合はだいたいブラウザか Godot に描画を投げている。
2. **VRM 実装深度。** マルチスキン修正、VRM 0.x / 1.0、VRMA、LookAt、IK、一人称レイヤー、顔トラ extra。README の「表示できます」系との差は本当。
3. **エージェントループ。** `docs/API_INDEX.md`（AST 生成）/ `kagra.verify` / MCP / `docs/agent-runs/`。Heart Catch・Switch Room・Dodge Room までログ付き。この形のエンジンは他に無い。
4. **配布。** `pip install kagra` 約 5MB、Rust 不要。torch をコアに入れない判断は正しい。
5. **棚分け。** Front（VRM / 3D / エージェント）と Shelf（2D ECS / tilemap / エディタ / boids）。`kagra-shared` + `mobile/` は運転デモで、レンダラを混ぜない。ここを戻すとまた広くなる。

## 弱い（正直）

### 絵（three.js との差）

| 項目 | 今 | three.js が当たり前にやっていること |
|---|---|---|
| ライト | 平行光 1 + 点 **または** スポット 1（影無し）+ 半球 | 複数、影付きスポット |
| IBL | 半球 + HDRI + 小さな irradiance キューブ。露出は `set_exposure` | フル PMREM / スペキュラ LOD |
| 影 | 2048、VRM + ワールド AABB に合わせた ortho、床・箱・Prop もキャスター、9-tap PCF。カスケード無し | CSM、複数ライト |
| 汎用メッシュ | baseColor + 金属/粗さ（既定は Lambert）。MToon は別 | MeshStandard / 法線 / 複数マテリアル |
| ポスト | bloom / vignette / fog | SSAO、トーンマップ、アウトライン（VRM 以外） |
| シーングラフ | 2D `Entity` は棚。`Prop` は 1 段の親子 | Object3D の深い階層 |
| カメラ | orbit / showcase / follow | 一人称・ポインタロック・直交 3D |

描画性能トラック（視錐台・ボーン AABB・インスタンス・マテリアルソート・影 + 点/スポット + HDRI + irradiance + 汎用 PBR）は「同じ種類の仕事」として正しい。**次はフル PMREM LOD / 法線マップ / 複数ライトで、カリングの続きではない。**

### 体（three-vrm との差）

体そのものは近い。足りないのは「体が複数」と「レイヤ」:

- 複数アバターの性能が未計測（バス係数 1）
- ダンス + ジェスチャのレイヤ（VRMA の上に手だけ）
- Web / XR はやらない（決めごとどおり）

### 書き味（Ursina との差）

`kagra/play.py` の方向は正しい。まだ初日。

| Ursina | KAGRA 今 | 穴 |
|---|---|---|
| `Entity(model='cube', color=color.orange, parent=…)` | `Prop("box", color="orange", texture=…, parent=…)` | 親子は 1 段。孫は不可 |
| `FirstPersonController` | `Walk(first_person=True)` + パッド | ポインタロックはまだ |
| `mouse.hovered_entity` / `raycast` | `hovered_prop(cam)`（`plane` 除外） | クリック・ボタンはまだ |
| `destroy(e)` / `e.animate` | `destroy(p)` / `p.x` / `vx` | `animate` / Sequence はまだ |
| `Entity(model='model.glb')` | `Prop("crate.glb")`（静的に畳む）。`stage()` は会場 | スキン / PBR は載せない。当たりは AABB |
| 球コライダ | `add_sphere` / `add_cylinder`（ホバーも同形） | 非均一球は外接。メッシュコライダは無い |

Ursina を丸コピーしない。**短いスクリプトで「置いて・歩いて・触る」**が到達点。2D の `Entity` / Tk エディタ / tilemap は使わない。

### 頭脳（ロードマップとの食い違い）

`docs/ROADMAP.ja.md` は Stage 0 で `kagra.brain.KairiBrain` と `docs/recipes/ai-brain.md` を完了にしている。**リポジトリにどちらも無い。** あるのは棚の `AiCharacter`（LLM / TTS を外から差し込む古い面）。頭脳接続は「レシピとクラスが無い」が正しい現状。盛らない。

### ドキュメントの二枚看板

`KAGRA_ENGINE_GUIDE.md` は「v3 / Phase 6」、`pip install` 不可、VRM 1.0 のみ、テスト 2 本、と書いてある。**現行仕様ではない。** エージェントがこれを読むと API を発明する。現行は README / `docs/API_INDEX.md` / このレビュー / ロードマップ。

## 構造（触るな / 畳め）

```
Python ゲーム  ──► kagra/  ──► kagra_core (wgpu, VRM, 2D/3D)
kagra-shared + mobile/     ──► 別の運転デモ（道路・トラック・Wasm）
```

- **レンダラは統合しない。** 共有コアの絵は運転デモ用。VRM スタックに足さない。
- **公開 API 388。** Front に寄せたのは正しい。エージェントはまだ Shelf の `Entity` / `world_to_screen`（2D）を踏む。Front を厚くする（`Prop` / `room` を増やす）方が、Shelf を消すより先。
- **物理は Rapier を入れない。** キャラコン（カプセル + 高さ関数の接平面 / 滑り + AABB / yaw OBB）で島は歩ける。三角形メッシュ当たりと積み木物理は後回し。
- **Windows は EventLoop 1 本。** GPU は必ず subprocess（`kagra.verify`）。この制約を壊さない。

## 楔との関係

能力を足す理由は「エンジンが欲しいから」ではない。楔が詰むから足す。

| 楔 | 今足りる絵 | 能力が無いと詰むところ |
|---|---|---|
| A ローカル LLM に体を | `avatar` + `sing` / `speak` | 頭脳の公式面（Kairi / Ollama）が未実装 |
| B 無人 3D VTuber | 歌・ダンス・HUD・仮想カメラ | オートパイロット / セーフティ（Stage 3。今はやらない） |
| C VRoid マスコット | 透明窓・常駐 | 配布物（zip）。エンジン不足ではない |
| D エージェントがゲームを | 箱部屋 3 本 + Prop Garden + Pretty Room + Overworld 島 | 閉じた部屋と、タイル化した高さ場（坂・階段・箱街区）。次は D-6 か頭脳 |

Stage 1 の「4 楔を数字で選ぶ」は残す。**D の天井を上げるのが能力トラックの第一目的。** A の頭脳は「完了」ではなく次の実装。

## 今やらない（レビュー側の釘）

- pygame / 2D ECS / tilemap を Front に戻す
- Tk エディタを 3D エディタに育てる
- Web で three-vrm と戦う
- kagra-core と kagra-shared のレンダラ統合
- Rapier / ボクセル / 街ファイルのストリーミング（高さ場タイルは別）
- torch をコア依存にする
- `KAGRA_ENGINE_GUIDE.md` を現行仕様として増やす（履歴にする）

次のチェックリストはロードマップの能力トラック。
