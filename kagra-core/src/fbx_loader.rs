// kagra-core/src/fbx_loader.rs
// ufbx を使った FBX アニメーション読み込み
// maturin develop 後、Python から load_fbx(path) で呼ぶ

use ufbx;

/// 1フレーム分のボーンデータ
/// Python 側の BvhMotion と同じ形式に合わせる
#[derive(Clone)]
pub struct FbxBoneFrame {
    pub name:        String,
    pub translation: [f32; 3],    // x, y, z
    pub rotation:    [f32; 4],    // qx, qy, qz, qw
    pub has_trans:   bool,
}

/// FBX アニメーションクリップ（1アニメーションスタック）
pub struct FbxClip {
    pub name:       String,
    pub frame_time: f64,          // 1フレームの秒数
    /// frames[frame_idx] = Vec<FbxBoneFrame>
    pub frames:     Vec<Vec<FbxBoneFrame>>,
}

/// クォータニオン delta = inv(bind) × abs を計算する
fn qmul_delta(bind: [f32; 4], abs: [f32; 4]) -> [f32; 4] {
    // inv(bind) = conjugate for unit quaternion
    let inv = [-bind[0], -bind[1], -bind[2], bind[3]];
    // quaternion multiply: inv × abs
    let [ax, ay, az, aw] = inv;
    let [bx, by, bz, bw] = abs;
    let rx = aw*bx + ax*bw + ay*bz - az*by;
    let ry = aw*by - ax*bz + ay*bw + az*bx;
    let rz = aw*bz + ax*by - ay*bx + az*bw;
    let rw = aw*bw - ax*bx - ay*by - az*bz;
    // normalize
    let len = (rx*rx + ry*ry + rz*rz + rw*rw).sqrt();
    if len < 1e-8 { return [0.0, 0.0, 0.0, 1.0]; }
    [rx/len, ry/len, rz/len, rw/len]
}

/// FBX ファイルからアニメーションを読み込む
/// 戻り値: Vec<FbxClip>（アニメーションスタックごと）
pub fn load_fbx_anim(path: &str) -> Result<Vec<FbxClip>, String> {
    // Y-up・メートル単位に正規化して読み込む
    let opts = ufbx::LoadOpts {
        target_axes:        ufbx::CoordinateAxes::right_handed_y_up(),
        target_unit_meters: 1.0,
        // Blender/Mixamo 等の座標系変換を正しく処理
        // AdjustTransforms: トップレベルノードに変換を適用（推奨）
        space_conversion:   ufbx::SpaceConversion::AdjustTransforms,
        ..Default::default()
    };

    let scene = ufbx::load_file(path, opts)
        .map_err(|e| format!("FBX load error: {}", e.description))?;

    let mut clips = Vec::new();

    for stack in &scene.anim_stacks {
        let clip_name = stack.element.name.to_string();

        // アニメーションをベイク（ufbx が全ての複雑な計算を処理）
        let bake_opts = ufbx::BakeOpts {
            resample_rate: 60.0,  // 60fps でサンプリング
            ..Default::default()
        };

        let baked = match ufbx::bake_anim(&scene, &stack.anim, bake_opts) {
            Ok(b)  => b,
            Err(e) => {
                log::warn!("FBX bake error for '{}': {}", clip_name, e.description);
                continue;
            }
        };

        if baked.nodes.is_empty() {
            continue;
        }

        // フレーム数を rotation_keys の最大値から取得
        let num_frames = baked.nodes.iter()
            .map(|n| n.rotation_keys.len())
            .max()
            .unwrap_or(0);

        if num_frames == 0 {
            continue;
        }

        let frame_time = 1.0 / 60.0_f64;

        // フレームごとに全ボーンのデータを収集
        let mut frames: Vec<Vec<FbxBoneFrame>> = vec![Vec::new(); num_frames];

        for baked_node in &baked.nodes {
            let node = &scene.nodes[baked_node.typed_id as usize];
            let name = node.element.name.to_string();

            // ── バインドポーズ回転を取得 ──────────────────────────
            // node.local_transform.rotation = FBX の静的バインドポーズ回転
            // これがアニメーション適用前の「素の回転」
            let bind = &node.local_transform.rotation;
            let bind_q = [bind.x as f32, bind.y as f32,
                          bind.z as f32, bind.w as f32];

            // 回転キーフレーム
            let rot_keys   = &baked_node.rotation_keys;
            let trans_keys = &baked_node.translation_keys;
            let has_trans  = !trans_keys.is_empty();

            for fi in 0..num_frames {
                // 絶対回転を取得
                let q_abs = if fi < rot_keys.len() {
                    let kq = &rot_keys[fi].value;
                    [kq.x as f32, kq.y as f32, kq.z as f32, kq.w as f32]
                } else if !rot_keys.is_empty() {
                    let kq = &rot_keys.last().unwrap().value;
                    [kq.x as f32, kq.y as f32, kq.z as f32, kq.w as f32]
                } else {
                    [0.0, 0.0, 0.0, 1.0]
                };

                // デルタ回転を計算: delta = inv(bind) × abs
                // inv(q) = conjugate(q) for unit quaternion
                let q = qmul_delta(bind_q, q_abs);

                // 位置
                let t = if has_trans {
                    let ti = fi.min(trans_keys.len() - 1);
                    let kv = &trans_keys[ti].value;
                    [kv.x as f32, kv.y as f32, kv.z as f32]
                } else {
                    [0.0, 0.0, 0.0]
                };

                frames[fi].push(FbxBoneFrame {
                    name:        name.clone(),
                    translation: t,
                    rotation:    q,
                    has_trans,
                });
            }
        }

        log::info!(
            "[FBX] clip='{}' frames={} bones={}",
            clip_name, num_frames, baked.nodes.len()
        );

        clips.push(FbxClip {
            name: clip_name,
            frame_time,
            frames,
        });
    }

    if clips.is_empty() {
        return Err("FBX にアニメーションが見つかりませんでした".into());
    }

    Ok(clips)
}
