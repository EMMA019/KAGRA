//! Deterministic input sync shared by desktop, Android, and wasm.
//!
//! Lockstep (input sync), not state sync:
//! - A versus round is a pure function of the start dump, both players'
//!   inputs, and a fixed `dt`. The human-vs-human fight path has no unseeded
//!   RNG.
//! - Phones send a few quantized axes per tick. Full world dumps are large,
//!   and float text can drift if two hosts reserialize state.
//! - This module has no sockets. Hosts relay [`NetMessage`] JSON. Prediction
//!   and rollback stay out of this first step: a tick runs only when every
//!   slot for that tick has arrived, so both sims stay identical.
//!
//! The reference transport is `server/netplay_relay.py` plus `kagra/netplay.py`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::collectathon::WalkInput;

/// Wire protocol. Bump when a field changes meaning.
pub const PROTO_VERSION: u32 = 1;

/// Versus on one phone is two slots. More than that is a later phase.
pub const MAX_PLAYERS: usize = 2;

/// Stick axes on the wire are thousandths of a unit, in `[-1000, 1000]`.
pub const INPUT_SCALE: f32 = 1000.0;

/// Quantize one stick axis. Non-finite values become 0 so every host agrees.
pub fn quant_axis(v: f32) -> i16 {
    if !v.is_finite() {
        return 0;
    }
    let scaled = (v.clamp(-1.0, 1.0) * INPUT_SCALE).round();
    scaled.clamp(-INPUT_SCALE, INPUT_SCALE) as i16
}

/// Inverse of [`quant_axis`]. Both hosts rebuild the same `f32`.
pub fn dequant_axis(q: i16) -> f32 {
    let q = q.clamp(-(INPUT_SCALE as i16), INPUT_SCALE as i16);
    (q as f32) / INPUT_SCALE
}

/// `num/den` as `f32`, computed in Rust so Python and wasm do not pick a
/// different rounding. `den == 0` falls back to 1/60.
pub fn fixed_dt(num: u32, den: u32) -> f32 {
    if den == 0 {
        return 1.0 / 60.0;
    }
    (num as f32) / (den as f32)
}

/// One player's stick for one tick. Integers only, so JSON stays exact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireInput {
    pub slot: u8,
    pub tick: u32,
    pub lx: i16,
    pub lz: i16,
    pub jump: bool,
    pub attack: bool,
    pub dodge: bool,
}

impl WireInput {
    pub fn from_walk(slot: u8, tick: u32, input: WalkInput) -> Self {
        let input = input.clamped();
        Self {
            slot,
            tick,
            lx: quant_axis(input.lx),
            lz: quant_axis(input.lz),
            jump: input.jump,
            attack: input.attack,
            dodge: input.dodge,
        }
    }

    pub fn to_walk(self) -> WalkInput {
        WalkInput {
            lx: dequant_axis(self.lx),
            lz: dequant_axis(self.lz),
            jump: self.jump,
            attack: self.attack,
            dodge: self.dodge,
        }
    }
}

/// Relay messages. `op` is the serde tag (`hello`, `welcome`, `start`,
/// `input`, `bye`). `start.dt_num/dt_den` is a ratio, not a JSON float.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum NetMessage {
    Hello {
        proto: u32,
        room: String,
        name: String,
    },
    Welcome {
        slot: u8,
        room: String,
        seed: u64,
        peers: u8,
    },
    Start {
        tick0: u32,
        dt_num: u32,
        dt_den: u32,
    },
    Input {
        input: WireInput,
    },
    Bye {
        reason: String,
    },
}

impl NetMessage {
    pub fn hello(room: &str, name: &str) -> Self {
        Self::Hello {
            proto: PROTO_VERSION,
            room: clip_token(room, 32),
            name: clip_token(name, 24),
        }
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }
}

