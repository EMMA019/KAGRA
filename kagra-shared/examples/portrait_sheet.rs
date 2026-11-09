//! A contact sheet of line-art portraits, drawn on the CPU (no GPU).
//!
//!     cargo run -p kagra-shared --example portrait_sheet -- sheet.png
use kagra_shared::game_core::portrait::{self, Expr, Face, Outfit};
use kagra_shared::raster2d;
use kagra_shared::scene::DrawList;

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "portrait_sheet.png".into());
    let mut list = DrawList {
        clear: [236, 230, 218, 255],
        ..Default::default()
    };
    let exprs = [
        Expr::Neutral,
        Expr::Happy,
        Expr::Smile,
        Expr::Serious,
        Expr::Fiery,
        Expr::Sad,
        Expr::Surprised,
        Expr::Pain,
    ];
    let mut girl = Outfit::bare([236, 112, 124, 255]);
    girl.shirt = [236, 112, 124, 255];
    girl.trim = [255, 255, 255, 255];
    girl.long_hair = true;
    girl.ribbon = true;
    let girl_face = Face {
        skin: 0,
        hair_style: 2,
        hair_color: 1,
        eyes: 1,
        brows: 0,
        mouth: 1,
        width: 0.96,
        blush: true,
        glasses: false,
    };
    for (i, e) in exprs.iter().enumerate() {
        portrait::draw(
            &mut list.mesh,
            10.0 + i as f32 * 156.0,
            10.0,
            150.0,
            &girl_face,
            *e,
            &girl,
        );
        portrait::draw(
            &mut list.mesh,
            10.0 + i as f32 * 156.0,
            170.0,
            150.0,
            &Face::from_seed(i as u64 * 7 + 1),
            *e,
            &Outfit::cap([28, 72, 160, 255]),
        );
        portrait::draw(
            &mut list.mesh,
            10.0 + i as f32 * 156.0,
            330.0,
            150.0,
            &Face::from_seed(i as u64 * 13 + 2),
            *e,
            &Outfit::bare([28, 72, 160, 255]),
        );
    }
    let mut coach = Outfit::cap([160, 40, 50, 255]);
    coach.shirt = [58, 62, 80, 255];
    coach.trim = [200, 204, 214, 255];
    coach.adult = true;
    portrait::draw(
        &mut list.mesh,
        10.0,
        490.0,
        150.0,
        &Face::from_seed(99),
        Expr::Neutral,
        &coach,
    );
    portrait::draw(
        &mut list.mesh,
        166.0,
        490.0,
        150.0,
        &Face::from_seed(99),
        Expr::Smile,
        &coach,
    );
    for i in 0..10 {
        portrait::draw(
            &mut list.mesh,
            340.0 + i as f32 * 50.0,
            500.0,
            44.0,
            &Face::from_seed(200 + i),
            Expr::Neutral,
            &Outfit::cap([28, 72, 160, 255]),
        );
        portrait::draw(
            &mut list.mesh,
            340.0 + i as f32 * 50.0,
            560.0,
            34.0,
            &Face::from_seed(300 + i),
            Expr::Happy,
            &Outfit::cap([160, 40, 50, 255]),
        );
    }
    portrait::draw(
        &mut list.mesh,
        900.0,
        420.0,
        300.0,
        &girl_face,
        Expr::Happy,
        &girl,
    );
    let t0 = std::time::Instant::now();
    let pic = raster2d::picture(&list, 1280, 720);
    pic.save_png(std::path::Path::new(&out)).expect("write png");
    println!(
        "{} triangles, raster {:?} -> {out}",
        list.mesh.triangles(),
        t0.elapsed()
    );
}
