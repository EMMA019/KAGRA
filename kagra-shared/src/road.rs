//! 道路の経路。GPU に依存しない。
//!
//! Catmull-Rom スプラインで制御点を結び、弧長テーブルを作ってから
//! 「距離 s での位置と向き」を取り出す。道のメッシュもチャンクも、
//! ここから取り出したフレームに乗せるだけ。

use glam::{Mat3, Mat4, Quat, Vec3};

/// 経路上の 1 点。路面の「今ここ」を表す。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoadFrame {
    pub pos: Vec3,
    /// 進行方向。正規化済み。
    pub tangent: Vec3,
    /// 進行方向の右。正規化済み。
    pub right: Vec3,
    /// ほぼ +Y。急カーブでも路面が傾かないよう、上は世界の Y に固定する。
    pub up: Vec3,
    /// 経路始点からの弧長（m）。
    pub distance: f32,
}

impl RoadFrame {
    /// 路面上のローカル座標 `(lateral, height, along)` をワールドへ。
    /// 道路セグメントのローカル空間は「幅が X、長さが Z」。
    pub fn model(&self, lateral: f32, height: f32, along: f32) -> Mat4 {
        let pos = self.pos + self.right * lateral + self.up * height + self.tangent * along;
        let rot = Quat::from_mat3(&Mat3::from_cols(self.right, self.up, self.tangent));
        Mat4::from_rotation_translation(rot, pos)
    }

    pub fn heading(&self) -> f32 {
        self.tangent.x.atan2(self.tangent.z)
    }
}

/// 弧長パラメータ化された道路経路。
#[derive(Clone, Debug)]
pub struct RoadPath {
    /// `distance` は単調増加。
    samples: Vec<RoadFrame>,
    length: f32,
}

impl RoadPath {
    /// 制御点から Catmull-Rom 経路を作る。点は xz 平面上で十分。
    ///
    /// 端点は延長して端のセグメントも曲がるようにする。閉じたループにはしない
    /// （配送ルートは往復の方が自然なので、S4 では開いた経路で足りる）。
    pub fn from_waypoints(points: &[Vec3], samples_per_segment: usize) -> Self {
        assert!(points.len() >= 2, "need at least two waypoints");
        let n = samples_per_segment.max(2);

        // 端を延長した仮想点。始点・終点でも接線が消えないようにする。
        let mut pts = Vec::with_capacity(points.len() + 2);
        pts.push(points[0] * 2.0 - points[1]);
        pts.extend_from_slice(points);
        pts.push(points[points.len() - 1] * 2.0 - points[points.len() - 2]);

        let mut raw = Vec::with_capacity((pts.len() - 3) * n + 1);
        for i in 0..pts.len() - 3 {
            for k in 0..=n {
                // セグメント境界の二重登録を避ける。
                if k == 0 && i > 0 {
                    continue;
                }
                let t = k as f32 / n as f32;
                let p = catmull_rom(pts[i], pts[i + 1], pts[i + 2], pts[i + 3], t);
                let tangent = catmull_rom_tangent(pts[i], pts[i + 1], pts[i + 2], pts[i + 3], t);
                raw.push((p, tangent));
            }
        }

        let mut samples: Vec<RoadFrame> = Vec::with_capacity(raw.len());
        let mut distance = 0.0;
        for (i, (p, tangent)) in raw.into_iter().enumerate() {
            if i > 0 {
                distance += samples[i - 1].pos.distance(p);
            }
            samples.push(make_frame(p, tangent, distance));
        }

        let length = samples.last().map(|s| s.distance).unwrap_or(0.0);
        Self { samples, length }
    }

