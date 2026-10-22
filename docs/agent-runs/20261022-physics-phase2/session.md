# 2026-10-22 physics phase 2

## 方針

Rapier は `WorldPlay` のジャンルにしない。`kagra-shared` の `PhysicsWorld` が Android からも呼ぶ面で、Python は `kagra.rigid` の薄い包み。ゲーム（振り子で宝箱を落とす）は `examples/physics_minimal.py`。

`WorldProp` に `body` / `collider` / `sensor` / `mass`、`WorldDoc.joints` を足した。既存の構造体リテラルは `..Default::default()` で新しいフィールドを埋める（action2d / rpg / td）。

maturin の features を `python,physics` にして、通常の Python ビルドで `PhysicsWorld` が出るようにした。cargo の `python` feature 自体には physics を畳んでいない（wasm は physics なしのまま）。

## 実装

- 剛体: `static` / `dynamic` / `kinematic`。`body` が `is_static` より優先。
- コライダー: box / sphere / capsule / cylinder / cone / none。未知の model は景観だけ。
- ジョイント: fixed / revolute / spherical（weld / hinge / ball）。アンカーはローカル。接触は切る。
- 力積・力・角力積、レイキャスト（`exclude`）、`take_events("collision"|"contact")`。
- `sync` は動的 prop とキネマティック prop の姿勢を書き戻す。歩行者は上書きしない。
- 例: 支点の revolute に岩（`rock`）、箱二つ（`crate.glb`）、宝箱（`chest`）、松（`stylized_pine`、コライダーなし）、キネマティックの押し箱。A/D と画面左右タップ、R でやり直し。HUD は ASCII。

## 躓き

- `let mut box` は予約語で fmt 以前にパースできなかった。
- 新しい `WorldProp` フィールドで、`..Default` の無いリテラル（action2d / rpg / td）がコンパイルできなくなった。
- 地面にめり込んだ箱同士の fixed joint は力積で外れた。少し持ち上げた配置なら相対姿勢が残る。テストはそちらに合わせた。
- レイキャストは 1 step 後でも、真上からだと支点の箱に当たる。例では X を 0.2 ずらして宝箱に当てている。
- `physics_stack.py` は `kagra` を import する前にリポジトリを `sys.path` に足さないと、拡張だけの venv で PNG を書けずに落ちた。PhysicsWorld が無いときも rest pose の PNG を書く。
- `rpg_town_smoke` は walker 名 `player` を期待していたが、フィクスチャの name は `hero`。クエリを `hero` に合わせた（この差分を入れる前から失敗していた）。
- 振り子の数値（支点 `[0, 3.15, 0]`、ボブ `[1.60, 2.57, 0]`、軸 Z、長さ約 1.70）は Rust テストと Python 例で共有。240 step で宝箱が飛ぶ。

## 検証

- `cargo test -p kagra-shared --lib --locked` 420 passed。
- `cargo test -p kagra-shared --lib --features physics --locked` の physics 17 件 passed。
- clippy `-D warnings`: default、`render`、`physics`、`python,physics`（いずれも `--all-targets`）。
- `pytest tests -m "not golden"` passed（PhysicsWorld あり）。
- `python tools/gen_api_index.py --check` OK。
- lavapipe（`lvp_icd.json`）で `examples/physics_minimal.py --headless`。PNG 79923 bytes、480×270。宝箱はノック済み。
- `examples/verify_scenarios/*.json` は `demo_live_smoke`（スクリプト欠落）以外 pass。`physics_smoke` と `physics_stack_smoke` を含む。master をマージしたあとの action / sports / survival / stealth / fishcook / rhythm / rpg / td の smoke も同じ。
- master マージで増えた `WorldProp` リテラル（RPG の HP バー、TD の設置）にも `..Default::default()` を足した。
