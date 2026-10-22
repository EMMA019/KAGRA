"""rhythm 最小例 - 4 レーンの短いチャート。判定は Rust (`RhythmSession`).

120 BPM。perfect / good / miss、コンボ、スコア、リザルト。
キーは D F J K。画面を 4 列に割ったタップでも同じレーン。
タイトルとリザルトは Space / Enter / タップ。

曲は `assets/cute_song_trial.wav` があればそれを再生する。
チャートの時計は Rust の固定 BPM（無音でも同じ）。

    python examples/rhythm_minimal.py
    python examples/rhythm_minimal.py --headless scratch/rhythm_play.png

HUD は ASCII のみ（ハイフン。em dash は使わない）。
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.contracts import AssetKind, resolve_asset  # noqa: E402
from kagra.gameloop import (  # noqa: E402
    Scene,
    draw_world,
    mouse_clicked,
    mouse_pos,
    run,
    was_pressed,
)

DUMP = Path(__file__).resolve().parents[1] / "kagra-shared/tests/fixtures/rhythm_world.json"
KEYS = ("d", "f", "j", "k")
LABELS = ("D", "F", "J", "K")


def _load_session():
    text = DUMP.read_text(encoding="utf-8")
    try:
        from kagra.kagra_shared import RhythmSession
    except ImportError:
        from kagra_shared import RhythmSession  # type: ignore[no-redef]
    return RhythmSession.from_json(text)


def _song_bytes() -> bytes | None:
    path = resolve_asset(AssetKind.AUDIO, "cute_song_trial", required=False)
    if path is None or not path.is_file():
        return None
    return path.read_bytes()


def _judge_label(name: str) -> str:
    return {"perfect": "PERFECT", "good": "GOOD", "miss": "MISS"}.get(name, "")


class RhythmPlay(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.session = _load_session()
        self.world = json.loads(self.session.dump())
        self._music = False

    def update(self, dt: float) -> None:
        if was_pressed("escape"):
            self.quit()
            return
        session = self.session
        if session.phase != "playing":
            if was_pressed("enter") or was_pressed("space") or mouse_clicked():
                session.confirm()
        else:
            for lane, key in enumerate(KEYS):
                if was_pressed(key):
                    session.press_lane(lane)
            if mouse_clicked():
                x, y = mouse_pos()
                session.pointer(float(x), float(y), float(self.width), float(self.height))
        session.set_input(0.0, 0.0, False, False, False)
        session.tick(dt)
        self._maybe_music()
        self.world = json.loads(session.dump())

    def _maybe_music(self) -> None:
        if self._music or self.session.phase != "playing":
            return
        self._music = True
        wav = _song_bytes()
        if wav is None:
            return
        from kagra.audio import play_wav

        play_wav(wav)

    def _status(self) -> str:
        session = self.session
        if session.phase == "title":
            return "rhythm - title   Space or tap   keys D F J K"
        if session.phase == "complete":
            return (
                f"RESULT  {session.rank}   score {session.score}   "
                f"perfect {session.perfect_count}   good {session.good_count}   "
                f"miss {session.miss_count}   combo {session.max_combo}"
            )
        label = _judge_label(session.last_judge)
        extra = f"   {label}" if label else ""
        return f"combo {session.combo}   score {session.score}{extra}"

    def draw(self) -> None:
        w = self.width
        h = self.height
        quads = [
            {"x": 8, "y": 8, "w": w - 16, "h": 36, "color": [12, 10, 22, 220]},
        ]
        texts = [
            {
                "text": self._status(),
                "x": w * 0.5,
                "y": 26,
                "size": 15,
                "color": [240, 236, 248, 255],
                "align": "center",
            }
        ]
        lane_w = w / 4.0
        colors = (
            [214, 72, 78, 70],
            [64, 136, 214, 70],
            [236, 196, 72, 70],
            [72, 186, 104, 70],
        )
        for i, label in enumerate(LABELS):
            x = int(i * lane_w)
            quads.append(
                {
                    "x": x + 6,
                    "y": h - 64,
                    "w": int(lane_w) - 12,
                    "h": 52,
                    "color": colors[i],
                }
            )
            texts.append(
                {
                    "text": label,
                    "x": x + lane_w * 0.5,
                    "y": h - 46,
                    "size": 18,
                    "color": [255, 255, 255, 255],
                    "align": "center",
                }
            )
        hud = {"quads": quads, "texts": texts}
        self._canvas_png = draw_world(self.world, w, h, hud=hud)


def _play_until(scene: RhythmPlay, seconds: float) -> None:
    frames = max(1, int(seconds * 60.0))
    for _ in range(frames):
        scene.session.tick(1.0 / 60.0)
    scene.world = json.loads(scene.session.dump())


def main() -> None:
    headless = "--headless" in sys.argv
    out = None
    if headless:
        idx = sys.argv.index("--headless")
        out = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else "scratch/rhythm_play.png"
        Path(out).parent.mkdir(parents=True, exist_ok=True)
    scene = RhythmPlay()
    if headless:
        scene.width = 960
        scene.height = 540
        scene.session.confirm()
        scene.session.set_auto(True)
        _play_until(scene, 3.2)
        scene.draw()
        Path(out).write_bytes(scene._canvas_png or b"")
        guard = 0
        while scene.session.phase != "complete" and guard < 900:
            guard += 1
            scene.session.tick(1.0 / 60.0)
        scene.world = json.loads(scene.session.dump())
        result = str(Path(out).with_name(Path(out).stem + "_result.png"))
        scene.draw()
        Path(result).write_bytes(scene._canvas_png or b"")
        print(
            f"wrote {out} and {result} phase={scene.session.phase} "
            f"rank={scene.session.rank} score={scene.session.score} "
            f"perfect={scene.session.perfect_count} good={scene.session.good_count} "
            f"miss={scene.session.miss_count} combo={scene.session.max_combo}"
        )
        return
    run(scene, width=960, height=540, title="rhythm")


if __name__ == "__main__":
    main()
