"""Neon Cat's placeholder sounds (CC0-1.0, synthesized here, no input audio).

Thirteen one-shots and one looping BGM, 22.05 kHz mono 16-bit WAV, written to
`assets/audio/neon_cat/`. They stand in until real sounds are chosen: point
`game.audio` at other files (wav, mp3 or ogg) and nothing else changes.

    python3 tools/gen_assets/cat_audio.py          # write the files
    python3 tools/gen_assets/cat_audio.py --check  # exit 1 on drift

Everything is plain Python (sines, saws, filtered noise, a little delay), seeded,
so two runs write the same bytes.
"""
from __future__ import annotations

import math
import random
import struct
import sys
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "assets" / "audio" / "neon_cat"
RATE = 22_050
TAU = math.pi * 2.0


def hz(note: int) -> float:
    """MIDI note number to frequency."""
    return 440.0 * 2.0 ** ((note - 69) / 12.0)


class Buf:
    def __init__(self, secs: float) -> None:
        self.n = int(secs * RATE)
        self.s = [0.0] * self.n

    def add(self, other: list[float], at: float = 0.0, gain: float = 1.0, wrap: bool = False) -> None:
        i0 = int(at * RATE)
        for i, v in enumerate(other):
            j = i0 + i
            if j >= self.n:
                if not wrap:
                    break
                j %= self.n
            self.s[j] += v * gain


def env(i: int, n: int, attack: float, release: float) -> float:
    t = i / RATE
    left = (n - i) / RATE
    a = min(1.0, t / attack) if attack > 0 else 1.0
    r = min(1.0, left / release) if release > 0 else 1.0
    return a * r


def tone(freq: float, secs: float, *, wave_: str = "sine", attack: float = 0.004, release: float = 0.06,
         decay: float = 0.0, slide: float = 0.0, vib: tuple[float, float] = (0.0, 0.0), phase: float = 0.0) -> list[float]:
    """One voice. `decay` is an exponential fall in 1/s, `slide` an octave drift per second."""
    n = int(secs * RATE)
    out = []
    ph = phase
    for i in range(n):
        t = i / RATE
        f = freq * 2.0 ** (slide * t)
        if vib[0]:
            f *= 1.0 + vib[1] * math.sin(TAU * vib[0] * t)
        ph += TAU * f / RATE
        if wave_ == "sine":
            v = math.sin(ph)
        elif wave_ == "tri":
            v = 2.0 / math.pi * math.asin(math.sin(ph))
        elif wave_ == "square":
            v = 1.0 if math.sin(ph) >= 0 else -1.0
            v *= 0.6
        else:  # saw
            v = 2.0 * ((ph / TAU) % 1.0) - 1.0
        out.append(v * env(i, n, attack, release) * math.exp(-decay * t))
    return out


def noise(secs: float, rng: random.Random, *, attack: float = 0.002, release: float = 0.05, decay: float = 0.0) -> list[float]:
    n = int(secs * RATE)
    return [(rng.random() * 2 - 1) * env(i, n, attack, release) * math.exp(-decay * i / RATE) for i in range(n)]


def lowpass(x: list[float], cutoff: float) -> list[float]:
    a = 1.0 - math.exp(-TAU * cutoff / RATE)
    y, out = 0.0, []
    for v in x:
        y += a * (v - y)
        out.append(y)
    return out


def highpass(x: list[float], cutoff: float) -> list[float]:
    lp = lowpass(x, cutoff)
    return [v - l for v, l in zip(x, lp)]


def sweep_lowpass(x: list[float], c0: float, c1: float) -> list[float]:
    y, out, n = 0.0, [], max(1, len(x) - 1)
    for i, v in enumerate(x):
        c = c0 + (c1 - c0) * i / n
        y += (1.0 - math.exp(-TAU * c / RATE)) * (v - y)
        out.append(y)
    return out


