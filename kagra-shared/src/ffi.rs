//! C ABI — Android NDK / iOS / デスクトップ FFI 共通。
//!
//! ハンドルは opaque pointer。スレッドセーフではない（UI スレッド専用を想定）。
//! ハンドルを受け取る関数はすべて `unsafe`：呼び出し側が生存を保証する。
//! C/Kotlin/Swift から見た ABI は safe 版と同一。

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_float, c_int, c_uint, c_void};
use std::sync::Mutex;

use once_cell::sync::Lazy;

use crate::assets::{resolve_alias, AssetKind};
use crate::input::{PointerEvent, PointerPhase};
use crate::session::SharedSession;
use crate::KAGRA_SHARED_VERSION;

static LAST_ERROR: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new(String::new()));

fn set_err(msg: impl Into<String>) {
    if let Ok(mut g) = LAST_ERROR.lock() {
        *g = msg.into();
    }
}

/// # Safety
/// `ptr` は `kagra_shared_create` が返した生存中のハンドルか NULL でなければならない。
unsafe fn session<'a>(ptr: *mut SharedSession) -> Option<&'a mut SharedSession> {
    if ptr.is_null() {
        set_err("null session");
        None
    } else {
        Some(&mut *ptr)
    }
}

/// NUL 終端文字列を呼び出し側バッファへ書く。戻り値は必要バイト数（NUL 含む）。
///
/// # Safety
/// `buf` は NULL か、`buflen` バイト以上の書き込み可能領域を指すこと。
unsafe fn write_cstr(src: &str, buf: *mut c_char, buflen: c_uint) -> c_int {
    let need = src.len() + 1;
    if buf.is_null() || (buflen as usize) < need {
        return need as c_int;
    }
    std::ptr::copy_nonoverlapping(src.as_ptr(), buf as *mut u8, src.len());
    *buf.add(src.len()) = 0;
    need as c_int
}

#[no_mangle]
pub extern "C" fn kagra_shared_version() -> *const c_char {
    static VER: Lazy<CString> =
        Lazy::new(|| CString::new(KAGRA_SHARED_VERSION).unwrap_or_default());
    VER.as_ptr()
}

#[no_mangle]
pub extern "C" fn kagra_shared_last_error() -> *const c_char {
    static BUF: Lazy<Mutex<Option<CString>>> = Lazy::new(|| Mutex::new(None));
    let msg = LAST_ERROR.lock().map(|g| g.clone()).unwrap_or_default();
    let c = CString::new(msg).unwrap_or_default();
    let ptr = c.as_ptr();
    if let Ok(mut g) = BUF.lock() {
        *g = Some(c);
    }
    ptr
}

#[no_mangle]
pub extern "C" fn kagra_shared_create() -> *mut SharedSession {
    Box::into_raw(Box::new(SharedSession::default()))
}

/// # Safety
/// `ptr` は `kagra_shared_create` の戻り値で、まだ破棄されていないこと。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_destroy(ptr: *mut SharedSession) {
    if !ptr.is_null() {
        drop(Box::from_raw(ptr));
    }
}

/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_create_surface(
    ptr: *mut SharedSession,
    width: c_uint,
    height: c_uint,
) -> c_int {
    match session(ptr) {
        Some(s) => {
            s.create_surface(width, height);
            0
        }
        None => -1,
    }
}

/// # Safety
/// `ptr` は生存中のハンドルか NULL。`root` は NUL 終端文字列か NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_set_asset_root(
    ptr: *mut SharedSession,
    root: *const c_char,
) -> c_int {
    if root.is_null() {
        set_err("null asset_root");
        return -1;
    }
    let s = CStr::from_ptr(root).to_string_lossy().into_owned();
    match session(ptr) {
        Some(sess) => {
            sess.set_asset_root(s);
            0
        }
        None => -1,
    }
}

/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_pause(ptr: *mut SharedSession) -> c_int {
    match session(ptr) {
        Some(s) => {
            s.pause();
            0
        }
        None => -1,
    }
}

/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_resume(ptr: *mut SharedSession) -> c_int {
    match session(ptr) {
        Some(s) => {
            s.resume();
            0
        }
        None => -1,
    }
}

/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_push_pointer(
    ptr: *mut SharedSession,
    id: c_uint,
    x: c_float,
    y: c_float,
    phase: c_uint,
    pressure: c_float,
) -> c_int {
    let Some(ph) = PointerPhase::from_u8(phase as u8) else {
        set_err("bad pointer phase");
        return -1;
    };
    match session(ptr) {
        Some(s) => {
            s.push_pointer(PointerEvent {
                id,
                x,
                y,
                phase: ph,
                pressure,
            });
            0
        }
        None => -1,
    }
}

