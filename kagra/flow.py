"""プラットフォームごとのセーブ先。

本体の読み書きは Rust（``GameFlow``）。ここは同じ規則を拡張無しのテストと、
シェルがディレクトリを渡す前の案内に使う。

- Windows: ``%APPDATA%/kagra``
- Android: シェルが内部ストレージ（``filesDir/kagra``）を ``set_save_dir`` で渡す。
  渡す前の仮パスは ``kagra-save``
- その他: ``$XDG_DATA_HOME/kagra``、無ければ ``~/.local/share/kagra``

``KAGRA_SAVE_DIR`` があればそれを優先する。
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

__all__ = ["save_dir"]


def save_dir() -> Path:
    override = os.environ.get("KAGRA_SAVE_DIR", "").strip()
    if override:
        return Path(override)
    if sys.platform == "win32":
        app = os.environ.get("APPDATA", "").strip()
        if app:
            return Path(app) / "kagra"
        return Path("kagra-save")
    # Android の Python は稀。ネイティブは filesDir を渡す。
    if hasattr(sys, "getandroidapilevel"):
        return Path("kagra-save")
    xdg = os.environ.get("XDG_DATA_HOME", "").strip()
    if xdg:
        return Path(xdg) / "kagra"
    home = os.environ.get("HOME", "").strip()
    if home:
        return Path(home) / ".local" / "share" / "kagra"
    return Path("kagra-save")
