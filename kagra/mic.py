"""マイク振幅 → ``avatar.lipsync_amplitude``。``pip install "kagra[mic]"``。

本体は sounddevice。コアに入れない。
"""
from __future__ import annotations

import math
def amplitude_from_samples(samples, *, gain: float = 8.0) -> float:
    """PCM サンプル（-1..1）から 0..1 の口の開き。"""
    if not samples:
        return 0.0
    acc = 0.0
    n = 0
    for s in samples:
        acc += float(s) * float(s)
        n += 1
    if n <= 0:
        return 0.0
    rms = math.sqrt(acc / n)
    return max(0.0, min(1.0, rms * float(gain)))


class MicLipsync:
    """バックグラウンドで RMS を更新する。``update()`` で avatar に渡す。"""

    def __init__(self, *, samplerate: int = 16000, blocksize: int = 512, gain: float = 8.0):
        self.samplerate = int(samplerate)
        self.blocksize = int(blocksize)
        self.gain = float(gain)
        self._amp = 0.0
        self._stream = None

    @property
    def amplitude(self) -> float:
        return self._amp

    def start(self):
        try:
            import sounddevice as sd
        except ImportError as e:
            raise ImportError(
                'マイクには pip install "kagra[mic]" が必要です（sounddevice）。'
            ) from e

        def _cb(indata, frames, time, status):
            if status:
                return
            flat = indata.reshape(-1)
            self._amp = amplitude_from_samples(flat, gain=self.gain)

        self._stream = sd.InputStream(
            channels=1,
            samplerate=self.samplerate,
            blocksize=self.blocksize,
            callback=_cb,
        )
        self._stream.start()
        return self

    def apply(self, avatar, vowel: str = "aa"):
        if avatar is None:
            return self._amp
        if getattr(avatar, "lipsync", None) is None and hasattr(avatar, "enable_lipsync"):
            avatar.enable_lipsync()
        if hasattr(avatar, "lipsync_amplitude"):
            avatar.lipsync_amplitude(self._amp, vowel)
        return self._amp

    def close(self):
        stream = self._stream
        self._stream = None
        self._amp = 0.0
        if stream is not None:
            stream.stop()
            stream.close()
