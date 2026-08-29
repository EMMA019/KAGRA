"""2D UI パネル（kagra.ui2d）の純ロジックテスト。

kagra_shared が無い環境でも近似幅で動く（AGENTS.md: tests は拡張非依存）。
draw_world を通す検証だけ kagra_shared があれば実行する。
"""
from tests.conftest import ROOT, load_kagra_submodule

ui2d = load_kagra_submodule("ui2d")


def test_measure_positive():
    assert ui2d.measure("A", 14) > 0
    assert ui2d.measure("あ", 14) > 0


def test_wrap_text_preserves_content():
    text = "トルネコは 50G を手に入れた！ すごい！"
    lines = ui2d.wrap_text(text, 14, 120)
    assert "".join(lines) == text
    assert len(lines) >= 2, "120px 幅なら 14px テキストは複数行になる"
    for ln in lines:
        assert ui2d.measure(ln, 14) <= 120 + 1e-6


def test_wrap_text_explicit_newline():
    lines = ui2d.wrap_text("あ\nい", 14, 1000)
    assert lines == ["あ", "い"]


def test_panel_two_quads():
    p = ui2d.panel(10, 10, 100, 40)
    assert len(p["quads"]) == 2  # 枠 + 中身
    assert p["texts"] == []


def test_choice_menu_cursor_on_selected():
    m = ui2d.choice_menu(["はい", "いいえ", "やめる"], selected=1, x=10, y=10, w=120)
    texts = [t["text"] for t in m["texts"]]
    assert ">" in texts
    selected_opt = [t for t in m["texts"] if t["text"] == "いいえ"][0]
    assert selected_opt["color"] != [240, 235, 220, 255], "選択中はカーソル色"


def test_bar_clamps_ratio():
    b = ui2d.bar(0, 0, 100, 8, ratio=1.5)
    fill = [q for q in b["quads"] if q["w"] == 100.0]
    assert fill, "ratio 1.5 は 1.0 に clamp されて全幅"
    b2 = ui2d.bar(0, 0, 100, 8, ratio=-0.2)
    ws = sorted(q["w"] for q in b2["quads"])
    assert ws[0] < ws[-1], "負の ratio は最小幅に落ちる（back は全幅のまま）"


def test_merge_concatenates():
    m = ui2d.merge(ui2d.panel(0, 0, 10, 10), ui2d.list_lines(["a"], 0, 0))
    assert len(m["quads"]) == 2
    assert len(m["texts"]) == 1


def test_draw_world_with_ui2d_when_shared_installed():
    try:
        import kagra_shared  # noqa: F401
    except ImportError:
        return
    import json

    from kagra.gameloop import draw_world

    dump = json.loads(
        (ROOT / "kagra-shared/tests/fixtures/crest_isle_world.json").read_text(
            encoding="utf-8"
        )
    )
    hud = ui2d.merge(
        ui2d.message("トルネコは 50G を手に入れた！", 40, 100, 240),
        ui2d.choice_menu(["はい", "いいえ"], selected=0, x=40, y=70, w=240),
        ui2d.bar(40, 40, 120, 8, ratio=0.7, label="HP"),
    )
    png = draw_world(dump, 320, 180, hud=hud)
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    assert len(png) > 500
