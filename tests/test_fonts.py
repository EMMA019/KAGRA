"""システムフォント検出。"""
from tests.conftest import load_kagra_submodule


def test_find_system_font_does_not_raise():
    fonts = load_kagra_submodule("fonts")
    path = fonts.find_system_font()
    assert path is None or path.endswith((".ttf", ".ttc", ".otf"))
