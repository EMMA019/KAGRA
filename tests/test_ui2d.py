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


# ── Phase 5: スクロール / ページ送り ────────────────────────────────────

def test_page_count():
    assert ui2d.page_count(0, 6) == 1
    assert ui2d.page_count(6, 6) == 1
    assert ui2d.page_count(7, 6) == 2
    assert ui2d.page_count(13, 6) == 3


def test_clamp_scroll_bounds():
    assert ui2d.clamp_scroll(0, 3, 10) == 0, "1 ページに収まるなら 0"
    assert ui2d.clamp_scroll(0, 10, 3) == 0
    assert ui2d.clamp_scroll(8, 10, 3) == 7, "max = n - visible"
    assert ui2d.clamp_scroll(-5, 10, 3) == 0, "負は 0 に"
    assert ui2d.clamp_scroll(99, 10, 3) == 7, "大きすぎは max に"


def test_scroll_window_shows_tail_slice():
    lines = [f"L{i}" for i in range(5)]
    w = ui2d.scroll_window(lines, offset=99, visible=3)
    assert w["_offset"] == 2, "末尾に合わせてクランプ"
    shown = [t["text"] for t in w["texts"]]
    assert shown == ["L2", "L3", "L4"], "末尾 3 行が見える"
    w2 = ui2d.scroll_window(lines, offset=0, visible=3)
    assert w2["_offset"] == 0
    assert [t["text"] for t in w2["texts"]] == ["L0", "L1", "L2"]


def test_scroll_window_all_fit():
    lines = [f"L{i}" for i in range(3)]
    w = ui2d.scroll_window(lines, offset=0, visible=10)
    assert w["_offset"] == 0
    assert len(w["texts"]) == 3


def test_paged_menu_single_page_no_page_label():
    m = ui2d.paged_menu(["a", "b"], selected=0, x=0, y=0, w=100, per_page=6)
    assert m["_pages"] == 1 and m["_page"] == 0
    assert not any(t["text"] == "1/1" for t in m["texts"]), "1 ページなら n/N を出さない"


def test_paged_menu_flips_page_with_selection():
    opts = [f"item{i}" for i in range(10)]
    m = ui2d.paged_menu(opts, selected=7, x=0, y=0, w=120, per_page=6)
    assert m["_pages"] == 2 and m["_page"] == 1, "selected=7 は 2 ページ目"
    texts = [t["text"] for t in m["texts"]]
    assert "2/2" in texts
    assert "item7" in texts
    assert "item0" not in texts, "1 ページ目の項目は出ない"
    # カーソルはページ内相対位置（7 - 6 = 1 行目）
    cursor_rows = [t["y"] for t in m["texts"] if t["text"] == ">"]
    item7_row = [t["y"] for t in m["texts"] if t["text"] == "item7"]
    assert cursor_rows and item7_row and cursor_rows[0] == item7_row[0]


def test_paged_menu_last_page_when_selected_beyond():
    opts = [f"item{i}" for i in range(10)]
    m = ui2d.paged_menu(opts, selected=99, x=0, y=0, w=120, per_page=6)
    assert m["_page"] == 1, "範囲外は最終ページへクランプ"
    assert "item9" in [t["text"] for t in m["texts"]]


def test_paged_menu_empty():
    m = ui2d.paged_menu([], selected=0, x=0, y=0, w=100, per_page=6)
    assert m["_pages"] == 1 and m["_page"] == 0
    assert m["texts"] == []
