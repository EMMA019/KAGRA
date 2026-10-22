//! Offscreen `render_world_doc` + HUD JSON parsing.

use pyo3::prelude::*;

use crate::world_doc::WorldDoc;

use super::common::pyerr;

/// HUD JSON（Python が quad / テキストを渡す。オプション）。
#[derive(serde::Deserialize, Default)]
struct HudJson {
    #[serde(default)]
    quads: Vec<HudQuadJson>,
    #[serde(default)]
    texts: Vec<HudTextJson>,
    #[serde(default)]
    images: Vec<HudImageJson>,
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

#[derive(serde::Deserialize)]
struct HudImageJson {
    path: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
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
        list.quads
            .push(crate::scene::Quad::new(q.x, q.y, q.w, q.h, q.color));
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
    for im in hud.images {
        list.images.push(crate::scene::ImageQuad::new(
            &im.path, im.x, im.y, im.w, im.h,
        ));
    }
    Ok(list)
}

/// オフスクリーンレンダラのプロセス内キャッシュ。
static CACHED_RENDERER: once_cell::sync::Lazy<
    std::sync::Mutex<Option<(u32, u32, crate::render::Renderer)>>,
> = once_cell::sync::Lazy::new(|| std::sync::Mutex::new(None));

/// dump JSON を shared wgpu 30 でオフスクリーン描画し、RGBA8 を bytes で返す。
#[pyfunction(name = "render_world_doc")]
pub fn render_world_doc_py(
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
            let r = pollster::block_on(crate::render::Renderer::new_offscreen(width, height))
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
