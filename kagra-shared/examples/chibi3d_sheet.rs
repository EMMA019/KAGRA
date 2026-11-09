//! A row of 3D chibi players in every pose on a patch of grass, rendered offscreen (needs a
//! GPU or lavapipe).
//!
//!     cargo run -p kagra-shared --features render --example chibi3d_sheet -- sheet.png
use kagra_shared::game_core::chibi::{Kit, Pose};
use kagra_shared::game_core::chibi3d::{self, Figure};
use kagra_shared::game_core::portrait::{Expr, Face};
use kagra_shared::raster2d::Canvas;
use kagra_shared::world_doc::{WorldCamera, WorldDoc, WorldProp};

fn main() -> Result<(), String> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "chibi3d_sheet.png".into());
    // `close`: the first few faces up close.
    let close = std::env::args().nth(2).is_some_and(|a| a == "close");
    let (w, h) = (1600u32, 700u32);
    let mut doc = WorldDoc {
        version: 1,
        ground: Some(false),
        contact_blobs: Some(false),
        sky: Some([150, 200, 240, 255]),
        exposure: Some(1.0),
        fxaa: Some(true),
        ..WorldDoc::default()
    };
    doc.props.push(WorldProp {
        id: "grass".into(),
        kind: "prop".into(),
        model: "box".into(),
        position: [0.0, -0.05, 0.0],
        scale: [60.0, 0.1, 30.0],
        enabled: true,
        color: Some([92, 160, 76]),
        roughness: 1.0,
        ..WorldProp::default()
    });
    let poses = [
        Pose::Stand,
        Pose::Ready,
        Pose::Run(0.25),
        Pose::Pitch(0.3),
        Pose::Pitch(0.55),
        Pose::Pitch(0.7),
        Pose::Pitch(1.0),
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
        ..Kit::road([176, 40, 52, 255])
    };
    let catcher = Kit {
        gear: true,
        ..Kit::home([34, 78, 168, 255])
    };
    let faces: Vec<Face> = (0..poses.len() as u64)
        .map(|i| Face::from_seed(11 + i))
        .collect();
    let ids: Vec<String> = (0..poses.len()).map(|i| format!("f{i}")).collect();
    for (i, pose) in poses.iter().enumerate() {
        let x = (i as f32 - (poses.len() as f32 - 1.0) * 0.5) * 2.1;
        let kit = match pose {
            Pose::Squat => &catcher,
            Pose::Bat(_) => &red,
            _ => &blue,
        };
        let expr = match pose {
            Pose::Cheer => Expr::Happy,
            Pose::Pitch(_) => Expr::Fiery,
            _ => Expr::Neutral,
        };
        chibi3d::build(
            &Figure::new(&ids[i], [x, 0.0, 0.0], 0.35, 1.9, &faces[i], kit)
                .pose(*pose)
                .expr(expr)
                .mitt(matches!(pose, Pose::Squat)),
            &mut doc.props,
        );
    }
    doc.cameras.push(WorldCamera {
        id: "cam".into(),
        kind: "camera".into(),
        position: if close {
            [-8.0, 1.7, 4.0]
        } else {
            [0.0, 2.6, 15.0]
        },
        target: if close {
            [-8.0, 1.2, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        },
        fov: 34.0,
        ..WorldCamera::default()
    });
    let rgba = kagra_shared::render::render_world_doc(&doc, w, h)?;
    let mut c = Canvas::new(w, h, [0, 0, 0, 255]);
    c.rgba = rgba;
    c.save_png(std::path::Path::new(&out))
        .map_err(|e| e.to_string())?;
    println!("{out}");
    Ok(())
}
