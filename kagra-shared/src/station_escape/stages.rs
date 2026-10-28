//! The stages that ship (the spec's defaults). Coordinates: metres, x right, y up; a block's
//! (x, y) is its bottom-left corner. Floors have their top at y = 0 unless they climb.

use super::{Block, Boss, Deco, Drone, Hint, Laser, Rect, Stage, Turret};

fn solid(x: f32, y: f32, w: f32, h: f32) -> Block {
    Block {
        x,
        y,
        w,
        h,
        ..Block::default()
    }
}

/// A floor slab whose top is at `top`.
fn floor(x0: f32, x1: f32, top: f32) -> Block {
    solid(x0, top - 3.0, x1 - x0, 3.0)
}

fn oneway(x: f32, top: f32, w: f32) -> Block {
    Block {
        kind: "oneway".into(),
        ..solid(x, top - 0.25, w, 0.25)
    }
}

fn crumble(x: f32, top: f32, w: f32) -> Block {
    Block {
        kind: "crumble".into(),
        ..solid(x, top - 0.4, w, 0.4)
    }
}

/// A platform that moves `to` (dx, dy) and back, `secs` each way.
fn mover(x: f32, top: f32, w: f32, to: [f32; 2], secs: f32) -> Block {
    Block {
        kind: "move".into(),
        to,
        secs,
        ..solid(x, top - 0.4, w, 0.4)
    }
}

fn laser(x: f32, y: f32, h: f32, on: f32, off: f32, phase: f32) -> Laser {
    Laser {
        x,
        y,
        w: 0.18,
        h,
        on,
        off,
        phase,
    }
}

fn drone(x0: f32, x1: f32, y: f32, speed: f32, phase: f32) -> Drone {
    Drone {
        x0,
        x1,
        y,
        speed,
        phase,
    }
}

/// A turret whose muzzle is at (x, y), firing left (-1) or right (+1).
fn turret(x: f32, y: f32, dir: f32, every: f32, phase: f32) -> Turret {
    Turret {
        x,
        y,
        dir,
        every,
        phase,
        speed: 6.5,
        range: 18.0,
    }
}

fn spikes(x0: f32, x1: f32, top: f32) -> Rect {
    Rect {
        x: x0,
        y: top,
        w: x1 - x0,
        h: 0.45,
        ..Rect::default()
    }
}

/// Spikes that stay out `on` seconds and pull in for `off`.
fn spikes_cycle(x0: f32, x1: f32, top: f32, on: f32, off: f32, phase: f32) -> Rect {
    Rect {
        on,
        off,
        phase,
        ..spikes(x0, x1, top)
    }
}

fn hint(x: f32, en: &str, ja: &str) -> Hint {
    Hint {
        x,
        en: en.into(),
        ja: ja.into(),
    }
}

fn deco(model: &str, x: f32, y: f32, z: f32, scale: f32, yaw: f32) -> Deco {
    Deco {
        model: format!("assets/models/kenney/space-kit/{model}.glb"),
        x,
        y,
        z: z * 1.6,
        scale,
        yaw,
    }
}

