//! PyO3 bindings: Python ゲームマスターが Rust（shared wgpu 30）を呼ぶ橋。
//!
//! - `WorldDoc` — dump JSON の読み書き（世界はデータ）
//! - `WorldPlay` — 1 tick のステップ、入力、接着 API（emit_event /
//!   take_events / start_timer / step_interact）
//! - `render_world_doc` — dump → RGBA（オフスクリーン描画）
//!
//! Python 側はこれを使って「ループを所有」する。ジャンルロジック（敵 AI・
//! ターン・識別）は Python で書く。Rust は世界のデータ化・tick・描画を担う。

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::world_doc::WorldDoc;
use crate::world_play::WorldPlay;

fn pyerr<E: std::fmt::Display>(e: E) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// dump JSON の読み書き。世界の「データ」としての入口。
#[pyclass(name = "WorldDoc")]
struct PyWorldDoc {
    inner: WorldDoc,
}

#[pymethods]
impl PyWorldDoc {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: WorldDoc::from_json(json).map_err(pyerr)?,
        })
    }

    fn to_json(&self) -> PyResult<String> {
        self.inner.to_json().map_err(pyerr)
    }

    /// Player の位置 [x, y, z]（無ければ None）。
    fn player_position(&self) -> Option<[f32; 3]> {
        self.inner.player.as_ref().map(|w| w.position)
    }

    /// 指定 name の prop 一覧（id, position）。
    fn props_named(&self, name: &str) -> Vec<(String, [f32; 3])> {
        self.inner
            .props
            .iter()
            .filter(|p| p.name == name && p.enabled)
            .map(|p| (p.id.clone(), p.position))
            .collect()
    }
}

/// ゲームループの 1 ステップ。tick が世界を進め、dump() で読む。
#[pyclass(name = "WorldPlay")]
struct PyWorldPlay {
    inner: WorldPlay,
}

#[pymethods]
impl PyWorldPlay {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: WorldPlay::from_json(json).map_err(pyerr)?,
        })
    }

    /// 1 ステップ進める。`dt` は秒（1/60 など）。タイトル待ちは start() で。
    fn tick(&mut self, dt: f32) {
        self.inner.tick(dt);
    }

    /// タイトル / 結果画面で Space 相当。プレイ中は無視。
    fn confirm(&mut self) {
        self.inner.confirm();
    }

    /// 現在の世界を dump JSON で返す（Python が読んでロジックに使う）。
    fn dump(&self) -> PyResult<String> {
        self.inner.doc.to_json().map_err(pyerr)
    }

    /// WASD 入力。lx/lz は -1..1（カメラ基準）。attack = J / click。
    fn set_input(&mut self, lx: f32, lz: f32, jump: bool, attack: bool, dodge: bool) {
        self.inner.input = WalkInput {
            lx,
            lz,
            jump,
            attack,
            dodge,
        }
        .clamped();
    }

    /// 接着 API: 出来事を記録（同名は集約）。
    fn emit_event(&mut self, name: &str, data_json: Option<&str>) -> PyResult<()> {
        let payload = match data_json {
            Some(s) if !s.trim().is_empty() => Some(serde_json::from_str(s).map_err(pyerr)?),
            _ => None,
        };
        self.inner.emit_event(name, payload);
        Ok(())
    }

    /// 接着 API: 指定名の出来事を消費して JSON 文字列のリストで返す。
    fn take_events(&mut self, name: &str) -> Vec<String> {
        self.inner
            .take_events(name)
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect()
    }

    /// 接着 API: タイマーを起動。on_done は 0 秒時にイベントとして発火。
    fn start_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {
        self.inner.start_timer(name, seconds, on_done)
    }

    /// 接着 API: J を押した状態で、近くの interactable があれば on_use を発火。
    fn step_interact(&mut self) {
        self.inner.step_interact();
    }

    /// 現在のアニメーション状態（walker.anim）。
    fn anim(&self) -> String {
        self.inner
            .doc
            .player
            .as_ref()
            .map(|w| w.anim.clone())
            .unwrap_or_default()
    }
}

/// テキスト行幅（ピクセル）。UI パネルの折り返し用。
#[pyfunction(name = "measure_text")]
fn measure_text_py(text: &str, size: f32) -> f32 {
    crate::font::measure_text(text, size)
}

/// Rapier 剛体物理（Phase 1）。dump JSON からワールドを作り、step で進め、
/// to_json で位置を書き戻す。
///
/// 使い方（Python）::
///
///     import kagra_shared as ks
///     phys = ks.PhysicsWorld.from_json(json.dumps(world))
///     phys.step(1/60)
///     world = json.loads(phys.to_json())
#[cfg(feature = "physics")]
#[pyclass(name = "PhysicsWorld")]
struct PyPhysicsWorld {
    inner: crate::physics::PhysicsWorld,
    doc: WorldDoc,
}

