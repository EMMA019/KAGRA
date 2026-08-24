# KAGRA 全体レビュー — 2026-08-24

0.1.3（PyPI）+ 未リリースの描画 P0–P8 + play surface
（`Prop` / `Walk` / `sky` / `room` / タイル島 + 坂沿い + ストリーム）を読んだ時点の棚卸し。
対象は「three.js / three.vrm 級の 3D」と「Ursina 級のゲーム面」。
数値目標や投稿計画は [ROADMAP.ja.md](ROADMAP.ja.md)。ここは能力の話だけ。

## 1 行

**VRM の体は three-vrm 級に近い。3D の絵は three.js の簡易フォワード（点/スポット 1 + HDRI + irradiance キューブ、フル PMREM / CSM 無し）。ゲームの書き味は Ursina の最初の週 + 閉じた部屋とタイル島。エージェント開発ループは他に無い。能力は楔より先に走った。**

「Python で AI に体を与える」北極星は正しい。エンジンとして Ursina / three.js を名指しするなら、能力トラックを需要トラックと並べて走らせる。ただし **今の天井は絵でも島でもない。** 楔 D は箱部屋を出た。次に詰むのは「Prop / Walk だけでエージェントに書かせたことが無い」と、楔 A の頭脳面が無いこと。

## 何と比べるか

| ものさし | 戦う場所 | KAGRA の位置 |
|---|---|---|
| **three-vrm** | デスクトップ Python。Web では戦わない | 体（MToon / Spring / VRMA / 表情 / 一人称レイヤー / `play_upper`）は対抗できる。配布と AI 接続はこちらが厚い。複数体の実測と「ダンスの上に手だけ」の公式つなぎは未了 |
| **three.js** | 同じ種類の仕事（ライト・影・IBL・マテリアル・カリング）。ブラウザ対決ではない | カリング / インスタンス / 1 本影（ワールド含む） / 点またはスポット 1（影無し） / HDRI + irradiance キューブ（フル PMREM 無し） / 汎用金属・粗さ。法線マップと CSM は無い |
| **Ursina** | 「短い Python で部屋を置いて歩く」 | `Prop` / `Walk` / `sky` / `room` / `water` + 一人称・ホバー・destroy・テクスチャ・1 段の親子・glTF 部品・パッド・高さ場の島。`animate` / ポインタロック / クリック面はまだ |
| **Unity + UniVRM** | インストールと「歌って踊るまで」 | 5MB wheel で勝つ。エディタと量産パイプラインでは負ける（戦わない） |
| **pygame / pyxel** | 2D エンジン | 棚に下げた。戻さない |

## 強い（残す）

1. **縦の貫通。** glTF → MToon → SpringBone（コライダー付き）→ node constraint → 表情 override → リップシンク（WAV / VOICEVOX mora）→ TTS レシピ → OBS 仮想カメラが 1 プロセス。競合はだいたいブラウザか Godot に描画を投げている。
2. **VRM 実装深度。** マルチスキン修正、VRM 0.x / 1.0、VRMA、LookAt、IK、一人称レイヤー、顔トラ extra、上半身レイヤー（`play_upper`）。README の「表示できます」系との差は本当。
3. **エージェントループ。** `docs/API_INDEX.md`（AST 生成、Front 397 のうち推奨面）/ `kagra.verify` / MCP / `docs/agent-runs/`。Heart Catch・Switch Room・Dodge Room までゲームログ付き。Garden / Pretty Room / Overworld は play-surface 実証（エージェント製ではない）。この形のエンジンは他に無い。
4. **配布。** `pip install kagra` 約 5MB、Rust 不要。torch をコアに入れない判断は正しい。ただし **PyPI 0.1.3 には play surface も P5–P8 も入っていない。** `pip install` した人の絵と、このリポジトリの絵は別物。
5. **棚分け。** Front（VRM / 3D / エージェント）と Shelf（2D ECS / tilemap / エディタ / boids）。`kagra-shared` + `mobile/` は運転デモで、レンダラを混ぜない。ここを戻すとまた広くなる。

## 弱い（正直）

### 絵（three.js との差）

