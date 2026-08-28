# Session

## A: SpringBone コリジョン（spring.rs）

- 0.19 vrm_spring.rs の collide_sphere / collide_capsule / collide_chain を
  glam ベースで移植。`SpringCollider`（node / offset / radius / tail）を追加。
- `SpringJoint.radius`（hitRadius、デフォルト 0.02）、`SpringChain.collider_ids`、
  `SpringState.colliders` を追加。
- parse_spring_bones: VRM 0 colliderGroups（球）+ VRM 1 colliders（球/カプセル）
  + colliderGroups をパース。チェーンの collider_ids が空なら全部に衝突。
- step 内で、長さ制約の後に collide_chain で押し出し。
- テスト: collider_parses_v0_and_v1 / collider_pushes_joint_out。

## B: MToon matcap / normal テクスチャ

- `MeshData` に matcap / normal（AlbedoRgba）を追加。`MtoonShade` に
  has_matcap / has_normal（gpu_shift_lift の z / w に乗せる）。
- gltf_load: VRM 1 matcapTexture / normalTexture、VRM 0 _SphereAdd / _BumpMap
  を index パース → texture_by_index でデコード。
- レンダラ: albedo_layout を 6 バインディング（alb / matcap / normal + サンプラ）
  に拡張。デフォルトは白 albedo / 黒 matcap / フラット normal（128,128,255）。
  GpuMesh に _matcap_tex / _normal_tex。
- シェーダー: Toon で normal マップ（ワールド法線 + デルタ）と matcap
  （反射ベクトル → UV → サンプル加算、view 行列が無いので反射方式）。
- Emma.vrm 実測: **matcap 16 / normal 17 パーツ**（VRoid は全メッシュに
  matcap + normal を持つ）。アサートで固定。

## C: ボーン制約 + firstPerson

- `constraint.rs`（新規）: VRMC_node_constraint 1.0（rotation / roll / aim）。
  parse_from_node_extensions（GltfNode.extensions から）。apply_rotation /
  apply_roll を glam で移植。Aim はパースのみ（適用は未実装）。
- SkinnedMesh.constraints + sample_locals の最後で apply_constraints。
- `first_person.rs`（新規）: VRM 0.x / VRM 1.0 の meshAnnotations をパースして
  FirstPerson（by_mesh / by_node）に保持。適用（FPS カメラで頭部を隠す）は
  一人称カメラのスライスで。
- GltfNode に extensions フィールドを追加（serde）。

## 次（未実施）

- firstPerson の適用（一人称カメラ）
- aim constraint の適用
- MToon の uv アニメーション / shade テクスチャ
