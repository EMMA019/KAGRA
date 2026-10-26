//! The 3D world: the persistent prop list (`init_doc`), the per-frame edits (`build_doc`), the night (`darken`) and the tier hints.

use super::*;

impl SurvivorArena {
    /// `arena.dark`: from the blue-lit arena toward dim moonlight. Fill, sky, fog and the key
    /// light ease toward the night values; the floor and the walls lose some light.
    pub(super) fn darken(&self, doc: &mut WorldDoc) {
        let k = self.spec.arena.dark.clamp(0.0, 1.0);
        if k <= 0.0 {
            return;
        }
        let mix = |a: f32, b: f32| a + (b - a) * k;
        let mix3 = |a: [f32; 3], b: [f32; 3]| [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])];
        doc.exposure = doc.exposure.map(|e| mix(e, 0.85));
        doc.hemi_sky = doc.hemi_sky.map(|c| mix3(c, [0.14, 0.16, 0.30]));
        doc.hemi_ground = doc.hemi_ground.map(|c| mix3(c, [0.08, 0.07, 0.12]));
        doc.ibl = doc.ibl.map(|v| mix(v, 0.15));
        doc.sky = doc.sky.map(|c| {
            let m = mix3([c[0] as f32, c[1] as f32, c[2] as f32], [5.0, 7.0, 18.0]);
            [m[0] as u8, m[1] as u8, m[2] as u8, c[3]]
        });
        doc.vignette = doc.vignette.map(|v| mix(v, 0.5));
        doc.fog_start = doc.fog_start.map(|v| mix(v, 20.0));
        doc.fog_end = doc.fog_end.map(|v| mix(v, 48.0));
        if let Some(key) = doc.lights.get_mut(0) {
            key.radius = mix(key.radius, 12.0);
            key.intensity = mix(key.intensity, 1.5);
            key.color = key.color.map(|c| mix3(c, [0.70, 0.78, 1.0]));
        }
        if let Some(lantern) = doc.lights.get_mut(1) {
            lantern.radius = mix(lantern.radius, 6.0);
            lantern.intensity = mix(lantern.intensity, 2.2);
        }
        let dim = |c: [u32; 3], f: f32| c.map(|v| (v as f32 * mix(1.0, f)) as u32);
        for p in doc.props.iter_mut() {
            match p.name.as_str() {
                "floor" => p.color = p.color.map(|c| dim(c, 0.6)),
                "wall" => p.color = p.color.map(|c| dim(c, 0.62)),
                _ => {}
            }
        }
    }

    /// Build the persistent prop list once: scenery, then the pools (hero, foes,
    /// gems, shots). After this a frame only edits fields in place.
    pub(super) fn init_doc(&mut self) {
        let half = self.spec.arena.half;
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: half + 4.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some([12, 18, 40, 255]),
            hemi_sky: Some([0.30, 0.38, 0.62]),
            hemi_ground: Some([0.20, 0.20, 0.30]),
            env_map: Some("preset:studio".into()),
            shadow_softness: Some(1.0),
            bloom: Some(0.42),
            bloom_threshold: Some(0.95),
            vignette: Some(0.35),
            lut: Some("punch".into()),
            lut_strength: Some(0.3),
            fxaa: Some(true),
            exposure: Some(1.0),
            ibl: Some(0.3),
            tonemap: Some(true),
            cull: Some(true),
            prop_lod: Some(80.0),
            fog_start: Some(40.0),
            fog_end: Some(90.0),
            ..WorldDoc::default()
        };
        doc.quality = self.picture_quality.clone();
        let keep = self.spec.fx.foe_detail[self.tier.min(2)];
        if keep < 0.98 {
            doc.mesh_lod = Some(crate::world_doc::MeshLod {
                keep,
                models: vec![GRAVEYARD.to_string() + "character-"],
            });
        }
        if self.tier == 2 && self.spec.fx.high_post {
            doc.shadow_softness = Some(2.6);
            doc.ssao = Some(10.0);
            doc.bloom = doc.bloom.map(|b| b * 1.3);
        }
        // One tiny buried prop per model keeps the renderer's glTF table constant.
        let hero_path = hero_glb(&self.spec.hero.skin);
        let mut wanted: Vec<String> = vec![
            hero_path.to_string(),
            BLASTER_GLB.to_string(),
            GHOST_GLB.to_string(),
            ZOMBIE_GLB.to_string(),
            GEM_GLB.to_string(),
        ];
        wanted.extend(FOE_GLBS.iter().map(|m| m.to_string()));
        wanted.push(CHEST_GLB.to_string());
        wanted.push(BLADE_GLB.to_string());
        wanted.push(WALL_GLB.to_string());
        wanted.push(COLUMN_GLB.to_string());
        wanted.extend(DECOR.iter().map(|(n, _, _)| decor_path(n)));
        wanted.extend(TREES.iter().map(|(n, _)| decor_path(n)));
        let models: Vec<String> = wanted.into_iter().filter(|m| glb_ready(m)).collect();
        for (i, m) in models.iter().enumerate() {
            doc.props.push(glb_prop(
                &format!("anchor-{i}"),
                m,
                "anchor",
                [0.0, -60.0, 60.0],
                [1e-4; 3],
                [0, 0, 0],
                0.0,
                None,
            ));
        }
        let has = |p: &str| models.iter().any(|m| m == p);
        let glb_hero = has(hero_path);
        self.slots.glb_gun = glb_hero && has(BLASTER_GLB);
        let glb_foe = has(GHOST_GLB);
        self.slots.glb_chest = has(CHEST_GLB);
        self.slots.glb_blade = has(BLADE_GLB);
        self.slots.glb_gem = has(GEM_GLB);
        let a = &self.spec.arena;
        let side = half * 2.0;
        let mut floor = prop(
            "floor",
            "box",
            "floor",
            [0.0, -0.12, 0.0],
            [side + 8.0, 0.24, side + 8.0],
            [255, 255, 255],
            0.0,
            0.9,
            0.0,
        );
        floor.texture = Some(FLOOR_TEX.into());
        let reps = (side + 8.0) / FLOOR_TILE_M;
        floor.uv_scale = Some([reps, reps]);
        doc.props.push(floor);
        // Graveyard dressing: a fixed scatter in the outer ring so the floor shows
        // the hero moving and the middle stays clear. Primitive pebbles when the
        // models are missing.
        let mut r = Rng::new(77);
        let mut n_decor = 0;
        for (name, scale, count) in DECOR {
            let path = decor_path(name);
            for _ in 0..count {
                let (mut x, mut z) = (0.0f32, 0.0f32);
                while x.abs().max(z.abs()) < half * 0.45 {
                    x = r.range(-half + 1.0, half - 1.0);
                    z = r.range(-half + 1.0, half - 1.0);
                }
                let yaw = r.range(0.0, std::f32::consts::TAU);
                let mut p = if has(&path) {
                    glb_prop(
                        &format!("decor-{n_decor}"),
                        &path,
                        "decor",
                        [x, 0.0, z],
                        [scale; 3],
                        [165, 170, 210],
                        0.0,
                        None,
                    )
                } else {
                    prop(
                        &format!("decor-{n_decor}"),
                        "sphere",
                        "decor",
                        [x, 0.12, z],
                        [0.4, 0.25, 0.4],
                        [70, 80, 100],
                        0.0,
                        0.9,
                        0.0,
                    )
                };
                p.yaw = yaw;
                doc.props.push(p);
                n_decor += 1;
            }
        }
        // The boundary: 2 m stone wall pieces just outside the playable square, a column
        // on each corner. Plain boxes when the models are missing.
        let (wall_ok, col_ok) = (has(WALL_GLB), has(COLUMN_GLB));
        let pieces = (side / 2.0).ceil() as i32 + 1;
        let off = half + 0.45;
        let mut n_wall = 0;
        for side_i in 0..4 {
            for k in 0..pieces {
                let along = -half - 0.9 + k as f32 * 2.0;
                let (x, z, yaw) = match side_i {
                    0 => (along, -off, 0.0),
                    1 => (along, off, std::f32::consts::PI),
                    2 => (-off, along, std::f32::consts::FRAC_PI_2),
                    _ => (off, along, -std::f32::consts::FRAC_PI_2),
                };
                if wall_ok {
                    let mut p = glb_prop(
                        &format!("wall-{n_wall}"),
                        WALL_GLB,
                        "wall",
                        [x, 0.0, z],
                        [2.0, 1.1, 1.5],
                        STONE_TINT,
                        0.0,
                        None,
                    );
                    p.yaw = yaw;
                    doc.props.push(p);
                } else if k == 0 {
                    let (pos, sc) = if side_i < 2 {
                        (
                            [0.0, 0.4, off * if side_i == 0 { -1.0 } else { 1.0 }],
                            [side + 1.0, 0.8, 0.5],
                        )
                    } else {
                        (
                            [off * if side_i == 2 { -1.0 } else { 1.0 }, 0.4, 0.0],
                            [0.5, 0.8, side + 1.0],
                        )
                    };
                    doc.props.push(prop(
                        &format!("wall-{n_wall}"),
                        "box",
                        "wall",
                        pos,
                        sc,
                        a.wall,
                        0.45,
                        0.6,
                        0.0,
                    ));
                }
                n_wall += 1;
            }
        }
        if col_ok {
            for (i, (sx, sz)) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                .into_iter()
                .enumerate()
            {
                doc.props.push(glb_prop(
                    &format!("column-{i}"),
                    COLUMN_GLB,
                    "wall",
                    [sx * (half + 0.6), 0.0, sz * (half + 0.6)],
                    [2.4, 2.4, 2.4],
                    STONE_TINT,
                    0.0,
                    None,
                ));
            }
        }
        // Pines ringed outside the wall.
        let mut n_tree = 0;
        for (name, count) in TREES {
            let path = decor_path(name);
            for _ in 0..count {
                if !has(&path) {
                    continue;
                }
                let a_ = r.range(0.0, std::f32::consts::TAU);
                let ring = half + r.range(2.6, 4.2);
                let (c, s_) = (a_.cos(), a_.sin());
                // Push the angle out to the square ring so trees hug the walls.
                let m = c.abs().max(s_.abs());
                let sc = r.range(1.5, 2.3);
                let mut p = glb_prop(
                    &format!("tree-{n_tree}"),
                    &path,
                    "decor",
                    [c / m * ring, 0.0, s_ / m * ring],
                    [sc; 3],
                    [110, 150, 135],
                    0.0,
                    None,
                );
                p.yaw = r.range(0.0, std::f32::consts::TAU);
                doc.props.push(p);
                n_tree += 1;
            }
        }
        self.slots.shadow = doc.props.len();
        doc.props.push(prop(
            "hero-shadow",
            "sphere",
            "shadow",
            [0.0, 0.02, 0.0],
            [0.95, 0.02, 0.95],
            [6, 8, 16],
            0.0,
            1.0,
            0.0,
        ));
        self.slots.hero = doc.props.len();
        if glb_hero {
            doc.props.push(glb_prop(
                "hero-body",
                hero_path,
                "hero",
                [0.0, 0.0, 0.0],
                [HERO_MODEL_SCALE; 3],
                [255, 255, 255],
                0.12,
                Some(lin(HERO_GLOW)),
            ));
        } else {
            doc.props.push(prop(
                "hero-body",
                "sphere",
                "hero",
                [0.0, 0.4, 0.0],
                [0.8, 0.8, 0.8],
                HERO_GLOW,
                0.35,
                0.5,
                0.0,
            ));
        }
        self.slots.gun = doc.props.len();
        if self.slots.glb_gun {
            doc.props.push(glb_prop(
                "hero-gun",
                BLASTER_GLB,
                "gun",
                [0.0, -50.0, 0.0],
                [GUN_SCALE; 3],
                [255, 255, 255],
                0.1,
                Some(lin(HERO_GLOW)),
            ));
        }
        self.slots.glb_foe = glb_foe;
        let cap = self.foes.cap();
        for m in 0..POOLS {
            // A missing model has no pool: its foes draw as ghosts (or spheres).
            self.slots.pool_cap[m] = if m == 0 {
                if glb_foe {
                    cap * POOL_SHARE[0] / 100 + 4
                } else {
                    cap
                }
            } else if glb_foe && has(FOE_GLBS[m]) {
                cap * POOL_SHARE[m] / 100 + 4
            } else {
                0
            };
        }
        let foe_prop = |id: String, m: usize| {
            let mut p = if glb_foe {
                glb_prop(
                    &id,
                    FOE_GLBS[m],
                    "foe",
                    [0.0, -50.0, 0.0],
                    [1.0; 3],
                    POOL_TINT[m],
                    0.06,
                    Some([0.2, 0.8, 0.3]),
                )
            } else {
                prop(
                    &id,
                    "sphere",
                    "foe",
                    [0.0, -50.0, 0.0],
                    [0.8; 3],
                    [120, 240, 150],
                    0.15,
                    0.5,
                    0.0,
                )
            };
            p.enabled = false;
            p
        };
        for m in 0..POOLS {
            self.slots.pool0[m] = doc.props.len();
            for i in 0..self.slots.pool_cap[m] {
                doc.props.push(foe_prop(format!("foe{m}-{i}"), m));
            }
        }
        self.slots.gem0 = doc.props.len();
        for i in 0..self.spec.caps.gems as usize {
            let mut p = if self.slots.glb_gem {
                glb_prop(
                    &format!("gem-{i}"),
                    GEM_GLB,
                    "gem",
                    [0.0, -50.0, 0.0],
                    [1.3; 3],
                    [255, 255, 255],
                    0.9,
                    Some([0.35, 0.9, 1.0]),
                )
            } else {
                prop(
                    &format!("gem-{i}"),
                    "sphere",
                    "gem",
                    [0.0, -50.0, 0.0],
                    [0.28; 3],
                    [90, 230, 255],
                    1.6,
                    0.3,
                    0.0,
                )
            };
            p.enabled = false;
            doc.props.push(p);
        }
        self.slots.shot0 = doc.props.len();
        for i in 0..self.spec.caps.shots as usize {
            let mut p = prop(
                &format!("shot-{i}"),
                "sphere",
                "shot",
                [0.0, -50.0, 0.0],
                [0.24, 0.24, 0.6],
                [255, 150, 60],
                1.2,
                0.4,
                0.0,
            );
            p.enabled = false;
            doc.props.push(p);
        }
        // Enemy orbs.
        self.slots.orb0 = doc.props.len();
        for i in 0..ORB_CAP {
            let mut p = prop(
                &format!("orb-{i}"),
                "sphere",
                "orb",
                [0.0, -50.0, 0.0],
                [ORB_R * 2.0; 3],
                [200, 120, 255],
                2.2,
                0.3,
                0.0,
            );
            p.enabled = false;
            doc.props.push(p);
        }
        // The orbiting blades.
        self.slots.blade0 = doc.props.len();
        for i in 0..BLADE_CAP {
            let mut p = if self.slots.glb_blade {
                glb_prop(
                    &format!("blade-{i}"),
                    BLADE_GLB,
                    "blade",
                    [0.0, -50.0, 0.0],
                    [BLADE_SCALE; 3],
                    [255, 255, 255],
                    0.9,
                    Some([0.55, 0.85, 1.0]),
                )
            } else {
                prop(
                    &format!("blade-{i}"),
                    "box",
                    "blade",
                    [0.0, -50.0, 0.0],
                    [0.12, 0.08, 0.9],
                    [150, 220, 255],
                    1.2,
                    0.3,
                    0.0,
                )
            };
            p.enabled = false;
            doc.props.push(p);
        }
        // Pickups: a glowing orb and a soft ground glow each, plus a few chest models.
        let ncap = self.spec.drops.cap as usize;
        self.slots.pick0 = doc.props.len();
        for i in 0..ncap {
            let mut p = prop(
                &format!("pick-{i}"),
                "sphere",
                "pickup",
                [0.0, -50.0, 0.0],
                [0.36; 3],
                [255, 255, 255],
                1.6,
                0.3,
                0.0,
            );
            p.enabled = false;
            doc.props.push(p);
        }
        self.slots.glow0 = doc.props.len();
        for i in 0..ncap {
            let mut p = prop(
                &format!("glow-{i}"),
                "sphere",
                "glow",
                [0.0, -50.0, 0.0],
                [0.9, 0.03, 0.9],
                [255, 255, 255],
                1.0,
                1.0,
                0.0,
            );
            p.enabled = false;
            doc.props.push(p);
        }
        self.slots.chest0 = doc.props.len();
        for i in 0..CHEST_CAP {
            let mut p = if self.slots.glb_chest {
                glb_prop(
                    &format!("chest-{i}"),
                    CHEST_GLB,
                    "chest",
                    [0.0, -50.0, 0.0],
                    [CHEST_SCALE; 3],
                    [255, 235, 190],
                    0.5,
                    Some([1.0, 0.8, 0.3]),
                )
            } else {
                prop(
                    &format!("chest-{i}"),
                    "box",
                    "chest",
                    [0.0, -50.0, 0.0],
                    [0.7, 0.5, 0.5],
                    [255, 200, 80],
                    0.6,
                    0.5,
                    0.0,
                )
            };
            p.enabled = false;
            doc.props.push(p);
        }
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "arena".into(),
            position: [0.0, 20.0, 11.0],
            target: [0.0, 0.0, 0.0],
            fov: 50.0,
            roll: 0.0,
        });
        doc.lights.push(light(
            0,
            "key",
            [0.0, 9.0, 3.0],
            [0.0; 3],
            [0.78, 0.86, 1.0],
            1.1,
            34.0,
            false,
        ));
        // A warm lantern that follows the hero: it lifts the hero off the cold floor.
        doc.lights.push(light(
            1,
            "lantern",
            [0.0, 2.4, 0.0],
            [0.0; 3],
            [1.0, 0.78, 0.5],
            1.3,
            7.0,
            false,
        ));
        // Effect clouds (one per kind and third of life) and shot streaks: fixed lists whose
        // vectors are refilled in place each frame.
        for (k, (rgb, size, alpha)) in FX_LOOK.iter().enumerate() {
            for b in 0..3 {
                doc.points.push(crate::strokes::PointCloud {
                    id: format!("fx-{k}-{b}"),
                    positions: Vec::with_capacity(self.fx_cap),
                    color: [rgb[0], rgb[1], rgb[2], alpha[b]],
                    size: size[b],
                });
            }
        }
        for i in 0..self.spec.caps.shots {
            doc.lines.push(crate::strokes::FatLine {
                id: format!("trail-{i}"),
                points: Vec::with_capacity(2),
                color: [150, 235, 255, 150],
                width: 3.0,
            });
        }
        // The lock-on ring and the ground-aim line (empty unless used).
        let line = |id: String, cap: usize, color: [u8; 4], width: f32| crate::strokes::FatLine {
            id,
            points: Vec::with_capacity(cap),
            color,
            width,
        };
        self.slots.ln_ring = doc.lines.len();
        doc.lines.push(line(
            "lock-ring".into(),
            LOCK_SEG + 1,
            [255, 96, 96, 235],
            4.0,
        ));
        self.slots.ln_aim = doc.lines.len();
        doc.lines
            .push(line("aim-line".into(), 2, [255, 220, 120, 150], 3.0));
        // Loot beams, lightning, floor rings, the shield bubble and the blades' track.
        self.slots.ln_beam = doc.lines.len();
        for i in 0..ncap {
            doc.lines
                .push(line(format!("beam-{i}"), 2, [255, 255, 255, 120], 5.0));
        }
        self.slots.ln_zap = doc.lines.len();
        for i in 0..ZAP_CAP {
            doc.lines.push(line(
                format!("zap-{i}"),
                (ZAP_PTS - 1) * 4 + 2,
                [255, 245, 130, 255],
                5.0,
            ));
        }
        self.slots.ln_fring = doc.lines.len();
        for i in 0..RING_CAP {
            doc.lines.push(line(
                format!("ring-{i}"),
                RING_SEG + 1,
                [255, 255, 255, 200],
                5.0,
            ));
        }
        self.slots.ln_shield = doc.lines.len();
        doc.lines.push(line(
            "shield".into(),
            RING_SEG + 1,
            [120, 220, 255, 200],
            4.0,
        ));
        self.slots.ln_orbit = doc.lines.len();
        doc.lines
            .push(line("orbit".into(), RING_SEG + 1, [150, 220, 255, 50], 3.0));
        self.darken(&mut doc);
        self.doc = doc;
        self.apply_hints();
    }

    /// Thin the graveyard dressing to the tier's `atmo` share (every n-th piece stays, so the
    /// ring never gaps on one side). Static props only; the sim has no say in them.
    pub(super) fn apply_hints(&mut self) {
        let keep = self.hint_atmo.clamp(0.0, 1.0);
        let total = self.doc.props.iter().filter(|p| p.name == "decor").count();
        let shown = (total as f32 * keep).ceil() as usize;
        for (k, p) in self
            .doc
            .props
            .iter_mut()
            .filter(|p| p.name == "decor")
            .enumerate()
        {
            // Bresenham-style spread: piece k stays when the running share steps up.
            let on = total > 0 && (k + 1) * shown / total > k * shown / total;
            p.enabled = on;
        }
    }

    pub(super) fn build_doc(&mut self) {
        if self.doc.props.is_empty() {
            self.init_doc();
        }
        let (w, h) = self.size.get();
        let (mut eye, mut target, fov) = self.camera(w / h.max(1.0));
        let fov = fov * (1.0 - 0.035 * self.zoom_kick * self.spec.fx.zoom_pulse);
        let sh = self.core.juice.shake.min(1.0) * 0.25 * self.spec.fx.shake;
        eye[0] += (self.anim_t * 61.0).sin() * sh;
        eye[2] += (self.anim_t * 47.0).sin() * sh;
        target[0] += (self.anim_t * 61.0).sin() * sh;
        target[2] += (self.anim_t * 47.0).sin() * sh;
        let (hx, hz, face) = (self.hx, self.hz, self.face);
        let speed = self.vx.hypot(self.vz);
        let bob = if speed > 0.4 {
            0.09 * (self.anim_t * 12.0).sin().abs()
        } else {
            0.02 * (self.anim_t * 3.0).sin()
        };
        let blink = self.hurt_cd > 0.0 && (self.anim_t * 24.0) as i32 % 2 == 0;
        let (hero_em, hero_glow) = if blink {
            (1.6, [1.0, 0.25, 0.25])
        } else if self.shield_t > 0.0 {
            (0.55, [0.4, 0.85, 1.0])
        } else if self.frenzy_t > 0.0 {
            (0.6, [1.0, 0.6, 0.2])
        } else {
            (0.12, lin(HERO_GLOW))
        };
        let mut slots = self.slots;
        // The blaster is in her hands only while she owns something that shoots from it.
        // Blades, lightning and the pulse have no muzzle, and a gun beside two orbiting
        // swords read as three weapons while the HUD showed one.
        let gun_held = self.spec.weapons.iter().enumerate().any(|(i, w)| {
            self.weapon_lv[i] > 0
                && matches!(
                    w.kind,
                    WeaponKind::Projectile | WeaponKind::Spread | WeaponKind::Missile
                )
        });
        let glb_hero = props_has_model(&self.doc.props[slots.hero]);
        let props = &mut self.doc.props;
        {
            let p = &mut props[slots.shadow];
            p.position = [hx, 0.02, hz];
        }
        {
            let p = &mut props[slots.hero];
            p.yaw = face;
            if glb_hero {
                p.position = [hx, bob, hz];
                p.emissive = Some(hero_em);
                p.emissive_color = Some(hero_glow);
                // A little squash as the hero runs (the models are rest-pose statics).
                let sq = if speed > 0.4 {
                    0.05 * (self.anim_t * 12.0).sin()
                } else {
                    0.0
                };
                p.scale = [
                    HERO_MODEL_SCALE * (1.0 + sq),
                    HERO_MODEL_SCALE * (1.0 - sq),
                    HERO_MODEL_SCALE * (1.0 + sq),
                ];
            } else {
                p.position = [hx, 0.4 + bob, hz];
                p.color = Some(if blink { [255, 60, 60] } else { HERO_GLOW });
            }
        }
        if slots.glb_gun {
            let (sn, cs) = face.sin_cos();
            let p = &mut props[slots.gun];
            p.enabled = gun_held;
            p.position = [hx + sn * GUN_FWD, bob + GUN_UP, hz + cs * GUN_FWD];
            p.yaw = face + std::f32::consts::PI;
            p.emissive = Some(hero_em * 0.5);
        }
        let n = self.foes.len();
        let mut drawn = [0usize; POOLS];
        for i in 0..n {
            let k = &self.kinds[self.foes.kind[i] as usize];
            let mut m = match k.model {
                MODEL_MIX => usize::from(self.foes.uid[i].is_multiple_of(4)),
                v => v as usize,
            };
            if slots.pool_cap[m] == 0 {
                m = 0; // that model is missing: draw it as the ghost
            }
            if drawn[m] >= slots.pool_cap[m] {
                continue;
            }
            drawn[m] += 1;
            let idx = slots.pool0[m] + drawn[m] - 1;
            let p = &mut props[idx];
            let age = self.foes.age[i];
            let pop = (age / 0.25).min(1.0);
            let mk = if slots.glb_foe { FOE_MODEL_K } else { 2.2 };
            let d = k.radius * k.scale * mk * (0.4 + 0.6 * pop);
            let (phase, tempo) = motion::phase_tempo(self.foes.uid[i]);
            let pose = motion::pose(
                POOL_MOTION[m],
                self.anim_t,
                phase,
                tempo,
                self.spec.fx.foe_motion,
                self.foes.flash[i],
            );
            let (x, z) = (self.foes.x[i], self.foes.z[i]);
            p.enabled = true;
            // The ghost and the vampire float a little; the others walk on the ground.
            let float = if slots.glb_foe && (m == 0 || k.model == 3) {
                0.12
            } else {
                0.0
            };
            let rest = if slots.glb_foe { float } else { d * 0.4 };
            p.position = [x, rest + pose.lift * d.min(1.5), z];
            p.yaw = (hx - x).atan2(hz - z);
            if slots.glb_foe {
                // A prop's +pitch lifts its nose (YXZ, front +Z toward the hero): lean is minus.
                p.pitch = -pose.pitch;
                p.roll = pose.roll;
            }
            p.scale = [d * pose.wide, d * pose.tall, d * pose.wide];
            let fl = self.foes.flash[i] * 10.0;
            let glow = if k.elite {
                0.35 + 0.25 * (self.anim_t * 4.0).sin()
            } else {
                0.06
            };
            p.emissive = Some(glow + 2.2 * fl);
            p.emissive_color = Some(if fl > 0.0 { [1.0, 1.0, 1.0] } else { k.linear });
            if slots.glb_foe {
                let base = POOL_TINT[m];
                p.color = Some([
                    base[0] * k.tint[0] / 255,
                    base[1] * k.tint[1] / 255,
                    base[2] * k.tint[2] / 255,
                ]);
            } else {
                p.color = Some(k.color);
            }
        }
        for m in 0..POOLS {
            for j in drawn[m]..slots.drawn[m] {
                props[slots.pool0[m] + j].enabled = false;
            }
        }
        slots.drawn = drawn;
        self.slots = slots;
        let gn = self.gems.len();
        for i in 0..self.spec.caps.gems as usize {
            let p = &mut props[slots.gem0 + i];
            if i >= gn {
                if p.enabled {
                    p.enabled = false;
                }
                continue;
            }
            let g = &self.gems[i];
            p.enabled = true;
            let bob = 0.08 * (self.anim_t * 5.0 + i as f32).sin();
            // A merged gem is bigger (and goldens past a handful of XP).
            let m = g.v.max(1.0).powf(0.33).min(2.0);
            if slots.glb_gem {
                p.position = [g.x, 0.12 + bob * m, g.z];
                p.yaw = self.anim_t * 2.2 + i as f32;
                p.scale = [1.3 * m; 3];
            } else {
                p.position = [g.x, 0.3 + bob, g.z];
                p.scale = [0.28 * m; 3];
            }
            let gold = g.v >= 6.0;
            p.color = Some(match (gold, slots.glb_gem) {
                (true, _) => [255, 215, 110],
                (false, true) => [255, 255, 255],
                (false, false) => [90, 230, 255],
            });
            if slots.glb_gem {
                p.emissive_color = Some(if gold {
                    [1.0, 0.7, 0.2]
                } else {
                    [0.35, 0.9, 1.0]
                });
            }
        }
        let sn = self.shots.len();
        for i in 0..self.spec.caps.shots as usize {
            let p = &mut props[slots.shot0 + i];
            if i >= sn {
                if p.enabled {
                    p.enabled = false;
                }
                continue;
            }
            let s = &self.shots[i];
            let wc = self
                .spec
                .weapons
                .get(s.w as usize)
                .map_or([255, 190, 90], |w| w.color.map(|c| c.min(255) as u8));
            p.enabled = true;
            p.position = [s.x, 0.55, s.z];
            p.yaw = s.vx.atan2(s.vz);
            p.color = Some(wc.map(u32::from));
            p.emissive_color = Some(lin(wc.map(u32::from)));
            // A petal is a small flat disc that tumbles. It used to be an egg-shaped bar
            // with a long streak, which read as a second and third gun barrel.
            p.emissive = Some(if s.kind == 1 { 0.7 } else { 1.2 });
            if s.kind == 1 {
                p.yaw = self.anim_t * 11.0 + i as f32 * 1.7;
            }
            p.scale = match s.kind {
                1 => [s.r * 1.6, s.r * 0.45, s.r * 1.6],
                2 => {
                    let pulse = 1.0 + 0.15 * (self.anim_t * 14.0 + i as f32).sin();
                    [s.r * 1.7 * pulse; 3]
                }
                _ => [s.r * 0.8, s.r * 0.8, s.r * 2.0],
            };
        }
        self.build_extras(&slots);
        for c in &mut self.doc.points {
            c.positions.clear();
        }
        for p in &self.fx {
            let b = ((p.age / p.life * 3.0) as usize).min(2);
            self.doc.points[p.kind as usize * 3 + b]
                .positions
                .push([p.x, p.y, p.z]);
        }
        let streak = self.spec.fx.trails;
        let nshots = self.spec.caps.shots as usize;
        for (i, l) in self.doc.lines.iter_mut().take(nshots).enumerate() {
            l.points.clear();
            // No streak behind petals: they are discs, not bolts.
            if let (true, Some(s)) = (streak, self.shots.get(i).filter(|s| s.kind != 1)) {
                let wc = self
                    .spec
                    .weapons
                    .get(s.w as usize)
                    .map_or([255, 190, 90], |w| w.color.map(|c| c.min(255) as u8));
                let len = if s.kind == 2 { 0.16 } else { 0.05 };
                l.points.push([s.x - s.vx * len, 0.58, s.z - s.vz * len]);
                l.points.push([s.x, 0.58, s.z]);
                l.color = [wc[0], wc[1], wc[2], 150];
                l.width = if s.kind == 2 { 6.0 } else { 3.0 };
            }
        }
        let lock = if self.aim_mode != 0 {
            self.lock_foe()
        } else {
            None
        };
        let aim_dir = if self.aim_mode != 0 && lock.is_none() {
            self.aim_dir
        } else {
            None
        };
        let (head, tail) = self.doc.lines.split_at_mut(slots.ln_aim);
        if let (Some(ring), Some(aim)) = (head.get_mut(slots.ln_ring), tail.first_mut()) {
            ring.points.clear();
            aim.points.clear();
            if let Some(j) = lock {
                let r = self.kinds[self.foes.kind[j] as usize].radius
                    + 0.3
                    + 0.07 * (self.anim_t * 7.0).sin();
                let spin = self.anim_t * 1.5;
                for k in 0..=LOCK_SEG {
                    let a = spin + k as f32 / LOCK_SEG as f32 * std::f32::consts::TAU;
                    ring.points.push([
                        self.foes.x[j] + a.sin() * r,
                        0.07,
                        self.foes.z[j] + a.cos() * r,
                    ]);
                }
            }
            if let Some(a) = aim_dir {
                let (sn, cs) = a.sin_cos();
                aim.points.push([hx + sn * 1.2, 0.3, hz + cs * 1.2]);
                aim.points.push([hx + sn * 4.5, 0.3, hz + cs * 4.5]);
            }
        }
        let cam = &mut self.doc.cameras[0];
        cam.position = eye;
        cam.target = target;
        cam.fov = fov;
        let l = &mut self.doc.lights[0];
        l.position = [hx, 9.0, hz + 3.0];
        self.doc.lights[1].position = [hx, 2.4, hz + 0.8];
    }
}