| 項目 | 今 | three.js が当たり前にやっていること |
|---|---|---|
| ライト | 平行光 1 + 点 **または** スポット 1（影無し）+ 半球 | 複数、影付きスポット |
| IBL | 半球 + HDRI + 小さな irradiance キューブ。露出は `set_exposure`。スペキュラは鋭いキューブのまま | フル PMREM / スペキュラ LOD |
| 影 | 2048、VRM + ワールド AABB に合わせた ortho、床・箱・Prop・地形タイルもキャスター、9-tap PCF。カスケード無し | CSM、複数ライト |
| 汎用メッシュ | baseColor + 金属/粗さ（既定は Lambert）。MToon は別 | MeshStandard / 法線 / 複数マテリアル |
| ポスト | bloom / vignette / fog | SSAO、トーンマップ、アウトライン（VRM 以外） |
| シーングラフ | 2D `Entity` は棚。`Prop` は 1 段の親子 | Object3D の深い階層 |
| カメラ | orbit / showcase / follow / `look`（一人称は `Walk`） | ポインタロック・直交 3D |

描画性能トラック（視錐台・ボーン AABB・インスタンス・マテリアルソート・影 + 点/スポット + HDRI + irradiance + 汎用 PBR）は「同じ種類の仕事」として正しい。**P0–P8 は実装済み。次の絵（フル PMREM / 法線 / 複数ライト）は楔が「絵が安い」で詰まってから。今は始めない。**

README の「まだ無い」が「HDRI / 点光源は半球まで」のままなのは嘘になる。実装は先に行った。入口の正直リストを直す。

### 体（three-vrm との差）

体そのものは近い。足りないのは「体が複数」と「レイヤの公式つなぎ」:

- 複数アバターの性能が未計測（バス係数 1）
- `play_upper` はある。**VRMA ダンスの上に `ActionController` の手だけ**が公式に繋がっていない（上書きで idle に戻す）
- 表情プリセットに `ActionController.names()` 相当が無い（`EmotionController` に `names()` が無い）
- `Walk` はカプセルを動かす。Overworld の VRM は `idle` のまま滑る。歩行クリップは組み込みにあるが、play surface が繋いでいない
- Web / XR はやらない（決めごとどおり）

### 書き味（Ursina との差）

`kagra/play.py` の方向は正しい。初日を出た。島まで歩ける。まだ「最初の週の終わり」ではない。

| Ursina | KAGRA 今 | 穴 |
|---|---|---|
| `Entity(model='cube', color=color.orange, parent=…)` | `Prop("box", color="orange", texture=…, parent=…)` | 親子は 1 段。孫は不可 |
| `FirstPersonController` | `Walk(first_person=True)` + パッド + ジャンプ | ポインタロックはまだ。マウスが画面端で止まる |
| `mouse.hovered_entity` / `raycast` / `on_click` | `hovered_prop(cam)`（`plane` 除外）+ `Physics3D.raycast` | クリック面は無い。Garden は `E` キー |
| `destroy(e)` / `e.animate` / `Sequence` | `destroy(p)` / `p.x` / `vx` | `animate` は無い。棚の `Tween` は 2D UI。エージェントはタイマーを自作する |
| `Entity(model='model.glb')` | `Prop("crate.glb")`（静的に畳む）。`stage()` は会場 | スキン / 法線は載せない。当たりは AABB |
| 球コライダ | `add_sphere` / `add_cylinder`（ホバーも同形） | 非均一球は外接。メッシュコライダは無い |
| 高さマップ / Terrain | `World3D.set_height_fn` + タイル + `stream_radius` + 接平面の坂 | 三角形メッシュ当たりは無い。街区は箱（`city_boxes`）。街ファイルではない |
| パーティクル / 3D テキスト | ビルボード + 2D `text` + 棚の `effects` | 3D パーティクル API は無い。Orb Rush はビルボードで足りている |

Ursina を丸コピーしない。**短いスクリプトで「置いて・歩いて・触る」**が到達点。2D の `Entity` / Tk エディタ / tilemap は使わない。
`__init__.py` はまだ Shelf を再エクスポートする。索引で分けても、エージェントは `kagra.Entity` を見つけられる。

### 頭脳（ロードマップとの食い違い）

`kagra.brain.KairiBrain` と `docs/recipes/ai-brain.md` は **リポジトリに無い。** あるのは `AiCharacter`（LLM / TTS を外から差し込む古い面）。索引の Front に載っている。棚に下げると宣言したのに手前に置いてある。頭脳接続は「レシピとクラスが無い」が正しい現状。盛らない。

### ドキュメントの二枚看板