/// Stage 1, Docking Bay: the moves, one at a time.
pub fn docking_bay() -> Stage {
    Stage {
        name: "Docking Bay".into(),
        name_ja: "ドッキングベイ".into(),
        story: "The alarms woke you. The station is breaking up. The last shuttle waits in the hangar: get there.".into(),
        story_ja: "警報で目が覚めた。ステーションが崩れはじめている。最後のシャトルは格納庫。たどり着け。".into(),
        spawn: [1.5, 0.0],
        exit: [72.0, 0.0],
        kill_y: -5.0,
        par_secs: 45.0,
        blocks: vec![
            floor(-4.0, 12.0, 0.0),
            solid(-4.0, 0.0, 1.0, 8.0),
            floor(14.5, 26.0, 0.0),
            solid(26.0, -3.0, 4.0, 4.2),
            solid(30.0, -3.0, 4.0, 5.4),
            oneway(35.0, 3.4, 3.0),
            oneway(39.0, 4.4, 3.0),
            oneway(43.0, 3.4, 2.5),
            floor(45.0, 56.0, 0.0),
            floor(60.6, 76.0, 0.0),
            solid(76.0, 0.0, 1.0, 8.0),
        ],
        lasers: vec![laser(21.0, 0.0, 3.2, 1.3, 1.3, 0.0)],
        spikes: vec![spikes_cycle(48.0, 49.6, 0.0, 1.4, 1.4, 0.0)],
        cells: vec![
            [5.0, 1.0],
            [7.0, 1.0],
            [13.2, 2.4],
            [19.0, 1.0],
            [28.0, 2.4],
            [36.5, 4.4],
            [40.5, 5.4],
            [44.2, 4.4],
            [59.0, 2.2],
            [68.0, 1.0],
        ],
        checkpoints: vec![[16.0, 0.0], [47.0, 0.0]],
        hints: vec![
            hint(1.0, "Slide your left thumb to run", "左の親指をすべらせて走る"),
            hint(9.0, "JUMP: hold for higher", "ジャンプ：長押しで高く"),
            hint(17.0, "Lasers blink. Wait, then go", "レーザーは点滅する。待ってから進め"),
            hint(21.6, "Land on a drone, or DASH through it", "ドローンは上から踏むか、ダッシュで突き抜けろ"),
            hint(33.0, "Jump across the ledges", "足場から足場へ飛び移れ"),
            hint(46.5, "Spikes pull in. Wait, or jump them", "トゲは引っこむ。待つか、飛び越えろ"),
            hint(51.0, "Run, jump at the edge, then DASH in the air", "走って端でジャンプ、空中でダッシュ"),
        ],
        decos: vec![
            deco("hangar_smallA", 8.0, -0.2, -7.0, 4.5, 0.4),
            deco("structure_closed", 22.0, -0.2, -6.0, 4.0, 0.0),
            deco("machine_generatorLarge", 30.0, -0.2, -4.5, 3.0, 0.3),
            deco("satelliteDish", 41.0, -0.2, -9.0, 6.0, 0.8),
            deco("turret_single", 52.0, -0.2, -5.0, 3.0, -0.5),
            deco("monorail_trainBox", 60.0, -0.2, -6.0, 4.0, 1.57),
            deco("hangar_smallA", 72.0, -0.2, -6.5, 5.5, 0.0),
            deco("craft_speederC", 70.0, 0.3, -3.6, 1.6, 0.35),
            deco("rock_crystals", 15.0, -0.2, -3.5, 2.0, 0.0),
            deco("rock_crystals", 47.0, -0.2, -3.0, 2.0, 1.0),
        ],
        drones: vec![drone(22.8, 25.4, 1.0, 1.5, 0.0)],
        ..Stage::default()
    }
}

/// Stage 2, Reactor Ring: moving and crumbling ledges, timed lasers, a climb.
pub fn reactor_ring() -> Stage {
    Stage {
        name: "Reactor Ring".into(),
        name_ja: "リアクターリング".into(),
        story: "The hangar is sealed. The way round goes through the reactor ring. The floor is coming apart.".into(),
        story_ja: "格納庫は閉ざされていた。リアクターリングを抜けて回り込む。床が崩れていく。".into(),
        spawn: [2.0, 0.0],
        exit: [82.0, 8.4],
        kill_y: -6.0,
        par_secs: 70.0,
        blocks: vec![
            floor(-4.0, 8.0, 0.0),
            solid(-4.0, 0.0, 1.0, 8.0),
            mover(8.5, 0.0, 3.0, [7.0, 0.0], 2.6),
            crumble(20.0, 0.8, 2.0),
            crumble(23.5, 1.6, 2.0),
            crumble(27.0, 2.4, 2.0),
            floor(30.0, 40.0, 2.4),
            solid(40.0, -3.0, 1.0, 11.4),
            oneway(36.0, 4.4, 3.0),
            oneway(31.0, 6.4, 3.0),
            oneway(36.0, 8.4, 4.0),
            floor(41.0, 58.0, 8.4),
            mover(58.5, 8.4, 2.5, [0.0, 3.0], 2.0),
            floor(62.0, 66.0, 11.4),
            mover(66.5, 11.4, 2.5, [6.0, -3.0], 2.4),
            floor(76.0, 86.0, 8.4),
            solid(86.0, 8.4, 1.0, 8.0),
        ],
        lasers: vec![
            laser(34.0, 2.4, 2.0, 1.0, 1.4, 0.0),
            laser(45.0, 8.4, 3.0, 1.2, 1.0, 0.0),
            laser(48.5, 8.4, 3.0, 1.2, 1.0, 1.1),
            laser(51.5, 8.4, 3.0, 1.2, 1.0, 0.0),
        ],
        spikes: vec![spikes_cycle(54.0, 56.0, 8.4, 1.2, 1.0, 0.5)],
        cells: vec![
            [5.0, 1.0],
            [12.0, 1.4],
            [17.0, 1.4],
            [21.0, 2.0],
            [24.5, 2.8],
            [28.0, 3.6],
            [37.5, 5.6],
            [32.5, 7.6],
            [38.0, 9.6],
            [46.8, 9.4],
            [50.3, 9.4],
            [60.0, 13.0],
            [64.0, 12.6],
            [72.0, 11.0],
            [80.0, 9.4],
        ],
        checkpoints: vec![[31.0, 2.4], [42.0, 8.4], [63.0, 11.4]],
        hints: vec![
            hint(6.0, "Ride the lift", "リフトに乗れ"),
            hint(19.0, "These ledges fall. Keep moving", "この足場は落ちる。止まるな"),
            hint(33.0, "Climb", "のぼれ"),
        ],
        decos: vec![
            deco("machine_generatorLarge", 14.0, -3.0, -6.0, 6.0, 0.0),
            deco("structure_closed", 26.0, -3.0, -8.0, 7.0, 0.3),
            deco("rock_crystals", 33.0, 2.2, -3.5, 2.5, 0.5),
            deco("satelliteDish", 46.0, 8.2, -7.0, 5.0, -0.6),
            deco("turret_single", 55.0, 8.2, -4.0, 3.0, 0.4),
            deco("machine_generatorLarge", 64.0, 11.2, -4.5, 3.0, 1.2),
            deco("hangar_smallA", 82.0, 8.2, -6.5, 5.5, 0.0),
            deco("craft_speederC", 80.0, 8.7, -3.6, 1.6, 0.35),
        ],
        drones: vec![drone(4.5, 7.5, 1.0, 1.6, 0.5)],
        ..Stage::default()
    }
}

