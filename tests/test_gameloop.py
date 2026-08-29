"""Python ゲームマスターの最小テスト（shared wgpu 30 バインディング）。

`kagra_shared`（PyO3）が無い環境でも純ロジック（rgba_to_png / Scene）は
動く。draw_world は kagra_shared がある環境でのみ検証する。
"""
from tests.conftest import ROOT, load_kagra_submodule

gm = load_kagra_submodule("gameloop")


def test_rgba_to_png_signature():
    png = gm.rgba_to_png(bytes([255, 0, 0, 255] * 4), 2, 2)
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    assert len(png) > 30


def test_draw_world_when_shared_installed():
    try:
        import kagra_shared  # noqa: F401
    except ImportError:
        return  # kagra_shared 未ビルド環境はスキップ
    import json

    dump = json.loads(
        (ROOT / "kagra-shared/tests/fixtures/crest_isle_world.json").read_text(
            encoding="utf-8"
        )
    )
    png = gm.draw_world(dump, 32, 32)
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    assert len(png) > 500


def test_draw_world_hud_text_when_shared_installed():
    """HUD テキスト（日本語含む）を世界の上に重ねて描画できる。"""
    try:
        import kagra_shared  # noqa: F401
    except ImportError:
        return
    import json

    dump = json.loads(
        (ROOT / "kagra-shared/tests/fixtures/crest_isle_world.json").read_text(
            encoding="utf-8"
        )
    )
    hud = {
        "quads": [{"x": 4, "y": 4, "w": 24, "h": 10, "color": [30, 40, 30, 220]}],
        "texts": [
            {"text": "こんにちは KAGRA", "x": 16, "y": 6, "size": 6, "color": [255, 255, 255, 255]},
        ],
    }
    png = gm.draw_world(dump, 32, 32, hud=hud)
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    assert len(png) > 500


def test_scene_basics():
    calls = []

    class S(gm.Scene):
        def update(self, dt):
            calls.append(dt)
            self.quit()

    s = S()
    assert s.clock == 0.0  # clock は run() のループが進める
    s.update(1 / 60)
    assert calls == [1 / 60]
    assert not s.running
    assert s.clock == 0.0


def test_mouse_handlers_record_state():
    from types import SimpleNamespace

    gm._mouse["buttons"].clear()
    gm._mouse["just"].clear()
    gm._on_mouse_motion(SimpleNamespace(x=123, y=45))
    assert gm.mouse_pos() == (123, 45)
    assert not gm.mouse_down(1)
    gm._on_mouse_down(SimpleNamespace(num=1))
    assert gm.mouse_down(1)
    assert gm.mouse_clicked(1)
    gm._on_mouse_up(SimpleNamespace(num=1))
    assert not gm.mouse_down(1)
    assert not gm.mouse_down(3)
    gm._on_mouse_down(SimpleNamespace(num=3))
    assert gm.mouse_down(3)