#[cfg(feature = "physics")]
#[pymethods]
impl PyPhysicsWorld {
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let doc = WorldDoc::from_json(json).map_err(pyerr)?;
        Ok(Self {
            inner: crate::physics::PhysicsWorld::from_doc(&doc),
            doc,
        })
    }

    /// 1 フレーム進める。`dt` は秒（1/60 など）。
    fn step(&mut self, dt: f32) {
        self.inner.step(dt);
    }

    /// 剛体位置を書き戻した dump JSON。
    fn to_json(&self) -> PyResult<String> {
        let mut doc = self.doc.clone();
        self.inner.sync(&mut doc);
        doc.to_json().map_err(pyerr)
    }

    /// 同期前の dump JSON（from_json した時点の世界）。
    fn dump(&self) -> PyResult<String> {
        self.doc.to_json().map_err(pyerr)
    }

    /// 動的剛体（is_static=false の prop / walker）の現在位置。
    fn position(&self, id: &str) -> Option<[f32; 3]> {
        self.inner.position(id)
    }

    /// 動的剛体かどうか。
    fn is_dynamic(&self, id: &str) -> bool {
        self.inner.is_dynamic(id)
    }

    /// 動的剛体の速度を設定（投げる / 吹き飛ばす）。
    fn set_velocity(&mut self, id: &str, v: [f32; 3]) -> bool {
        self.inner.set_velocity(id, v)
    }

    /// 動的剛体の位置を直接設定（テレポート / リスポーン）。
    fn set_position(&mut self, id: &str, p: [f32; 3]) -> bool {
        self.inner.set_position(id, p)
    }
}

/// HUD JSON（Python が quad / テキストを渡す。オプション）。
#[derive(serde::Deserialize, Default)]
struct HudJson {
    #[serde(default)]
    quads: Vec<HudQuadJson>,
    #[serde(default)]
    texts: Vec<HudTextJson>,
}

#[derive(serde::Deserialize)]
struct HudQuadJson {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    #[serde(default = "default_white")]
    color: [u8; 4],
}

#[derive(serde::Deserialize)]
struct HudTextJson {
    text: String,
    x: f32,
    y: f32,
    #[serde(default = "default_text_size")]
    size: f32,
    #[serde(default = "default_white")]
    color: [u8; 4],
    #[serde(default)]
    align: String,
}

fn default_white() -> [u8; 4] {
    [255, 255, 255, 255]
}

fn default_text_size() -> f32 {
    16.0
}

fn parse_hud(hud_json: Option<&str>) -> PyResult<crate::scene::DrawList> {
    let mut list = crate::scene::DrawList::default();
    let Some(s) = hud_json else {
        return Ok(list);
    };
    if s.trim().is_empty() {
        return Ok(list);
    }
    let hud: HudJson = serde_json::from_str(s).map_err(pyerr)?;
    for q in hud.quads {
        list.quads.push(crate::scene::Quad::new(q.x, q.y, q.w, q.h, q.color));
    }
    for t in hud.texts {
        let align = match t.align.as_str() {
            "center" => crate::scene::TextAlign::Center,
            "right" => crate::scene::TextAlign::Right,
            _ => crate::scene::TextAlign::Left,
        };
        list.texts
            .push(crate::scene::TextQuad::new(&t.text, t.x, t.y, t.size, t.color).aligned(align));
    }
    Ok(list)
}

/// オフスクリーンレンダラのプロセス内キャッシュ。
///
/// 毎フレーム `Renderer::new_offscreen` を呼ぶと wgpu デバイス生成（数百 ms）
/// が毎回走り、Python ゲームマスターの窓が数 FPS になる。サイズが同じ間は
/// 1 個の Renderer を使い回す（ゲームループは GIL 保持なので排他は不要だが、
/// 念のため Mutex で守る）。
static CACHED_RENDERER: once_cell::sync::Lazy<
    std::sync::Mutex<Option<(u32, u32, crate::render::Renderer)>>,
> = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(None));

/// dump JSON を shared wgpu 30 でオフスクリーン描画し、RGBA8 を bytes で返す。
/// `hud_json`（省略可）: `{"quads":[{x,y,w,h,color}], "texts":[{text,x,y,size,color,align}]}`。
#[pyfunction(name = "render_world_doc")]
fn render_world_doc_py(
    py: Python<'_>,
    json: &str,
    width: u32,
    height: u32,
    hud_json: Option<&str>,
) -> PyResult<pyo3::Py<pyo3::types::PyBytes>> {
    let doc = WorldDoc::from_json(json).map_err(pyerr)?;
    let hud = parse_hud(hud_json)?;
    let rgba = {
        let mut guard = CACHED_RENDERER
            .lock()
            .map_err(|_| pyerr("cached renderer lock poisoned"))?;
        let need_new = match &*guard {
            Some((w, h, r)) => *w != width || *h != height || !r.is_offscreen(),
            None => true,
        };
        if need_new {
            let r =
                pollster::block_on(crate::render::Renderer::new_offscreen(width, height))
                    .map_err(pyerr)?;
            *guard = Some((width, height, r));
        }
        let (_, _, renderer) = guard.as_mut().expect("renderer present");
        renderer
            .render_world_doc_with_hud(&doc, &hud)
            .map_err(pyerr)?
    };
    Ok(pyo3::types::PyBytes::new_bound(py, &rgba).unbind())
}

#[pymodule]
fn kagra_shared(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyWorldDoc>()?;
    m.add_class::<PyWorldPlay>()?;
    m.add_function(wrap_pyfunction!(render_world_doc_py, m)?)?;
    m.add_function(wrap_pyfunction!(measure_text_py, m)?)?;
    #[cfg(feature = "physics")]
    m.add_class::<PyPhysicsWorld>()?;
    m.add("__version__", crate::KAGRA_SHARED_VERSION)?;
    Ok(())
}
