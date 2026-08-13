//! Bake 済み道路網（OSM → tools/osm_bake.py → *.kagra.json）。
//!
//! 座標系: 局所メートル、x=東、z=北、y=上。GPU に依存しない。

use crate::road::RoadPath;
use glam::Vec3;
use serde::Deserialize;

/// 手書きの小さいグリッド（単体テスト・フォールバック用）。
pub const DEMO_CITY_JSON: &str = include_str!("../assets/maps/demo_city.kagra.json");
/// OSMnx Bake の渋谷周辺（Corridor Haul 既定）。
pub const SHIBUYA_DEMO_JSON: &str = include_str!("../assets/maps/shibuya_demo.kagra.json");

#[derive(Clone, Debug, Deserialize)]
pub struct MapFile {
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub origin_lonlat: [f64; 2],
    #[serde(default)]
    pub nodes: Vec<MapNode>,
    #[serde(default)]
    pub edges: Vec<MapEdge>,
    #[serde(default)]
    pub buildings: Vec<MapBuilding>,
    #[serde(default)]
    pub spawn_node: u32,
    #[serde(default)]
    pub mission_end_node: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MapNode {
    pub id: u32,
    pub x: f32,
    pub z: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MapEdge {
    pub id: u32,
    pub from: u32,
    pub to: u32,
    #[serde(default = "default_highway")]
    pub highway: String,
    #[serde(default = "default_width")]
    pub width: f32,
    #[serde(default)]
    pub oneway: bool,
    /// 中心線。各点は [x, z]。
    pub points: Vec<[f32; 2]>,
}

fn default_highway() -> String {
    "residential".into()
}
fn default_width() -> f32 {
    8.0
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct MapBuilding {
    pub x: f32,
    pub z: f32,
    #[serde(default)]
    pub yaw: f32,
    pub sx: f32,
    pub sy: f32,
    pub sz: f32,
}

#[derive(Clone, Debug)]
pub struct RoadNetwork {
    pub name: String,
    pub origin_lonlat: [f64; 2],
    pub nodes: Vec<MapNode>,
    pub edges: Vec<MapEdge>,
    pub buildings: Vec<MapBuilding>,
    pub spawn_node: u32,
    pub mission_end_node: u32,
    /// 空間インデックス: cell -> edge ids
    grid: Vec<Vec<u32>>,
    cell: f32,
    origin_xz: [f32; 2],
    grid_w: i32,
    grid_h: i32,
}

impl RoadNetwork {
    pub fn demo_city() -> Result<Self, String> {
        Self::from_json(DEMO_CITY_JSON)
    }

    /// プレイ用既定。Bake 済み OSM → 失敗時は手書きグリッド。
    pub fn default_playable() -> Self {
        Self::from_json(SHIBUYA_DEMO_JSON)
            .or_else(|_| Self::demo_city())
            .expect("bundled map")
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let file: MapFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if file.version > 1 {
            return Err(format!("unsupported map version {}", file.version));
        }
        if file.nodes.is_empty() || file.edges.is_empty() {
            return Err("map has no nodes/edges".into());
        }
        Self::from_file(file)
    }

    pub fn from_file(file: MapFile) -> Result<Self, String> {
        let cell = 40.0;
        let (min_x, max_x, min_z, max_z) = bounds_of(&file.edges);
        let pad = cell;
        let origin_xz = [min_x - pad, min_z - pad];
        let grid_w = (((max_x + pad) - origin_xz[0]) / cell).ceil().max(1.0) as i32;
        let grid_h = (((max_z + pad) - origin_xz[1]) / cell).ceil().max(1.0) as i32;
        let mut grid = vec![Vec::new(); (grid_w * grid_h) as usize];

        for e in &file.edges {
            let mut seen = Vec::new();
            for p in &e.points {
                let cx = ((p[0] - origin_xz[0]) / cell).floor() as i32;
                let cz = ((p[1] - origin_xz[1]) / cell).floor() as i32;
                if cx < 0 || cz < 0 || cx >= grid_w || cz >= grid_h {
                    continue;
                }
                let idx = (cz * grid_w + cx) as usize;
                if !seen.contains(&idx) {
                    grid[idx].push(e.id);
                    seen.push(idx);
                }
            }
        }

        Ok(Self {
            name: file.name,
            origin_lonlat: file.origin_lonlat,
            nodes: file.nodes,
            edges: file.edges,
            buildings: file.buildings,
            spawn_node: file.spawn_node,
            mission_end_node: file.mission_end_node,
            grid,
            cell,
            origin_xz,
            grid_w,
            grid_h,
        })
    }

    pub fn node_pos(&self, id: u32) -> Option<Vec3> {
        self.nodes
            .iter()
            .find(|n| n.id == id)
            .map(|n| Vec3::new(n.x, 0.0, n.z))
    }

    pub fn edge(&self, id: u32) -> Option<&MapEdge> {
        self.edges.iter().find(|e| e.id == id)
    }

    /// プレイヤー周辺のエッジ（描画・衝突ストリーム用）。
    pub fn edges_near(&self, pos: Vec3, radius: f32) -> Vec<&MapEdge> {
        let r = radius.max(1.0);
        let min_cx = (((pos.x - r) - self.origin_xz[0]) / self.cell).floor() as i32;
        let max_cx = (((pos.x + r) - self.origin_xz[0]) / self.cell).floor() as i32;
        let min_cz = (((pos.z - r) - self.origin_xz[1]) / self.cell).floor() as i32;
        let max_cz = (((pos.z + r) - self.origin_xz[1]) / self.cell).floor() as i32;
        let mut ids = Vec::new();
        for cz in min_cz..=max_cz {
            for cx in min_cx..=max_cx {
                if cx < 0 || cz < 0 || cx >= self.grid_w || cz >= self.grid_h {
                    continue;
                }
                let idx = (cz * self.grid_w + cx) as usize;
                for &id in &self.grid[idx] {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            }
        }
        let r2 = r * r;
        ids.into_iter()
            .filter_map(|id| self.edge(id))
            .filter(|e| edge_dist2(e, pos) <= r2)
            .collect()
    }

    /// Dijkstra で spawn → end の点列。失敗時は最長エッジ列フォールバック。
    pub fn mission_waypoints(&self) -> Vec<Vec3> {
        if let Some(pts) = self.shortest_path_points(self.spawn_node, self.mission_end_node) {
            if pts.len() >= 2 {
                return pts;
            }
        }
        // フォールバック: id=0 のエッジから伸ばす。
        if let Some(e) = self.edges.first() {
            return e
                .points
                .iter()
                .map(|p| Vec3::new(p[0], 0.0, p[1]))
                .collect();
        }
        vec![Vec3::ZERO, Vec3::new(0.0, 0.0, 100.0)]
    }

    pub fn mission_path(&self) -> RoadPath {
        let pts = self.mission_waypoints();
        RoadPath::from_waypoints(&pts, 6)
    }

    pub fn shortest_path_points(&self, start: u32, goal: u32) -> Option<Vec<Vec3>> {
        use std::cmp::Ordering;
        use std::collections::{BinaryHeap, HashMap};

        #[derive(Copy, Clone, PartialEq)]
        struct State {
            cost: f32,
            node: u32,
        }
        impl Eq for State {}
        impl Ord for State {
            fn cmp(&self, other: &Self) -> Ordering {
                other
                    .cost
                    .total_cmp(&self.cost)
                    .then_with(|| self.node.cmp(&other.node))
            }
        }
        impl PartialOrd for State {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        // adjacency: node -> (next, edge_id, cost)
        let mut adj: HashMap<u32, Vec<(u32, u32, f32)>> = HashMap::new();
        for e in &self.edges {
            let cost = polyline_length(&e.points).max(0.1);
            adj.entry(e.from).or_default().push((e.to, e.id, cost));
            if !e.oneway {
                adj.entry(e.to).or_default().push((e.from, e.id, cost));
            }
        }

        let mut dist: HashMap<u32, f32> = HashMap::new();
        let mut prev: HashMap<u32, (u32, u32)> = HashMap::new(); // node -> (prev_node, edge)
        let mut heap = BinaryHeap::new();
        dist.insert(start, 0.0);
        heap.push(State {
            cost: 0.0,
            node: start,
        });

        while let Some(State { cost, node }) = heap.pop() {
            if node == goal {
                break;
            }
            if cost > *dist.get(&node).unwrap_or(&f32::INFINITY) {
                continue;
            }
            for &(next, eid, w) in adj.get(&node).into_iter().flatten() {
                let next_cost = cost + w;
                if next_cost < *dist.get(&next).unwrap_or(&f32::INFINITY) {
                    dist.insert(next, next_cost);
                    prev.insert(next, (node, eid));
                    heap.push(State {
                        cost: next_cost,
                        node: next,
                    });
                }
            }
        }

        if !dist.contains_key(&goal) {
            return None;
        }

        // Reconstruct edge chain goal <- ... <- start
        let mut edge_chain = Vec::new();
        let mut cur = goal;
        while cur != start {
            let (p, eid) = prev.get(&cur)?;
            edge_chain.push((*eid, *p, cur));
            cur = *p;
        }
        edge_chain.reverse();

        let mut points = Vec::new();
        for (i, (eid, from, to)) in edge_chain.iter().enumerate() {
            let e = self.edge(*eid)?;
            let mut pts: Vec<Vec3> = e
                .points
                .iter()
                .map(|p| Vec3::new(p[0], 0.0, p[1]))
                .collect();
            // 向きを from→to に合わせる。
            if let (Some(a), Some(b)) = (pts.first(), pts.last()) {
                let want_start = self.node_pos(*from)?;
                if a.distance(want_start) > b.distance(want_start) {
                    pts.reverse();
                }
            }
            let skip = if i > 0 { 1 } else { 0 };
            points.extend(pts.into_iter().skip(skip));
            let _ = to;
        }
        if points.len() < 2 {
            return None;
        }
        Some(points)
    }
}

fn bounds_of(edges: &[MapEdge]) -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for e in edges {
        for p in &e.points {
            min_x = min_x.min(p[0]);
            max_x = max_x.max(p[0]);
            min_z = min_z.min(p[1]);
            max_z = max_z.max(p[1]);
        }
    }
    if !min_x.is_finite() {
        return (-100.0, 100.0, -100.0, 100.0);
    }
    (min_x, max_x, min_z, max_z)
}

fn polyline_length(points: &[[f32; 2]]) -> f32 {
    let mut len = 0.0;
    for w in points.windows(2) {
        let dx = w[1][0] - w[0][0];
        let dz = w[1][1] - w[0][1];
        len += (dx * dx + dz * dz).sqrt();
    }
    len
}

fn edge_dist2(e: &MapEdge, pos: Vec3) -> f32 {
    let mut best = f32::INFINITY;
    for w in e.points.windows(2) {
        let a = Vec3::new(w[0][0], 0.0, w[0][1]);
        let b = Vec3::new(w[1][0], 0.0, w[1][1]);
        best = best.min(dist2_point_segment(pos, a, b));
    }
    if e.points.len() == 1 {
        let p = Vec3::new(e.points[0][0], 0.0, e.points[0][1]);
        best = best.min(pos.distance_squared(p));
    }
    best
}

fn dist2_point_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    let q = a + ab * t;
    p.distance_squared(q)
}

/// エッジ中心線を道路セグメントのサンプル列にする。
pub fn edge_samples(edge: &MapEdge, step: f32) -> Vec<(Vec3, Vec3, f32)> {
    // (pos, tangent, width)
    let step = step.max(2.0);
    let mut out = Vec::new();
    for w in edge.points.windows(2) {
        let a = Vec3::new(w[0][0], 0.0, w[0][1]);
        let b = Vec3::new(w[1][0], 0.0, w[1][1]);
        let delta = b - a;
        let len = delta.length().max(1e-3);
        let tangent = delta / len;
        let n = (len / step).ceil().max(1.0) as usize;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let pos = a.lerp(b, t);
            out.push((pos, tangent, edge.width));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_city_loads() {
        let net = RoadNetwork::demo_city().expect("demo city");
        assert!(net.nodes.len() >= 10);
        assert!(net.edges.len() >= 10);
        assert!(!net.buildings.is_empty());
    }

    #[test]
    fn mission_path_has_length() {
        let net = RoadNetwork::demo_city().unwrap();
        let path = net.mission_path();
        assert!(path.length() > 200.0, "len={}", path.length());
    }

    #[test]
    fn edges_near_finds_spawn_roads() {
        let net = RoadNetwork::demo_city().unwrap();
        let spawn = net.node_pos(net.spawn_node).unwrap();
        let near = net.edges_near(spawn, 80.0);
        assert!(!near.is_empty());
    }

    #[test]
    fn shortest_path_reaches_mission_end() {
        let net = RoadNetwork::demo_city().unwrap();
        let pts = net
            .shortest_path_points(net.spawn_node, net.mission_end_node)
            .expect("path");
        assert!(pts.len() >= 2);
        let end = net.node_pos(net.mission_end_node).unwrap();
        assert!(pts.last().unwrap().distance(end) < 5.0);
    }
}