fn clip_token(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Buffers inputs until every slot for a tick is present.
#[derive(Clone, Debug)]
pub struct Lockstep {
    local_slot: u8,
    players: u8,
    buf: BTreeMap<u32, [Option<WireInput>; MAX_PLAYERS]>,
}

impl Lockstep {
    pub fn new(local_slot: u8, players: u8) -> Result<Self, String> {
        if players == 0 || players as usize > MAX_PLAYERS {
            return Err(format!("players must be 1..={MAX_PLAYERS}"));
        }
        if local_slot >= players {
            return Err(format!("slot {local_slot} is outside 0..{players}"));
        }
        Ok(Self {
            local_slot,
            players,
            buf: BTreeMap::new(),
        })
    }

    pub fn local_slot(&self) -> u8 {
        self.local_slot
    }

    pub fn players(&self) -> u8 {
        self.players
    }

    /// Store this host's stick and return the message to send.
    pub fn submit_local(&mut self, tick: u32, input: WalkInput) -> Result<NetMessage, String> {
        let wire = WireInput::from_walk(self.local_slot, tick, input);
        self.store(wire)?;
        Ok(NetMessage::Input { input: wire })
    }

    /// Store a relayed message. Non-input messages are ignored here.
    /// The same bytes twice are fine. A different stick for the same
    /// slot and tick is an error (the sims would diverge).
    pub fn ingest(&mut self, msg: &NetMessage) -> Result<(), String> {
        let NetMessage::Input { input } = msg else {
            return Ok(());
        };
        self.store(*input)
    }

    pub fn ready(&self, tick: u32) -> bool {
        let Some(row) = self.buf.get(&tick) else {
            return false;
        };
        (0..self.players).all(|slot| row[slot as usize].is_some())
    }

    /// Both (or all) sticks for `tick`, in slot order. `None` until [`ready`].
    pub fn inputs(&self, tick: u32) -> Option<Vec<WalkInput>> {
        if !self.ready(tick) {
            return None;
        }
        let row = self.buf.get(&tick)?;
        let mut out = Vec::with_capacity(self.players as usize);
        for slot in 0..self.players {
            out.push(row[slot as usize].unwrap_or_default().to_walk());
        }
        Some(out)
    }

    fn store(&mut self, input: WireInput) -> Result<(), String> {
        if input.slot >= self.players {
            return Err(format!("slot {} is outside this match", input.slot));
        }
        let row = self.buf.entry(input.tick).or_insert([None; MAX_PLAYERS]);
        let slot = input.slot as usize;
        if let Some(prev) = row[slot] {
            if prev != input {
                return Err(format!(
                    "input conflict at tick {} slot {}",
                    input.tick, input.slot
                ));
            }
            return Ok(());
        }
        row[slot] = Some(input);
        Ok(())
    }
}

/// FNV-1a over explicit bytes. Used by fight state hashes.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// --- C ABI (Android / iOS / desktop). No sockets. ---

use std::ffi::CStr;
use std::os::raw::{c_char, c_float, c_int, c_uint};

fn write_cstr(src: &str, buf: *mut c_char, buflen: c_uint) -> c_int {
    let need = src.len() + 1;
    if buf.is_null() || (buflen as usize) < need {
        return need as c_int;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(src.as_ptr(), buf as *mut u8, src.len());
        *buf.add(src.len()) = 0;
    }
    need as c_int
}

/// # Safety
/// `room` and `name` are NUL-terminated or NULL. `buf` is NULL or writable
/// for `buflen` bytes. Returns the byte count including NUL, or -1.
#[no_mangle]
pub unsafe extern "C" fn kagra_net_hello(
    room: *const c_char,
    name: *const c_char,
    buf: *mut c_char,
    buflen: c_uint,
) -> c_int {
    let room = if room.is_null() {
        ""
    } else {
        match CStr::from_ptr(room).to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        }
    };
    let name = if name.is_null() {
        ""
    } else {
        match CStr::from_ptr(name).to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        }
    };
    match NetMessage::hello(room, name).to_json() {
        Ok(json) => write_cstr(&json, buf, buflen),
        Err(_) => -1,
    }
}

