"""ローカライズ（Phase 7）。文字列テーブル + 言語切替。

ゲームロジックは Python のみ。UI 文字列をハードコードから
``kagra.i18n.t("key", ...)`` に移し、``set_lang`` で切り替える。

- キー → ``{lang}`` テーブル → 見つからなければ ``ja`` へフォールバック
  → それも無ければキー文字列そのもの（壊れない）。
- ``{name}`` プレースホルダは ``t(key, name=...)`` で埋める。
- テーブルは dict または JSON ファイル（``add_table`` / ``load_json``）。

使い方::

    from kagra import i18n
    i18n.add_table("ja", {"menu.talk": "話す", "menu.quit": "閉店"})
    i18n.add_table("en", {"menu.talk": "Talk", "menu.quit": "Close"})
    i18n.set_lang("en")
    i18n.t("menu.talk")   # "Talk"
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

__all__ = ["t", "set_lang", "get_lang", "add_table", "load_json", "available_langs"]

_TABLES: dict[str, dict[str, str]] = {}
_LANG = "ja"


def add_table(lang: str, table: dict[str, str]) -> None:
    """``lang`` の文字列テーブルを追加（既存キーは上書き）。"""
    merged = _TABLES.setdefault(lang, {})
    merged.update(table)


def load_json(lang: str, path: str | Path) -> None:
    """JSON ファイル（``{"key": "text"}``）をテーブルに追加。"""
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError(f"{path}: table must be a JSON object")
    add_table(lang, {k: str(v) for k, v in data.items()})


def set_lang(lang: str) -> None:
    """表示言語を切り替える。テーブルが無い言語でもエラーにしない。"""
    global _LANG
    _LANG = lang


def get_lang() -> str:
    return _LANG


def available_langs() -> list[str]:
    """テーブルが登録されている言語（昇順）。"""
    return sorted(_TABLES)


def _format(text: str, **kw: Any) -> str:
    try:
        return text.format(**kw)
    except (KeyError, IndexError, ValueError):
        return text


def t(key: str, **kw: Any) -> str:
    """``key`` の翻訳を返す。

    優先: 現在言語 → ``ja`` → キーそのもの。``{name}`` は kwargs で埋める。
    """
    text = _TABLES.get(_LANG, {}).get(key)
    if text is None:
        text = _TABLES.get("ja", {}).get(key)
    if text is None:
        text = key
    if kw:
        return _format(text, **kw)
    return text
