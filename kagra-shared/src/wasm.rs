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
        // ブラウザデモはタイトルから。Wasm はプレビューなので直してここを出す。
        inner.show_title();
        inner.set_playtest_enabled(true);
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

    /// Repo-relative file for [`crate::assets::resolve_asset`] (no filesystem on wasm).
    #[wasm_bindgen(js_name = mountFile)]
    pub fn mount_file(&mut self, path: &str, bytes: &[u8]) {
        crate::assets::mount_file(path, bytes.to_vec());
    }

    #[wasm_bindgen(js_name = clearMount)]
    pub fn clear_mount(&mut self) {
        crate::assets::clear_mount();
    }

    /// World dump JSON. The scene waits on the title until a tap confirms.
    #[wasm_bindgen(js_name = loadGame)]
    pub fn load_game(&mut self, json: &str) -> Result<(), JsValue> {
        self.inner
            .load_game_json(json)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Packaged play. Sticks are -1..1. Lumen uses `lx`/`lz` to steer and `jump` to boost.
    #[wasm_bindgen(js_name = setPlayInput)]
    pub fn set_play_input(&mut self, lx: f32, lz: f32, jump: bool, attack: bool, dodge: bool) {
        self.inner.set_play_input(lx, lz, jump, attack, dodge);
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

    /// True when the loaded game reads 4-way swipes from `pushPointer`.
    #[wasm_bindgen(js_name = usesSwipe)]
    pub fn uses_swipe(&self) -> bool {
        self.inner.uses_swipe()
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

    /// 0 = 運転（3D）、1 = タッチデモ（2D）、2 = Crest Isle。
    #[wasm_bindgen(js_name = setScene)]
    pub fn set_scene(&mut self, kind: u8) {
        let kind = match kind {
            1 => crate::session::SceneKind::Demo2D,
            2 => crate::session::SceneKind::Collectathon,
            _ => crate::session::SceneKind::Driving,
        };
        self.inner.set_scene_kind(kind);
        if kind == crate::session::SceneKind::Collectathon {
            self.inner.boot_collectathon();
        }
    }

    /// Crest Isle の歩き。`lx`/`lz` は -1..1。
    #[wasm_bindgen(js_name = setWalk)]
    pub fn set_walk(&mut self, lx: f32, lz: f32, jump: bool) {
        self.inner.set_walk(lx, lz, jump);
    }

    #[wasm_bindgen(js_name = setJump)]
    pub fn set_jump(&mut self, held: bool) {
        self.inner.set_jump(held);
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
        if self.inner.kind == crate::session::SceneKind::Neon {
            return self
                .inner
                .neon
                .as_ref()
                .map(|neon| neon.state_json())
                .unwrap_or_else(|| "{}".into());
        }
        if self.inner.kind == crate::session::SceneKind::NeonCat {
            return self
                .inner
                .cat
                .as_ref()
                .map(|cat| cat.state_json())
                .unwrap_or_else(|| "{}".into());
        }
        if self.inner.kind == crate::session::SceneKind::HopBeat {
            return self
                .inner
                .hop
                .as_ref()
                .map(|hop| hop.state_json())
                .unwrap_or_else(|| "{}".into());
        }
        if self.inner.kind == crate::session::SceneKind::Collectathon {
            serde_json::to_string(&self.inner.isle.game).unwrap_or_else(|_| "{}".into())
        } else {
            serde_json::to_string(&self.inner.game).unwrap_or_else(|_| "{}".into())
        }
    }

    /// ジャンル dump。`"genre": "neon_run"` は Lumen を起動する。
    #[wasm_bindgen(js_name = loadGameJson)]
    pub fn load_game_json(&mut self, json: &str) -> Result<(), JsValue> {
        self.inner
            .load_game_json(json)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// HOP-BEAT: seconds of music heard so far (the audio clock). NaN = none.
    #[wasm_bindgen(js_name = setMusicClock)]
    pub fn set_music_clock(&mut self, secs: f32) {
        self.inner.set_music_clock(secs);
    }

    /// HOP-BEAT: the player's latency setting, ms.
    #[wasm_bindgen(js_name = setLatencyMs)]
    pub fn set_latency_ms(&mut self, ms: f32) {
        self.inner.set_latency_ms(ms);
    }

    /// HOP-BEAT: skip the title card; the first tap starts the music.
    #[wasm_bindgen(js_name = setDirectPlay)]
    pub fn set_direct_play(&mut self, on: bool) {
        self.inner.set_direct_play(on);
    }

    /// Continue after game over. True when it worked.
    pub fn revive(&mut self) -> bool {
        self.inner.revive()
    }

    /// Mixer cues for the page to play (wasm has no speakers of its own).
    #[wasm_bindgen(js_name = audioCuesJson)]
    pub fn audio_cues_json(&self) -> String {
        crate::mixer::cues_json()
    }

    /// `"auto"` (default), `"webgpu"`, or `"gl"` / `"webgl2"`. Call before
    /// `attachCanvas`. `?backend=gl` on the page forces WebGL2; a blank
    /// WebGPU frame does the same with a fresh canvas.
    #[cfg(feature = "render")]
    #[wasm_bindgen(js_name = setGraphicsBackend)]
    pub fn set_graphics_backend(&self, name: &str) {
        let _ = self;
        crate::render::set_graphics_backend(crate::render::parse_graphics_backend(name));
    }

    /// canvas を描画先にする。WebGPU / WebGL2 のどちらかが使えれば成功する。
    ///
    /// アダプタ取得が非同期なので JS からは `await session.attachCanvas(canvas)`。
    /// `createSurface` before this only stores the size. The canvas's own
    /// width/height is what the swapchain is configured with, and
    /// `attach_renderer` applies it again so a pre-attach resize is not lost.
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

    /// `mobile webgpu fxaa=1 bloom=1 ssao=4 c2 post` after a 3D frame.
    /// `pending` until the first present. The page prints this on screen.
    #[cfg(feature = "render")]
    #[wasm_bindgen(js_name = pictureStatus)]
    pub fn picture_status(&self) -> String {
        self.inner.picture_status()
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

    #[wasm_bindgen(js_name = setPlaytestEnabled)]
    pub fn set_playtest_enabled(&mut self, on: bool) {
        self.inner.set_playtest_enabled(on);
    }

    #[wasm_bindgen(js_name = setPlaytestExternalChrome)]
    pub fn set_playtest_external_chrome(&mut self, on: bool) {
        self.inner.set_playtest_external_chrome(on);
    }

    /// HTML owns the title and result card. Hides the in-canvas flow column.
    #[wasm_bindgen(js_name = setExternalMenu)]
    pub fn set_external_menu(&mut self, on: bool) {
        self.inner.set_external_menu(on);
    }

    /// Neon Cat hitbox cages. The page turns this on for `?debug=1`.
    #[wasm_bindgen(js_name = setDebug)]
    pub fn set_debug(&mut self, on: bool) {
        self.inner.set_debug(on);
    }

    #[wasm_bindgen(js_name = setPlaytestDevice)]
    pub fn set_playtest_device(&mut self, label: &str, lang: &str) {
        self.inner.set_playtest_device(label, lang);
    }

    /// Gesture-only pointer. Does not start the game.
    #[wasm_bindgen(js_name = playtestPointer)]
    pub fn playtest_pointer(&mut self, id: u32, x: f32, y: f32, phase: u8, pressure: f32) -> bool {
        if let Some(ph) = PointerPhase::from_u8(phase) {
            self.inner.playtest_pointer(PointerEvent {
                id,
                x,
                y,
                phase: ph,
                pressure,
            })
        } else {
            false
        }
    }

    #[wasm_bindgen(js_name = pushKey)]
    pub fn push_key(&mut self, name: &str, down: bool) -> bool {
        self.inner.push_key(name, down)
    }

    #[wasm_bindgen(js_name = setPlaytestNote)]
    pub fn set_playtest_note(&mut self, note: &str) {
        self.inner.playtest_set_note(note);
    }

    pub fn set_playtest_shot_path(&mut self, path: &str) {
        self.inner.playtest_set_shot_path(path);
    }

    #[wasm_bindgen(js_name = playtestAction)]
    pub fn playtest_action(&mut self, action: &str) {
        self.inner.playtest_action(action);
    }

    #[wasm_bindgen(js_name = playtestStateJson)]
    pub fn playtest_state_json(&mut self) -> String {
        self.inner.playtest_state_json()
    }

    #[wasm_bindgen(js_name = playtestPacketJson)]
    pub fn playtest_packet_json(&self) -> String {
        self.inner.playtest_packet_json()
    }
}

#[wasm_bindgen(js_name = sharedVersion)]
pub fn shared_version() -> String {
    KAGRA_SHARED_VERSION.to_string()
}

/// Procedural SFX as a mono 16-bit WAV. Empty when `name` is not a preset.
/// Bundled clips stay on the mixer; this is the synth (`jump`, `hit`, `coin`,
/// `explode`, …) so the page can play a cue without a file.
#[wasm_bindgen(js_name = synthWav)]
pub fn synth_wav(name: &str) -> Vec<u8> {
    crate::sfx::synth_wav(name).unwrap_or_default()
}

#[wasm_bindgen(start)]
pub fn wasm_start() {
    // ブラウザ console に一度だけ出す（web-sys feature）
    #[cfg(feature = "wasm")]
    {
        std::panic::set_hook(Box::new(|info| {
            web_sys::console::error_1(&format!("kagra panic: {info}").into());
        }));
        web_sys::console::log_1(
            &format!("kagra-shared wasm {} ready", KAGRA_SHARED_VERSION).into(),
        );
    }
}
