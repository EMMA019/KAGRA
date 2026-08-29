"""2D UI パネル（Python のみ）。shared wgpu 30 の hud データに変換する。

ゲームロジックと同じく全部 Python。メッセージウィンドウ・選択肢・リスト・
バーを組み立て、`kagra.gameloop.draw_world(world, w, h, hud=...)` に渡す
dict（quads + texts）を作る。文字の折り返しは shared の埋め込み
PixelMplus の実測幅（kagra_shared.measure_text）で行う。kagra_shared が
無い環境では近似幅で動く（テスト / 純ロジック）。

使い方（トルネコのメッセージ + メニュー）::

    from kagra.ui2d import choice_menu, merge, message

    hud = merge(
        message("トルネコは 50G を手に入れた！", 40, 120, 240),
        choice_menu(["はい", "いいえ"], selected=0, x=40, y=90, w=240),
    )
    png = draw_world(world, 320, 180, hud=hud)
"""
from __future__ import annotations

from typing import Any

try:
    import kagra_shared as _ks
except ImportError:  # pragma: no cover - 未ビルド環境は近似幅
    _ks = None

__all__ = [
    "merge",
    "panel",
    "measure",
    "wrap_text",
    "message",
    "choice_menu",
    "bar",
    "list_lines",
    "fit_lines",
    "page_count",
    "clamp_scroll",
    "scroll_window",
    "paged_menu",
]

_WHITE = [255, 255, 255, 255]
_TEXT = [240, 235, 220, 255]


def measure(text: str, size: float) -> float:
    """行幅（ピクセル）。kagra_shared があれば実測、無ければ近似。"""
    if _ks is not None:
        return float(_ks.measure_text(text, float(size)))
    # 近似: CJK（広義の全角）は 1em、ASCII は 0.5em。
    w = 0.0
    for ch in text:
        o = ord(ch)
        full = o > 0x2E7F  # CJK 記号以降は全角扱い
        w += float(size) if full else float(size) * 0.5
    return w


def merge(*parts: dict[str, Any]) -> dict[str, Any]:
    """複数の UI 部品を 1 つの hud dict に合成する。"""
    out: dict[str, Any] = {"quads": [], "texts": []}
    for p in parts:
        if not p:
            continue
        out["quads"].extend(p.get("quads", []))
        out["texts"].extend(p.get("texts", []))
    return out


def panel(
    x: float,
    y: float,
    w: float,
    h: float,
    color=(16, 20, 16, 230),
    border=(100, 110, 100, 255),
    border_w: float = 2.0,
) -> dict[str, Any]:
    """背景 + 枠の四角。"""
    return {
        "quads": [
            {"x": x, "y": y, "w": w, "h": h, "color": list(border)},
            {
                "x": x + border_w,
                "y": y + border_w,
                "w": max(1.0, w - 2 * border_w),
                "h": max(1.0, h - 2 * border_w),
                "color": list(color),
            },
        ],
        "texts": [],
    }


def wrap_text(text: str, size: float, max_w: float) -> list[str]:
    """実測幅で折り返す。``\\n`` は明示改行。"""
    if max_w <= 0:
        return text.split("\n")
    lines: list[str] = []
    for raw in text.split("\n"):
        line = ""
        for ch in raw:
            trial = line + ch
            if measure(trial, size) <= max_w or not line:
                line = trial
            else:
                lines.append(line)
                line = ch
        lines.append(line)
    return lines


def _text_quads(lines, x, y, size, color, align="left") -> list[dict]:
    """複数行テキスト → hud texts（行送りは size * 1.4）。"""
    texts = []
    for i, ln in enumerate(lines):
        texts.append(
            {
                "text": ln,
                "x": x,
                "y": y + i * size * 1.4,
                "size": size,
                "color": list(color),
                "align": align,
            }
        )
    return texts


def message(
    text: str,
    x: float,
    y: float,
    w: float,
    *,
    size: float = 14.0,
    color=_TEXT,
    title: str | None = None,
) -> dict[str, Any]:
    """メッセージウィンドウ。幅に合わせて折り返し、高さは自動。"""
    pad = 8.0
    lines = wrap_text(text, size, w - pad * 2)
    h = size * 1.4 * len(lines) + pad * 2
    parts = [panel(x, y, w, h)]
    if title:
        parts.append(
            {"texts": _text_quads([title], x + pad, y + pad - size * 1.2, size * 0.8, [200, 200, 190, 255])}
        )
    parts.append({"texts": _text_quads(lines, x + pad, y + pad, size, color)})
    return merge(*parts)


def choice_menu(
    options: list[str],
    selected: int,
    x: float,
    y: float,
    w: float,
    *,
    size: float = 14.0,
    color=_TEXT,
    cursor: str = ">",
    cursor_color=(255, 220, 90, 255),
) -> dict[str, Any]:
    """選択肢メニュー。`selected` の行にカーソルを出す。"""
    pad = 8.0
    line_h = size * 1.4
    h = line_h * len(options) + pad * 2
    parts = [panel(x, y, w, h)]
    texts = []
    for i, opt in enumerate(options):
        ty = y + pad + i * line_h
        if i == selected:
            texts.append(
                {
                    "text": cursor,
                    "x": x + pad,
                    "y": ty,
                    "size": size,
                    "color": list(cursor_color),
                }
            )
            texts.append(
                {"text": opt, "x": x + pad + size * 0.9, "y": ty, "size": size, "color": list(cursor_color)}
            )
        else:
            texts.append({"text": opt, "x": x + pad + size * 0.9, "y": ty, "size": size, "color": list(color)})
    return merge(parts[0], {"texts": texts})


