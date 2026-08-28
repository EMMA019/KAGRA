# KAGRA ロードマップ — 画面を見ないエージェントがゲームを出す

最終更新: 2026-08-27（kagra **0.1.4**）。
旧「今 63% / 到達点 80%（three-vrm + three.js + Ursina 置換）」は
[archive](archive/ROADMAP.ja.md.63pct-2026-08-25.md) に移した。エージェントは 63% をコピーしない。

棚卸しの能力表は [REVIEW.ja.md](REVIEW.ja.md)。ここは **何が 100% / 80% / 今か** と山の順。

## 北極星（第一想起）は残す

**「Python で AI に体を与えるなら KAGRA」。**
測るのは Pull（オーガニック install、「動いた」報告、外部発のコンテンツ）。
エンジンの % は北極星の代わりではない。

## 新しい 80%（2026-08-27 Emma 再掲）

| 語 | 定義 |
|---|---|
| **100%** | AI エージェントが、人間が画面を見ずに、普通のインディー 2D/3D ゲームを出荷できる。 |
| **80%** | その 100% から、ネット・破壊・布・乗り物・GI bake・DOTS・HDRP・人間用エディタ・Shader Graph・Visual Scripting・Addressables・Terrain sculpt・ProBuilder・Cinemachine・PhysX 完全・VRM-on-Wasm を外したもの。**マルチは 100%。** 絵は別山ではない。ジャンルを閉じるたびに shared wgpu 30 の絵を上げる。HQ 密度/光/材質は保つ。VRM はローダ/衣装であり天井ではない。世界の絵は Prop / 高さ場 / glTF / 光。 |
| **今** | **約 40%。** M0–M2 は閉じた。閉じたジャンルは **collectathon（1）**。M3 はジャンル+絵（40%→80%）。M4 はエージェントが一人で出荷。 |

Unity 機能パリティではない。圧縮した高レベル API。人間用エディタは禁止。
VRM はオプションのローダであり、背骨ではない。

## 山（この順。飛ばさない）

| # | 山 | 状態 |
|---|---|---|
| **M0** | 看板（タイル）。Crest Isle 16m タイル / UV。PR #97 | 閉じた。`uv_rect` / `_upload_tile` / `stream_tiles` / `TERRAIN_UV_*` は触らない |
| **M1** | 世界をデータに。`World.query` / `dump` / `load`。15% → 35% へ | 閉じた（#99） |
| **M2** | **ランタイムは一つ。** スキーマ + shared オフスクリーン + wgpu 30 窓 + WASD/高さ場/glTF/ライブ tick。公式プレイは `python -m kagra.play_world`。`examples/vrm_open_world.py` は leftover VRM。新しいゲームは V2 に乗らない | 閉じた（#100–#102, #104, #105） |
| **M3** | ゲームとして足りる（ジャンル+絵）。本スライスは **collectathon**（歩く・拾う・数える・終わる。タイトル→プレイ→結果）+ shared wgpu 30 の島の絵 | collectathon を閉じた。action / RPG は次 |
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
kagra-shared の `WorldDoc::from_json` が同じ JSON を読む。`compile_scene` が `Scene3D` を出す（高さ場バッチ + 草 albedo + glTF + 金属コイン + ライト slot 1:1、空 dump は key+fill + 接地 blob + カプセル）。`WorldPlay` がタイトル→プレイ→結果（WASD / 拾う / 数える / 終わる）で dump を進める。`render_world_doc` が shared wgpu 30 オフスクリーンでそのフレームを RGBA にする。`python -m kagra.play_world` が同じ Renderer を **普通のデスクトップ窓** に present する。**公式 Crest プレイはこの窓**（カプセル collectathon）。`examples/vrm_open_world.py` は旧 VRM / RendererV2 のまま残してよい。新しいゲームは RendererV2 で始めない。`(-12800,-12800)` は旧 V2 スモーク専用。人間のスクショ修正ではジャンルは閉じない。

## 今あるもの（嘘にしない）

