# KAGRA ロードマップ — 画面を見ないエージェントがゲームを出す

最終更新: 2026-08-27（kagra **0.1.4**）。
旧「今 63% / 到達点 80%（three-vrm + three.js + Ursina 置換）」は
[archive](archive/ROADMAP.ja.md.63pct-2026-08-25.md) に移した。エージェントは 63% をコピーしない。

棚卸しの能力表は [REVIEW.ja.md](REVIEW.ja.md)。ここは **何が 100% / 80% / 今か** と山の順。

## 北極星（第一想起）は残す

**「Python で AI に体を与えるなら KAGRA」。**
測るのは Pull（オーガニック install、「動いた」報告、外部発のコンテンツ）。
エンジンの % は北極星の代わりではない。

## 新しい 80%（2026-08-26）

| 語 | 定義 |
|---|---|
| **100%** | AI エージェントが、人間が画面を見ずに、普通のインディー 2D/3D ゲームを出荷できる。 |
| **80%** | その 100% から、ネット・破壊・布・乗り物・GI bake・DOTS・HDRP・人間用エディタ・Shader Graph・Visual Scripting・Addressables・Terrain sculpt・ProBuilder・Cinemachine・PhysX 完全・VRM-on-Wasm を外したもの。 |
| **今** | **約 15%。** 描ける。WorldDoc を shared wgpu 30 の窓で歩ける（カプセル）。VRM の付け替えはしない。検証は PNG サイズ + 世界アサーション + shared オフスクリーン。 |

Unity 機能パリティではない。圧縮した高レベル API。人間用エディタは禁止。
VRM はオプションのローダであり、背骨ではない。

## 山（この順。飛ばさない）

| # | 山 | 状態 |
|---|---|---|
| **M0** | 看板（タイル）。Crest Isle 16m タイル / UV。PR #97 | 閉じた。`uv_rect` / `_upload_tile` / `stream_tiles` / `TERRAIN_UV_*` は触らない |
| **M1** | 世界をデータに。`World.query` / `dump` / `load`。15% → 35% へ | 閉じた（#99） |
| **M2** | **ランタイムは一つ。** スキーマ + shared オフスクリーン + wgpu 30 窓は閉じた。本スライスは **窓入力 + 高さ場/glTF + ライブ tick**: WASD/視点が WorldDoc の walker/camera を進める。`compile_scene` が高さ場バッチ（named fn / samples）と glTF 部品を出す。Crest の公式プレイは `python -m kagra.play_world`（カプセル。VRM は `vrm_open_world.py` に残してよい）。新しいゲームは RendererV2 で始めない。`(-12800,-12800)` は旧 V2 スモーク専用。0.19 と 30 を混ぜない | 今ここ。collectathon を公式パスへ |
| **M3** | ゲームとして足りる（30 秒の見本が遊べる） | その次 |
| **M4** | 出荷（エージェントが画面なしで普通のゲームを出す） | 80% の手前まで |

## 公式の公開面

エージェントが使う名前はこれだけ:

**`World` / `Prop` / `Walk` / mesh-or-avatar / `Camera` / input / sound**

`World` は `World3D` と同じ型（`kagra.World is kagra.World3D`）。
2D の `Entity` / tilemap / Tk エディタはディスクに残るが `import kagra` の表には出ない
（`kagra.entity` / `kagra.tilemap` / `kagra.editor_app`）。

世界のオブジェクトは **安定した文字列 id**（GPU 整数ではない）。
`world.query(type=, name=, aabb=)` はスクショなしで position / name / type / id を返す。
タイルは `type="terrain_tile"` で `loaded` / `albedo_ok`（はげを PNG なしで検出）。
`world.dump()` / `world.load()` のスキーマは [schemas/world.json](schemas/world.json)。
kagra-shared の `WorldDoc::from_json` が同じ JSON を読む。`compile_scene` が `Scene3D` を出す（高さ場バッチ + glTF 部品 + カプセル）。`WorldPlay` が WASD / 視点で dump を毎フレーム進める。`render_world_doc` が shared wgpu 30 オフスクリーンでそのフレームを RGBA にする。`python -m kagra.play_world` が同じ Renderer を **普通のデスクトップ窓** に present する。**公式 Crest プレイはこの窓**（カプセル）。`examples/vrm_open_world.py` は旧 VRM / RendererV2 のまま残してよい。新しいゲームは RendererV2 で始めない。`(-12800,-12800)` は旧 V2 スモーク専用。