def bar(
    x: float,
    y: float,
    w: float,
    h: float,
    ratio: float,
    color=(240, 150, 90, 255),
    back=(34, 38, 34, 200),
    label: str | None = None,
    *,
    size: float = 10.0,
) -> dict[str, Any]:
    """プログレスバー。`ratio` は 0..1（clamp）。ラベルはバー左。"""
    r = max(0.0, min(1.0, ratio))
    parts = [
        {"quads": [{"x": x, "y": y, "w": w, "h": h, "color": list(back)}]},
        {
            "quads": [
                {"x": x, "y": y, "w": max(1.0, w * r), "h": h, "color": list(color)}
            ]
        },
    ]
    if label:
        parts.append({"texts": _text_quads([label], x, y - size * 1.2, size, _TEXT)})
    return merge(*parts)


def list_lines(
    items: list[str],
    x: float,
    y: float,
    *,
    size: float = 12.0,
    color=_TEXT,
    line_h: float | None = None,
) -> dict[str, Any]:
    """在庫・ステータスなどの単純リスト（パネルなし）。"""
    lh = line_h if line_h is not None else size * 1.4
    texts = [
        {"text": it, "x": x, "y": y + i * lh, "size": size, "color": list(color)}
        for i, it in enumerate(items)
    ]
    return {"quads": [], "texts": texts}


# ── Phase 5: スクロール / ページ送り ────────────────────────────────────

def fit_lines(text: str, size: float, max_w: float) -> list[str]:
    """``wrap_text`` と同じ折り返し。別名として純ロジックテストから使う。"""
    return wrap_text(text, size, max_w)


def page_count(n_items: int, per_page: int) -> int:
    """``n_items`` を 1 ページ ``per_page`` 件で分割したページ数。"""
    if per_page <= 0:
        return 1
    return max(1, (n_items + per_page - 1) // per_page)


def clamp_scroll(offset: int, n_lines: int, visible: int) -> int:
    """スクロールオフセットを有効範囲に収める（0..max）。

    ``visible`` 行しか見えない窓で ``n_lines`` 行のテキストを表示するとき、
    offset は ``max(0, n_lines - visible)`` まで。1 ページに収まれば 0。
    """
    if visible <= 0:
        return 0
    return max(0, min(offset, max(0, n_lines - visible)))


def scroll_window(
    lines: list[str],
    offset: int,
    visible: int,
    *,
    size: float = 12.0,
    color=_TEXT,
    line_h: float | None = None,
) -> dict[str, Any]:
    """末尾側が「今の行」に見えるスクロール窓（ログ表示）。

    純ロジック: ``clamp_scroll`` で offset を補正し、``visible`` 行だけ
    hud texts にする。offset を進めると古い行が上に流れる。
    """
    off = clamp_scroll(offset, len(lines), visible)
    shown = lines[off : off + visible]
    lh = line_h if line_h is not None else size * 1.4
    texts = [
        {"text": ln, "x": 0.0, "y": i * lh, "size": size, "color": list(color)}
        for i, ln in enumerate(shown)
    ]
    return {"quads": [], "texts": texts, "_offset": off}


def paged_menu(
    options: list[str],
    selected: int,
    x: float,
    y: float,
    w: float,
    *,
    per_page: int = 6,
    size: float = 14.0,
    color=_TEXT,
    cursor: str = ">",
    cursor_color=(255, 220, 90, 255),
) -> dict[str, Any]:
    """ページ送り付き選択肢メニュー。

    ``selected`` が ``per_page`` を超えると自動でページを切り替え、
    右上に「n/N」を出す（SLG のユニット一覧など長いメニュー用）。
    返り値の ``_page`` / ``_pages`` は選択がどのページか。
    """
    pages = page_count(len(options), per_page)
    page = min(selected // per_page, pages - 1) if options else 0
    start = page * per_page
    page_opts = options[start : start + per_page]
    sel_on_page = selected - start if start <= selected < start + per_page else -1
    pad = 8.0
    line_h = size * 1.4
    h = line_h * len(page_opts) + pad * 2
    parts = [panel(x, y, w, h)]
    texts = []
    if pages > 1:
        texts.append(
            {
                "text": f"{page + 1}/{pages}",
                "x": x + w - pad - size * 1.2,
                "y": y + pad * 0.4,
                "size": size * 0.8,
                "color": [180, 180, 170, 255],
                "align": "right",
            }
        )
    for i, opt in enumerate(page_opts):
        ty = y + pad + i * line_h
        is_sel = i == sel_on_page
        texts.append(
            {
                "text": cursor if is_sel else " ",
                "x": x + pad,
                "y": ty,
                "size": size,
                "color": list(cursor_color) if is_sel else list(color),
            }
        )
        texts.append(
            {
                "text": opt,
                "x": x + pad + size * 0.9,
                "y": ty,
                "size": size,
                "color": list(cursor_color) if is_sel else list(color),
            }
        )
    out = merge(parts[0], {"texts": texts})
    out["_page"] = page
    out["_pages"] = pages
    return out
