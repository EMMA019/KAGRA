# Agent build logs / エージェント実証ログ

エージェントに KAGRA で何かを作らせたときの記録を残す場所。
**ログ自体がコンテンツ**（「エージェントがゲームを作れる」証拠）なので、
作らせるときは最初からログを残す前提で始めること。

## 形式

1 セッション = 1 ディレクトリ: `docs/agent-runs/YYYYMMDD-<slug>/`

| ファイル | 内容 |
|---|---|
| `prompt.md` | 最初に与えた指示（一言一句そのまま） |
| `session.md` | 経過の要約: API 検索 → 実装 → verify の往復。躓いた箇所は省略せず残す |
| `result.md` | 成果物へのリンク、verify 結果、かかった往復数 |

躓きログは恥ではなく資産。エージェントが詰まった箇所 = API かドキュメントの
穴なので、セッション後に issue 化する。

## 記録の正直さ

ログが無いものを「エージェント製」と呼ばない。
`examples/vrm_orb_rush.py` はこのループの参照実装だが、生成ログは残って
いないため「エージェント製」とは主張しない。ログ付きの実証:

- `20260823-heart-catch/` — 3 レーンでハートをキャッチ
- `20260823-switch-room/` — 箱部屋を歩いてスイッチを踏む
- `20260823-dodge-room/` — 降ってくる箱を避ける（独立エージェント）
- `20260823-world-shadows/` — 絵トラック P5（ワールド影。ゲームではない）
- `20260824-picture-p6-p8/` — 絵トラック P6–P8（点光・HDRI・金属/粗さ。ゲームではない）
- `20260824-pretty-room/` — 閉じた部屋 + スポット + irradiance（ゲームではない）
- `20260824-overworld/` — 高さ関数の島（海・草原・山。ゲームではない）
- `20260824-slope-stream/` — 坂の沿い／滑り、タイル影、歩きながら読み込み（ゲームではない）
- `20260824-city-mesh-stack-csm/` — 街 JSON・三角形当たり・積み木・2 段影（ゲームではない）
- `20260824-usable-week/` — 使える週の API（トーンマップ / IBL mip / スポット影 / ロック / click / animate。ゲームではない。GPU 未確認）
- `20260824-normals-pad/` — 法線マップ API + USB/XInput gilrs（ゲームではない。GPU 未確認）
- `20260824-pixel-close/` — 室内スポット影をローカル光に掛ける + ACES / IBL ペアワイズ golden（ゲームではない。GPU は CI。室内影は mean_abs=0.614 で失敗）
- `20260824-roadmap-80/` — エンジン到達点を 80% に（今約 33%。北極星は第一想起のまま）
- `20260824-indoor-shadow/` — 室内ウンブラを濃くする + 親子 4 段 + 法線ペアワイズ（ゲームではない。CI `indoor_spot` / `normal_bump` 通過）
- `20260824-local-lights/` — 局所ライト 4 スロット（ゲームではない。CI `local_four` 通過）
- `20260824-crawl-rigid/` — 屋外這いの pairwise golden + AABB 剛体（乗る）。這いは CI #65 通過。剛体は #64
- `20260824-relic-run-walk-assets/` — Relic Run 前傾腕（Mixamo/BVH T-pose deltas）修正 + Kenney/Poly Haven CC0 で島を 30s 見本に
- `20260824-open-world/` — Crest Isle（広い草原・海・山の収集。Kenney 密度 + LOD 地形）
- `20260824-crest-isle-mobile/` — Crest Isle を kagra-shared / wasm / Android で遊ぶ（カプセル。VRM ではない）
- `20260825-agent-eyes/` — annotate + debug_trace + follow 壁クリップ + Prop/地形 toon（エディタではない）
- `20260825-unshadow-stage/` — `kagra.stage` が submodule に隠れて Crest Isle が TypeError だったのを直す
- `20260825-slope-ground/` — 斜面接地（足 AABB を絞る + 接平面。`debug_trace` で測る。Rapier は入れない）
- `20260825-crest-isle-white-world/` — Crest Isle 白世界 + 長押し後の遅れ停止（ゲーム修正。GPU は CI / Emma の Windows）
- `20260825-crest-isle-repress-look/` — 長押し後の再押し無視・茶色い草原・白い空・チェイスカメラが頭に刺さる（ゲーム修正。GPU は CI / Emma の Windows）
- `20260825-crest-isle-title/` — Crest Isle タイトルが半透明で割れた島の上に乗っていたのを不透明メニューにする（ゲーム修正。GPU は SMOKE=play のまま）
- `20260825-sleeve-stiffness/` — Crest Isle 袖/布の剛性（SpringBone Verlet を UniVRM に合わせ、Alicia に袖ヘルパー。ゲームではない）
- `20260825-crest-isle-loco-blend/` — Crest Isle 歩き速度ブレンド + 上半身レイヤ（idle/walk/run。Mixamo は入れない）
- `20260825-crest-isle-spatial-audio/` — Crest Isle 立体音（listener + 海ループ + 拾い SE。HRTF ではない）
- `20260825-multi-avatar/` — 複数 VRM の GPU 共有 + FPS 見本（Crest Isle は 1 人のまま）
- `20260825-mixamo-vroid-locomotion/` — Mixamo FBX を VRoid に rest+roll 補償して歩きに載せる（前傾腕をやめる）
- `20260826-crest-isle-trees-ao-seams-sparks/` — Crest Isle Kenney colormap / blob AO / tile seams / CPU sparks + hair rim / gold PBR orbs
- `20260826-crest-isle-bald-ground/` — Crest Isle meadow ハゲ (JPEG dirt rim / ClampToEdge stamp)
- `20260826-world3d-stream-barcode/` — World3D stream retry + Crest barcode (period 9.5 < TILE)
- `20260826-crest-isle-meadow-window/` — Crest Isle meadow ハゲ after #95 (JPEG biome windows / TERRAIN_UV_RECT)
- `20260826-crest-isle-ggx-tile/` — remaining ハゲ: one 16 m TILE dead albedo / GGX-only (upload swallow + Mesh3D pin)
- `20260826-character-controller/` — game-ready capsule CharacterController (accel / slope sit / step-up / jump). No Rapier.
- `20260826-world-as-data/` — World query/dump/load + 80% redefined (now ~15%; old 63% archived). Not a game; the M1 mountain.
- `20260826-one-runtime-schema/` — M2 first slice: WorldDoc ingests World.dump() JSON and compiles a Scene3D draw list. Schema only.
- `20260826-one-runtime-offscreen/` — M2 offscreen: shared wgpu 30 renders a compiled WorldDoc to RGBA. No RendererV2 / no kagra-core window.
- `20260826-verify-shared-offscreen/` — wire `kagra.verify` to shared wgpu 30 offscreen (no `(-12800,-12800)`). Not a game.
- `20260826-crest-isle-black-peel-zoom/` — remaining Nature Kit black trees, orbit peel (tiles/fog/hair), chase-cam zoom keys
- `20260826-crest-isle-black-tile-spec/` — Crest Isle hillside black 16 m tile + gold GGX (coin PBR leftover / mesh_mat slot), not #94/#95/#96 JPEG UV