/// # Safety
/// The pointer from this function is freed with [`kagra_net_lockstep_free`].
#[no_mangle]
pub unsafe extern "C" fn kagra_net_lockstep_new(slot: u8, players: u8) -> *mut Lockstep {
    match Lockstep::new(slot, players) {
        Ok(step) => Box::into_raw(Box::new(step)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// # Safety
/// `ptr` is NULL or a pointer returned by [`kagra_net_lockstep_new`].
#[no_mangle]
pub unsafe extern "C" fn kagra_net_lockstep_free(ptr: *mut Lockstep) {
    if !ptr.is_null() {
        drop(Box::from_raw(ptr));
    }
}

/// # Safety
/// `ptr` is a live lockstep. `buf` is NULL or writable for `buflen` bytes.
/// `buttons` bit 0 = jump, bit 1 = attack, bit 2 = dodge.
#[no_mangle]
pub unsafe extern "C" fn kagra_net_submit_local(
    ptr: *mut Lockstep,
    tick: u32,
    lx: c_float,
    lz: c_float,
    buttons: c_uint,
    buf: *mut c_char,
    buflen: c_uint,
) -> c_int {
    if ptr.is_null() {
        return -1;
    }
    let step = &mut *ptr;
    let input = WalkInput {
        lx,
        lz,
        jump: buttons & 1 != 0,
        attack: buttons & 2 != 0,
        dodge: buttons & 4 != 0,
    };
    match step.submit_local(tick, input).and_then(|m| m.to_json()) {
        Ok(json) => write_cstr(&json, buf, buflen),
        Err(_) => -1,
    }
}

/// # Safety
/// `ptr` is a live lockstep. `json` is a NUL-terminated message.
/// Returns 0 on success, -1 on a bad message or an input conflict.
#[no_mangle]
pub unsafe extern "C" fn kagra_net_ingest(ptr: *mut Lockstep, json: *const c_char) -> c_int {
    if ptr.is_null() || json.is_null() {
        return -1;
    }
    let text = match CStr::from_ptr(json).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let msg = match NetMessage::from_json(text) {
        Ok(m) => m,
        Err(_) => return -1,
    };
    match (*ptr).ingest(&msg) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// # Safety
/// `ptr` is NULL or a live lockstep. Returns 1 when every slot is present.
#[no_mangle]
pub unsafe extern "C" fn kagra_net_ready(ptr: *const Lockstep, tick: u32) -> c_int {
    if ptr.is_null() {
        return 0;
    }
    if (*ptr).ready(tick) {
        1
    } else {
        0
    }
}

/// # Safety
/// `ptr` is a live lockstep. `out_lx` / `out_lz` each have room for `players`
/// floats when non-NULL. Returns 0 when the tick is ready.
#[no_mangle]
pub unsafe extern "C" fn kagra_net_input_axis(
    ptr: *const Lockstep,
    tick: u32,
    slot: u8,
    out_lx: *mut c_float,
    out_lz: *mut c_float,
    out_buttons: *mut c_uint,
) -> c_int {
    if ptr.is_null() {
        return -1;
    }
    let Some(rows) = (*ptr).inputs(tick) else {
        return -1;
    };
    let Some(input) = rows.get(slot as usize) else {
        return -1;
    };
    if !out_lx.is_null() {
        *out_lx = input.lx;
    }
    if !out_lz.is_null() {
        *out_lz = input.lz;
    }
    if !out_buttons.is_null() {
        let mut bits = 0u32;
        if input.jump {
            bits |= 1;
        }
        if input.attack {
            bits |= 2;
        }
        if input.dodge {
            bits |= 4;
        }
        *out_buttons = bits;
    }
    0
}

#[cfg(feature = "wasm")]
mod wasm_api {
    use super::{fixed_dt, Lockstep, NetMessage};
    use crate::collectathon::WalkInput;
    use wasm_bindgen::prelude::*;

    fn js_err(err: impl std::fmt::Display) -> JsValue {
        JsValue::from_str(&err.to_string())
    }

    #[wasm_bindgen(js_name = netHello)]
    pub fn net_hello(room: &str, name: &str) -> Result<String, JsValue> {
        NetMessage::hello(room, name).to_json().map_err(js_err)
    }

    #[wasm_bindgen(js_name = netDecode)]
    pub fn net_decode(json: &str) -> Result<String, JsValue> {
        let msg = NetMessage::from_json(json).map_err(js_err)?;
        msg.to_json().map_err(js_err)
    }

    #[wasm_bindgen(js_name = netDt)]
    pub fn net_dt(num: u32, den: u32) -> f32 {
        fixed_dt(num, den)
    }

    #[wasm_bindgen]
    pub struct WasmLockstep {
        inner: Lockstep,
    }

    #[wasm_bindgen]
    impl WasmLockstep {
        #[wasm_bindgen(constructor)]
        pub fn new(slot: u8, players: u8) -> Result<WasmLockstep, JsValue> {
            Lockstep::new(slot, players)
                .map(|inner| Self { inner })
                .map_err(js_err)
        }

        #[wasm_bindgen(js_name = submitLocal)]
        pub fn submit_local(
            &mut self,
            tick: u32,
            lx: f32,
            lz: f32,
            jump: bool,
            attack: bool,
            dodge: bool,
        ) -> Result<String, JsValue> {
            let msg = self
                .inner
                .submit_local(
                    tick,
                    WalkInput {
                        lx,
                        lz,
                        jump,
                        attack,
                        dodge,
                    },
                )
                .map_err(js_err)?;
            msg.to_json().map_err(js_err)
        }

        pub fn ingest(&mut self, json: &str) -> Result<(), JsValue> {
            let msg = NetMessage::from_json(json).map_err(js_err)?;
            self.inner.ingest(&msg).map_err(js_err)
        }

        pub fn ready(&self, tick: u32) -> bool {
            self.inner.ready(tick)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_uint};

    fn stick(lx: f32, attack: bool) -> WalkInput {
        WalkInput {
            lx,
            lz: 0.0,
            jump: false,
            attack,
            dodge: false,
        }
    }

    #[test]
    fn quant_roundtrip_is_stable() {
        for v in [-1.0, -0.5, 0.0, 0.25, 1.0, 4.0, f32::NAN, f32::INFINITY] {
            let q = quant_axis(v);
            let back = dequant_axis(q);
            assert_eq!(quant_axis(back), q);
            assert!((-1.0..=1.0).contains(&back));
        }
        assert_eq!(quant_axis(1.0), 1000);
        assert_eq!(quant_axis(-1.0), -1000);
    }

    #[test]
    fn messages_roundtrip_and_match_the_relay_templates() {
        let hello = NetMessage::hello("ring", "p1");
        let json = hello.to_json().unwrap();
        assert!(json.contains("\"op\":\"hello\""));
        assert_eq!(NetMessage::from_json(&json).unwrap(), hello);

        let welcome =
            NetMessage::from_json(r#"{"op":"welcome","slot":0,"room":"ring","seed":1,"peers":1}"#)
                .unwrap();
        assert!(matches!(
            welcome,
            NetMessage::Welcome {
                slot: 0,
                peers: 1,
                ..
            }
        ));

        let start =
            NetMessage::from_json(r#"{"op":"start","tick0":0,"dt_num":1,"dt_den":60}"#).unwrap();
        match start {
            NetMessage::Start {
                tick0,
                dt_num,
                dt_den,
            } => {
                assert_eq!(tick0, 0);
                assert!((fixed_dt(dt_num, dt_den) - (1.0 / 60.0)).abs() < 1e-6);
            }
            other => panic!("start, got {other:?}"),
        }

        let bye = NetMessage::from_json(r#"{"op":"bye","reason":"room full"}"#).unwrap();
        assert!(matches!(bye, NetMessage::Bye { .. }));
    }

    #[test]
    fn lockstep_waits_for_both_slots_and_rejects_conflicts() {
        let mut a = Lockstep::new(0, 2).unwrap();
        let mut b = Lockstep::new(1, 2).unwrap();
        let m0 = a.submit_local(3, stick(1.0, false)).unwrap();
        assert!(!a.ready(3));
        let m1 = b.submit_local(3, stick(-1.0, true)).unwrap();
        a.ingest(&m1).unwrap();
        b.ingest(&m0).unwrap();
        assert!(a.ready(3) && b.ready(3));
        let rows = a.inputs(3).unwrap();
        let other = b.inputs(3).unwrap();
        assert_eq!(rows.len(), other.len());
        for (left, right) in rows.iter().zip(other.iter()) {
            assert_eq!(left.lx.to_bits(), right.lx.to_bits());
            assert_eq!(left.lz.to_bits(), right.lz.to_bits());
            assert_eq!(
                (left.jump, left.attack, left.dodge),
                (right.jump, right.attack, right.dodge)
            );
        }
        assert_eq!(rows[0].lx, dequant_axis(1000));
        assert!(rows[1].attack);
        assert!(!a.ready(4));
        a.ingest(&m1).unwrap();
        let mut flipped = m1.clone();
        if let NetMessage::Input { input } = &mut flipped {
            input.attack = false;
        }
        assert!(a.ingest(&flipped).is_err());
        assert!(Lockstep::new(2, 2).is_err());
        assert!(Lockstep::new(0, 0).is_err());
    }

    #[test]
    fn c_abi_submit_and_read_back() {
        unsafe {
            let step = kagra_net_lockstep_new(0, 2);
            assert!(!step.is_null());
            let mut buf = [0u8; 256];
            let n = kagra_net_submit_local(
                step,
                1,
                1.0,
                0.0,
                2,
                buf.as_mut_ptr() as *mut c_char,
                buf.len() as c_uint,
            );
            assert!(n > 0);
            let text = CStr::from_ptr(buf.as_ptr() as *const c_char)
                .to_str()
                .unwrap();
            let peer = Lockstep::new(1, 2).unwrap();
            let peer = Box::into_raw(Box::new(peer));
            let remote = NetMessage::Input {
                input: WireInput::from_walk(1, 1, stick(-0.5, false)),
            };
            let remote_json = CString::new(remote.to_json().unwrap()).unwrap();
            assert_eq!(kagra_net_ingest(step, remote_json.as_ptr()), 0);
            assert_eq!(
                kagra_net_ingest(peer, CString::new(text).unwrap().as_ptr()),
                0
            );
            assert_eq!(kagra_net_ingest(peer, remote_json.as_ptr()), 0);
            assert_eq!(kagra_net_ready(step, 1), 1);
            let mut lx = 0f32;
            let mut buttons = 0u32;
            assert_eq!(
                kagra_net_input_axis(step, 1, 0, &mut lx, std::ptr::null_mut(), &mut buttons),
                0
            );
            assert!((lx - 1.0).abs() < 1e-6);
            assert_eq!(buttons & 2, 2);
            kagra_net_lockstep_free(step);
            kagra_net_lockstep_free(peer);
            kagra_net_lockstep_free(std::ptr::null_mut());
        }
    }
}
