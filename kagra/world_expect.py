"""GPU-free world dump assertions (shared wgpu 30 / WorldDoc JSON).

Used by ``kagra.verify`` so the runner never imports the archived
RendererV2 ``World3D`` path.
"""
from __future__ import annotations

from typing import Any

__all__ = ["eval_world_expect"]


def _type_ok(got: str, want: str | None) -> bool:
    if want is None or want == "":
        return True
    a = str(got).lower()
    b = str(want).lower()
    if a == b:
        return True
    aliases = {
        "tile": "terrain_tile",
        "terrain": "terrain_tile",
        "walk": "walker",
        "player": "walker",
    }
    return aliases.get(b, b) == a


def _name_ok(got: str | None, want: str | None) -> bool:
    if want is None or want == "":
        return True
    return str(got or "") == str(want)


def _flatten_dump(data: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for key in ("props", "walkers", "lights", "cameras"):
        rows.extend(list(data.get(key) or []))
    hf = data.get("heightfield") or {}
    for t in hf.get("tiles") or []:
        row = dict(t)
        row.setdefault("type", "terrain_tile")
        row.setdefault("name", f"tile:{row.get('ix')},{row.get('iz')}")
        rows.append(row)
    return rows


def eval_world_expect(data: dict[str, Any], spec: dict[str, Any]) -> list[str]:
    """GPU-free world assertions. Returns error strings (empty = ok)."""
    errors: list[str] = []
    player = data.get("player") or {}
    if "player.on_ground" in spec:
        want = bool(spec["player.on_ground"])
        got = bool(player.get("on_ground"))
        if got != want:
            errors.append(f"player.on_ground: want {want} got {got}")
    if "coins" in spec:
        want_n = int(spec["coins"])
        got_n = int(data.get("coins", -1))
        if got_n < 0:
            got_n = sum(
                1
                for p in data.get("props") or []
                if p.get("name") == "coin" and p.get("enabled", True)
            )
        if got_n != want_n:
            errors.append(f"coins: want {want_n} got {got_n}")
    if "genre" in spec:
        want_g = spec["genre"]
        got_g = data.get("genre")
        if got_g != want_g:
            errors.append(f"genre: want {want_g!r} got {got_g!r}")
    for item in spec.get("query") or []:
        if not isinstance(item, dict):
            continue
        typ = item.get("type")
        nam = item.get("name")
        rows = [
            r
            for r in _flatten_dump(data)
            if _type_ok(str(r.get("type") or ""), typ) and _name_ok(r.get("name"), nam)
        ]
        if "count" in item and len(rows) != int(item["count"]):
            errors.append(
                f"query type={typ!r} name={nam!r}: want count {item['count']} got {len(rows)}"
            )
        if "min_count" in item and len(rows) < int(item["min_count"]):
            errors.append(
                f"query type={typ!r} name={nam!r}: want min {item['min_count']} got {len(rows)}"
            )
        if "albedo_ok" in item:
            bad = [r for r in rows if bool(r.get("albedo_ok", True)) != bool(item["albedo_ok"])]
            if item.get("require_any") and not rows:
                errors.append(f"query type={typ!r}: no rows for albedo_ok check")
            elif "count" not in item and "min_count" not in item and bad and not item.get("allow_mixed"):
                if all(bool(r.get("albedo_ok", True)) != bool(item["albedo_ok"]) for r in rows) or item.get("all"):
                    errors.append(
                        f"query type={typ!r}: albedo_ok want {item['albedo_ok']}"
                    )
        if item.get("loaded") is True and not any(r.get("loaded") for r in rows):
            errors.append(f"query type={typ!r}: expected a loaded tile")
    return errors
