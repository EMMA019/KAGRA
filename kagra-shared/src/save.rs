//! セーブデータと設定。ファイル I/O は持たず、JSON の読み書きだけをする。
//!
//! シェル側が `asset_root/save.json` などに書き出す。Wasm なら localStorage、
//! モバイルならアプリの Documents へ、という分担。

use crate::driving::DrivingScene;
use crate::session::{SceneKind, SharedSession};
use crate::vehicle::Truck;
use glam::Vec3;
use serde::{Deserialize, Serialize};

pub const SAVE_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// 0..1。シェルが実音量に掛ける。
    pub master_volume: f32,
    /// ハンドル感度。1 が既定。
    pub steer_sensitivity: f32,
    pub muted: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            master_volume: 0.8,
            steer_sensitivity: 1.0,
            muted: false,
        }
    }
}

impl Settings {
    pub fn clamped(self) -> Self {
        Self {
            master_volume: self.master_volume.clamp(0.0, 1.0),
            steer_sensitivity: self.steer_sensitivity.clamp(0.2, 2.5),
            muted: self.muted,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TruckSave {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub heading: f32,
    pub speed: f32,
}

impl TruckSave {
    pub fn from_truck(t: &Truck) -> Self {
        Self {
            x: t.pos.x,
            y: t.pos.y,
            z: t.pos.z,
            heading: t.heading,
            speed: t.speed,
        }
    }

    pub fn apply(&self, t: &mut Truck) {
        t.pos = Vec3::new(self.x, self.y, self.z);
        t.heading = self.heading;
        t.speed = self.speed.max(0.0);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SaveGame {
    pub version: u32,
    pub kind: SceneKind,
    pub truck: TruckSave,
    pub path_s: f32,
    pub odometer: f32,
    pub settings: Settings,
}

impl SaveGame {
    pub fn capture(session: &SharedSession) -> Self {
        Self {
            version: SAVE_VERSION,
            kind: session.kind,
            truck: TruckSave::from_truck(&session.driving.truck),
            path_s: session.driving.path_s,
            odometer: session.driving.odometer,
            settings: session.settings.clone(),
        }
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn from_json(s: &str) -> Result<Self, String> {
        let mut save: Self = serde_json::from_str(s).map_err(|e| e.to_string())?;
        if save.version > SAVE_VERSION {
            return Err(format!(
                "save version {} is newer than supported {}",
                save.version, SAVE_VERSION
            ));
        }
        save.settings = save.settings.clamped();
        Ok(save)
    }

    pub fn apply(&self, session: &mut SharedSession) {
        session.kind = self.kind;
        session.settings = self.settings.clone().clamped();
        apply_driving(&mut session.driving, self);
    }
}

fn apply_driving(driving: &mut DrivingScene, save: &SaveGame) {
    save.truck.apply(&mut driving.truck);
    driving.path_s = save.path_s.clamp(0.0, driving.streamer.path.length());
    driving.odometer = save.odometer.max(0.0);
    // カメラを車体の後ろへスナップし直す。
    driving.camera = crate::vehicle::ChaseCamera::default();
    driving
        .camera
        .update(&driving.truck, crate::scene::FIXED_DT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_truck_pose() {
        let mut s = SharedSession::default();
        s.driving.truck.pos = Vec3::new(10.0, 0.0, 40.0);
        s.driving.truck.heading = 0.5;
        s.driving.truck.speed = 12.0;
        s.driving.path_s = 120.0;
        s.driving.odometer = 99.0;
        s.settings.master_volume = 0.4;

        let json = SaveGame::capture(&s).to_json().unwrap();
        let mut loaded = SharedSession::default();
        SaveGame::from_json(&json).unwrap().apply(&mut loaded);

        assert!((loaded.driving.truck.pos.x - 10.0).abs() < 1e-4);
        assert!((loaded.driving.truck.heading - 0.5).abs() < 1e-4);
        assert!((loaded.driving.truck.speed - 12.0).abs() < 1e-4);
        assert!((loaded.driving.path_s - 120.0).abs() < 1e-4);
        assert!((loaded.settings.master_volume - 0.4).abs() < 1e-4);
    }

    #[test]
    fn rejects_future_versions() {
        let mut save = SaveGame::capture(&SharedSession::default());
        save.version = 999;
        let json = save.to_json().unwrap();
        assert!(SaveGame::from_json(&json).is_err());
    }

    #[test]
    fn clamps_settings_on_load() {
        let json = r#"{
            "version": 1,
            "kind": "Driving",
            "truck": {"x":0,"y":0,"z":0,"heading":0,"speed":0},
            "path_s": 0,
            "odometer": 0,
            "settings": {"master_volume": 9.0, "steer_sensitivity": 0.01, "muted": true}
        }"#;
        let save = SaveGame::from_json(json).unwrap();
        assert_eq!(save.settings.master_volume, 1.0);
        assert_eq!(save.settings.steer_sensitivity, 0.2);
        assert!(save.settings.muted);
    }
}
