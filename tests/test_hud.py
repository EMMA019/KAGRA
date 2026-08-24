"""HUD 部品は状態だけ。GPU 無しでも生成できる。"""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

hud = load_kagra_submodule("hud")


def test_label_and_button_store_layout():
    lab = hud.Label("hi", 10, 20, size=16)
    assert lab.text == "hi"
    assert lab.x == 10
    btn = hud.Button(0, 0, 80, 28, "ok")
    assert btn.w == 80
    assert btn.clicked is False
    assert btn.draw() is False
