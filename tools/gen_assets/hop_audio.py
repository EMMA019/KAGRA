"""HOP-BEAT's eight short effects (CC0-1.0, synthesized here, no input audio).

hop, land, perfect, good, combo, hit, fall, stomp: 22.05 kHz mono 16-bit WAV in
`assets/audio/hop_beat/`. They are pitched to A minor to sit under the music
pack (hop_beat_loop.mp3, hop_beat_{gameover,newbest,revive}.mp3, which are
committed as built and described in assets/LEDGER.json).

    python3 tools/gen_assets/hop_audio.py          # write the files
    python3 tools/gen_assets/hop_audio.py --check  # exit 1 on drift

Reuses the oscillators and filters of cat_audio.py; seeded, so two runs write
the same bytes.
"""
from __future__ import annotations

import random
import struct
import sys
import wave
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import cat_audio as c  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "assets" / "audio" / "hop_beat"


def _place(out: list[float], part: list[float], at: float, gain: float = 1.0) -> None:
    i0 = int(at * c.RATE)
    for i, v in enumerate(part):
        if i0 + i < len(out):
            out[i0 + i] += v * gain


def hop(rng):
    a = c.tone(c.hz(69), 0.13, wave_="tri", slide=1.4, release=0.06, decay=8)
    b = c.tone(c.hz(81), 0.09, wave_="sine", slide=1.0, release=0.05, decay=14)
    return c.mix(a, c.scale(b, 0.3)), 0.5


def land(rng):
    thud = c.tone(120, 0.1, wave_="sine", slide=-1.0, release=0.05, decay=26)
    puff = c.lowpass(c.noise(0.05, rng, release=0.04, decay=40), 700)
    return c.mix(thud, c.scale(puff, 0.6)), 0.45


def perfect(rng):
    out = [0.0] * int(0.4 * c.RATE)
    _place(out, c.tone(c.hz(88), 0.34, wave_="sine", release=0.2, decay=7), 0.0)
    _place(out, c.tone(c.hz(93), 0.3, wave_="tri", release=0.18, decay=8), 0.035, 0.6)
    _place(out, c.tone(c.hz(100), 0.2, wave_="sine", release=0.1, decay=10), 0.035, 0.25)
    return out, 0.5


def good(rng):
    return c.mix(c.tone(c.hz(81), 0.22, wave_="tri", release=0.12, decay=9),
                 c.scale(c.tone(c.hz(88), 0.12, wave_="sine", release=0.06, decay=14), 0.3)), 0.4


def combo(rng):
    out = [0.0] * int(0.5 * c.RATE)
    for k, n in enumerate((81, 84, 88, 93)):
        _place(out, c.tone(c.hz(n), 0.18, wave_="tri", release=0.08, decay=8), k * 0.055)
    return c.delay(out, 0.08, 0.5, 0.3, 2), 0.5


def hit(rng):
    low = c.tone(190, 0.2, wave_="saw", slide=-1.5, release=0.09, decay=10)
    burst = c.lowpass(c.noise(0.14, rng, release=0.09, decay=16), 2800)
    return c.mix(c.scale(c.lowpass(low, 800), 0.7), burst), 0.6


def fall(rng):
    t = c.lowpass(c.tone(c.hz(76), 0.55, wave_="saw", slide=-1.3, release=0.25, decay=3), 1600)
    return t, 0.5


def stomp(rng):
    """Landing on an enemy: a wet squish, then a springy boing up to the bounce."""
    out = [0.0] * int(0.34 * c.RATE)
    squish = c.lowpass(c.noise(0.07, rng, release=0.05, decay=30), 1100)
    _place(out, squish, 0.0, 0.7)
    _place(out, c.tone(c.hz(64), 0.07, wave_="sine", slide=-0.5, release=0.04, decay=20), 0.0, 0.6)
    _place(out, c.tone(c.hz(69), 0.2, wave_="tri", slide=1.8, release=0.1, decay=9), 0.05)
    _place(out, c.tone(c.hz(81), 0.14, wave_="sine", slide=1.2, release=0.07, decay=12), 0.09, 0.35)
    return out, 0.6


SHOTS = {
    "hop": hop,
    "land": land,
    "perfect": perfect,
    "good": good,
    "combo": combo,
    "hit": hit,
    "fall": fall,
    "stomp": stomp,
}


def render(name: str) -> bytes:
    x, peak = SHOTS[name](random.Random(sum(map(ord, name))))
    top = max(1e-9, max(abs(v) for v in x))
    pcm = b"".join(struct.pack("<h", int(max(-1.0, min(1.0, v * peak / top)) * 32767)) for v in x)
    tmp = OUT / f".{name}.tmp"
    with wave.open(str(tmp), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(c.RATE)
        w.writeframes(pcm)
    data = tmp.read_bytes()
    tmp.unlink()
    return data


def main(argv: list[str]) -> int:
    check = "--check" in argv
    OUT.mkdir(parents=True, exist_ok=True)
    bad = 0
    for name in SHOTS:
        data = render(name)
        path = OUT / f"{name}.wav"
        if check:
            if not path.is_file() or path.read_bytes() != data:
                print(f"drift: {path.relative_to(ROOT)}")
                bad += 1
        else:
            path.write_bytes(data)
            print(f"wrote {path.relative_to(ROOT)} ({len(data)} bytes)")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
