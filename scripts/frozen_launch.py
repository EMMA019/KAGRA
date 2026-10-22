"""PyInstaller の入口。ステージした例を ``__main__`` として起動する。

データファイルは PyInstaller 6 の ``_internal``（``sys._MEIPASS``）に入る。
例の ``Path(__file__).parents[1]`` がそのディレクトリになるので、
``kagra-shared/tests/fixtures`` と ``assets/`` の相対パスがそのまま使える。
"""
from __future__ import annotations

import json
import os
import runpy
import sys
from pathlib import Path


def bundle_root() -> Path:
    if getattr(sys, "frozen", False):
        meipass = getattr(sys, "_MEIPASS", None)
        if meipass:
            return Path(meipass)
        return Path(sys.executable).resolve().parent / "_internal"
    root = os.environ.get("KAGRA_GAME_ROOT", "").strip()
    if not root:
        raise SystemExit("KAGRA_GAME_ROOT is not set (this launcher is not frozen)")
    return Path(root)


def main() -> None:
    root = bundle_root()
    os.environ["KAGRA_ROOT"] = str(root)
    os.chdir(root)
    if str(root) not in sys.path:
        sys.path.insert(0, str(root))
    manifest_path = root / "kagra_game.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    entry = root / str(manifest["entry"])
    if not entry.is_file():
        raise SystemExit(f"game entry not found: {entry}")
    runpy.run_path(str(entry), run_name="__main__")


def _pyinstaller_sees_these() -> None:
    """呼ばない。解析器に実行時依存を見せるためだけの import。"""
    import tkinter  # noqa: F401
    import kagra  # noqa: F401
    import kagra.gameloop  # noqa: F401
    import kagra.kagra_shared  # noqa: F401


if __name__ == "__main__":
    main()
