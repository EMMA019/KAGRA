//! C ABI — Android NDK / iOS / デスクトップ FFI 共通。
//!
//! ハンドルは opaque pointer。スレッドセーフではない（UI スレッド専用を想定）。

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_float, c_int, c_uint};
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

fn session<'a>(ptr: *mut SharedSession) -> Option<&'a mut SharedSession> {
    if ptr.is_null() {
        set_err("null session");
        None
    } else {
        Some(unsafe { &mut *ptr })
    }
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

#[no_mangle]
pub extern "C" fn kagra_shared_destroy(ptr: *mut SharedSession) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

#[no_mangle]
pub extern "C" fn kagra_shared_create_surface(
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

#[no_mangle]
pub extern "C" fn kagra_shared_set_asset_root(
    ptr: *mut SharedSession,
    root: *const c_char,
) -> c_int {
    if root.is_null() {
        set_err("null asset_root");
        return -1;
    }
    let s = unsafe { CStr::from_ptr(root) }.to_string_lossy().into_owned();
    match session(ptr) {
        Some(sess) => {
            sess.set_asset_root(s);
            0
        }
        None => -1,
    }
}

#[no_mangle]
pub extern "C" fn kagra_shared_pause(ptr: *mut SharedSession) -> c_int {
    match session(ptr) {
        Some(s) => {
            s.pause();
            0
        }
        None => -1,
    }
}

#[no_mangle]
pub extern "C" fn kagra_shared_resume(ptr: *mut SharedSession) -> c_int {
    match session(ptr) {
        Some(s) => {
            s.resume();
            0
        }
        None => -1,
    }
}

#[no_mangle]
pub extern "C" fn kagra_shared_push_pointer(
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

#[no_mangle]
pub extern "C" fn kagra_shared_set_pad(
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

/// 次フレームへ進み、frame 番号を返す。エラー時 -1。
#[no_mangle]
pub extern "C" fn kagra_shared_request_frame(ptr: *mut SharedSession) -> i64 {
    match session(ptr) {
        Some(s) => s.request_frame().frame as i64,
        None => -1,
    }
}

/// stats JSON を呼び出し側バッファに書く。戻り値は必要バイト数（NUL 含む）。
/// buf が短い場合は書き込まず必要長だけ返す。
#[no_mangle]
pub extern "C" fn kagra_shared_stats_json(
    ptr: *mut SharedSession,
    buf: *mut c_char,
    buflen: c_uint,
) -> c_int {
    let Some(s) = session(ptr) else {
        return -1;
    };
    let json = s.stats_json();
    let need = json.len() + 1;
    if buf.is_null() || (buflen as usize) < need {
        return need as c_int;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(json.as_ptr(), buf as *mut u8, json.len());
        *buf.add(json.len()) = 0;
    }
    need as c_int
}

/// alias 候補を `\n` 区切りで buf に書く。
#[no_mangle]
pub extern "C" fn kagra_shared_resolve_alias(
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
    let n = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    let joined = resolve_alias(&n).join("\n");
    let need = joined.len() + 1;
    if buf.is_null() || (buflen as usize) < need {
        return need as c_int;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(joined.as_ptr(), buf as *mut u8, joined.len());
        *buf.add(joined.len()) = 0;
    }
    need as c_int
}