    /// デモ用の配送ルート。まっすぐ → S 字 → 長い直線 → 緩い右カーブ。
    pub fn demo_route() -> Self {
        let pts = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 200.0),
            Vec3::new(-40.0, 0.0, 320.0),
            Vec3::new(40.0, 0.0, 440.0),
            Vec3::new(0.0, 0.0, 560.0),
            Vec3::new(0.0, 0.0, 900.0),
            Vec3::new(-120.0, 0.0, 1100.0),
            Vec3::new(-280.0, 0.0, 1200.0),
            Vec3::new(-400.0, 0.0, 1400.0),
        ];
        Self::from_waypoints(&pts, 12)
    }

    pub fn length(&self) -> f32 {
        self.length
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// 弧長 `s`（0..length）でのフレーム。範囲外は端にクランプ。
    pub fn sample(&self, s: f32) -> RoadFrame {
        let s = s.clamp(0.0, self.length);
        if self.samples.is_empty() {
            return make_frame(Vec3::ZERO, Vec3::Z, 0.0);
        }
        if self.samples.len() == 1 {
            return self.samples[0];
        }

        let mut lo = 0usize;
        let mut hi = self.samples.len() - 1;
        while lo + 1 < hi {
            let mid = (lo + hi) / 2;
            if self.samples[mid].distance <= s {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let a = &self.samples[lo];
        let b = &self.samples[hi];
        let span = (b.distance - a.distance).max(1e-4);
        let t = ((s - a.distance) / span).clamp(0.0, 1.0);
        let pos = a.pos.lerp(b.pos, t);
        let tangent = a.tangent.lerp(b.tangent, t).normalize_or(Vec3::Z);
        make_frame(pos, tangent, s)
    }

    /// `origin` に最も近い経路上の点。粗い走査で十分（毎フレーム呼ばない想定）。
    pub fn nearest(&self, origin: Vec3) -> RoadFrame {
        let mut best = self.samples[0];
        let mut best_d = origin.distance_squared(best.pos);
        for s in &self.samples[1..] {
            let d = origin.distance_squared(s.pos);
            if d < best_d {
                best_d = d;
                best = *s;
            }
        }
        best
    }

    /// `start_s..end_s` を `step` 間隔で刻んだフレーム列。端を必ず含む。
    pub fn walk(&self, start_s: f32, end_s: f32, step: f32) -> Vec<RoadFrame> {
        let start = start_s.clamp(0.0, self.length);
        let end = end_s.clamp(0.0, self.length).max(start);
        let step = step.max(0.5);
        let mut out = Vec::new();
        let mut s = start;
        while s < end - 1e-3 {
            out.push(self.sample(s));
            s += step;
        }
        out.push(self.sample(end));
        out
    }
}

fn make_frame(pos: Vec3, tangent: Vec3, distance: f32) -> RoadFrame {
    let tangent = tangent.normalize_or(Vec3::Z);
    let up = Vec3::Y;
    let right = up.cross(tangent).normalize_or(Vec3::X);
    let tangent = right.cross(up).normalize_or(Vec3::Z);
    RoadFrame {
        pos,
        tangent,
        right,
        up,
        distance,
    }
}

/// 均一 Catmull-Rom。`t` は 0..1 で p1→p2。
fn catmull_rom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

fn catmull_rom_tangent(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    0.5 * ((-p0 + p2)
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * (2.0 * t)
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * (3.0 * t2))
}

/// 経路をチャンクに分け、トラック周辺だけを生かすストリーマ。
#[derive(Clone, Debug)]
pub struct RoadStreamer {
    pub path: RoadPath,
    /// 1 チャンクの弧長（m）。
    pub chunk_length: f32,
    /// 前方に残す距離。
    pub keep_ahead: f32,
    /// 後方に残す距離。
    pub keep_behind: f32,
    /// 近い LOD の境界（トラックからの弧長差）。
    pub near_dist: f32,
    /// 中間 LOD の境界。これより遠いと道だけ。
    pub mid_dist: f32,
}

impl Default for RoadStreamer {
    fn default() -> Self {
        Self {
            path: RoadPath::demo_route(),
            chunk_length: 80.0,
            keep_ahead: 400.0,
            keep_behind: 80.0,
            near_dist: 120.0,
            mid_dist: 260.0,
        }
    }
}

/// 距離に応じた景物の密度。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LodLevel {
    /// 道 + 中央線 + ポール。
    Near,
    /// 道 + ポール（間引き）。
    Mid,
    /// 道だけ。
    Far,
}

/// アクティブな 1 チャンク。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoadChunk {
    pub index: i32,
    pub start_s: f32,
    pub end_s: f32,
    pub lod: LodLevel,
}

impl RoadStreamer {
    pub fn chunk_count(&self) -> i32 {
        (self.path.length() / self.chunk_length).ceil().max(1.0) as i32
    }

    pub fn chunk_span(&self, index: i32) -> (f32, f32) {
        let start = index as f32 * self.chunk_length;
        let end = (start + self.chunk_length).min(self.path.length());
        (start, end)
    }

    /// トラックの弧長位置まわりで生きているチャンク。
    pub fn active_chunks(&self, truck_s: f32) -> Vec<RoadChunk> {
        let lo = (truck_s - self.keep_behind).max(0.0);
        let hi = (truck_s + self.keep_ahead).min(self.path.length());
        let first = (lo / self.chunk_length).floor() as i32;
        let last = ((hi / self.chunk_length).floor() as i32)
            .min(self.chunk_count() - 1)
            .max(0);
        let first = first.clamp(0, last);

        let mut out = Vec::with_capacity((last - first + 1) as usize);
        for index in first..=last {
            let (start_s, end_s) = self.chunk_span(index);
            let mid = (start_s + end_s) * 0.5;
            let d = (mid - truck_s).abs();
            let lod = if d <= self.near_dist {
                LodLevel::Near
            } else if d <= self.mid_dist {
                LodLevel::Mid
            } else {
                LodLevel::Far
            };
            out.push(RoadChunk {
                index,
                start_s,
                end_s,
                lod,
            });
        }
        out
    }

    /// 道メッシュの長さ（8m）以下に収める。隙間を重ねで埋める。
    pub fn segment_step(&self, lod: LodLevel) -> f32 {
        match lod {
            LodLevel::Near => 6.0,
            LodLevel::Mid => 7.0,
            LodLevel::Far => 8.0,
        }
    }

