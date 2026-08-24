"""画面空間の Label / Button。2D ``Entity`` ではない。GPU は draw 時だけ。"""
from __future__ import annotations


class Label:
    """``kagra.text`` の持ち歩き。"""

    def __init__(
        self,
        text: str = "",
        x: float = 16.0,
        y: float = 16.0,
        size: int = 18,
        color=(230, 230, 220),
    ):
        self.text = str(text)
        self.x = float(x)
        self.y = float(y)
        self.size = int(size)
        self.color = color
        self.enabled = True

    def draw(self) -> None:
        if not self.enabled or not self.text:
            return
        try:
            import kagra
            kagra.text(self.text, int(self.x), int(self.y), int(self.size), self.color)
        except Exception:
            return


class Button:
    """``kagra.button`` の持ち歩き。``draw()`` がクリックなら True。"""

    def __init__(
        self,
        x: float,
        y: float,
        w: float,
        h: float,
        label: str = "",
        *,
        size: int = 18,
    ):
        self.x = float(x)
        self.y = float(y)
        self.w = float(w)
        self.h = float(h)
        self.label = str(label)
        self.size = int(size)
        self.enabled = True
        self.clicked = False

    def draw(self) -> bool:
        self.clicked = False
        if not self.enabled:
            return False
        try:
            import kagra
            hit = bool(kagra.button(
                int(self.x), int(self.y), int(self.w), int(self.h),
                self.label, size=self.size,
            ))
            self.clicked = hit
            return hit
        except Exception:
            return False
