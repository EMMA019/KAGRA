"""線分の矩形近似（kagra.line の NameError 再発防止）。"""
from tests.conftest import load_kagra_submodule


def test_line_rects_diagonal():
    geom = load_kagra_submodule("geom2d")
    rects = geom.line_rects(0, 0, 100, 50, width=2)
    assert rects
    xs = [r[0] for r in rects]
    ys = [r[1] + r[3] / 2 for r in rects]  # 中心 y
    assert xs[0] == 0
    assert abs(ys[0] - 0) < 1e-6
    assert xs[-1] > 50
    assert ys[-1] > 20


def test_line_rects_short_is_empty():
    geom = load_kagra_submodule("geom2d")
    assert geom.line_rects(0, 0, 0.1, 0.1) == []
