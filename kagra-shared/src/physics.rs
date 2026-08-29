//! Rapier 剛体物理（Phase 1）。
//!
//! `WorldDoc` の props を Rapier の剛体に変換し、`step` で落下・衝突・
//! 積み重ねをシミュレートして、位置を props に書き戻す。
//!
//! - `is_static=true`（既定）の prop → 固定（床・壁・景観）。動かない。
//! - `is_static=false` の prop → 動的剛体。落ちて積もる（old の
//!   `add_box(is_static=False)` と同じ契約、ただし Rapier で本物の物理）。
//! - 地形は `height_at` をサンプリングして高さ場コライダーにする
//!   （`halfspace` に比べて島の丘に沿って箱が転がる）。
//! - 歩行者は**キネマティック剛体**（カプセル）。位置はゲーム（WorldPlay /
//!   Python）が所有し、毎フレーム `sync_walkers` で押し込む。重力で落ちず、
//!   箱にぶつかって押し、箱の上に立つ。`sync` は歩行者の位置を上書きしない
//!   （WorldPlay の WASD 移動と共存する）。
//!
//! `enhanced-determinism` feature により、同一プラットフォームで
//! 同入力 → 同結果（ゲームの決定論ルールを満たす）。
//!
//! Python からは `kagra_shared.PhysicsWorld`（py.rs）で使う。ゲームロジック
//! （いつ箱を落とすか等）は Python 側。

use std::collections::HashMap;

use glam::Vec3;
use rapier3d::prelude::*;

use crate::world_doc::{WorldDoc, WorldProp};

/// 1 物理ステップの固定 dt（秒）。Rapier は固定 dt 前提で決定論的。
pub const PHYSICS_DT: f32 = 1.0 / 120.0;

/// Rapier 剛体ワールド + props との対応。
#[derive(Default)]
pub struct PhysicsWorld {
    inner: rapier3d::prelude::PhysicsWorld,
    /// prop id → 剛体ハンドル（動的のみ）。
    dynamic: HashMap<String, RigidBodyHandle>,
    /// prop id → コライダーハンドル（静的 + 動的）。
    colliders: HashMap<String, ColliderHandle>,
    /// 歩行者 id → キネマティック剛体ハンドル。
    kinematic: HashMap<String, RigidBodyHandle>,
}

/// コライダー形状を作る。None は「物理に参加しない」。
///
/// 形状は描画と同じ中心基準（prop.position は中心、scale は全体サイズ）:
/// - box / crate → cuboid（半辺）
/// - sphere → ball（半径 = max(x, z) 半幅。coin 等の円盤に近い）
/// - capsule / cylinder → capsule_y（半径 = min(x, z) 半幅、高さ = y）
fn collider_for(p: &WorldProp) -> Option<ColliderBuilder> {
    let s = p.scale;
    let friction = p.friction;
    let restitution = p.restitution;
    let base = |b: ColliderBuilder| b.friction(friction).restitution(restitution);
    let m = p.model.to_ascii_lowercase();
    match m.as_str() {
        "box" | "crate" | "" => Some(base(ColliderBuilder::cuboid(
            s[0].abs() * 0.5,
            s[1].abs() * 0.5,
            s[2].abs() * 0.5,
        ))),
        "sphere" => {
            let r = (s[0].abs().max(s[2].abs()) * 0.5).max(0.05);
            Some(base(ColliderBuilder::ball(r)))
        }
        "capsule" | "cylinder" => {
            let r = (s[0].abs().min(s[2].abs()) * 0.5).max(0.05);
            let half = (s[1].abs() * 0.5 - r).max(0.02);
            Some(base(ColliderBuilder::capsule_y(half, r)))
        }
        _ => None,
    }
}

