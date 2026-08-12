// src/fbx_loader.rs
// FBX アニメーションローダー – KaguraResult 対応版

use ufbx;
use crate::error::{KaguraError, KaguraResult};

#[derive(Clone)]
pub struct FbxBoneFrame {
    pub name: String,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub has_trans: bool,
}

pub struct FbxClip {
    pub name: String,
    pub frame_time: f64,
    pub frames: Vec<Vec<FbxBoneFrame>>,
}

fn qmul(a: [f32;4], b: [f32;4]) -> [f32;4] {
    let [ax,ay,az,aw]=a; let [bx,by,bz,bw]=b;
    qnorm([
        aw*bx + ax*bw + ay*bz - az*by,
        aw*by - ax*bz + ay*bw + az*bx,
        aw*bz + ax*by - ay*bx + az*bw,
        aw*bw - ax*bx - ay*by - az*bz,
    ])
}

fn qinv(q: [f32;4]) -> [f32;4] {
    [-q[0], -q[1], -q[2], q[3]]
}

fn qnorm(q: [f32;4]) -> [f32;4] {
    let l = (q[0]*q[0] + q[1]*q[1] + q[2]*q[2] + q[3]*q[3]).sqrt();
    if l < 1e-8 {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [q[0]/l, q[1]/l, q[2]/l, q[3]/l]
    }
}

fn to_q(q: &ufbx::Quat) -> [f32;4] {
    qnorm([q.x as f32, q.y as f32, q.z as f32, q.w as f32])
}

pub fn load_fbx_anim(path: &str) -> KaguraResult<Vec<FbxClip>> {
    let opts = ufbx::LoadOpts {
        target_axes: ufbx::CoordinateAxes::right_handed_y_up(),
        target_unit_meters: 1.0,
        ..Default::default()
    };
    let scene = ufbx::load_file(path, opts)
        .map_err(|e| KaguraError::FbxLoad(format!("FBX load error: {}", e.description)))?;

    // バインドポーズのローカル回転を収集
    let bind_local: Vec<[f32;4]> = scene.nodes.iter()
        .map(|n| to_q(&n.local_transform.rotation))
        .collect();

    let mut clips = Vec::new();

    for stack in &scene.anim_stacks {
        let clip_name = stack.element.name.to_string();
        let bake_opts = ufbx::BakeOpts {
            resample_rate: 60.0,
            ..Default::default()
        };
        let baked = ufbx::bake_anim(&scene, &stack.anim, bake_opts)
            .map_err(|e| KaguraError::FbxLoad(format!("Bake error: {}", e.description)))?;
        if baked.nodes.is_empty() {
            continue;
        }

        let num_frames = baked.nodes.iter()
            .map(|n| n.rotation_keys.len())
            .max()
            .unwrap_or(0);
        if num_frames == 0 {
            continue;
        }

        // Armature/Root の初期位置を記録（接地基準）
        let mut arm_base_x = 0.0f32;
        let mut arm_base_y = 0.0f32;
        let mut arm_base_z = 0.0f32;
        for bn in &baked.nodes {
            let name = &scene.nodes[bn.typed_id as usize].element.name;
            let ns = name.to_string();
            if ns == "Armature" || ns == "Root" || ns == "root" {
                if let Some(k) = bn.translation_keys.first() {
                    arm_base_x = k.value.x as f32;
                    arm_base_y = k.value.y as f32;
                    arm_base_z = k.value.z as f32;
                }
                break;
            }
        }

        let frame_time = 1.0 / 60.0_f64;
        let mut frames: Vec<Vec<FbxBoneFrame>> = vec![Vec::new(); num_frames];

        for baked_node in &baked.nodes {
            let node_idx = baked_node.typed_id as usize;
            let node = &scene.nodes[node_idx];
            let name = node.element.name.to_string();
            let bind_q = bind_local[node_idx];
            let rot_keys = &baked_node.rotation_keys;
            let trans_keys = &baked_node.translation_keys;
            let has_trans = !trans_keys.is_empty();
            let is_root = name == "Armature" || name == "Root" || name == "root";

            for fi in 0..num_frames {
                let frame_local = if fi < rot_keys.len() {
                    to_q(&rot_keys[fi].value)
                } else if !rot_keys.is_empty() {
                    to_q(&rot_keys.last().unwrap().value)
                } else {
                    bind_q
                };

                let delta = qmul(qinv(bind_q), frame_local);

                let t = if has_trans {
                    let ti = fi.min(trans_keys.len() - 1);
                    let kv = &trans_keys[ti].value;
                    if is_root {
                        [
                            (kv.x as f32) - arm_base_x,
                            (kv.y as f32) - arm_base_y,
                            (kv.z as f32) - arm_base_z,
                        ]
                    } else {
                        [kv.x as f32, kv.y as f32, kv.z as f32]
                    }
                } else {
                    [0.0, 0.0, 0.0]
                };

                frames[fi].push(FbxBoneFrame {
                    name: name.clone(),
                    translation: t,
                    rotation: delta,
                    has_trans,
                });
            }
        }

        log::info!("[FBX] '{}' frames={} nodes={}", clip_name, num_frames, baked.nodes.len());
        clips.push(FbxClip {
            name: clip_name,
            frame_time,
            frames,
        });
    }

    if clips.is_empty() {
        return Err(KaguraError::FbxLoad("FBX にアニメーションが見つかりませんでした".into()));
    }
    Ok(clips)
}