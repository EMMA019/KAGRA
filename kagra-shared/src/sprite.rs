//! 2D sprite/quad as the same `WorldDoc` as 3D.
//!
//! `model: "sprite"` / `"quad"` compiles to a standing XY card in
//! `WorldDoc::compile_scene` (shared wgpu 30 Scene3D). Platformer / 2D-action
//! need this so 2D art is not a second runtime. No new ECS, no RendererV2,
//! no VRM skin, no Rapier.

use crate::world_doc::{WorldDoc, WorldProp};

pub const GAME_ID: &str = "sprite_card";

pub fn is_sprite_prop(prop: &WorldProp) -> bool {
    matches!(
        prop.model.to_ascii_lowercase().as_str(),
        "sprite" | "quad"
    )
}

pub fn is_sprite(doc: &WorldDoc) -> bool {
    doc.props.iter().any(is_sprite_prop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_doc::{WorldDoc, MESH_QUAD};
    use crate::world_play::WorldPlay;

    const SPRITE_CARD: &str = include_str!("../tests/fixtures/sprite_card_world.json");

    #[test]
    fn sprite_dump_roundtrips_and_compiles_to_quad_batch() {
        let doc = WorldDoc::from_json(SPRITE_CARD).expect("parse sprite dump");
        assert!(is_sprite(&doc));
        assert_eq!(GAME_ID, "sprite_card");
        assert_eq!(doc.version, 1);
        let sprites: Vec<_> = doc.props.iter().filter(|p| is_sprite_prop(p)).collect();
        assert!(
            sprites.len() >= 2,
            "fixture needs standing cards, got {}",
            sprites.len()
        );
        assert!(sprites.iter().any(|p| p.model == "sprite"));
        assert!(sprites.iter().any(|p| p.model == "quad"));
        let again = doc.to_json().expect("emit");
        let doc2 = WorldDoc::from_json(&again).expect("reparse");
        assert_eq!(doc.stable_ids(), doc2.stable_ids());

        let scene = doc.compile_scene(16.0 / 9.0);
        let quad = scene
            .batches
            .iter()
            .find(|b| b.mesh == MESH_QUAD)
            .expect("compile_scene must emit a sprite/quad batch");
        assert!(
            quad.instances.len() >= 2,
            "standing cards share MESH_QUAD, got {}",
            quad.instances.len()
        );
        let meshes = doc.compile_meshes();
        assert!(
            meshes
                .iter()
                .any(|(id, m)| *id == MESH_QUAD && m.vertices.len() == 8),
            "compile_meshes must include the standing XY card"
        );
        // Floor plane and walker still compile — 2D cards sit in the 3D world.
        assert!(scene.batches.iter().any(|b| b.mesh.0 == 3), "floor plane");
        assert!(scene.instance_count() >= 4);
    }

    #[test]
    fn play_world_loads_sprite_dump_without_a_second_runtime() {
        let play = WorldPlay::from_json(SPRITE_CARD).unwrap();
        assert!(play.is_sprite());
        assert!(!play.is_action());
        assert!(!play.is_platformer());
        assert!(!play.is_rpg());
        assert!(!play.is_collectathon());
        let scene = play.doc.compile_scene(1.0);
        assert!(scene.batches.iter().any(|b| b.mesh == MESH_QUAD));
    }

    #[test]
    fn platformer_dump_compiles_sprite_on_same_world() {
        const HOP: &str = include_str!("../tests/fixtures/box_hop_world.json");
        let play = WorldPlay::from_json(HOP).unwrap();
        assert!(play.is_platformer());
        assert!(play.is_sprite());
        let scene = play.doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.batches.iter().any(|b| b.mesh == MESH_QUAD),
            "box-hop checkpoint card must compile as the same MESH_QUAD"
        );
        assert!(
            scene.batches.iter().any(|b| b.mesh.0 == 0),
            "platforms stay boxes on the same Scene3D"
        );
    }
}
