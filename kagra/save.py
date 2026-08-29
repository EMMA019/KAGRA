"""セーブ深化（Phase 6）。共通のバージョン付きセーブ + マイグレーション。

ゲームロジックは Python のみ。バニーガーデンとトルネコはそれぞれ独自に
JSON を書いていたが、ここに寄せる:

- **バージョン付き**: ``{"version": N, "data": ...}``。将来のスキーマ変更は
  ``migrations`` で過去データを引き上げる（壊れたセーブを出さない）。
- **後方互換**: ``version`` キーが無い旧形式（生 dict）は version 0 として
  読む。
- **アトミック書き込み**: ``tmp`` に書いて ``os.replace``。途中で落ちても
  旧ファイルが残る。
- **バックアップ**: 保存時に直前の内容を ``.bak`` に残す（オートセーブ
  保険）。壊れた新セーブは ``.bak`` から戻せる。
- **スロット**: ``SlotStore`` で複数スロット（SLG の「続きから」等）。

決定論的・拡張非依存（テストは kagra_shared 不要）。
"""
from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any, Callable

__all__ = ["save_data", "load_data", "migrate_data", "atomic_write", "SlotStore"]

Migration = Callable[[dict], dict]


def atomic_write(path: Path, text: str) -> None:
    """``tmp`` に書いて ``os.replace``。クラッシュしても元ファイルは無傷。"""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(text, encoding="utf-8")
    os.replace(tmp, path)


def save_data(
    path: Path,
    data: dict,
    *,
    version: int = 1,
    backup: bool = True,
) -> None:
    """``{"version": N, "data": ...}`` をアトミックに保存。

    ``backup`` で直前の内容を ``<path>.bak`` に残す（オートセーブ保険）。
    """
    path = Path(path)
    if backup and path.exists():
        try:
            path.with_name(path.name + ".bak").write_text(
                path.read_text(encoding="utf-8"), encoding="utf-8"
            )
        except Exception:  # pragma: no cover - 読み取り失敗は静かに
            pass
    payload = {"version": int(version), "data": data}
    atomic_write(path, json.dumps(payload, ensure_ascii=False, indent=1))


def load_data(
    path: Path,
    *,
    version: int = 1,
    migrations: dict[int, Migration] | None = None,
    default: dict | None = None,
) -> dict | None:
    """保存を読み、``migrations`` で現在の ``version`` まで引き上げる。

    - 無い / 壊れている → ``default``（未指定なら None）
    - 旧形式（version キー無しの生 dict）→ version 0 として扱う
    - ``migrations[v]`` は version v → v+1 の変換
    """
    path = Path(path)
    if not path.exists():
        return default
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return default
    if not isinstance(raw, dict):
        return default
    if "version" not in raw or "data" not in raw:
        # 旧形式: 生ゲーム dict を version 0 とみなす
        data, from_v = raw, 0
    else:
        data, from_v = raw.get("data"), int(raw.get("version", 0))
    if not isinstance(data, dict):
        return default
    return migrate_data(data, from_v, int(version), migrations or {})


def migrate_data(
    data: dict,
    from_version: int,
    to_version: int,
    migrations: dict[int, Migration],
) -> dict:
    """``migrations[v]``（v → v+1）を順に適用して ``to_version`` へ。"""
    out = dict(data)
    v = int(from_version)
    while v < int(to_version):
        fn = migrations.get(v)
        if fn is None:
            break  # 変換が無いならそのまま（壊れたセーブを出さない）
        out = fn(out)
        v += 1
    return out


class SlotStore:
    """複数スロット（``<dir>/<name>_<n>.json``、n は 1..count）。"""

    def __init__(
        self,
        directory: Path,
        *,
        name: str = "slot",
        count: int = 3,
        version: int = 1,
        migrations: dict[int, Migration] | None = None,
    ) -> None:
        self.directory = Path(directory)
        self.name = name
        self.count = max(1, int(count))
        self.version = int(version)
        self.migrations = migrations or {}

    def _path(self, slot: int) -> Path:
        slot = max(1, min(int(slot), self.count))
        return self.directory / f"{self.name}_{slot}.json"

    def save(self, slot: int, data: dict, *, backup: bool = True) -> None:
        save_data(self._path(slot), data, version=self.version, backup=backup)

    def load(self, slot: int, default: dict | None = None) -> dict | None:
        return load_data(
            self._path(slot),
            version=self.version,
            migrations=self.migrations,
            default=default,
        )

    def delete(self, slot: int) -> None:
        p = self._path(slot)
        p.unlink(missing_ok=True)
        p.with_name(p.name + ".bak").unlink(missing_ok=True)

    def slots(self) -> list[int]:
        """保存済みスロット番号（昇順）。"""
        out = []
        for n in range(1, self.count + 1):
            if self._path(n).exists():
                out.append(n)
        return out

    def latest(self) -> int | None:
        """最後に保存したスロット（無ければ None）。"""
        s = self.slots()
        return s[-1] if s else None
