//! Desktop speakers. Missing devices, `cargo test`, wasm, and Android no-op.
//!
//! Android drains [`crate::mixer::cues_json`] into SoundPool. The `playback`
//! feature (on the Python wheel) uses rodio and treats `try_default` errors
//! as silence so lavapipe CI never fails a frame.

/// Push one SFX if a device is already open or opens cleanly. Never panics.
pub fn submit_sfx(name: &str) {
    #[cfg(all(
        feature = "playback",
        not(target_arch = "wasm32"),
        not(target_os = "android")
    ))]
    desktop::submit_sfx(name);
    #[cfg(not(all(
        feature = "playback",
        not(target_arch = "wasm32"),
        not(target_os = "android")
    )))]
    {
        let _ = name;
    }
}

/// Match the BGM sink volume / loop to the mixer. Safe with no device.
pub fn sync_bgm() {
    #[cfg(all(
        feature = "playback",
        not(target_arch = "wasm32"),
        not(target_os = "android")
    ))]
    desktop::sync_bgm();
}

#[cfg(all(
    feature = "playback",
    not(target_arch = "wasm32"),
    not(target_os = "android")
))]
mod desktop {
    use std::cell::RefCell;
    use std::io::Cursor;

    use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

    use crate::mixer;

    pub fn submit_sfx(name: &str) {
        if !device_wanted() {
            return;
        }
        let audible = mixer::with_mixer(|m| m.sfx_audible());
        if !audible {
            return;
        }
        let Some(bytes) = mixer::clip_wav(name) else {
            return;
        };
        let Some(handle) = handle() else {
            return;
        };
        let gain = mixer::with_mixer(|m| (m.master * m.sfx).clamp(0.0, 1.0));
        if let Ok(sink) = Sink::try_new(&handle) {
            if let Ok(source) = Decoder::new(Cursor::new(bytes.to_vec())) {
                sink.set_volume(gain);
                sink.append(source);
                sink.detach();
            }
        }
    }

    pub fn sync_bgm() {
        if !device_wanted() {
            return;
        }
        let (name, gain, looping) =
            mixer::with_mixer(|m| (m.state().bgm_name, m.bgm_audible_gain(), m.state().loop_bgm));
        DEVICE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let Some(dev) = slot.as_mut() else {
                if name.is_empty() || gain <= 0.0 {
                    return;
                }
                // Open only when there is something to hear.
                drop(slot);
                if handle().is_none() {
                    return;
                }
                sync_bgm();
                return;
            };
            if name.is_empty() || gain <= 0.0 {
                dev.bgm = None;
                dev.bgm_key.clear();
                return;
            }
            let key = format!("{name}:{looping}");
            if dev.bgm_key != key {
                dev.bgm = None;
                let Some(bytes) = mixer::clip_wav(&name) else {
                    return;
                };
                if let Ok(sink) = Sink::try_new(&dev.handle) {
                    if let Ok(source) = Decoder::new(Cursor::new(bytes.to_vec())) {
                        if looping {
                            sink.append(source.repeat_infinite());
                        } else {
                            sink.append(source);
                        }
                        sink.set_volume(gain);
                        dev.bgm = Some(sink);
                        dev.bgm_key = key;
                    }
                }
            } else if let Some(sink) = dev.bgm.as_ref() {
                sink.set_volume(gain);
            }
        });
    }

    struct Device {
        // Kept alive: dropping the stream stops every sink.
        _stream: OutputStream,
        handle: OutputStreamHandle,
        bgm: Option<Sink>,
        bgm_key: String,
    }

    thread_local! {
        static DEVICE: RefCell<Option<Device>> = const { RefCell::new(None) };
        static FAILED: RefCell<bool> = const { RefCell::new(false) };
    }

    fn device_wanted() -> bool {
        match std::env::var("KAGRA_AUDIO").ok().as_deref() {
            Some("0") | Some("off") => return false,
            Some("1") | Some("on") => return true,
            _ => {}
        }
        // Unit tests must not touch the host mixer (CI, lavapipe, headless).
        #[cfg(test)]
        {
            false
        }
        #[cfg(not(test))]
        {
            true
        }
    }

    fn handle() -> Option<OutputStreamHandle> {
        if !device_wanted() {
            return None;
        }
        if FAILED.with(|f| *f.borrow()) {
            return None;
        }
        DEVICE.with(|slot| {
            if let Some(dev) = slot.borrow().as_ref() {
                return Some(dev.handle.clone());
            }
            let opened = std::panic::catch_unwind(OutputStream::try_default);
            match opened {
                Ok(Ok((stream, handle))) => {
                    let cloned = handle.clone();
                    *slot.borrow_mut() = Some(Device {
                        _stream: stream,
                        handle,
                        bgm: None,
                        bgm_key: String::new(),
                    });
                    Some(cloned)
                }
                _ => {
                    FAILED.with(|f| *f.borrow_mut() = true);
                    None
                }
            }
        })
    }
}
