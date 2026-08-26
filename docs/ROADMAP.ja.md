# KAGRA ロードマップ — 画面を見ないエージェントがゲームを出す

最終更新: 2026-08-26（kagra **0.1.4**）。
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
| **今** | **約 15%。** 描ける。歩けない世界をデータとして聞けない。検証は PNG のファイルサイズが主。 |

Unity 機能パリティではない。圧縮した高レベル API。人間用エディタは禁止。
VRM はオプションのローダであり、背骨ではない。

## 山（この順。飛ばさない）

| # | 山 | 状態 |
|---|---|---|
| **M0** | 看板（タイル）。Crest Isle 16m タイル / UV。PR #97 | 閉じた。`uv_rect` / `_upload_tile` / `stream_tiles` / `TERRAIN_UV_*` は触らない |
| **M1** | 世界をデータに。`World.query` / `dump` / `load`。15% → 35% へ | 閉じた（#99） |
| **M2** | **ランタイムは一つ。** 本 PR は第一スライス: `world.dump()` JSON（[schemas/world.json](schemas/world.json)）を既存 `Scene3D` が読む。レンダラ切替（wgpu 0.19 と 30 を混ぜない / デスクトップ窓を shared に付け替えない）は次 | 今ここ。スキーマ。2D ECS に z を足さない |
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
kagra-shared の `Scene3D::from_world_json` が同じ JSON を読む（M2 第一スライス）。レンダラ切替は次。

## 今あるもの（嘘にしない）

- `Prop` / `Walk` / `World`（旧 `World3D`）/ `Camera3D` / 手続きテクスチャと SE
- 高さ場タイル（Relic Run の UV 既定は維持。Crest の meadow 窓は #97）
- AABB の箱（落ちる・積む・乗る）。Rapier は入れない
- VRM ローダ（歌・踊り・リップ・LookAt）。体の背骨ではない
- `kagra.verify` の PNG サイズ煙 + **世界アサーション**（#99）
- `Scene3D::from_world_json`（dump JSON。描画はまだ Python / shared 別）
- エージェントループ: `docs/API_INDEX.md` / MCP / `docs/agent-runs/`

## 嘘（今 15% を大きく呼ばない）

- 「今約 63%」「絵は three.js 級 85%」——旧定義。アーカイブへ
- 「エージェントがゲームを出荷できる」——まだ。ランタイムが二つ
- 「2D ECS に z を足せば 3D」——やらない
- 「VRM がエンジンの背骨」「Wasm に VRM を移植」——やらない
- 「Tk / Inspector が人間用エディタ」——禁止。目は `annotate` / `debug_trace` / `world.query`
- dump JSON を読んだだけで「ランタイムは一つ」——本スライスはスキーマ。レンダラ切替は次

## 80% の外（今やらない）

ネット、破壊、布、乗り物、GI bake、DOTS、HDRP、人間用エディタ、
Shader Graph、Visual Scripting、Addressables、Terrain sculpt、ProBuilder、
Cinemachine、PhysX 完全、VRM-on-Wasm。

加えて（エンジン都合）: Rapier、SSAO / 4 段 CSM、wgpu 0.19 と 30 の混合、
OSM、ボクセル、ナビメッシュ、lights/joints/prefab-instantiate/TRS 階層/particles の新規山。

## 本 PR の Done（M2 第一スライス — スキーマ）

- GPU 無し: Crest Isle 形 / Orb Rush 形の `World.dump()` JSON が Rust の `Scene3D` としてパースされ、安定 id・position・parent・heightfield `fn` / tile key がラウンドトリップする
- デスクトップ Python の dump JSON を shared クレートが受け取る（新しい公開ゲーム API は足さない）
- [schemas/world.json](schemas/world.json) と `Scene3D` が揃っている
- レンダラ切替は次。`(-12800,-12800)` の fake-headless はレンダラ切替が要るのでこのスライスでは触らない
- `pytest tests -m "not golden"` と `cargo test -p kagra-shared` が緑

M1（query / dump / load）は #99 で閉じた。
