"""VRM chats through kairi (or a smoke stub).

Live: start https://github.com/EMMA019/kairi on :8000, then:
  python examples/vrm_kairi_chat.py

Smoke (no HTTP): KAGRA_SMOKE=1 python examples/vrm_kairi_chat.py
"""
from __future__ import annotations

import os
import sys

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(_HERE))

import kagra
from kagra.camera3d import Camera3D

SW, SH = 960, 540
SMOKE = os.environ.get("KAGRA_SMOKE") == "1"
SMOKE_FRAMES = int(os.environ.get("KAGRA_SMOKE_FRAMES", "40"))
SMOKE_SHOT = os.environ.get("KAGRA_SMOKE_OUT", "scratch/kairi_chat_smoke.png")


class KairiChat(kagra.Scene):
    def on_enter(self):
        kagra.font()
        kagra.apply_live_look()
        self.cam = Camera3D(SW, SH, fov_deg=32.0)
        self.cam.use_orbit(radius=2.6, target=(0, 0.9, 0))
        kagra.set_camera3d(self.cam)
        self.av = kagra.avatar(str(kagra.ensure_vrm()))
        self.av.enable_lipsync()
        self.av.play("idle", loop=True)
        self.line = kagra.Label("ENTER  to ask kairi", 18, 16, 18, (230, 220, 200))
        self.mind = None
        self._asked = False
        if SMOKE:
            self._say("smoke: kairi is HTTP, not in the wheel")
        else:
            self.mind = kagra.brain("kairi")

    def _say(self, text: str):
        self.line.text = text[:80]
        dur = min(8.0, 0.06 * max(8, len(text)))
        try:
            self.av.lipsync_text(text, duration=dur)
        except Exception:
            pass

    def update(self, dt):
        dt = min(dt, 0.05)
        if kagra.pressed("ESCAPE"):
            kagra.quit()
            return
        if SMOKE:
            n = kagra.tick_count()
            if n == 8 and not self._asked:
                self._asked = True
            if n == 24:
                kagra.screenshot(SMOKE_SHOT)
            if n >= SMOKE_FRAMES:
                kagra.quit()
                return
        elif kagra.pressed("RETURN") and self.mind is not None and not self._asked:
            self._asked = True
            self.line.text = "thinking…"
            try:
                reply = self.mind.ask("こんにちは。一文で自己紹介して。")
            except Exception as e:
                reply = f"（kairi に届かない）{e}"
            self._say(reply)
            self._asked = False
        self.av.update(dt)
        eng = kagra.get_engine()
        if eng:
            self.cam.update(eng)

    def draw(self):
        kagra.cls(12, 10, 28)
        kagra.draw_vrm(self.av.vrm_id)
        kagra.fill(0, 0, 520, 48, (8, 12, 24), 150)
        self.line.draw()


if __name__ == "__main__":
    os.makedirs(os.path.dirname(SMOKE_SHOT) or ".", exist_ok=True)
    kagra.init(
        width=SW,
        height=SH,
        title="KAGRA kairi chat",
        fps=60,
        visible=not SMOKE,
    )
    kagra.run(
        start_scene=KairiChat(),
        max_frames=SMOKE_FRAMES + 2 if SMOKE else None,
        fixed_dt=1.0 / 60.0 if SMOKE else None,
    )
