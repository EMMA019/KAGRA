#!/usr/bin/env python3
"""Scaffold a new KAGRA genre binding (Rust GenreSession + py + example + verify).

Usage:
    python scripts/new_genre.py my_genre
    python scripts/new_genre.py --id fish_cast --title "Fish cast" FishCast

Creates (under repo root):
  - kagra-shared/src/py/<module>.rs   (PyO3 stub; register in py/mod.rs manually)
  - kagra-shared/src/<module>.rs note  (reminder: implement GenreSession in genre_session.rs)
  - examples/<module>_minimal.py
  - examples/verify_scenarios/<module>_smoke.json

Does not edit genre_session.rs or WorldPlay — those are the next manual steps.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def slug(s: str) -> str:
    s = s.strip().lower().replace(" ", "_").replace("-", "_")
    s = re.sub(r"[^a-z0-9_]", "", s)
    return s or "new_genre"


def pascal(sl: str) -> str:
    return "".join(p.capitalize() for p in sl.split("_") if p)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("name", nargs="?", default="new_genre", help="module slug (e.g. fish_cast)")
    ap.add_argument("--id", dest="genre_id", help="WorldDoc.genre id (default: name)")
    ap.add_argument("--title", default="", help="Human title for smoke JSON notes")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    module = slug(args.name)
    genre_id = args.genre_id or module
    title = args.title or genre_id.replace("_", " ").title()
    class_name = pascal(module) + "Session"
    py_class = f"Py{class_name}"
    fixture = f"kagra-shared/tests/fixtures/{module}_world.json"

    py_stub = ROOT / "kagra-shared/src/py" / f"{module}.rs"
    example = ROOT / "examples" / f"{module}_minimal.py"
    smoke = ROOT / "examples/verify_scenarios" / f"{module}_smoke.json"

    if py_stub.exists():
        print(f"refusing to overwrite {py_stub}", file=sys.stderr)
        return 1

    rs_body = f'''//! `{genre_id}` — generated stub. Wire `GenreSession` in `genre_session.rs`.

use pyo3::prelude::*;

use crate::collectathon::WalkInput;
use crate::genre_session::{{GenreSession, {class_name}}};

use super::common::{{phase_str, pyerr}};

#[pyclass(name = "{class_name}")]
pub struct {py_class} {{
    inner: {class_name},
}}

#[pymethods]
impl {py_class} {{
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {{
        Ok(Self {{
            inner: {class_name}::from_json(json).map_err(pyerr)?,
        }})
    }}

    #[getter]
    fn genre_id(&self) -> &'static str {{
        self.inner.genre_id()
    }}

    fn phase(&self) -> &'static str {{
        phase_str(self.inner.phase())
    }}

    fn tick(&mut self, dt: f32) {{
        self.inner.tick(dt);
    }}

    fn confirm(&mut self) {{
        self.inner.confirm();
    }}

    fn dump(&self) -> PyResult<String> {{
        self.inner.dump().map_err(pyerr)
    }}

    fn set_input(&mut self, lx: f32, lz: f32, jump: bool, attack: bool, dodge: bool) {{
        self.inner.set_input(
            WalkInput {{ lx, lz, jump, attack, dodge }}.clamped(),
        );
    }}

    fn emit_event(&mut self, name: &str, data_json: Option<&str>) -> PyResult<()> {{
        let payload = match data_json {{
            Some(s) if !s.trim().is_empty() => Some(serde_json::from_str(s).map_err(pyerr)?),
            _ => None,
        }};
        self.inner.emit_event(name, payload).map_err(pyerr)?;
        Ok(())
    }}

    fn take_events(&mut self, name: &str) -> Vec<String> {{
        self.inner
            .take_events(name)
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect()
    }}

    fn start_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {{
        self.inner.start_timer(name, seconds, on_done)
    }}

    fn step_interact(&mut self) {{
        self.inner.step_interact();
    }}
}}
'''

    py_example = f'''"""{title} minimal — generated stub. Fill game logic + fixture."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from kagra.gameloop import Scene, run  # noqa: E402

DUMP = Path(__file__).resolve().parents[1] / "{fixture}"


class {pascal(module)}Play(Scene):
    def __init__(self) -> None:
        super().__init__()
        raise NotImplementedError("implement {class_name} + fixture {fixture}")


def main() -> None:
    run({pascal(module)}Play())


if __name__ == "__main__":
    main()
'''

    smoke_json = f"""{{
  "name": "{module}_smoke",
  "notes": "{title} smoke (generated). Point expect_world.path at {fixture}.",
  "expect_world": {{
    "path": "{fixture}",
    "player.on_ground": true
  }},
  "expect_offscreen": {{
    "world": "{fixture}",
    "out": "scratch/{module}_shared.png",
    "width": 320,
    "height": 180,
    "min_bytes": 200
  }}
}}
"""

    paths = [
        (py_stub, rs_body),
        (example, py_example),
        (smoke, smoke_json),
    ]
    for path, body in paths:
        if args.dry_run:
            print(f"would write {path.relative_to(ROOT)} ({len(body)} bytes)")
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(body, encoding="utf-8")
        print(f"wrote {path.relative_to(ROOT)}")

    if not args.dry_run:
        print()
        print("Next steps:")
        print(f"  1. impl GenreSession for {class_name} in kagra-shared/src/genre_session.rs")
        print(f"  2. mod {module}; in kagra-shared/src/py/mod.rs + add_class::<{py_class}>()")
        print(f"  3. Author {fixture} (genre: {genre_id!r}) with bundled .glb props")
        print("  4. cargo test -p kagra-shared --lib && python -m kagra.verify ...")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
