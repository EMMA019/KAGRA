//! wasm-bindgen エントリ（`cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm`）。

use wasm_bindgen::prelude::*;

use crate::input::{PointerEvent, PointerPhase};
use crate::session::SharedSession;
use crate::KAGRA_SHARED_VERSION;

#[wasm_bindgen]
pub struct WasmSession {
    inner: SharedSession,
}

#[wasm_bindgen]
impl WasmSession {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmSession {
        let mut inner = SharedSession::default();
        // ブラウザデモはタイトルから。
        inner.show_title();
        WasmSession { inner }
    }

    #[wasm_bindgen(js_name = createSurface)]
    pub fn create_surface(&mut self, width: u32, height: u32) {
        self.inner.create_surface(width, height);
    }

    #[wasm_bindgen(js_name = setAssetRoot)]
    pub fn set_asset_root(&mut self, root: &str) {
        self.inner.set_asset_root(root);
    }

    pub fn pause(&mut self) {
        self.inner.pause();
    }

    pub fn resume(&mut self) {
        self.inner.resume();
    }

    #[wasm_bindgen(js_name = pushPointer)]
    pub fn push_pointer(&mut self, id: u32, x: f32, y: f32, phase: u8, pressure: f32) {
        if let Some(ph) = PointerPhase::from_u8(phase) {
            self.inner.push_pointer(PointerEvent {
                id,
                x,
                y,
                phase: ph,
                pressure,
            });
        }
    }

    #[wasm_bindgen(js_name = setPad)]
    pub fn set_pad(&mut self, x: f32, y: f32) {
        self.inner.set_pad(x, y);
    }

    /// 連続値のドライバ入力。`steer` は -1..1、`throttle` と `brake` は 0..1。
    #[wasm_bindgen(js_name = setDrive)]
    pub fn set_drive(&mut self, steer: f32, throttle: f32, brake: f32) {
        self.inner.set_drive(steer, throttle, brake);
    }

    /// 0 = 運転（3D）、1 = タッチデモ（2D）。
    #[wasm_bindgen(js_name = setScene)]
    pub fn set_scene(&mut self, kind: u8) {
        let kind = match kind {
            1 => crate::session::SceneKind::Demo2D,
            _ => crate::session::SceneKind::Driving,
        };
        self.inner.set_scene_kind(kind);
    }

    /// 速度（km/h）。HUD をブラウザ側で出すとき用。
    #[wasm_bindgen(js_name = speedKmh)]
    pub fn speed_kmh(&self) -> f32 {
        self.inner.driving.truck.speed_kmh()
    }

    #[wasm_bindgen(js_name = requestFrame)]
    pub fn request_frame(&mut self) -> u32 {
        self.inner.request_frame().frame as u32
    }

    #[wasm_bindgen(js_name = statsJson)]
    pub fn stats_json(&self) -> String {
        self.inner.stats_json()
    }

    /// セーブ JSON（pretty）。シェルが localStorage 等へ書き出す。
    #[wasm_bindgen(js_name = saveJson)]
    pub fn save_json(&self) -> Result<String, JsValue> {
        self.inner.save_json().map_err(|e| JsValue::from_str(&e))
    }

    /// セーブ JSON を読み込んで状態を復元する。
    #[wasm_bindgen(js_name = loadJson)]
    pub fn load_json(&mut self, json: &str) -> Result<(), JsValue> {
        self.inner
            .load_json(json)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// `muted` は真なら無音。
    #[wasm_bindgen(js_name = setSettings)]
    pub fn set_settings(&mut self, master_volume: f32, steer_sensitivity: f32, muted: bool) {
        self.inner.set_settings(crate::save::Settings {
            master_volume,
            steer_sensitivity,
            muted,
        });
    }

    /// 音声レベル JSON（engine / wind / brake）。再生はブラウザ側。
    #[wasm_bindgen(js_name = audioJson)]
    pub fn audio_json(&self) -> String {
        serde_json::to_string(&self.inner.audio_levels()).unwrap_or_else(|_| "{}".into())
    }

    /// タイトル画面へ。
    #[wasm_bindgen(js_name = showTitle)]
    pub fn show_title(&mut self) {
        self.inner.show_title();
    }

    /// 配送ランを開始／再スタート。
    #[wasm_bindgen(js_name = startGame)]
    pub fn start_game(&mut self) {
        self.inner.start_game();
    }

    /// ゲーム状態 JSON（phase / time / score / objective）。
    #[wasm_bindgen(js_name = gameJson)]
    pub fn game_json(&self) -> String {
        serde_json::to_string(&self.inner.game).unwrap_or_else(|_| "{}".into())
    }

    /// canvas を描画先にする。WebGPU / WebGL2 のどちらかが使えれば成功する。
    ///
    /// アダプタ取得が非同期なので JS からは `await session.attachCanvas(canvas)`。
    #[cfg(feature = "render")]
    #[wasm_bindgen(js_name = attachCanvas)]
    pub async fn attach_canvas(
        &mut self,
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<(), JsValue> {
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        self.inner.create_surface(width, height);
        let renderer = crate::render::Renderer::new_for_surface(
            crate::render::SurfaceSource::Canvas(canvas),
            width,
            height,
        )
        .await
        .map_err(|e| JsValue::from_str(&e))?;
        self.inner.attach_renderer(renderer);
        Ok(())
    }

    /// 現在のシーンを 1 枚描く。`requestFrame()` の後に呼ぶ。
    #[cfg(feature = "render")]
    pub fn render(&mut self) -> Result<(), JsValue> {
        self.inner.render().map_err(|e| JsValue::from_str(&e))
    }

    /// 描画先が接続済みか。
    #[cfg(feature = "render")]
    #[wasm_bindgen(js_name = hasRenderer)]
    pub fn has_renderer(&self) -> bool {
        self.inner.has_renderer()
    }

    /// この wasm が描画機能付きでビルドされているか。
    #[wasm_bindgen(js_name = renderSupported)]
    pub fn render_supported(&self) -> bool {
        cfg!(feature = "render")
    }
}

#[wasm_bindgen(js_name = sharedVersion)]
pub fn shared_version() -> String {
    KAGRA_SHARED_VERSION.to_string()
}

#[wasm_bindgen(start)]
pub fn wasm_start() {
    // ブラウザ console に一度だけ出す（web-sys feature）
    #[cfg(feature = "wasm")]
    {
        web_sys::console::log_1(
            &format!("kagra-shared wasm {} ready", KAGRA_SHARED_VERSION).into(),
        );
    }
}