impl PhysicsWorld {
    /// `WorldDoc` からワールドを構築。地形 + props + 歩行者を剛体にする。
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let mut this = Self::default();
        // 構造体リテラル（Rust Default）だと gravity=0 になることがある。
        // serde の既定（9.8）に合わせる。
        let g = if doc.gravity > 0.0 { doc.gravity } else { 9.8 };
        this.inner.gravity = Vec3::new(0.0, -g, 0.0);
        // 地形: 高さ場サンプル。無ければ floor_y の平面。
        this.build_ground(doc);
        // props: is_static=false だけ動的。
        for p in &doc.props {
            if !p.enabled {
                continue;
            }
            if let Some(collider) = collider_for(p) {
                if p.is_static {
                    let body = this
                        .inner
                        .insert_body(RigidBodyBuilder::fixed().translation(p.position.into()));
                    let ch = this.inner.insert_collider(collider, Some(body));
                    this.colliders.insert(p.id.clone(), ch);
                } else {
                    let body = this.inner.insert_body(
                        RigidBodyBuilder::dynamic()
                            .translation(p.position.into())
                            .can_sleep(true),
                    );
                    let ch = this.inner.insert_collider(collider, Some(body));
                    this.colliders.insert(p.id.clone(), ch);
                    this.dynamic.insert(p.id.clone(), body);
                }
            }
        }
        // 歩行者（player + walkers）: キネマティックカプセル。
        // 位置はゲーム（WorldPlay / Python）が所有。重力で落ちず、箱に
        // ぶつかって押し、箱の上に立つ。sync では書き戻さない。
        for w in doc.player.iter().chain(doc.walkers.iter()) {
            let body = this.inner.insert_body(
                RigidBodyBuilder::kinematic_position_based()
                    .translation(w.position.into()),
            );
            let collider = ColliderBuilder::capsule_y(0.7, 0.28)
                .friction(0.0)
                .density(1.0);
            this.inner.insert_collider(collider, Some(body));
            this.kinematic.insert(w.id.clone(), body);
        }
        this
    }

    /// 地形（高さ場 or 平面）を固定コライダーにする。
    fn build_ground(&mut self, doc: &WorldDoc) {
        let half = doc.half.max(4.0);
        // height_at を格子サンプリングして高さ場コライダーを作る。
        // Rapier HeightField: サンプルは z-major（行 = z）、原点中心で
        // scale は全体サイズ（x_at(j) = (-0.5 + j/(n-1)) * scale.x）。
        const N: usize = 33; // 32 セル
        let step = (half * 2.0) / (N as f32 - 1.0);
        let mut heights = vec![0.0f32; N * N];
        for iz in 0..N {
            for ix in 0..N {
                let x = -half + ix as f32 * step;
                let z = -half + iz as f32 * step;
                let y = doc.height_at(x, z);
                heights[iz * N + ix] = y;
            }
        }
        let collider = ColliderBuilder::heightfield(
            Array2::new(N, N, heights),
            Vec3::new(half * 2.0, 1.0, half * 2.0), // 全体サイズ
        );
        // HeightField は原点中心で -half..half に広がる（translation 不要）。
        let body = self.inner.insert_body(RigidBodyBuilder::fixed());
        self.inner.insert_collider(collider, Some(body));
    }

    /// 1 フレーム進める。`dt` を固定ステップに分割して積む（爆発防止）。
    ///
    /// キネマティック歩行者は、直前の `sync_walkers` で設定された
    /// next_kinematic_translation に向かって動く。
    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        let mut remaining = dt;
        while remaining > 0.0 {
            let h = remaining.min(PHYSICS_DT);
            self.inner.integration_parameters.dt = h;
            self.inner.step();
            remaining -= h;
        }
    }

    /// 歩行者のゲーム側位置をキネマティック剛体へ押し込む（毎フレーム、
    /// `step` の前に呼ぶ）。WorldPlay の WASD 移動と共存する。
    pub fn sync_walkers(&mut self, doc: &WorldDoc) {
        for w in doc.player.iter().chain(doc.walkers.iter()) {
            if let Some(&body) = self.kinematic.get(&w.id) {
                if let Some(rb) = self.inner.bodies.get_mut(body) {
                    rb.set_next_kinematic_translation(Vec3::from_array(w.position));
                }
            }
        }
    }

    /// 歩行者 1 体のゲーム側位置を押し込む（`sync_walkers` の単体版）。
    pub fn set_walker_position(&mut self, id: &str, p: [f32; 3]) -> bool {
        let Some(&body) = self.kinematic.get(id) else {
            return false;
        };
        if let Some(rb) = self.inner.bodies.get_mut(body) {
            rb.set_next_kinematic_translation(Vec3::from_array(p));
            return true;
        }
        false
    }

    /// 剛体位置を props に書き戻す（歩行者はゲーム所有なので触らない）。
    pub fn sync(&self, doc: &mut WorldDoc) {
        for p in &mut doc.props {
            if let Some(&body) = self.dynamic.get(&p.id) {
                if let Some(rb) = self.inner.bodies.get(body) {
                    p.position = rb.translation().to_array();
                }
            }
        }
    }

    /// prop（動的）の速度を設定する（投げる・吹き飛ばす）。
    pub fn set_velocity(&mut self, id: &str, v: [f32; 3]) -> bool {
        let Some(&body) = self.dynamic.get(id) else {
            return false;
        };
        if let Some(rb) = self.inner.bodies.get_mut(body) {
            rb.set_linvel(Vec3::from_array(v), true);
            return true;
        }
        false
    }

    /// prop（動的）の位置を直接設定する（テレポート / リスポーン）。
    pub fn set_position(&mut self, id: &str, p: [f32; 3]) -> bool {
        let Some(&body) = self.dynamic.get(id) else {
            return false;
        };
        if let Some(rb) = self.inner.bodies.get_mut(body) {
            rb.set_translation(Vec3::from_array(p), true);
            rb.set_linvel(Vec3::ZERO, true);
            return true;
        }
        false
    }

    /// 動的剛体の現在位置。
    pub fn position(&self, id: &str) -> Option<[f32; 3]> {
        let &body = self.dynamic.get(id)?;
        let rb = self.inner.bodies.get(body)?;
        Some(rb.translation().to_array())
    }

    /// 動的 prop かどうか（is_static=false の prop）。
    pub fn is_dynamic(&self, id: &str) -> bool {
        self.dynamic.contains_key(id)
    }

    /// キネマティック歩行者かどうか（player / walkers）。
    pub fn is_kinematic(&self, id: &str) -> bool {
        self.kinematic.contains_key(id)
    }

    /// 物理に参加する剛体か（動的 prop + 歩行者）。
    pub fn is_body(&self, id: &str) -> bool {
        self.dynamic.contains_key(id) || self.kinematic.contains_key(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_with_box(y: f32) -> WorldDoc {
        WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            props: vec![WorldProp {
                id: "prop:box".into(),
                kind: "prop".into(),
                name: "box".into(),
                model: "box".into(),
                position: [0.0, y, 0.0],
                scale: [1.0, 1.0, 1.0],
                enabled: true,
                is_static: false,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn dynamic_box_falls_to_ground() {
        let doc = doc_with_box(5.0);
        let mut world = PhysicsWorld::from_doc(&doc);
        for _ in 0..240 {
            world.step(1.0 / 60.0);
        }
        let y = world.position("prop:box").unwrap()[1];
        assert!(y > 0.4 && y < 1.6, "箱は床に落ちて止まる（半辺 0.5）, y={y}");
        assert!(!world.is_dynamic("prop:box") || world.position("prop:box").is_some());
    }

    #[test]
    fn static_box_does_not_fall() {
        let mut doc = doc_with_box(3.0);
        doc.props[0].is_static = true;
        let mut world = PhysicsWorld::from_doc(&doc);
        for _ in 0..120 {
            world.step(1.0 / 60.0);
        }
        assert!(!world.is_dynamic("prop:box"), "静的 prop は動的剛体ではない");
        world.sync(&mut doc);
        assert_eq!(doc.props[0].position, [0.0, 3.0, 0.0], "静的 prop は動かない");
    }

    #[test]
    fn two_boxes_stack() {
        let mut doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            props: vec![
                WorldProp {
                    id: "prop:a".into(),
                    kind: "prop".into(),
                    name: "box".into(),
                    model: "box".into(),
                    position: [0.0, 0.5, 0.0],
                    scale: [1.0, 1.0, 1.0],
                    enabled: true,
                    is_static: false,
                    ..Default::default()
                },
                WorldProp {
                    id: "prop:b".into(),
                    kind: "prop".into(),
                    name: "box".into(),
                    model: "box".into(),
                    position: [0.0, 3.0, 0.0],
                    scale: [1.0, 1.0, 1.0],
                    enabled: true,
                    is_static: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        for _ in 0..600 {
            world.step(1.0 / 60.0);
        }
        let ya = world.position("prop:a").unwrap()[1];
        let yb = world.position("prop:b").unwrap()[1];
        assert!(ya > 0.4 && ya < 1.0, "下の箱は床に着く, ya={ya}");
        assert!(yb > ya + 0.7 && yb < ya + 1.6, "上の箱は下の箱に積もる, yb={yb} ya={ya}");
        world.sync(&mut doc);
        assert_eq!(doc.props[0].position[1], ya);
    }

    #[test]
    fn deterministic_same_input_same_result() {
        let doc = doc_with_box(4.0);
        let mut a = PhysicsWorld::from_doc(&doc);
        let mut b = PhysicsWorld::from_doc(&doc);
        for _ in 0..180 {
            a.step(1.0 / 60.0);
            b.step(1.0 / 60.0);
        }
        assert_eq!(a.position("prop:box"), b.position("prop:box"));
    }

    #[test]
    fn set_velocity_throws_box() {
        let doc = doc_with_box(2.0);
        let mut world = PhysicsWorld::from_doc(&doc);
        assert!(world.set_velocity("prop:box", [3.0, 0.0, 0.0]));
        assert!(!world.set_velocity("prop:nope", [0.0; 3]));
        let x0 = world.position("prop:box").unwrap()[0];
        for _ in 0..30 {
            world.step(1.0 / 60.0);
        }
        let x1 = world.position("prop:box").unwrap()[0];
        assert!(x1 > x0 + 0.5, "横に吹き飛ぶ, x0={x0} x1={x1}");
    }

    #[test]
    fn walker_is_kinematic_and_keeps_game_position() {
        // キネマティック: 重力で落ちない（位置はゲーム所有）。
        let mut doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            player: Some(crate::world_doc::WorldWalker {
                id: "walker:player".into(),
                kind: "walker".into(),
                name: "player".into(),
                position: [0.0, 3.0, 0.0],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        assert!(world.is_kinematic("walker:player"), "歩行者はキネマティック");
        for _ in 0..300 {
            world.sync_walkers(&doc);
            world.step(1.0 / 60.0);
        }
        world.sync(&mut doc);
        let p = doc.player.as_ref().unwrap();
        assert_eq!(p.position, [0.0, 3.0, 0.0], "sync は歩行者位置を上書きしない");
    }

    #[test]
    fn kinematic_walker_pushes_dynamic_box() {
        // 歩行者が箱に向かって進むと、キネマティックが動的箱を押す。
        let mut doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            props: vec![WorldProp {
                id: "prop:box".into(),
                kind: "prop".into(),
                name: "box".into(),
                model: "box".into(),
                position: [2.0, 0.6, 0.0],
                scale: [1.0, 1.0, 1.0],
                enabled: true,
                is_static: false,
                ..Default::default()
            }],
            player: Some(crate::world_doc::WorldWalker {
                id: "walker:player".into(),
                kind: "walker".into(),
                name: "player".into(),
                position: [0.0, 1.0, 0.0],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        let mut px = 0.0f32;
        for _ in 0..240 {
            px += 0.02; // 歩行者が +X へ進む（ゲーム所有の移動）
            doc.player.as_mut().unwrap().position[0] = px;
            world.sync_walkers(&doc);
            world.step(1.0 / 60.0);
        }
        let box_x = world.position("prop:box").unwrap()[0];
        assert!(box_x > 2.2, "歩行者が箱を押す, box_x={box_x}");
    }

    #[test]
    fn sphere_and_capsule_colliders_work() {
        let mut doc = WorldDoc {
            version: crate::world_doc::WORLD_DUMP_VERSION,
            half: 10.0,
            floor_y: 0.0,
            props: vec![
                WorldProp {
                    id: "prop:ball".into(),
                    kind: "prop".into(),
                    name: "ball".into(),
                    model: "sphere".into(),
                    position: [0.0, 3.0, 0.0],
                    scale: [1.0, 1.0, 1.0],
                    enabled: true,
                    is_static: false,
                    ..Default::default()
                },
                WorldProp {
                    id: "prop:pill".into(),
                    kind: "prop".into(),
                    name: "pill".into(),
                    model: "capsule".into(),
                    position: [2.0, 4.0, 0.0],
                    scale: [0.6, 1.8, 0.6],
                    enabled: true,
                    is_static: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut world = PhysicsWorld::from_doc(&doc);
        for _ in 0..360 {
            world.step(1.0 / 60.0);
        }
        // 球（半径 0.5）は床に着地 → 中心 y ≈ 0.5
        let by = world.position("prop:ball").unwrap()[1];
        assert!(by > 0.4 && by < 0.9, "球は床に落ちる, by={by}");
        // カプセルは床に落ちて転がって安定する。直立なら中心 y ≈ 0.9
        // （half 0.6 + r 0.3）、横倒しなら ≈ 0.3。0.3..1.0 の範囲。
        let py = world.position("prop:pill").unwrap()[1];
        assert!(py > 0.2 && py < 1.1, "カプセルは床に落ちて止まる, py={py}");
    }
}
