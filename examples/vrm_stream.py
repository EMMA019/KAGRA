"""配信の最小例。OBS は仮想カメラか窓キャプチャ。

    pip install "kagra[stream]"
    python examples/vrm_stream.py

チャットは JSONL（YouTube API は外）。VOICEVOX は別起動してから
``av.speak_voicevox("こんにちは")``。
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import kagra
from kagra.camera3d import Camera3D
from kagra.stream import ChatInbox, StreamHud, VirtualCam

kagra.init(title="KAGRA — stream", width=1280, height=720)
cam = Camera3D()
cam.use_orbit(radius=2.8, target=(0, 0.9, 0))
state = {"av": None, "hud": None, "inbox": None, "vcam": None}


def ready():
    av = kagra.avatar(str(kagra.ensure_vrm()))
    av.dance()
    av.sing()
    hud = StreamHud(song="♪ KAGRA", credit="Alicia Solid © Dwango")
    hud.subtitle = "chat → kagra-chat.jsonl"
    state["av"] = av
    state["hud"] = hud
    state["inbox"] = ChatInbox("kagra-chat.jsonl")
    try:
        state["vcam"] = VirtualCam(fps=30).start(1280, 720)
        print("[example] virtual cam on")
    except Exception as e:
        print(f"[example] virtual cam skipped: {e}")


def update(dt):
    if kagra.pressed("ESCAPE"):
        kagra.quit()
        return
    av = state["av"]
    if av is None:
        return
    if state["vcam"] is not None:
        state["vcam"].send()
    for msg in state["inbox"].poll():
        state["hud"].push_chat(msg)
        state["hud"].subtitle = msg.text
    av.update(dt)
    cam.orbit_by(dt * 0.2, 0)
    cam.update(kagra.get_engine())


def draw():
    kagra.cls(16, 12, 32)
    av = state["av"]
    if av is not None:
        kagra.draw_vrm(av.vrm_id)
    if state["hud"] is not None:
        state["hud"].draw()


kagra.run(update, draw, on_ready=ready)
