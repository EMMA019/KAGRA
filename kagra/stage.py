"""Drop-in venue: a Sketchfab hall or a sky image.

Dance is already ``av.dance("samba.fbx")``. Visual luxury is the same model —
drop ``venue.glb`` or a PNG, don't author VFX in the engine.

GPU is only required for ``Stage.load`` / ``draw``. Path resolution and the
backdrop sphere mesh are pure Python so tests can run without wgpu.
"""
from __future__ import annotations

import math
from pathlib import Path
from typing import Literal

from kagra.contracts import AssetKind, KagraContractError, project_root, resolve_asset

GLTF_EXTS = {".glb", ".gltf"}
IMAGE_EXTS = {".png", ".jpg", ".jpeg", ".webp", ".bmp"}
StageKind = Literal["gltf", "backdrop"]


def classify_stage_file(path: str | Path) -> StageKind:
    """``.glb`` / ``.gltf`` → hall. Image → inverted sky sphere."""
    suf = Path(path).suffix.lower()
    if suf in GLTF_EXTS:
        return "gltf"
    if suf in IMAGE_EXTS:
        return "backdrop"
    raise KagraContractError(
        code="UNSUPPORTED_STAGE",
        message=f"not a stage file: {path}",
        hint="Use a .glb/.gltf hall or a PNG/JPEG/WebP backdrop.",
        path=str(path),
    )


def resolve_stage_path(
    name: str,
    *,
    root: Path | None = None,
    required: bool = True,
) -> Path | None:
    """Resolve a hall or backdrop the same way dance clips resolve."""
    root = root or project_root()
    name = (name or "").strip()
    if not name or name.lower() in ("none", "-", "builtin"):
        if required:
            raise KagraContractError(
                code="ASSET_NOT_FOUND",
                message="stage asset not found: empty name",
                hint="Pass a .glb/.gltf path or a PNG backdrop, or alias 'stage'.",
                path=name,
            )
        return None
    for kind in (AssetKind.GLTF, AssetKind.TEXTURE, AssetKind.ANY):
        found = resolve_asset(kind, name, root=root, required=False)
        if found is not None:
            try:
                classify_stage_file(found)
            except KagraContractError:
                continue
            return found
    if not required:
        return None
    raise KagraContractError(
        code="ASSET_NOT_FOUND",
        message=f"stage asset not found: {name}",
        hint="Place venue.glb under assets/ (or assets/stage/), or pass an absolute path.",
        path=name,
    )


def backdrop_sphere(
    radius: float = 12.0,
    rings: int = 16,
    segs: int = 24,
) -> tuple[list[list[float]], list[int]]:
    """Inverted UV sphere for a sky / HDRI-like still.

    Vertices are ``[x, y, z, nx, ny, nz, u, v]``. Normals point inward so
    ``draw_mesh_3d`` shows the texture from inside the hall.
    """
    if radius <= 0:
        raise ValueError("backdrop radius must be > 0")
    rings = max(3, int(rings))
    segs = max(3, int(segs))
    verts: list[list[float]] = []
    indices: list[int] = []
    for yi in range(rings + 1):
        v = yi / rings
        phi = math.pi * (v - 0.5)
        cy, sy = math.cos(phi), math.sin(phi)
        for xi in range(segs + 1):
            u = xi / segs
            theta = 2.0 * math.pi * u
            x = math.cos(theta) * cy
            y = sy
            z = math.sin(theta) * cy
            verts.append([
                x * radius,
                y * radius,
                z * radius,
                -x,
                -y,
                -z,
                u,
                1.0 - v,
            ])
    cols = segs + 1
    for yi in range(rings):
        for xi in range(segs):
            a = yi * cols + xi
            b = a + cols
            # Inward winding (camera is inside)
            indices.extend([a, b, a + 1, a + 1, b, b + 1])
    return verts, indices


class Stage:
    """Loaded venue. Call ``draw()`` in the frame loop."""

    def __init__(
        self,
        kind: StageKind,
        path: Path,
        *,
        model_id: int | None = None,
        tex_id: int | None = None,
        verts: list[list[float]] | None = None,
        indices: list[int] | None = None,
    ):
        self.kind = kind
        self.path = Path(path)
        self.model_id = model_id
        self.tex_id = tex_id
        self.verts = verts or []
        self.indices = indices or []

    @classmethod
    def load(cls, name: str, *, radius: float = 12.0) -> "Stage":
        """Load after the renderer exists (``on_ready`` / ``run()``)."""
        import kagra

        path = resolve_stage_path(name, required=True)
        assert path is not None
        kind = classify_stage_file(path)
        if kind == "gltf":
            return cls(kind, path, model_id=kagra.load_gltf(str(path)))
        tex_id = kagra.load(str(path))
        verts, indices = backdrop_sphere(radius)
        return cls(kind, path, tex_id=tex_id, verts=verts, indices=indices)

    def draw(self) -> None:
        import kagra

        if self.kind == "gltf" and self.model_id is not None:
            kagra.draw_gltf(self.model_id)
            return
        if self.kind == "backdrop" and self.tex_id is not None:
            # SHADER_3D applies distance fog. A puresky sphere past fog_end
            # becomes the fog color (Crest Isle: r=140, fog_end=102 → grey).
            from kagra.look import current_fog

            fog = current_fog()
            if fog["enabled"]:
                kagra.set_fog(
                    fog["start"], fog["end"], fog["color"], enabled=False,
                )
            kagra.draw_mesh_3d(self.tex_id, self.verts, self.indices)
            if fog["enabled"]:
                kagra.set_fog(
                    fog["start"], fog["end"], fog["color"], enabled=True,
                )

    def unload(self) -> None:
        if self.model_id is None:
            return
        import kagra

        kagra.unload_gltf(self.model_id)
        self.model_id = None
