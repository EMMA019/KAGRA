//! The world under the HUD: only a sky colour. Every screen of this game is an illustration
//! drawn by the HUD (`hud.rs`, `art.rs`), so the 3D pass has nothing to draw and costs the
//! phone almost nothing.

use super::DiamondDays;
use crate::world_doc::{WorldCamera, WorldDoc};

impl DiamondDays {
    pub(super) fn update_doc(&mut self) {
        if !self.doc.cameras.is_empty() {
            return;
        }
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(super::GAME_ID.into()),
            half: 8.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some([40, 60, 100, 255]),
            ..WorldDoc::default()
        };
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "sky".into(),
            position: [0.0, 2.0, 6.0],
            target: [0.0, 0.0, 0.0],
            fov: 45.0,
            roll: 0.0,
        });
        self.doc = doc;
    }

    pub(super) fn build_doc(&mut self) {
        self.update_doc();
    }
}
