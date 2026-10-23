//! Procedural one-shots. The parameter shape follows sfxr (a wave, a slide,
//! a decay). The oscillator is original. No sfxr source is copied.
//!
//! Output is 16-bit mono PCM in a WAV so wasm, Android, and rodio can play
//! the same bytes. Unknown names return None. Bundled clips in `mixer`
//! still win when a file exists.

const RATE: f32 = 22_050.0;

#[derive(Clone, Copy)]
struct Voice {
    /// 0 sine, 1 square, 2 saw, 3 noise.
    wave: u8,
    freq: f32,
    /// Multiplied into `freq` every sample. 1 keeps the pitch.
    slide: f32,
    seconds: f32,
    volume: f32,
    /// Multiplied into the envelope every sample.
    decay: f32,
}

/// WAV bytes for a named preset, or None when the name is not a preset.
pub fn synth_wav(name: &str) -> Option<Vec<u8>> {
    let pcm = synth_pcm(name)?;
    Some(wav_mono_i16(&pcm, RATE as u32))
}

fn synth_pcm(name: &str) -> Option<Vec<i16>> {
    let voices: &[Voice] = match name.trim().to_ascii_lowercase().as_str() {
        "jump" => &[Voice {
            wave: 1,
            freq: 520.0,
            slide: 0.9992,
            seconds: 0.12,
            volume: 0.35,
            decay: 0.9995,
        }],
        "hit" | "pickup" => &[Voice {
            wave: 3,
            freq: 0.0,
            slide: 1.0,
            seconds: 0.08,
            volume: 0.45,
            decay: 0.992,
        }],
        "coin" => &[
            Voice {
                wave: 0,
                freq: 880.0,
                slide: 1.0,
                seconds: 0.06,
                volume: 0.32,
                decay: 0.999,
            },
            Voice {
                wave: 0,
                freq: 1320.0,
                slide: 1.0,
                seconds: 0.1,
                volume: 0.32,
                decay: 0.999,
            },
        ],
        "ok" => &[Voice {
            wave: 0,
            freq: 660.0,
            slide: 1.0,
            seconds: 0.08,
            volume: 0.3,
            decay: 0.998,
        }],
        "hurt" => &[Voice {
            wave: 2,
            freq: 180.0,
            slide: 0.9994,
            seconds: 0.18,
            volume: 0.34,
            decay: 0.999,
        }],
        "explode" => &[
            Voice {
                wave: 3,
                freq: 0.0,
                slide: 1.0,
                seconds: 0.28,
                volume: 0.5,
                decay: 0.996,
            },
            Voice {
                wave: 1,
                freq: 90.0,
                slide: 0.998,
                seconds: 0.28,
                volume: 0.2,
                decay: 0.997,
            },
        ],
        _ => return None,
    };
    let mut pcm = Vec::new();
    for voice in voices {
        mix_or_extend(&mut pcm, &render(voice));
    }
    Some(pcm)
}

/// Noise and a tone that share a duration are summed. A later, longer,
/// or following voice is appended once the overlap ends.
fn mix_or_extend(into: &mut Vec<i16>, next: &[i16]) {
    if into.is_empty() || next.len() > into.len() + into.len() / 8 {
        let n = into.len().max(next.len());
        let mut mixed = vec![0i16; n];
        for (i, s) in into.iter().enumerate() {
            mixed[i] = *s;
        }
        for (i, s) in next.iter().enumerate() {
            mixed[i] = mixed[i].saturating_add(*s);
        }
        *into = mixed;
        return;
    }
    into.extend_from_slice(next);
}

fn render(voice: &Voice) -> Vec<i16> {
    let n = ((RATE * voice.seconds).max(1.0)) as usize;
    let mut freq = voice.freq.max(0.0);
    let mut env = 1.0f32;
    let mut phase = 0.0f32;
    let mut noise = 0xA341_316Cu32;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let sample = match voice.wave {
            1 => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            2 => phase * 2.0 - 1.0,
            3 => {
                noise = noise.wrapping_mul(1664525).wrapping_add(1013904223);
                ((noise >> 16) as f32 / 32768.0) - 1.0
            }
            _ => (phase * std::f32::consts::TAU).sin(),
        };
        let s = (sample * env * voice.volume).clamp(-1.0, 1.0);
        out.push((s * 32767.0) as i16);
        if freq > 0.0 {
            phase = (phase + freq / RATE).fract();
            freq = (freq * voice.slide).clamp(20.0, 8_000.0);
        }
        env *= voice.decay;
    }
    out
}

fn wav_mono_i16(pcm: &[i16], rate: u32) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in pcm {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crossings(pcm_wav: &[u8]) -> usize {
        let pcm = &pcm_wav[44..];
        let mut n = 0;
        let mut prev = 0i16;
        for chunk in pcm.chunks_exact(2) {
            let s = i16::from_le_bytes([chunk[0], chunk[1]]);
            if prev.signum() != 0 && s.signum() != 0 && prev.signum() != s.signum() {
                n += 1;
            }
            if s != 0 {
                prev = s;
            }
        }
        n
    }

    #[test]
    fn presets_are_wavs_and_unknown_names_are_absent() {
        let jump = synth_wav("jump").expect("jump");
        assert!(jump.starts_with(b"RIFF"));
        assert!(jump.len() > 44);
        assert!(synth_wav("Jump").unwrap().len() == jump.len());
        assert_eq!(synth_wav("nope"), None);
        assert_eq!(synth_wav(""), None);
        assert_eq!(synth_wav("jump"), synth_wav("jump"));
    }

    #[test]
    fn hit_is_noisier_than_jump_and_explode_is_longer() {
        let jump = synth_wav("jump").unwrap();
        let hit = synth_wav("hit").unwrap();
        let boom = synth_wav("explode").unwrap();
        assert!(
            crossings(&hit) > crossings(&jump) * 2,
            "noise should cross zero more often than a falling square"
        );
        assert!(boom.len() > hit.len() * 2);
        let energy: i64 = jump[44..]
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as i64)
            .map(|s| s.abs())
            .sum();
        assert!(energy > 10_000, "jump is not silence, energy={energy}");
    }
}
