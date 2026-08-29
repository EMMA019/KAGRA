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
//! - 歩行者は動的剛体（カプセル）として入れ、床に落ちて箱に乗る。
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
}

/// prop の半辺（model → 形状）。y は半分の高さ（Rapier cuboid は中心基準）。
fn prop_half_extents(p: &WorldProp) -> Option<[f32; 3]> {
    let s = p.scale;
    match p.model.to_ascii_lowercase().as_str() {
        "box" | "crate" | "" => Some([s[0] * 0.5, s[1] * 0.5, s[2] * 0.5]),
        _ => None,
    }
}

/// コライダー形状を作る。box のみ（第一歩）。None は「物理に参加しない」。
fn collider_for(p: &WorldProp) -> Option<ColliderBuilder> {
    let half = prop_half_extents(p)?;
    Some(
        ColliderBuilder::cuboid(half[0], half[1], half[2])
            .friction(p.friction)
            .restitution(p.restitution),
    )
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
        // 歩行者（player + walkers）: カプセル剛体。箱に乗る・床に立つ。
        for w in doc.player.iter().chain(doc.walkers.iter()) {
            let body = this
                .inner
                .insert_body(RigidBodyBuilder::dynamic().translation(w.position.into()));
            let collider = ColliderBuilder::capsule_y(0.7, 0.28)
                .friction(0.0)
                .density(1.0);
            this.inner.insert_collider(collider, Some(body));
            this.dynamic.insert(w.id.clone(), body);
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

    /// 剛体位置を props / walkers に書き戻す。
    pub fn sync(&self, doc: &mut WorldDoc) {
        for p in &mut doc.props {
            if let Some(&body) = self.dynamic.get(&p.id) {
                if let Some(rb) = self.inner.bodies.get(body) {
                    p.position = rb.translation().to_array();
                }
            }
        }
        for w in doc.player.iter_mut().chain(doc.walkers.iter_mut()) {
            if let Some(&body) = self.dynamic.get(&w.id) {
                if let Some(rb) = self.inner.bodies.get(body) {
                    w.position = rb.translation().to_array();
                    let vel = rb.linvel();
                    w.on_ground = vel.y.abs() < 0.05 && rb.translation().y > -10.0;
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

    /// 動的剛体かどうか。
    pub fn is_dynamic(&self, id: &str) -> bool {
        self.dynamic.contains_key(id)
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
    fn walker_stands_on_ground() {
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
        for _ in 0..300 {
            world.step(1.0 / 60.0);
        }
        world.sync(&mut doc);
        let p = doc.player.as_ref().unwrap();
        assert!(p.position[1] > 0.8 && p.position[1] < 1.4, "歩行者は床に立つ, y={}", p.position[1]);
    }
}
