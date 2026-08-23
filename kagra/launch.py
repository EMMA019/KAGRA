"""First-run friction. Pure Python — no kagra_core.

The checkout folder named ``kagra/`` shadows an installed wheel when
``python -m kagra`` is run from the repo root. Contributors with
``maturin develop`` are fine; everyone else gets a missing ``kagra_core``.
"""
from __future__ import annotations

import os
import sys
from pathlib import Path


def checkout_kagra_dir(cwd: Path | None = None) -> Path | None:
    """``<cwd>/kagra/__init__.py`` があればそのディレクトリ。"""
    root = Path(cwd) if cwd is not None else Path.cwd()
    init = root / "kagra" / "__init__.py"
    return init.parent if init.is_file() else None


def extension_present(package_dir: Path) -> bool:
    """maturin が置いた ``kagra_core`` 拡張があるか。"""
    names = {p.name for p in package_dir.iterdir()} if package_dir.is_dir() else set()
    if "kagra_core.py" in names or "kagra_core.pyi" in names:
        return True
    for name in names:
        if name.startswith("kagra_core.") and name.endswith(
            (".so", ".pyd", ".dylib", ".dll")
        ):
            return True
        # maturin: kagra_core.cpython-312-x86_64-linux-gnu.so
        if name.startswith("kagra_core."):
            return True
    return False


def shadow_hint(
    *,
    cwd: Path | None = None,
    loaded_from: Path | None = None,
    has_core: bool | None = None,
) -> str | None:
    """警告文。問題なければ None。"""
    local = checkout_kagra_dir(cwd)
    if local is None:
        return None
    here = Path(loaded_from).resolve() if loaded_from is not None else local.resolve()
    if here != local.resolve():
        return None
    if has_core is None:
        has_core = extension_present(local)
    if has_core:
        return None
    if os.name == "nt":
        leave = "cd %TEMP%"
    else:
        leave = "cd /tmp"
    return (
        "このフォルダの kagra/ が pip の kagra を隠しています。\n"
        f"  wheel を使う:     {leave}   そのあと  python -m kagra\n"
        "  ソースを使う:     pip install maturin && maturin develop"
    )


def format_core_import_error(
    exc: BaseException,
    *,
    cwd: Path | None = None,
    loaded_from: Path | None = None,
) -> str:
    hint = shadow_hint(cwd=cwd, loaded_from=loaded_from, has_core=False)
    base = (
        "kagra_core が見つかりません。"
        " pip なら `pip install -U kagra`、ソースなら `maturin develop`。"
    )
    if hint:
        return f"{base}\n{hint}\n{exc}"
    return f"{base}\n{exc}"


def warn_checkout_shadow(*, file: object = None) -> None:
    """``python -m kagra`` の入口で呼ぶ。"""
    out = file if file is not None else sys.stderr
    loaded = Path(__file__).resolve().parent
    msg = shadow_hint(loaded_from=loaded)
    if msg:
        print(f"[kagra] {msg}", file=out)
