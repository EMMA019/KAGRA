"""システムフォント検出（外部依存なし）。"""
from __future__ import annotations

import os
import platform


def find_system_font(prefer: str = "meiryo") -> str | None:
    """システムフォントを自動検出する（クロスプラットフォーム）。

    Args:
        prefer: 優先したいフォント名のヒント（ファイル名に含まれるかで判定）。
    """
    system = platform.system()
    prefer_l = prefer.lower()
    if system == "Windows":
        dirs = [os.path.join(os.environ.get("WINDIR", "C:/Windows"), "Fonts")]
        candidates = [
            "meiryo.ttc", "meiryob.ttc", "msgothic.ttc",
            "yugothic.ttf", "msmincho.ttc", "arial.ttf",
        ]
    elif system == "Darwin":
        dirs = ["/System/Library/Fonts", "/Library/Fonts",
                "/System/Library/Fonts/Supplemental"]
        candidates = [
            "ヒラギノ角ゴシック W3.ttc", "HiraginoSans-W3.ttc",
            "AppleSDGothicNeo.ttc", "Arial.ttf",
        ]
    else:
        dirs = ["/usr/share/fonts", "/usr/local/share/fonts"]
        candidates = [
            "NotoSansCJK-Regular.ttc", "NotoSansJP-Regular.otf",
            "LiberationSans-Regular.ttf", "DroidSansFallback.ttf",
            "DejaVuSans.ttf", "FreeSans.ttf",
        ]

    # ヒントに合うファイルを先に試す
    hinted = [c for c in candidates if prefer_l in c.lower()]
    ordered = hinted + [c for c in candidates if c not in hinted]

    for d in dirs:
        if not os.path.isdir(d):
            continue
        for c in ordered:
            p = os.path.join(d, c)
            if os.path.exists(p):
                return p
    for d in dirs:
        if not os.path.isdir(d):
            continue
        for root, _, files in os.walk(d):
            for f in files:
                if not f.lower().endswith((".ttf", ".ttc", ".otf")):
                    continue
                low = f.lower()
                if prefer_l and prefer_l in low:
                    return os.path.join(root, f)
                if any(k in low for k in ["meiryo", "gothic", "noto", "arial",
                                          "liberation", "dejavu", "freesans", "hiragino"]):
                    return os.path.join(root, f)
    return None