def bandpass(x: list[float], centre: list[float] | float, q: float = 6.0) -> list[float]:
    """Biquad band-pass; `centre` may be a per-sample list so a formant can glide."""
    out = []
    x1 = x2 = y1 = y2 = 0.0
    for i, v in enumerate(x):
        f = centre[i] if isinstance(centre, list) else centre
        w = TAU * f / RATE
        alpha = math.sin(w) / (2 * q)
        b0, b2 = alpha, -alpha
        a0, a1, a2 = 1 + alpha, -2 * math.cos(w), 1 - alpha
        y = (b0 * v + b2 * x2 - a1 * y1 - a2 * y2) / a0
        x2, x1 = x1, v
        y2, y1 = y1, y
        out.append(y)
    return out


def mix(*parts: list[float]) -> list[float]:
    n = max(len(p) for p in parts)
    out = [0.0] * n
    for p in parts:
        for i, v in enumerate(p):
            out[i] += v
    return out


def scale(x: list[float], g: float) -> list[float]:
    return [v * g for v in x]


def delay(x: list[float], secs: float, feedback: float, wet: float, taps: int = 4) -> list[float]:
    d = int(secs * RATE)
    out = list(x)
    for k in range(1, taps + 1):
        g = wet * feedback ** (k - 1)
        for i in range(len(x)):
            j = i + d * k
            if j < len(out):
                out[j] += x[i] * g
    return out


