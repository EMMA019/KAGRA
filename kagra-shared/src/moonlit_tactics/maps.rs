//! The classes and maps that ship (the spec's defaults).

use super::rules::{ClassSpec, MapSpec, Placed, Side};

const CHARS: &str = "assets/models/kenney/mini-characters/";
const GRAVE: &str = "assets/models/kenney/graveyard-kit/";

#[allow(clippy::too_many_arguments)]
fn class(
    id: &str,
    name: &str,
    name_ja: &str,
    model: String,
    hp: i32,
    atk: i32,
    def: i32,
    mv: i32,
    range: (i32, i32),
) -> ClassSpec {
    ClassSpec {
        id: id.into(),
        name: name.into(),
        name_ja: name_ja.into(),
        model,
        hp,
        atk,
        def,
        mv,
        range_min: range.0,
        range_max: range.1,
        ..ClassSpec::default()
    }
}

pub fn classes() -> Vec<ClassSpec> {
    vec![
        // The watch: four heroes.
        class(
            "knight",
            "Knight",
            "騎士",
            format!("{CHARS}character-male-a.glb"),
            24,
            8,
            5,
            4,
            (1, 1),
        ),
        class(
            "lancer",
            "Lancer",
            "槍兵",
            format!("{CHARS}character-male-e.glb"),
            19,
            9,
            3,
            5,
            (1, 1),
        ),
        class(
            "archer",
            "Archer",
            "弓兵",
            format!("{CHARS}character-female-d.glb"),
            18,
            7,
            2,
            4,
            (2, 3),
        ),
        ClassSpec {
            heal: 9,
            ..class(
                "cleric",
                "Cleric",
                "僧侶",
                format!("{CHARS}character-female-a.glb"),
                17,
                3,
                2,
                4,
                (1, 1),
            )
        },
        // The dead.
        class(
            "zombie",
            "Zombie",
            "ゾンビ",
            format!("{GRAVE}character-zombie.glb"),
            17,
            7,
            2,
            3,
            (1, 1),
        ),
        class(
            "skeleton",
            "Skeleton",
            "スケルトン",
            format!("{GRAVE}character-skeleton.glb"),
            13,
            8,
            1,
            4,
            (1, 1),
        ),
        ClassSpec {
            flies: true,
            ..class(
                "ghost",
                "Ghost",
                "ゴースト",
                format!("{GRAVE}character-ghost.glb"),
                11,
                6,
                0,
                5,
                (1, 2),
            )
        },
        class(
            "vampire",
            "Vampire",
            "ヴァンパイア",
            format!("{GRAVE}character-vampire.glb"),
            20,
            9,
            3,
            4,
            (1, 1),
        ),
        ClassSpec {
            scale: 2.1,
            ..class(
                "keeper",
                "The Keeper",
                "墓守",
                format!("{GRAVE}character-keeper.glb"),
                28,
                9,
                3,
                3,
                (1, 2),
            )
        },
    ]
}

fn at(class: &str, side: Side, x: i32, y: i32) -> Placed {
    Placed {
        class: class.into(),
        side,
        x,
        y,
        leader: false,
    }
}

/// Map 1, Chapel Yard: learn the moves. Clear every foe.
pub fn chapel_yard() -> MapSpec {
    use Side::{Foe, Hero};
    MapSpec {
        name: "Chapel Yard".into(),
        name_ja: "礼拝堂の庭".into(),
        story: "The bells rang at midnight and the graves opened. Hold the chapel yard: clear every one of the dead.".into(),
        story_ja: "真夜中に鐘が鳴り、墓が開いた。礼拝堂の庭を守れ。亡者をすべて倒せ。".into(),
        rows: vec![
            "..#.#..".into(),
            ".T...T.".into(),
            "...C...".into(),
            "#.T...#".into(),
            "...~...".into(),
            ".T...T.".into(),
            "..#.#..".into(),
            ".......".into(),
            ".......".into(),
        ],
        units: vec![
            at("knight", Hero, 3, 7),
            at("lancer", Hero, 1, 7),
            at("archer", Hero, 2, 8),
            at("cleric", Hero, 4, 8),
            at("zombie", Foe, 1, 1),
            at("zombie", Foe, 5, 1),
            at("skeleton", Foe, 3, 1),
            at("ghost", Foe, 6, 4),
        ],
        par_turns: 8,
    }
}

/// Map 2, Keeper's Gate: the Keeper leads the dead. Bring it down to win.
pub fn keepers_gate() -> MapSpec {
    use Side::{Foe, Hero};
    MapSpec {
        name: "Keeper's Gate".into(),
        name_ja: "墓守の門".into(),
        story: "The dead answer to the Keeper at the cemetery gate. Bring the Keeper down and the rest fall with it.".into(),
        story_ja: "亡者を操るのは、墓地の門に立つ墓守。墓守を倒せば、残りも崩れ落ちる。".into(),
        rows: vec![
            "#.#C#.#".into(),
            ".......".into(),
            ".T#.#T.".into(),
            ".......".into(),
            "#~.T.~#".into(),
            ".......".into(),
            ".T.#.T.".into(),
            ".......".into(),
            ".......".into(),
        ],
        units: vec![
            at("knight", Hero, 3, 7),
            at("lancer", Hero, 5, 7),
            at("archer", Hero, 2, 8),
            at("cleric", Hero, 4, 8),
            Placed {
                leader: true,
                ..at("keeper", Foe, 3, 1)
            },
            at("vampire", Foe, 1, 1),
            at("skeleton", Foe, 5, 1),
            at("zombie", Foe, 1, 3),
            at("ghost", Foe, 6, 3),
        ],
        par_turns: 10,
    }
}
