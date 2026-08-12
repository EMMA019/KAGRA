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
        WasmSession {
            inner: SharedSession::default(),
        }
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

    #[wasm_bindgen(js_name = requestFrame)]
    pub fn request_frame(&mut self) -> u32 {
        self.inner.request_frame().frame as u32
    }

    #[wasm_bindgen(js_name = statsJson)]
    pub fn stats_json(&self) -> String {
        self.inner.stats_json()
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
