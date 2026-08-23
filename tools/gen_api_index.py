#!/usr/bin/env python3
"""kagra 公開 API 索引を自動生成する。

Usage:
    python tools/gen_api_index.py
    python tools/gen_api_index.py --check   # CI: 差分があれば非ゼロ終了

出力: docs/API_INDEX.md

シグネチャは AST のみから作る。Rust 拡張の有無で出力が変わると `--check` が
機能しなくなるため、実行時 import には依存しない。
"""
from __future__ import annotations

import argparse
import ast
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs" / "API_INDEX.md"
KAGRA_PKG = ROOT / "kagra"


def _sig_from_ast(fn: ast.FunctionDef | ast.AsyncFunctionDef) -> str:
    """ソース AST から簡易シグネチャ文字列を作る。"""
    args = fn.args
    parts: list[str] = []

    def fmt_arg(a: ast.arg, default=None):
        name = a.arg
        ann = ""
        if a.annotation is not None:
            try:
                ann = ": " + ast.unparse(a.annotation)
            except Exception:
                ann = ""
        if default is not None:
            # PEP 8: 注釈付きは ` = `、無注釈は `=`
            sep = " = " if ann else "="
            try:
                d = sep + ast.unparse(default)
            except Exception:
                d = sep + "..."
            return f"{name}{ann}{d}"
        return f"{name}{ann}"

    pos = list(args.posonlyargs) + list(args.args)
    defaults = list(args.defaults)
    # defaults は末尾に対応
    nd = len(defaults)
    for i, a in enumerate(pos):
        di = i - (len(pos) - nd)
        default = defaults[di] if di >= 0 else None
        parts.append(fmt_arg(a, default))

    if args.vararg:
        parts.append("*" + fmt_arg(args.vararg))
    elif args.kwonlyargs:
        parts.append("*")

    for a, d in zip(args.kwonlyargs, args.kw_defaults):
        parts.append(fmt_arg(a, d))

    if args.kwarg:
        parts.append("**" + fmt_arg(args.kwarg))

    ret = ""
    if fn.returns is not None:
        try:
            ret = " -> " + ast.unparse(fn.returns)
        except Exception:
            ret = ""
    return f"{fn.name}({', '.join(parts)}){ret}"


def _public_from_init() -> list[tuple[str, str, str]]:
    """kagra/__init__.py の公開 def / 再エクスポート名を列挙。

    Returns list of (name, signature, kind)
    """
    src = (KAGRA_PKG / "__init__.py").read_text(encoding="utf-8")
    tree = ast.parse(src)
    items: list[tuple[str, str, str]] = []

    # __all__ があれば優先
    all_names: set[str] | None = None
    for node in tree.body:
        if isinstance(node, ast.Assign):
            for t in node.targets:
                if isinstance(t, ast.Name) and t.id == "__all__":
                    try:
                        all_names = set(ast.literal_eval(node.value))
                    except Exception:
                        all_names = None

    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if node.name.startswith("_"):
                continue
            if all_names is not None and node.name not in all_names:
                continue
            items.append((node.name, _sig_from_ast(node), "function"))
        elif isinstance(node, ast.ClassDef):
            if node.name.startswith("_"):
                continue
            if all_names is not None and node.name not in all_names:
                continue
            items.append((node.name, f"class {node.name}", "class"))
        elif isinstance(node, ast.Assign):
            # audio = _Audio() のような公開シングルトン
            for t in node.targets:
                if isinstance(t, ast.Name) and not t.id.startswith("_"):
                    if all_names is not None and t.id not in all_names:
                        continue
                    if t.id in {n for n, _, _ in items}:
                        continue
                    # 関数・クラスと衝突しなければ定数/オブジェクトとして載せる
                    if t.id[0].islower() or t.id.isupper():
                        items.append((t.id, t.id, "object"))

    # from kagra.xxx import Foo の公開名
    for node in tree.body:
        if isinstance(node, ast.ImportFrom) and node.module and node.module.startswith("kagra"):
            for alias in node.names:
                name = alias.asname or alias.name
                if name.startswith("_"):
                    continue
                if all_names is not None and name not in all_names:
                    continue
                if name in {n for n, _, _ in items}:
                    continue
                kind = "class" if name[0].isupper() else "export"
                items.append((name, f"{kind} {name}  (from {node.module})", kind))

    # 名前順
    items.sort(key=lambda x: (x[2] != "function", x[0].lower()))
    return items


def render_markdown(items: list[tuple[str, str, str]]) -> str:
    lines = [
        "# KAGRA Public API Index",
        "",
        "このファイルは `tools/gen_api_index.py` により自動生成されます。手編集しないでください。",
        "",
        f"エントリ数: **{len(items)}**",
        "",
        "## Functions",
        "",
        "| Name | Signature |",
        "|---|---|",
    ]
    funcs = [i for i in items if i[2] == "function"]
    others = [i for i in items if i[2] != "function"]
    for name, sig, _ in funcs:
        esc = sig.replace("|", "\\|")
        lines.append(f"| `{name}` | `{esc}` |")

    lines += ["", "## Classes / Exports / Objects", "", "| Name | Note |", "|---|---|"]
    for name, sig, kind in others:
        esc = sig.replace("|", "\\|")
        lines.append(f"| `{name}` | `{esc}` ({kind}) |")

    lines += [
        "",
        "## Agent notes",
        "",
        "- 存在しない API を呼ばないこと。ここに無い名前は未公開か内部用です。",
        "- `world_to_screen(wx, wy)` は **2D**。3D は `Camera3D.world_to_screen(wx, wy, wz)`。",
        "- セーブは `save_json` / `load_json`。`load_data` はアセットレジストリ。",
        "- VRM が checkout に無いときは `ensure_vrm()`。パスを直書きしない。",
        "- ワンショットポーズは `ActionController`（`ActionController.names()`）。",
        "- Rust バインディングの整合は `tests/test_api_bindings.py` も参照。",
        "- 再生成: `python tools/gen_api_index.py`",
        "",
    ]
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="既存ファイルと一致するか検証")
    args = ap.parse_args()

    items = _public_from_init()
    text = render_markdown(items)

    if args.check:
        if not OUT.exists():
            print(f"MISSING {OUT}", file=sys.stderr)
            return 1
        cur = OUT.read_text(encoding="utf-8")
        if cur != text:
            print(f"OUTDATED {OUT} — run: python tools/gen_api_index.py", file=sys.stderr)
            return 1
        print(f"OK {OUT} ({len(items)} entries)")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8")
    print(f"Wrote {OUT} ({len(items)} entries)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
