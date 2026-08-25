// src/audio.rs
use std::fs::File;
use crate::error::lock_recover;
use std::io::BufReader;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

const MAX_SE_CHANNELS: usize = 32;
const MAX_LOOP_SOURCES: usize = 8;

/// Inverse-distance gain + equal-power stereo pan. Keep in sync with kagra/spatial.py.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialMix {
    pub gain: f32,
    pub pan: f32,
    pub left: f32,
    pub right: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Listener {
    pub pos: [f32; 3],
    pub forward: [f32; 3],
    pub up: [f32; 3],
}

impl Default for Listener {
    fn default() -> Self {
        Self {
            pos: [0.0, 0.0, 0.0],
            forward: [0.0, 0.0, 1.0],
            up: [0.0, 1.0, 0.0],
        }
    }
}

fn vsub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn vdot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn vcross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn vnorm(v: [f32; 3]) -> [f32; 3] {
    let len = vdot(v, v).sqrt();
    if len < 1e-8 {
        [0.0, 0.0, 0.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

/// `pan` -1 = left speaker, +1 = right. Listener right = up × forward
/// so look +Z makes world +X the right speaker.
pub fn spatial_mix(
    listener: &Listener,
    source: [f32; 3],
    ref_distance: f32,
    max_distance: f32,
) -> SpatialMix {
    let ref_d = ref_distance.max(1e-4);
    let max_d = max_distance.max(ref_d);
    let to = vsub(source, listener.pos);
    let dist = vdot(to, to).sqrt();
    let gain = if dist >= max_d {
        0.0
    } else {
        (ref_d / dist.max(ref_d)).clamp(0.0, 1.0)
    };
    let mut pan = 0.0;
    if dist > 1e-6 && gain > 0.0 {
        let fwd = vnorm(listener.forward);
        let up = vnorm(listener.up);
        let right = vnorm(vcross(up, fwd));
        let dir = vnorm(to);
        pan = vdot(dir, right).clamp(-1.0, 1.0);
    }
    let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
    SpatialMix {
        gain,
        pan,
        left: gain * angle.cos(),
        right: gain * angle.sin(),
    }
}

fn store_gain(cell: &AtomicU32, v: f32) {
    cell.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
}

fn load_gain(cell: &AtomicU32) -> f32 {
    f32::from_bits(cell.load(Ordering::Relaxed))
}

/// Mono → stereo (or stereo pass-through) with live left/right gains.
struct StereoPan<S> {
    input: S,
    in_ch: u16,
    emit_right: bool,
    hold: f32,
    left: Arc<AtomicU32>,
    right: Arc<AtomicU32>,
}

impl<S> StereoPan<S>
where
    S: Source<Item = f32>,
{
    fn new(input: S, left: Arc<AtomicU32>, right: Arc<AtomicU32>) -> Self {
        let in_ch = input.channels().max(1);
        Self {
            input,
            in_ch,
            emit_right: false,
            hold: 0.0,
            left,
            right,
        }
    }
}

impl<S> Iterator for StereoPan<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let left = load_gain(&self.left);
        let right = load_gain(&self.right);
        if self.in_ch == 1 {
            if !self.emit_right {
                let s = self.input.next()?;
                self.hold = s;
                self.emit_right = true;
                Some(s * left)
            } else {
                self.emit_right = false;
                Some(self.hold * right)
            }
        } else if !self.emit_right {
            let s = self.input.next()?;
            self.emit_right = true;
            Some(s * left)
        } else {
            let s = self.input.next()?;
            self.emit_right = false;
            Some(s * right)
        }
    }
}

impl<S> Source for StereoPan<S>
where
    S: Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }
}

struct SpatialLoop {
    id: u32,
    sink: Sink,
    pos: [f32; 3],
    volume: f32,
    ref_distance: f32,
    max_distance: f32,
    left: Arc<AtomicU32>,
    right: Arc<AtomicU32>,
}

impl SpatialLoop {
    fn apply(&self, listener: &Listener) {
        let mix = spatial_mix(listener, self.pos, self.ref_distance, self.max_distance);
        let vol = self.volume.clamp(0.0, 1.0);
        store_gain(&self.left, mix.left * vol);
        store_gain(&self.right, mix.right * vol);
    }
}