/// Stage 3, Cargo Spine: the station fights back. Drones to stomp or dash through, turrets
/// to jump over.
pub fn cargo_spine() -> Stage {
    Stage {
        name: "Cargo Spine".into(),
        name_ja: "カーゴスパイン".into(),
        story: "Security has woken up and does not know you. Drones patrol the cargo spine: land on them, or dash straight through.".into(),
        story_ja: "警備システムが目を覚ました。こちらを侵入者だと思っている。ドローンは上から踏むか、ダッシュで突き抜けろ。".into(),
        spawn: [1.5, 0.0],
        exit: [82.0, 0.0],
        kill_y: -5.0,
        par_secs: 60.0,
        blocks: vec![
            floor(-4.0, 18.0, 0.0),
            solid(-4.0, 0.0, 1.0, 8.0),
            floor(21.0, 52.0, 0.0),
            solid(38.5, 0.0, 2.0, 1.6),
            oneway(44.0, 3.2, 3.0),
            oneway(48.0, 3.2, 3.0),
            crumble(53.2, 0.0, 1.6),
            floor(56.5, 86.0, 0.0),
            solid(73.5, 0.0, 1.5, 1.6),
            solid(86.0, 0.0, 1.0, 8.0),
        ],
        lasers: vec![],
        spikes: vec![],
        drones: vec![
            drone(8.0, 14.0, 1.0, 2.0, 0.0),
            drone(43.0, 50.0, 1.0, 3.0, 0.3),
            drone(60.0, 68.0, 2.1, 2.5, 0.0),
            drone(60.0, 68.0, 1.0, 2.0, 0.5),
        ],
        turrets: vec![turret(38.3, 0.5, -1.0, 2.4, 1.0), turret(73.3, 0.5, -1.0, 2.0, 0.5)],
        cells: vec![
            [5.0, 1.0],
            [11.0, 2.4],
            [19.5, 2.4],
            [30.0, 1.0],
            [39.5, 2.6],
            [45.5, 4.2],
            [49.5, 4.2],
            [54.0, 1.6],
            [64.0, 3.4],
            [78.0, 1.0],
        ],
        checkpoints: vec![[22.0, 0.0], [57.5, 0.0]],
        hints: vec![
            hint(4.0, "Land on a drone, or DASH through it", "ドローンは上から踏むか、ダッシュで突き抜けろ"),
            hint(24.0, "Jump the turret's bolts", "砲台の弾を飛び越えろ"),
            hint(41.0, "Or take the high road", "上の道もある"),
        ],
        decos: vec![
            deco("monorail_trainBox", 10.0, -0.2, -6.0, 4.0, 1.57),
            deco("monorail_trainBox", 14.5, -0.2, -6.0, 4.0, 1.57),
            deco("structure_closed", 30.0, -0.2, -7.0, 4.5, 0.2),
            deco("machine_generatorLarge", 47.0, -0.2, -5.0, 3.0, 0.6),
            deco("satelliteDish", 62.0, -0.2, -9.0, 6.0, -0.4),
            deco("hangar_smallA", 82.0, -0.2, -6.5, 5.5, 0.0),
            deco("rock_crystals", 25.0, -0.2, -3.5, 2.0, 0.4),
        ],
        boss: None,
    }
}