## 今あるもの（嘘にしない）

- `Prop` / `Walk` / `World`（旧 `World3D`）/ `Camera3D` / 手続きテクスチャと SE
- 高さ場タイル（Relic Run の UV 既定は維持。Crest の meadow 窓は #97）
- AABB の箱（落ちる・積む・乗る）。Rapier は入れない
- VRM ローダ（歌・踊り・リップ・LookAt）。体の背骨ではない
- `kagra.verify` の PNG サイズ煙 + **世界アサーション**（#99）+ 任意の **shared オフスクリーン煙**（IHDR / バイト数。golden ではない。ヘルパ無しはスキップ）
- `WorldDoc`（dump JSON。`compile_scene` → 高さ場 + glTF + カプセルの 1 フレーム `Scene3D`。`WorldPlay` が WASD で進める。shared wgpu 30 オフスクリーン / `play_world` 窓。公式 Crest プレイはカプセル。旧 VRM デモは RendererV2 のまま）
- エージェントループ: `docs/API_INDEX.md` / MCP / `docs/agent-runs/`

## 嘘（今 15% を大きく呼ばない）

- 「今約 63%」「絵は three.js 級 85%」——旧定義。アーカイブへ
- 「エージェントがゲームを出荷できる」——まだ。ランタイムが二つ
- 「2D ECS に z を足せば 3D」——やらない
- 「VRM がエンジンの背骨」「Wasm に VRM を移植」——やらない
- 「Tk / Inspector が人間用エディタ」——禁止。目は `annotate` / `debug_trace` / `world.query`
- dump JSON を読んだだけで「ランタイムは一つ」——スキーマ + shared オフスクリーン + wgpu 30 窓 + WASD tick までは来た。VRM skin は shared に載せない。公式プレイはカプセル

## 80% の外（今やらない）

ネット、破壊、布、乗り物、GI bake、DOTS、HDRP、人間用エディタ、
Shader Graph、Visual Scripting、Addressables、Terrain sculpt、ProBuilder、
Cinemachine、PhysX 完全、VRM-on-Wasm。

加えて（エンジン都合）: Rapier、SSAO / 4 段 CSM、wgpu 0.19 と 30 の混合、
OSM、ボクセル、ナビメッシュ、lights/joints/prefab-instantiate/TRS 階層/particles の新規山。

## 本 PR の Done（M2 — WASD + 高さ場/glTF + ライブ WorldDoc）

- WASD + 視点（マウス / 矢印）が WorldDoc の walker 位置/yaw と camera に流れる。`python -m kagra.play_world window`（example `window`）。kagra-core `RendererV2` / `window.rs` / `(-12800,-12800)` には触れない
- `compile_scene` が高さ場バッチを出す（named fn `open_world_height` / `island_height` / `overworld_height`、または dump samples）。glTF 部品は既存 `gltf_load.rs`（`cube.glb` エイリアス可）。プレイヤーはカプセル。VRM skin は移植しない
- ライブ毎フレーム: shared `WorldPlay` が wish → 高さ場に座る。Python が毎フレーム JSON を流す必要はない。RendererV2 に戻らない
- Crest の公式プレイは `play_world`（wgpu 30 窓で Crest dump を歩く）。`examples/vrm_open_world.py` は旧 VRM 用に RendererV2 を残してよい。新しいゲームは RendererV2 で始めない。fake-headless は旧 V2 スモーク専用
- GPU 無し: WASD が WorldDoc の walker を動かす / compile が高さ場 + glTF バッチを出す / tick が walker を進める。`pytest tests -m "not golden"` と `cargo test -p kagra-shared` が緑
- Rapier / SSAO / エディタ / 追加 PNG golden / タイル UV / M3 キット / wgpu 混合は触らない。RendererV2 は消さない

M1 は #99。M2 schema は #100。M2 offscreen verify は #101。M2 窓楔は #102。このスライスが残 M2（キットの前）。