pub struct AudioEngine {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    bgm_sink: Arc<Mutex<Option<Sink>>>,
    se_pool: Arc<Mutex<Vec<Sink>>>,
    listener: Mutex<Listener>,
    loops: Mutex<Vec<SpatialLoop>>,
    next_loop_id: Mutex<u32>,
}

impl AudioEngine {
    pub fn new() -> Result<Self, String> {
        let (_stream, handle) = OutputStream::try_default().map_err(|e| e.to_string())?;
        Ok(AudioEngine {
            _stream,
            handle,
            bgm_sink: Arc::new(Mutex::new(None)),
            se_pool: Arc::new(Mutex::new(Vec::new())),
            listener: Mutex::new(Listener::default()),
            loops: Mutex::new(Vec::new()),
            next_loop_id: Mutex::new(1),
        })
    }

    fn new_sink(&self) -> Result<Sink, String> {
        Sink::try_new(&self.handle).map_err(|e| e.to_string())
    }

    fn decode_f32(
        &self,
        path: &str,
        kind: &str,
    ) -> Result<impl Source<Item = f32>, String> {
        let file = File::open(Path::new(path))
            .map_err(|e| format!("{kind}ファイルを開けません: {} ({})", path, e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("{kind}デコード失敗: {}", e))?;
        Ok(source.convert_samples::<f32>())
    }

    pub fn play_bgm(&self, path: &str, loop_: bool, volume: f32) -> Result<(), String> {
        {
            let mut g = lock_recover(&self.bgm_sink);
            if let Some(s) = g.take() {
                s.stop();
            }
        }
        let file = File::open(Path::new(path))
            .map_err(|e| format!("BGMファイルを開けません: {} ({})", path, e))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("BGMデコード失敗: {}", e))?;

        let sink = self.new_sink()?;
        sink.set_volume(volume.clamp(0.0, 1.0));
        if loop_ {
            sink.append(source.repeat_infinite());
        } else {
            sink.append(source);
        }

        *lock_recover(&self.bgm_sink) = Some(sink);
        Ok(())
    }