- `Prop` / `Walk` / `World`（旧 `World3D`）/ `Camera3D` / 手続きテクスチャと SE
- 高さ場タイル（Relic Run の UV 既定は維持。Crest の meadow 窓は #97）
- AABB の箱（落ちる・積む・乗る）。Rapier は入れない
- VRM ローダ（歌・踊り・リップ・LookAt）。体の背骨ではない
- `kagra.verify` の PNG サイズ煙 + **世界アサーション**（coins / on_ground / query / albedo_ok）+ 任意の **shared オフスクリーン煙**（空でない。golden ではない。ヘルパ無しはスキップ）
- `WorldDoc` / `WorldPlay`（dump JSON。collectathon ループ。`compile_scene` は島の高さ場 + 金属コイン + ライト 4 スロット（空 dump は key+fill）+ 接地 blob。shared wgpu 30 オフスクリーン / `play_world` 窓。公式 Crest プレイはカプセル。旧 VRM デモは RendererV2 のまま）
- **接着 API 4本（フェーズ1 圧縮）**: `prop.interact`（調べる/話す/使う → on_use イベント）、`doc.timers`（待つ。0 で on_done イベント）、`doc.events`（出来事。emit → take で複数システムが読む。コールバック無し）、`walker.anim`（状態→アニメ。エンジンは wish から walk/idle を導出、ジャンル名は静止中保持）。ジャンル専用ロジックはゲーム側。`docs/agent-runs/20260831-adhesive-api/`
- **HDR + ブルーム**: 3D パスは線形 HDR フレーム（Rgba16Float）、`set_bloom(threshold, intensity)` の閾値ブルームを HDR 空間で適用し、composite が exposure + ACES + sRGB を掛ける。HUD はトーン後に重ねる。play_world / offscreen はデフォルト有効（0.85 / 0.35）。`docs/agent-runs/20260831-hdr-bloom/`
- **FXAA**: composite 出力（sRGB）に輝度エッジ検出 + 勾配ブレンド。`set_fxaa(bool)`（デフォルト有効）。HUD は FXAA 後に重ねる。`docs/agent-runs/20260831-fxaa/`
- **MToon 完全移植**: 影2段階（shade色 + toony + shift）、リム（色 + fresnel power + lift）、アウトライン（backface push-out）。VRM 1.0 / VRM 0.x の MToon 拡張をパース。matcap / normal テクスチャは次。`docs/agent-runs/20260831-mtoon-full/`
- エージェントループ: `docs/API_INDEX.md` / MCP / `docs/agent-runs/`

## 嘘（今 40% を大きく呼ばない）

- 「今約 63%」「絵は three.js 級 85%」——旧定義。アーカイブへ
- 「エージェントがゲームを出荷できる」——まだ。閉じたジャンルは collectathon だけ
- 「2D ECS に z を足せば 3D」——やらない
- 「VRM がエンジンの背骨」「Wasm に VRM を移植」——やらない
- 「Tk / Inspector が人間用エディタ」——禁止。目は `annotate` / `debug_trace` / `world.query`
- dump JSON を読んだだけで「ランタイムは一つ」——スキーマ + オフスクリーン + wgpu 30 窓 + WASD tick + collectathon ループまでは来た。VRM skin は shared に載せない。公式プレイはカプセル

## 80% の外（今やらない）

ネット、破壊、布、乗り物、GI bake、DOTS、HDRP、人間用エディタ、
Shader Graph、Visual Scripting、Addressables、Terrain sculpt、ProBuilder、
Cinemachine、PhysX 完全、VRM-on-Wasm。

加えて（エンジン都合）: Rapier、SSAO / 4 段 CSM、wgpu 0.19 と 30 の混合、
OSM、ボクセル、ナビメッシュ、lights/joints/prefab-instantiate/TRS 階層/particles の新規山。
VRM skin、GI、乗り物、エディタ、第二 HQ レンダラ、RendererV2 に絵を足す、
0.19 と 30 を混ぜる、RendererV2 を消す、action/RPG へ行く。Unreal 完成の葉は待たない。

## 本 PR の Done（M3 — collectathon を閉じる）

- 歩く・拾う・数える・終わる。タイトル → プレイ → 結果。`python -m kagra.play_world` が一本のループ。既存 collectathon / WorldDoc / WorldPlay を再利用。新しい ECS は作らない
- 絵（shared wgpu 30 のみ）: 高さ場が島に見える（平面プレースホルダではない）。草はハゲない（Grass 手続き + 高さバイオーム）。コインは金属（既存 GGX / `Material::Metal`）。ライト slot 1:1（漏れなし）。カプセルは読める（胴+頭）
- Verify: coins / on_ground / query / tile albedo_ok / オフスクリーンが空でない。`examples/verify_scenarios/collectathon_smoke.json`。GPU 無しテストは緑
- `new_for_window` は `cfg(not(target_arch = "wasm32"))` のまま。RendererV2 は消さない。0.19 と 30 を混ぜない

M0 は #97。M1 は #99。M2 は #100–#102 / #104 / #105。このスライスが最初の M3 ジャンル。
