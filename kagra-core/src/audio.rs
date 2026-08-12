// src/audio.rs
use std::fs::File;
use crate::error::lock_recover;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, Mutex};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

const MAX_SE_CHANNELS: usize = 32;

pub struct AudioEngine {
    _stream: OutputStream,
    handle:  OutputStreamHandle,
    bgm_sink: Arc<Mutex<Option<Sink>>>,
    se_pool:  Arc<Mutex<Vec<Sink>>>,
}

impl AudioEngine {
    pub fn new() -> Result<Self, String> {
        let (_stream, handle) = OutputStream::try_default()
            .map_err(|e| e.to_string())?;
        Ok(AudioEngine {
            _stream, handle,
            bgm_sink: Arc::new(Mutex::new(None)),
            se_pool:  Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn new_sink(&self) -> Result<Sink, String> {
        Sink::try_new(&self.handle).map_err(|e| e.to_string())
    }

    pub fn play_bgm(&self, path: &str, loop_: bool, volume: f32) -> Result<(), String> {
        {
            let mut g = lock_recover(&self.bgm_sink);
            if let Some(s) = g.take() { s.stop(); }
        }
        let file = File::open(Path::new(path))
            .map_err(|e| format!("BGMファイルを開けません: {} ({})", path, e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("BGMデコード失敗: {}", e))?;

        let sink = self.new_sink()?;
        sink.set_volume(volume.clamp(0.0, 1.0));
        if loop_ { sink.append(source.repeat_infinite()); }
        else      { sink.append(source); }

        *lock_recover(&self.bgm_sink) = Some(sink);
        Ok(())
    }

    pub fn stop_bgm(&self, fade: f32) {
        let sink_arc = Arc::clone(&self.bgm_sink);
        if fade <= 0.0 {
            if let Some(s) = lock_recover(&sink_arc).take() { s.stop(); }
            return;
        }
        std::thread::spawn(move || {
            let steps = 20u32;
            let interval = std::time::Duration::from_secs_f32(fade / steps as f32);
            for step in (0..steps).rev() {
                {
                    let g = lock_recover(&sink_arc);
                    if let Some(s) = g.as_ref() {
                        s.set_volume(step as f32 / steps as f32);
                    } else {
                        return;
                    }
                }
                std::thread::sleep(interval);
            }
            if let Some(s) = lock_recover(&sink_arc).take() { s.stop(); }
        });
    }

    pub fn pause_bgm(&self) {
        if let Some(s) = lock_recover(&self.bgm_sink).as_ref() { s.pause(); }
    }

    pub fn resume_bgm(&self) {
        if let Some(s) = lock_recover(&self.bgm_sink).as_ref() { s.play(); }
    }

    pub fn set_bgm_volume(&self, vol: f32) {
        if let Some(s) = lock_recover(&self.bgm_sink).as_ref() {
            s.set_volume(vol.clamp(0.0, 1.0));
        }
    }

    pub fn play_se(&self, path: &str, volume: f32) -> Result<(), String> {
        let file = File::open(Path::new(path))
            .map_err(|e| format!("SEファイルを開けません: {} ({})", path, e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("SEデコード失敗: {}", e))?;

        let mut pool = lock_recover(&self.se_pool);
        pool.retain(|s: &Sink| !s.empty());
        if pool.len() >= MAX_SE_CHANNELS {
            pool[0].stop();
            pool.remove(0);
        }
        let sink = self.new_sink()?;
        sink.set_volume(volume.clamp(0.0, 1.0));
        sink.append(source);
        pool.push(sink);
        Ok(())
    }

    pub fn stop_all_se(&self) {
        let mut pool = lock_recover(&self.se_pool);
        for s in pool.iter() { s.stop(); }
        pool.clear();
    }
}