    pub fn stop_bgm(&self, fade: f32) {
        let sink_arc = Arc::clone(&self.bgm_sink);
        if fade <= 0.0 {
            if let Some(s) = lock_recover(&sink_arc).take() {
                s.stop();
            }
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
            if let Some(s) = lock_recover(&sink_arc).take() {
                s.stop();
            }
        });
    }

    pub fn pause_bgm(&self) {
        if let Some(s) = lock_recover(&self.bgm_sink).as_ref() {
            s.pause();
        }
    }

    pub fn resume_bgm(&self) {
        if let Some(s) = lock_recover(&self.bgm_sink).as_ref() {
            s.play();
        }
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

    pub fn play_se_at(
        &self,
        path: &str,
        pos: [f32; 3],
        volume: f32,
        ref_distance: f32,
        max_distance: f32,
    ) -> Result<(), String> {
        let listener = *lock_recover(&self.listener);
        let mix = spatial_mix(&listener, pos, ref_distance, max_distance);
        let vol = volume.clamp(0.0, 1.0);
        let left = Arc::new(AtomicU32::new((mix.left * vol).clamp(0.0, 1.0).to_bits()));
        let right = Arc::new(AtomicU32::new((mix.right * vol).clamp(0.0, 1.0).to_bits()));
        let source = StereoPan::new(self.decode_f32(path, "SE")?, left, right);

        let mut pool = lock_recover(&self.se_pool);
        pool.retain(|s: &Sink| !s.empty());
        if pool.len() >= MAX_SE_CHANNELS {
            pool[0].stop();
            pool.remove(0);
        }
        let sink = self.new_sink()?;
        sink.set_volume(1.0);
        sink.append(source);
        pool.push(sink);
        Ok(())
    }

    pub fn set_listener(
        &self,
        x: f32,
        y: f32,
        z: f32,
        fx: f32,
        fy: f32,
        fz: f32,
        ux: f32,
        uy: f32,
        uz: f32,
    ) {
        let mut fwd = [fx, fy, fz];
        if vdot(fwd, fwd).sqrt() < 1e-8 {
            fwd = [0.0, 0.0, 1.0];
        }
        let mut up = [ux, uy, uz];
        if vdot(up, up).sqrt() < 1e-8 {
            up = [0.0, 1.0, 0.0];
        }
        *lock_recover(&self.listener) = Listener {
            pos: [x, y, z],
            forward: fwd,
            up,
        };
        self.refresh_loops();
    }

    fn refresh_loops(&self) {
        let listener = *lock_recover(&self.listener);
        let mut loops = lock_recover(&self.loops);
        loops.retain(|v| !v.sink.empty());
        for v in loops.iter() {
            v.apply(&listener);
        }
    }

    pub fn play_loop_at(
        &self,
        path: &str,
        pos: [f32; 3],
        volume: f32,
        ref_distance: f32,
        max_distance: f32,
    ) -> Result<u32, String> {
        let left = Arc::new(AtomicU32::new(0));
        let right = Arc::new(AtomicU32::new(0));
        let source = StereoPan::new(
            self.decode_f32(path, "SE")?.repeat_infinite(),
            Arc::clone(&left),
            Arc::clone(&right),
        );
        let sink = self.new_sink()?;
        sink.set_volume(1.0);
        sink.append(source);
        let id = {
            let mut n = lock_recover(&self.next_loop_id);
            let id = *n;
            *n = n.wrapping_add(1).max(1);
            id
        };
        let voice = SpatialLoop {
            id,
            sink,
            pos,
            volume: volume.clamp(0.0, 1.0),
            ref_distance,
            max_distance,
            left,
            right,
        };
        let listener = *lock_recover(&self.listener);
        voice.apply(&listener);
        let mut loops = lock_recover(&self.loops);
        loops.retain(|v| !v.sink.empty());
        if loops.len() >= MAX_LOOP_SOURCES {
            if let Some(old) = loops.first() {
                old.sink.stop();
            }
            loops.remove(0);
        }
        loops.push(voice);
        Ok(id)
    }

    pub fn stop_loop(&self, source_id: Option<u32>) {
        let mut loops = lock_recover(&self.loops);
        if let Some(id) = source_id {
            loops.retain(|v| {
                if v.id == id {
                    v.sink.stop();
                    false
                } else {
                    true
                }
            });
        } else {
            for v in loops.iter() {
                v.sink.stop();
            }
            loops.clear();
        }
    }

    pub fn stop_all_se(&self) {
        let mut pool = lock_recover(&self.se_pool);
        for s in pool.iter() {
            s.stop();
        }
        pool.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look_z() -> Listener {
        Listener::default()
    }

    #[test]
    fn closer_is_louder_than_far() {
        let near = spatial_mix(&look_z(), [0.0, 0.0, 4.0], 4.0, 48.0);
        let mid = spatial_mix(&look_z(), [0.0, 0.0, 8.0], 4.0, 48.0);
        let far = spatial_mix(&look_z(), [0.0, 0.0, 80.0], 4.0, 48.0);
        assert!((near.gain - 1.0).abs() < 1e-5);
        assert!((mid.gain - 0.5).abs() < 1e-5);
        assert_eq!(far.gain, 0.0);
        assert!(near.gain > mid.gain);
    }

    #[test]
    fn right_source_pans_right() {
        let mix = spatial_mix(&look_z(), [4.0, 0.0, 0.0], 4.0, 48.0);
        assert!(mix.pan > 0.9, "pan={}", mix.pan);
        assert!(mix.right > mix.left);
        assert!(mix.left < 0.05);
    }

    #[test]
    fn left_source_pans_left() {
        let mix = spatial_mix(&look_z(), [-8.0, 0.0, 0.0], 4.0, 48.0);
        assert!(mix.pan < -0.9, "pan={}", mix.pan);
        assert!(mix.left > mix.right);
        assert!((mix.gain - 0.5).abs() < 1e-5);
    }

    #[test]
    fn front_is_centered_equal_power() {
        let mix = spatial_mix(&look_z(), [0.0, 0.0, 4.0], 4.0, 48.0);
        assert!(mix.pan.abs() < 1e-5, "pan={}", mix.pan);
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!((mix.left - half).abs() < 1e-5);
        assert!((mix.right - half).abs() < 1e-5);
    }

    #[test]
    fn coincident_is_full_and_centered() {
        let lis = Listener {
            pos: [1.0, 2.0, 3.0],
            ..Listener::default()
        };
        let mix = spatial_mix(&lis, [1.0, 2.0, 3.0], 4.0, 48.0);
        assert!((mix.gain - 1.0).abs() < 1e-5);
        assert!(mix.pan.abs() < 1e-5);
    }
}
