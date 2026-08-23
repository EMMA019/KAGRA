"""kairi を頭脳に、VRM が答えて喋る（AI VTuber の骨格）。

    1. 頭脳: https://github.com/EMMA019/kairi
       docker compose up --build   # KAIRI_DEMO=1 ならキー無しで動く
    2. 声（任意）: VOICEVOX を起動 → 返答に声とリップシンクが付く
    3. python examples/vrm_kairi_chat.py

キーボードで話しかける。ENTER で送信、ESC で終了。
`kagra-chat.jsonl` に {"user","text"} を追記すれば視聴者コメントとしても届く
（echo '{"user":"alice","text":"こんにちは"}' >> kagra-chat.jsonl）。
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import kagra
from kagra.ai_character import AiCharacter
from kagra.brain import BrainError, KairiBrain
from kagra.camera3d import Camera3D
from kagra.stream import ChatInbox, StreamHud

KAIRI_URL = os.environ.get("KAIRI_URL", "http://127.0.0.1:8000")

kagra.init(title="KAGRA × kairi", width=1280, height=720)
cam = Camera3D(1280, 720, fov_deg=30.0)
cam.use_orbit(radius=2.3, phi=0.12, target=(0, 1.05, 0))

state = {"char": None, "hud": None, "inbox": None, "line": ""}


def ready():
    kagra.font()
    kagra.apply_live_look()
    kagra.set_camera3d(cam)
    char = AiCharacter(str(kagra.ensure_vrm()), tts="voicevox")
    brain = KairiBrain(KAIRI_URL)

    def think(text: str) -> str:
        try:
            return brain.ask(text)
        except BrainError as e:
            print(f"[kairi] {e}", file=sys.stderr)
            return "頭脳サーバーに繋がりません。kairi を起動してください。"

    char.set_llm_func(think)
    hud = StreamHud(song=f"kairi @ {KAIRI_URL}", credit="Alicia Solid © Dwango")
    state.update(char=char, hud=hud, inbox=ChatInbox("kagra-chat.jsonl"))
    print(f"[kagra] brain={KAIRI_URL}  ENTER=send  ESC=quit")


def update(dt):
    if kagra.pressed("ESCAPE"):
        kagra.quit()
        return
    char, hud, inbox = state["char"], state["hud"], state["inbox"]
    if char is None:
        return
    state["line"] += kagra.get_typed_chars()
    if kagra.backspace_pressed():
        state["line"] = state["line"][:-1]
    if kagra.enter_pressed() and state["line"].strip():
        text = state["line"].strip()
        state["line"] = ""
        hud.push_chat(text, user="you")
        char.chat(text)
    for msg in inbox.poll():
        hud.push_chat(msg)
        char.chat(f"{msg.user}: {msg.text}")
    hud.subtitle = char.last_char_text[:80]
    char.update(dt)
    cam.update(kagra.get_engine())


def draw():
    kagra.cls(8, 6, 18)
    char = state["char"]
    if char is not None:
        kagra.draw_vrm(char.avatar.vrm_id)
    kagra.draw_vignette()
    hud = state["hud"]
    if hud is not None:
        hud.draw()
    kagra.text(f"> {state['line']}", 16, 690, 16, (200, 220, 200))


kagra.run(update, draw, on_ready=ready)