    pub fn pole_step(&self, lod: LodLevel) -> Option<f32> {
        match lod {
            LodLevel::Near => Some(24.0),
            LodLevel::Mid => Some(48.0),
            LodLevel::Far => None,
        }
    }

    pub fn wants_dashes(&self, lod: LodLevel) -> bool {
        matches!(lod, LodLevel::Near)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_path_length_matches_distance() {
        let path = RoadPath::from_waypoints(&[Vec3::ZERO, Vec3::new(0.0, 0.0, 100.0)], 8);
        assert!((path.length() - 100.0).abs() < 1.0, "len={}", path.length());
        let mid = path.sample(50.0);
        assert!((mid.pos.z - 50.0).abs() < 0.5);
        assert!(mid.tangent.z > 0.9);
    }

    #[test]
    fn curved_path_is_longer_than_chord() {
        let path = RoadPath::from_waypoints(
            &[
                Vec3::ZERO,
                Vec3::new(50.0, 0.0, 50.0),
                Vec3::new(0.0, 0.0, 100.0),
            ],
            16,
        );
        assert!(path.length() > 110.0, "len={}", path.length());
    }

    #[test]
    fn sample_is_monotonic_and_clamped() {
        let path = RoadPath::demo_route();
        let mut prev = 0.0;
        for i in 0..20 {
            let s = path.length() * i as f32 / 19.0;
            let f = path.sample(s);
            assert!(f.distance + 1e-3 >= prev);
            prev = f.distance;
        }
        let past = path.sample(path.length() + 50.0);
        assert!((past.distance - path.length()).abs() < 1e-3);
    }

    #[test]
    fn frame_basis_is_orthonormal() {
        let path = RoadPath::demo_route();
        for s in [0.0, 100.0, 400.0, path.length() * 0.9] {
            let f = path.sample(s);
            assert!((f.tangent.length() - 1.0).abs() < 1e-3);
            assert!((f.right.length() - 1.0).abs() < 1e-3);
            assert!(f.tangent.dot(f.right).abs() < 1e-3);
            assert!(f.up.y > 0.9);
        }
    }

    #[test]
    fn right_turn_moves_frame_toward_negative_x() {
        // +Z へ進みながら右（画面右 = -X）へ曲がる経路。
        let path = RoadPath::from_waypoints(
            &[
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 50.0),
                Vec3::new(-40.0, 0.0, 100.0),
            ],
            12,
        );
        let end = path.sample(path.length());
        assert!(end.pos.x < -10.0, "path should bend to -X, got {}", end.pos);
    }

    #[test]
    fn nearest_finds_the_bend() {
        let path = RoadPath::demo_route();
        let probe = Vec3::new(-40.0, 0.0, 320.0);
        let n = path.nearest(probe);
        assert!(n.pos.distance(probe) < 30.0);
    }

    #[test]
    fn streamer_keeps_a_window_around_the_truck() {
        let stream = RoadStreamer::default();
        let truck_s = 300.0;
        let chunks = stream.active_chunks(truck_s);
        assert!(!chunks.is_empty());
        let min_s = chunks
            .iter()
            .map(|c| c.start_s)
            .fold(f32::INFINITY, f32::min);
        let max_s = chunks.iter().map(|c| c.end_s).fold(0.0, f32::max);
        assert!(min_s <= truck_s - 40.0);
        assert!(max_s >= truck_s + 200.0);
    }

    #[test]
    fn lod_gets_coarser_with_distance() {
        let stream = RoadStreamer::default();
        let chunks = stream.active_chunks(200.0);
        let near = chunks
            .iter()
            .find(|c| (c.start_s - 200.0).abs() < stream.chunk_length)
            .unwrap();
        let far = chunks
            .iter()
            .max_by(|a, b| a.start_s.partial_cmp(&b.start_s).unwrap())
            .unwrap();
        assert_eq!(near.lod, LodLevel::Near);
        assert!(matches!(far.lod, LodLevel::Mid | LodLevel::Far));
        assert!(stream.segment_step(LodLevel::Far) > stream.segment_step(LodLevel::Near));
        assert!(stream.wants_dashes(LodLevel::Near));
        assert!(!stream.wants_dashes(LodLevel::Far));
        assert!(stream.pole_step(LodLevel::Far).is_none());
    }

    #[test]
    fn active_chunk_count_stays_bounded() {
        let stream = RoadStreamer::default();
        let n0 = stream.active_chunks(0.0).len();
        let n1 = stream.active_chunks(stream.path.length() * 0.5).len();
        let n2 = stream.active_chunks(stream.path.length()).len();
        let max_expected =
            ((stream.keep_ahead + stream.keep_behind) / stream.chunk_length).ceil() as usize + 2;
        assert!(n0 <= max_expected && n1 <= max_expected && n2 <= max_expected);
    }

    #[test]
    fn walk_includes_endpoints() {
        let path = RoadPath::demo_route();
        let frames = path.walk(10.0, 50.0, 8.0);
        assert!((frames.first().unwrap().distance - 10.0).abs() < 1e-2);
        assert!((frames.last().unwrap().distance - 50.0).abs() < 1e-2);
    }
}