/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_set_pad(
    ptr: *mut SharedSession,
    x: c_float,
    y: c_float,
) -> c_int {
    match session(ptr) {
        Some(s) => {
            s.set_pad(x, y);
            0
        }
        None => -1,
    }
}

/// 連続値のドライバ入力。`steer` は -1..1、`throttle` と `brake` は 0..1。
///
/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_set_drive(
    ptr: *mut SharedSession,
    steer: c_float,
    throttle: c_float,
    brake: c_float,
) -> c_int {
    match session(ptr) {
        Some(s) => {
            s.set_drive(steer, throttle, brake);
            0
        }
        None => -1,
    }
}

/// シーンを切り替える。0 = 運転（3D）、1 = タッチデモ（2D）。
///
/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_set_scene(ptr: *mut SharedSession, kind: c_uint) -> c_int {
    let kind = match kind {
        0 => crate::session::SceneKind::Driving,
        1 => crate::session::SceneKind::Demo2D,
        _ => return -1,
    };
    match session(ptr) {
        Some(s) => {
            s.set_scene_kind(kind);
            0
        }
        None => -1,
    }
}

/// 次フレームへ進み、frame 番号を返す。エラー時 -1。
///
/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_request_frame(ptr: *mut SharedSession) -> i64 {
    match session(ptr) {
        Some(s) => s.request_frame().frame as i64,
        None => -1,
    }
}

/// stats JSON を呼び出し側バッファに書く。戻り値は必要バイト数（NUL 含む）。
/// buf が短い場合は書き込まず必要長だけ返す。
///
/// # Safety
/// `ptr` は生存中のハンドルか NULL。`buf` は NULL か `buflen` バイト以上の領域。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_stats_json(
    ptr: *mut SharedSession,
    buf: *mut c_char,
    buflen: c_uint,
) -> c_int {
    let Some(s) = session(ptr) else {
        return -1;
    };
    write_cstr(&s.stats_json(), buf, buflen)
}

/// セーブ JSON をバッファに書く。
///
/// # Safety
/// `ptr` は生存中のハンドルか NULL。`buf` は NULL か `buflen` バイト以上。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_save_json(
    ptr: *mut SharedSession,
    buf: *mut c_char,
    buflen: c_uint,
) -> c_int {
    let Some(s) = session(ptr) else {
        return -1;
    };
    match s.save_json() {
        Ok(j) => write_cstr(&j, buf, buflen),
        Err(e) => {
            set_err(&e);
            -1
        }
    }
}

/// セーブ JSON を読み込んで状態を復元する。
///
/// # Safety
/// `ptr` は生存中のハンドル。`json` は NUL 終端。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_load_json(
    ptr: *mut SharedSession,
    json: *const c_char,
) -> c_int {
    let Some(s) = session(ptr) else {
        return -1;
    };
    if json.is_null() {
        set_err("json is null");
        return -1;
    }
    let text = unsafe { CStr::from_ptr(json) }.to_str().unwrap_or("");
    match s.load_json(text) {
        Ok(()) => 0,
        Err(e) => {
            set_err(&e);
            -1
        }
    }
}

/// 設定を書く。`muted` は 0/1。
///
/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_set_settings(
    ptr: *mut SharedSession,
    master_volume: c_float,
    steer_sensitivity: c_float,
    muted: c_int,
) -> c_int {
    match session(ptr) {
        Some(s) => {
            s.set_settings(crate::save::Settings {
                master_volume,
                steer_sensitivity,
                muted: muted != 0,
            });
            0
        }
        None => -1,
    }
}

/// 音声レベル JSON（engine/wind/brake）。
///
/// # Safety
/// `ptr` は生存中のハンドルか NULL。`buf` は NULL か `buflen` バイト以上。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_audio_json(
    ptr: *mut SharedSession,
    buf: *mut c_char,
    buflen: c_uint,
) -> c_int {
    let Some(s) = session(ptr) else {
        return -1;
    };
    let j = serde_json::to_string(&s.audio_levels()).unwrap_or_else(|_| "{}".into());
    write_cstr(&j, buf, buflen)
}

// ── 描画 ─────────────────────────────────────────────────────
//
// `render` feature が無いビルドでもシンボルは残す。シェル側のバイナリを
// 作り直さずに feature を切り替えられるようにするため、失敗を戻り値で伝える。

#[cfg(not(feature = "render"))]
fn render_unavailable() -> c_int {
    set_err("built without the \"render\" feature");
    -1
}

