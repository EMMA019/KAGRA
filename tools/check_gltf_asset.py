#!/usr/bin/env python3
"""Desktop/CI checks for a glTF or GLB.

Uses pygltflib for images and material factors, and trimesh for the
triangle count and bounds. A metallic/roughness pair left at the glTF
default (1, 1) is a report. A missing image is an error. This script is
not imported by the wheel.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path


def check_gltf(path: Path) -> dict:
    path = Path(path)
    report = {
        "path": str(path),
        "triangles": 0,
        "bounds": None,
        "missing_images": [],
        "default_metal_rough": [],
        "errors": [],
    }
    try:
        import pygltflib
        import trimesh
    except ImportError as exc:
        report["errors"].append(f"asset packages are not installed: {exc}")
        return report

    try:
        gltf = pygltflib.GLTF2.load(str(path))
    except Exception as exc:
        report["errors"].append(f"pygltflib: {exc}")
        gltf = None

    if gltf is not None:
        report["missing_images"] = _missing_images(path, gltf)
        report["default_metal_rough"] = _default_metal_rough(gltf)
        if report["missing_images"]:
            joined = ", ".join(report["missing_images"])
            report["errors"].append(f"missing images: {joined}")

    try:
        loaded = trimesh.load(path, force="scene")
    except Exception as exc:
        report["errors"].append(f"trimesh: {exc}")
        return report

    geoms = list(getattr(loaded, "geometry", {}).values())
    if hasattr(loaded, "faces") and not geoms:
        geoms = [loaded]
    triangles = 0
    lows: list[float] | None = None
    highs: list[float] | None = None
    for geom in geoms:
        faces = getattr(geom, "faces", None)
        if faces is not None:
            triangles += len(faces)
        bounds = getattr(geom, "bounds", None)
        if bounds is None:
            continue
        low = [float(v) for v in bounds[0]]
        high = [float(v) for v in bounds[1]]
        if lows is None:
            lows, highs = low, high
        else:
            lows = [min(a, b) for a, b in zip(lows, low)]
            highs = [max(a, b) for a, b in zip(highs, high)]
    report["triangles"] = triangles
    if lows is None or highs is None:
        report["errors"].append("bounds are missing")
    elif not all(math.isfinite(v) for v in lows + highs):
        report["errors"].append("bounds are not finite")
        report["bounds"] = [lows, highs]
    else:
        report["bounds"] = [lows, highs]
    if triangles <= 0:
        report["errors"].append("triangle count is 0")
    return report


def _missing_images(path: Path, gltf) -> list[str]:
    missing: list[str] = []
    for index, image in enumerate(gltf.images or []):
        uri = image.uri
        if not uri:
            if image.bufferView is None:
                missing.append(f"image {index}")
            continue
        if uri.startswith("data:"):
            continue
        target = (path.parent / uri).resolve()
        if not target.is_file():
            missing.append(uri)
    return missing


def _default_metal_rough(gltf) -> list[str]:
    notes: list[str] = []
    for index, mat in enumerate(gltf.materials or []):
        pbr = mat.pbrMetallicRoughness
        metal = 1.0 if pbr is None or pbr.metallicFactor is None else float(pbr.metallicFactor)
        rough = 1.0 if pbr is None or pbr.roughnessFactor is None else float(pbr.roughnessFactor)
        if abs(metal - 1.0) < 1e-3 and abs(rough - 1.0) < 1e-3:
            notes.append(mat.name or f"material {index}")
    return notes


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: check_gltf_asset.py <file.glb> [more...]", file=sys.stderr)
        return 2
    failed = 0
    for arg in argv[1:]:
        report = check_gltf(Path(arg))
        print(json.dumps(report, indent=2))
        if report["errors"]:
            failed = 1
    return failed


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
