//! A contact sheet of chibi players in every pose, front and back, drawn on the CPU.
//!
//!     cargo run -p kagra-shared --example chibi_sheet -- sheet.png
use kagra_shared::game_core::chibi::{self, Facing, Kit, Pose};
use kagra_shared::game_core::portrait::{Expr, Face};
use kagra_shared::hud_art::{Painter, Stage};
use kagra_shared::raster2d;
use kagra_shared::scene::DrawList;

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "chibi_sheet.png".into());
    let mut list = DrawList {
        clear: [214, 196, 160, 255],
        ..Default::default()
    };
    let (w, h) = (1400u32, 760u32);
    let mut p = Painter::new(
        &mut list,
        Stage::fit(w as f32, h as f32, w as f32, h as f32),
    );
    let poses = [
        Pose::Stand,
        Pose::Ready,
        Pose::Run(0.25),
        Pose::Pitch(0.2),
        Pose::Pitch(0.5),
        Pose::Pitch(0.85),
        Pose::Bat(0.0),
        Pose::Bat(0.5),
        Pose::Bat(1.0),
        Pose::Squat,
        Pose::Cheer,
    ];
    let blue = Kit {
        number: Some(1),
        ..Kit::home([34, 78, 168, 255])
    };
    let red = Kit {
        helmet: true,
        number: Some(7),
        ..Kit::road([176, 40, 52, 255])
    };
    let catcher = Kit {
        gear: true,
        ..Kit::home([34, 78, 168, 255])
    };
    for (i, pose) in poses.iter().enumerate() {
        let x = 70.0 + i as f32 * 124.0;
        let kit = if matches!(pose, Pose::Squat) {
            &catcher
        } else {
            &blue
        };
        chibi::draw(
            &mut p,
            [x, 330.0],
            200.0,
            &Face::from_seed(11 + i as u64),
            Expr::Fiery,
            Facing::Front,
            *pose,
            kit,
        );
        let kit = if matches!(pose, Pose::Squat) {
            &catcher
        } else {
            &red
        };
        chibi::draw(
            &mut p,
            [x, 700.0],
            200.0,
            &Face::from_seed(40 + i as u64),
            Expr::Happy,
            Facing::Back,
            *pose,
            kit,
        );
    }
    raster2d::picture(&list, w, h)
        .save_png(std::path::Path::new(&out))
        .expect("write the sheet");
    println!("{out}");
}