/// Android の `ANativeWindow*` を描画先にする。
///
/// # Safety
/// `ptr` は生存中のハンドル。`window` は `kagra_shared_detach_surface` まで
/// 有効な `ANativeWindow*`。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_attach_android_surface(
    ptr: *mut SharedSession,
    window: *mut c_void,
    width: c_uint,
    height: c_uint,
) -> c_int {
    #[cfg(feature = "render")]
    {
        attach(
            ptr,
            crate::render::SurfaceSource::AndroidNativeWindow(window),
            width,
            height,
        )
    }
    #[cfg(not(feature = "render"))]
    {
        let _ = (ptr, window, width, height);
        render_unavailable()
    }
}

/// iOS の `UIView*`（CAMetalLayer を持つ view）を描画先にする。
///
/// # Safety
/// `ptr` は生存中のハンドル。`view` は detach まで有効な `UIView*`。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_attach_ios_view(
    ptr: *mut SharedSession,
    view: *mut c_void,
    width: c_uint,
    height: c_uint,
) -> c_int {
    #[cfg(feature = "render")]
    {
        attach(
            ptr,
            crate::render::SurfaceSource::UiKitView(view),
            width,
            height,
        )
    }
    #[cfg(not(feature = "render"))]
    {
        let _ = (ptr, view, width, height);
        render_unavailable()
    }
}

/// 画面を持たない描画先を作る（自己診断用）。
///
/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_attach_offscreen(
    ptr: *mut SharedSession,
    width: c_uint,
    height: c_uint,
) -> c_int {
    #[cfg(feature = "render")]
    {
        let Some(sess) = session(ptr) else {
            return -1;
        };
        sess.create_surface(width, height);
        match pollster::block_on(crate::render::Renderer::new_offscreen(width, height)) {
            Ok(r) => {
                sess.attach_renderer(r);
                0
            }
            Err(e) => {
                set_err(e);
                -1
            }
        }
    }
    #[cfg(not(feature = "render"))]
    {
        let _ = (ptr, width, height);
        render_unavailable()
    }
}

/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_detach_surface(ptr: *mut SharedSession) -> c_int {
    #[cfg(feature = "render")]
    {
        match session(ptr) {
            Some(s) => {
                s.detach_renderer();
                0
            }
            None => -1,
        }
    }
    #[cfg(not(feature = "render"))]
    {
        let _ = ptr;
        render_unavailable()
    }
}

/// 描画先が接続済みなら 1。
///
/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_has_renderer(ptr: *mut SharedSession) -> c_int {
    #[cfg(feature = "render")]
    {
        match session(ptr) {
            Some(s) => s.has_renderer() as c_int,
            None => -1,
        }
    }
    #[cfg(not(feature = "render"))]
    {
        let _ = ptr;
        0
    }
}

/// 現在のシーンを 1 枚描く。`kagra_shared_request_frame` の後に呼ぶ。
///
/// # Safety
/// `ptr` は生存中のハンドルか NULL。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_render(ptr: *mut SharedSession) -> c_int {
    #[cfg(feature = "render")]
    {
        let Some(sess) = session(ptr) else {
            return -1;
        };
        match sess.render() {
            Ok(()) => 0,
            Err(e) => {
                set_err(e);
                -1
            }
        }
    }
    #[cfg(not(feature = "render"))]
    {
        let _ = ptr;
        render_unavailable()
    }
}

#[cfg(feature = "render")]
unsafe fn attach(
    ptr: *mut SharedSession,
    source: crate::render::SurfaceSource,
    width: c_uint,
    height: c_uint,
) -> c_int {
    let Some(sess) = session(ptr) else {
        return -1;
    };
    sess.create_surface(width, height);
    match pollster::block_on(crate::render::Renderer::new_for_surface(
        source, width, height,
    )) {
        Ok(r) => {
            sess.attach_renderer(r);
            0
        }
        Err(e) => {
            set_err(e);
            -1
        }
    }
}

/// alias 候補を `\n` 区切りで buf に書く。
///
/// # Safety
/// `name` は NUL 終端文字列か NULL。`buf` は NULL か `buflen` バイト以上の領域。
#[no_mangle]
pub unsafe extern "C" fn kagra_shared_resolve_alias(
    kind: c_uint,
    name: *const c_char,
    buf: *mut c_char,
    buflen: c_uint,
) -> c_int {
    let _ = AssetKind::from_u8(kind as u8);
    if name.is_null() {
        set_err("null name");
        return -1;
    }
    let n = CStr::from_ptr(name).to_string_lossy();
    write_cstr(&resolve_alias(&n).join("\n"), buf, buflen)
}