/// Stage 4, Escape Pod: everything at once, up to the last shuttle.
pub fn escape_pod() -> Stage {
    Stage {
        name: "Escape Pod".into(),
        name_ja: "脱出ポッド".into(),
        story: "The shuttle bay is above the reactor. The station has minutes left. Everything you learned, all at once. Go!".into(),
        story_ja: "シャトルの格納庫はリアクターの真上。ステーションに残された時間はわずか。覚えたこと全部を使って、行け！".into(),
        spawn: [2.0, 0.0],
        exit: [74.0, 8.6],
        kill_y: -6.0,
        par_secs: 75.0,
        blocks: vec![
            floor(-4.0, 10.0, 0.0),
            solid(-4.0, 0.0, 1.0, 8.0),
            mover(10.5, 0.0, 3.0, [0.0, 4.0], 2.2),
            floor(14.0, 26.0, 4.0),
            oneway(26.5, 5.6, 3.5),
            oneway(30.0, 7.4, 3.0),
            floor(34.0, 46.5, 8.0),
            solid(45.0, 8.0, 1.5, 1.6),
            crumble(47.8, 8.0, 1.6),
            crumble(50.3, 8.6, 1.6),
            mover(52.8, 8.6, 2.5, [6.0, 0.0], 2.4),
            floor(62.0, 78.0, 8.6),
            solid(78.0, 8.6, 1.0, 8.0),
        ],
        lasers: vec![
            laser(20.0, 4.0, 3.0, 1.0, 1.2, 0.0),
            laser(66.0, 8.6, 3.0, 1.1, 1.0, 0.0),
            laser(69.5, 8.6, 3.0, 1.1, 1.0, 1.0),
        ],
        spikes: vec![],
        drones: vec![
            drone(21.8, 25.2, 5.0, 2.0, 0.0),
            drone(37.8, 43.0, 9.2, 2.5, 0.2),
            drone(71.5, 73.0, 9.6, 1.5, 0.0),
        ],
        turrets: vec![Turret {
            range: 10.5,
            ..turret(44.8, 8.5, -1.0, 2.2, 0.8)
        }],
        cells: vec![
            [6.0, 1.0],
            [12.0, 5.4],
            [17.0, 6.6],
            [24.0, 5.0],
            [28.2, 6.6],
            [31.5, 8.4],
            [40.0, 10.6],
            [48.6, 9.6],
            [56.0, 10.0],
            [64.0, 9.6],
            [68.0, 9.6],
            [72.0, 11.2],
        ],
        checkpoints: vec![[15.0, 4.0], [35.0, 8.0], [63.0, 8.6]],
        hints: vec![hint(8.0, "Last stretch. Don't stop", "最後のひとふんばり。止まるな")],
        decos: vec![
            deco("machine_generatorLarge", 6.0, -3.0, -6.0, 6.0, 0.0),
            deco("structure_closed", 22.0, 1.0, -8.0, 6.0, 0.3),
            deco("satelliteDish", 40.0, 7.8, -7.0, 5.0, -0.6),
            deco("turret_single", 58.0, 8.4, -4.0, 3.0, 0.4),
            deco("hangar_smallA", 74.0, 8.4, -6.5, 6.0, 0.0),
            deco("craft_speederC", 74.0, 8.9, -3.4, 2.0, 0.35),
        ],
        boss: None,
    }
}

/// Stage 5, Reactor Core: the Warden guards the last door.
pub fn reactor_core() -> Stage {
    Stage {
        name: "Reactor Core".into(),
        name_ja: "リアクターコア".into(),
        story: "The shuttle door is behind the reactor core, and the Warden guards it. When it swoops low, land on it or dash into it. Three hits.".into(),
        story_ja: "シャトルの扉はリアクターコアの奥。番人ウォーデンが守っている。低く突っこんでくる時に、上から踏むかダッシュで当てろ。3 回で倒せる。".into(),
        spawn: [1.0, 0.0],
        exit: [27.0, 0.0],
        kill_y: -5.0,
        par_secs: 60.0,
        blocks: vec![
            floor(-4.0, 30.0, 0.0),
            solid(-4.0, 0.0, 1.0, 10.0),
            solid(30.0, 0.0, 1.0, 10.0),
            oneway(7.0, 2.6, 2.5),
            oneway(19.5, 2.6, 2.5),
        ],
        cells: vec![[8.2, 3.6], [20.8, 3.6], [14.0, 1.0]],
        hints: vec![
            hint(0.0, "Stomp or DASH into the Warden when it swoops low", "低く来たウォーデンを踏むかダッシュで当てろ"),
        ],
        decos: vec![
            deco("machine_generatorLarge", 4.0, -0.2, -6.0, 5.0, 0.3),
            deco("machine_generatorLarge", 24.0, -0.2, -6.0, 5.0, -0.3),
            deco("structure_closed", 14.0, -0.2, -9.0, 8.0, 0.0),
            deco("hangar_smallA", 27.0, -0.2, -6.5, 5.0, 0.0),
            deco("craft_speederC", 27.5, 0.3, -3.4, 2.0, 0.35),
        ],
        boss: Some(Boss {
            x0: 3.0,
            x1: 25.0,
            ..Boss::default()
        }),
        ..Stage::default()
    }
}