def to_wav(name: str, x: list[float], peak: float) -> bytes:
    top = max(1e-9, max(abs(v) for v in x))
    k = peak / top
    pcm = b"".join(struct.pack("<h", int(max(-1.0, min(1.0, v * k)) * 32767)) for v in x)
    path = OUT / name
    tmp = path.with_suffix(".tmp")
    with wave.open(str(tmp), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(pcm)
    data = tmp.read_bytes()
    tmp.unlink()
    return data


# ---- one-shots -------------------------------------------------------------

def jump(rng):
    a = tone(330, 0.16, wave_="tri", slide=1.0, release=0.08, decay=6)
    b = tone(660, 0.12, wave_="sine", slide=0.8, release=0.07, decay=10)
    return mix(a, scale(b, 0.35)), 0.55


def roll(rng):
    x = noise(0.26, rng, attack=0.03, release=0.14)
    return sweep_lowpass(x, 2600, 500), 0.5


def coin(rng):
    a = tone(1318.5, 0.07, wave_="tri", release=0.03, decay=12)
    b = tone(1975.5, 0.2, wave_="sine", release=0.1, decay=9)
    out = [0.0] * int(0.28 * RATE)
    for i, v in enumerate(a):
        out[i] += v
    for i, v in enumerate(b):
        out[i + int(0.07 * RATE)] += v * 0.9
    return out, 0.5


def perch(rng):
    thud = tone(170, 0.14, wave_="sine", slide=-1.2, release=0.05, decay=18)
    pad = lowpass(noise(0.07, rng, release=0.05, decay=30), 900)
    blip = tone(587.3, 0.14, wave_="tri", release=0.07, decay=14)
    out = mix(thud, scale(pad, 1.2))
    out += [0.0] * int(0.1 * RATE)
    for i, v in enumerate(blip):
        out[i + int(0.05 * RATE)] += v * 0.4
    return out, 0.55


def knock(rng):
    a = tone(240, 0.12, wave_="sine", slide=-0.9, release=0.04, decay=34)
    b = tone(132, 0.14, wave_="sine", release=0.05, decay=26)
    click = highpass(noise(0.012, rng, release=0.008), 2500)
    return mix(a, scale(b, 0.8), scale(click, 0.6)), 0.55


def item(rng):
    out = [0.0] * int(0.8 * RATE)
    for k, n in enumerate((72, 76, 79, 84)):
        t = tone(hz(n), 0.22, wave_="tri", release=0.1, decay=7)
        for i, v in enumerate(t):
            out[i + int(k * 0.065 * RATE)] += v
    return delay(out, 0.09, 0.5, 0.35, 2), 0.5


def meow(rng):
    n = int(0.5 * RATE)
    glide = []
    for i in range(n):
        t = i / n
        # up, then down: a short "mi-aou"
        glide.append(540.0 + 330.0 * math.sin(math.pi * min(1.0, t * 1.15)) - 120.0 * t)
    ph, src = 0.0, []
    for i in range(n):
        f = glide[i] * (1.0 + 0.012 * math.sin(TAU * 6.5 * i / RATE))
        ph += TAU * f / RATE
        saw = 2.0 * ((ph / TAU) % 1.0) - 1.0
        src.append(saw * env(i, n, 0.03, 0.18))
    f1 = [650 + 520 * math.sin(math.pi * i / n) for i in range(n)]
    f2 = [1250 + 700 * math.sin(math.pi * i / n) for i in range(n)]
    voice = mix(scale(bandpass(src, f1, 5.0), 1.0), scale(bandpass(src, f2, 7.0), 0.8))
    return voice, 0.5


def hit(rng):
    low = tone(210, 0.24, wave_="saw", slide=-1.6, release=0.1, decay=9)
    burst = lowpass(noise(0.16, rng, release=0.1, decay=14), 3200)
    return mix(scale(lowpass(low, 900), 0.7), burst), 0.6


def robot_on(rng):
    out = [0.0] * int(0.3 * RATE)
    for k, f in enumerate((660, 880, 660)):
        t = lowpass(tone(f, 0.075, wave_="square", release=0.015), 2600)
        for i, v in enumerate(t):
            out[i + int(k * 0.095 * RATE)] += v
    return out, 0.4


def robot_off(rng):
    t = lowpass(tone(900, 0.3, wave_="square", slide=-1.2, release=0.12, decay=4), 2200)
    return t, 0.4


def gameover(rng):
    out = [0.0] * int(1.3 * RATE)
    for k, (n, d) in enumerate(((69, 0.0), (65, 0.28), (62, 0.56))):
        t = lowpass(tone(hz(n), 0.7, wave_="saw", attack=0.01, release=0.4, decay=2.2), 1400)
        for i, v in enumerate(t):
            out[i + int(d * RATE)] += v * 0.5
    thump = tone(58, 0.5, wave_="sine", slide=-0.6, release=0.2, decay=3.5)
    for i, v in enumerate(thump):
        out[i + int(0.56 * RATE)] += v * 0.9
    return out, 0.55


def card(rng):
    t = tone(1046.5, 0.22, wave_="sine", release=0.1, decay=10)
    t2 = tone(2093.0, 0.15, wave_="tri", release=0.08, decay=14)
    return mix(t, scale(t2, 0.3)), 0.45


def mission(rng):
    out = [0.0] * int(1.1 * RATE)
    for k, n in enumerate((79, 83, 86, 91)):
        t = tone(hz(n), 0.3, wave_="tri", release=0.12, decay=5)
        t2 = tone(hz(n) * 2, 0.2, wave_="sine", release=0.1, decay=8)
        for i, v in enumerate(t):
            out[i + int(k * 0.09 * RATE)] += v
        for i, v in enumerate(t2):
            out[i + int(k * 0.09 * RATE)] += v * 0.25
    return delay(out, 0.12, 0.5, 0.3, 2), 0.5


ONE_SHOTS = {
    "jump": jump, "roll": roll, "coin": coin, "perch": perch, "knock": knock, "item": item,
    "meow": meow, "hit": hit, "robot_on": robot_on, "robot_off": robot_off, "gameover": gameover,
    "card": card, "mission": mission,
}


# ---- BGM -------------------------------------------------------------------

BPM = 96
BARS = 4
SPB = 60.0 / BPM  # seconds per beat
LOOP = BARS * 4 * SPB  # 10 s

# Am9, Fmaj7, Cmaj7(add9), Esus4 -> E: a moody minor loop. (root, chord tones)
CHORDS = [
    (45, (57, 60, 64, 67)),   # A2 | A3 C4 E4 G4
    (41, (53, 57, 60, 64)),   # F2 | F3 A3 C4 E4
    (48, (55, 59, 62, 64)),   # C3 | G3 B3 D4 E4
    (40, (56, 59, 62, 64)),   # E2 | G#3 B3 D4 E4 (E7-ish pull back to Am)
]


def bgm() -> tuple[list[float], float]:
    rng = random.Random(7)
    tail = 2.4
    buf = Buf(LOOP + tail)
    for bar, (root, tones) in enumerate(CHORDS):
        t0 = bar * 4 * SPB
        # Pad: three slightly detuned saws per tone under a slow-opening low-pass.
        pad = [0.0] * int((4 * SPB + 1.2) * RATE)
        for n in tones[:3]:
            for det in (-0.004, 0.0, 0.004):
                v = tone(hz(n) * (1 + det), 4 * SPB + 1.2, wave_="saw", attack=0.5, release=1.2)
                for i, s in enumerate(v):
                    pad[i] += s * 0.05
        pad = sweep_lowpass(pad, 380, 1200)
        buf.add(pad, t0, 1.0, wrap=True)
        # Sub: a soft pulse on the root, eighths with a gap on the "and" of 3.
        for step in range(8):
            if step == 5:
                continue
            note = tone(hz(root), SPB * 0.5, wave_="sine", attack=0.01, release=0.1, decay=2.4)
            buf.add(note, t0 + step * SPB * 0.5, 0.42 if step % 2 == 0 else 0.26, wrap=True)
        # Arp: sixteenths over the chord, picked pattern, with a delay below.
        pattern = (0, 2, 1, 3, 2, 1, 3, 2, 0, 3, 1, 2, 3, 2, 1, 0)
        for k, idx in enumerate(pattern):
            if k % 4 == 3 and bar % 2 == 0:
                continue
            note = tones[idx % 4] + 12
            pluck = tone(hz(note), SPB * 0.55, wave_="tri", attack=0.004, release=0.08, decay=7)
            buf.add(pluck, t0 + k * SPB * 0.25, 0.10 + 0.04 * (k % 4 == 0), wrap=True)
        # Hat: dry ticks on the off-beats.
        for beat in range(4):
            tick = highpass(noise(0.03, rng, release=0.02, decay=60), 6000)
            buf.add(tick, t0 + (beat + 0.5) * SPB, 0.22, wrap=True)
        # Kick feel: a very soft low thump on 1 and 3 so the loop has a floor.
        for beat in (0, 2):
            thump = tone(70, 0.22, wave_="sine", slide=-1.5, release=0.1, decay=12)
            buf.add(thump, t0 + beat * SPB, 0.55, wrap=True)
    x = buf.s
    # Echo the whole bed lightly, wrapping, then add the rain bed.
    x = delay(x, SPB * 0.75, 0.5, 0.18, 3)
    n_loop = int(LOOP * RATE)
    folded = [0.0] * n_loop
    for i, v in enumerate(x):
        folded[i % n_loop] += v
    rain = lowpass(noise(LOOP, rng, attack=0.0, release=0.0), 5200)
    rain = highpass(rain, 1400)
    folded = [v + r * 0.07 for v, r in zip(folded, rain[:n_loop])]
    # Short fade at the seam so the folded noise cannot click.
    k = int(0.01 * RATE)
    for i in range(k):
        g = i / k
        folded[i] *= g
        folded[n_loop - 1 - i] *= g
    return folded, 0.42


def build() -> dict[str, bytes]:
    OUT.mkdir(parents=True, exist_ok=True)
    files: dict[str, bytes] = {}
    for name, fn in ONE_SHOTS.items():
        rng = random.Random(sum(map(ord, name)))
        x, peak = fn(rng)
        files[f"{name}.wav"] = to_wav(f"{name}.wav", x, peak)
    x, peak = bgm()
    files["bgm.wav"] = to_wav("bgm.wav", x, peak)
    return files


def main(argv: list[str]) -> int:
    files = build()
    if "--check" in argv:
        for name, data in files.items():
            p = OUT / name
            if not p.is_file() or p.read_bytes() != data:
                print("drift:", name)
                return 1
        return 0
    for name, data in files.items():
        (OUT / name).write_bytes(data)
        print("wrote", (OUT / name).relative_to(ROOT), len(data))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
