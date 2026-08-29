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