`KAGRA_ENGINE_GUIDE.md` は冒頭で履歴と書いた。中身は増やしていない。正しい。
残っていた嘘は README の「HDRI / 点光源は半球まで」。この見直しで入口を直した。無いのはフル PMREM / 複数ライト / CSM / 頭脳 / ポインタロック。日本語 README のサンプル列も Garden / Pretty Room / Overworld まで揃えた。

### 検証ループの穴

`kagra.verify` とシナリオ JSON は揃っている（blank / live / sing / 4 ゲーム / Garden / Pretty / Overworld）。
この種の環境では `kagra_core` 未ビルドで GPU verify をスキップしたログが多い。ループの設計は閉じている。**絵の「見た」はしばしば未実行。** エージェントが「verify した」と書いて画素を見ていない、が繰り返されている。シナリオを置いたことと、絵を確認したことをログで分ける。

## 構造（触るな / 畳め）

```
Python ゲーム  ──► kagra/  ──► kagra_core (wgpu, VRM, 2D/3D)
kagra-shared + mobile/     ──► 別の運転デモ（道路・トラック・Wasm）
```

- **レンダラは統合しない。** 共有コアの絵は運転デモ用。VRM スタックに足さない。OSM / 街ファイルを Python 面に持ち込まない。
- **公開 API 397。** Front に寄せたのは正しい。`AiCharacter` が Front なのは例外で、新しい頭脳面ができるまで推奨しない。
- **物理は Rapier を入れない。** キャラコン（カプセル + 高さ関数の接平面 / 滑り + AABB / yaw OBB / 球 / 円柱）で島は歩ける。三角形メッシュ当たりと積み木物理は後回し。
- **Windows は EventLoop 1 本。** GPU は必ず subprocess（`kagra.verify`）。この制約を壊さない。

## 楔との関係

能力を足す理由は「エンジンが欲しいから」ではない。楔が詰むから足す。

| 楔 | 今足りる絵 | 能力が無いと詰むところ |
|---|---|---|
| A ローカル LLM に体を | `avatar` + `sing` / `speak`。部屋も島もある | 頭脳の公式面（Kairi / Ollama）が未実装。`AiCharacter` は古い面 |
| B 無人 3D VTuber | 歌・ダンス・HUD・仮想カメラ | オートパイロット / セーフティ（Stage 3。今はやらない） |
| C VRoid マスコット | 透明窓・常駐 | 配布物（zip）。エンジン不足ではない |
| D エージェントがゲームを | 箱部屋 3 本 + Prop Garden + Pretty Room + Overworld 島 | **天井は「閉じた部屋」ではない。** 次の穴は D-6（Prop / Walk だけで書かせた実証が無い） |

Stage 1 の「4 楔を数字で選ぶ」は残す。**D の天井を上げる作業は一段落した。** 次の能力は D-6 が詰まった箇所と、A の頭脳だけ。絵の P9 は楔が「金属が鏡面すぎる / ライトが足りない」と言うまで触らない。

## 能力と需要のズレ

原則 4 は「楔が詰むところだけ足す」。直近は P5 → P8 → 部屋 → 島 → 坂/ストリームと、能力だけが連続した。Stage 1 の投稿はまだ無い。

これは失敗ではない。D-5 時点の箱部屋では D が頭打ちだった。今は頭打ちではない。**同じ勢いで PMREM や法線に入ると、原則 4 を自分で破る。**

今やることの順:

1. 入口の正直さ（README の「まだ無い」を実装に合わせる）
2. D-6 — `Prop` / `Walk` / `sky` だけで 4 本目を書かせる
3. `kagra.brain` — 楔 A を嘘にしない
4. D-6 が詰まった書き味だけ足す（ポインタロック / `animate` / クリック / 歩行クリップ）

## 今やらない（レビュー側の釘）

- pygame / 2D ECS / tilemap を Front に戻す
- Tk エディタを 3D エディタに育てる
- Web で three-vrm と戦う
- kagra-core と kagra-shared のレンダラ統合
- Rapier / ボクセル / 街ファイルのストリーミング（高さ場タイルは別）
- torch をコア依存にする
- `KAGRA_ENGINE_GUIDE.md` を現行仕様として増やす（履歴にする）
- 楔が詰む前にフル PMREM / 法線マップ / 複数ライト / CSM を始める
- 0.1.3 の wheel を、未リリースの島や HDRI があるかのように書く

次のチェックリストはロードマップの能力トラック。
