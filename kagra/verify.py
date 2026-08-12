"""宣言的シナリオ検証ランナー（エージェント閉ループ用）。

シナリオ JSON 例::

    {
      "name": "orb_smoke",
      "script": "scratch/smoke_orb_rush.py",
      "timeout_sec": 120,
      "expect_files": ["scratch/orb_rush_smoke.png"],
      "min_file_bytes": 1000
    }

またはインライン（別プロセスで小さなシーンを生成）::

    {
      "name": "blank",
      "inline": {
        "width": 320, "height": 180, "max_frames": 8,
        "screenshot_at": 4,
        "out": "scratch/verify_blank.png"
      }
    }

CLI::

    python -m kagra.verify path/to/scenario.json
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import time
import traceback
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]


@dataclass
class Expectation:
    path: str
    min_bytes: int = 500
    must_exist: bool = True


@dataclass
class Scenario:
    name: str
    script: str | None = None
    inline: dict[str, Any] | None = None
    cwd: str | None = None
    timeout_sec: float = 180.0
    expect: list[Expectation] = field(default_factory=list)
    env: dict[str, str] = field(default_factory=dict)


@dataclass
class VerifyResult:
    ok: bool
    name: str
    elapsed_sec: float
    stdout: str = ""
    stderr: str = ""
    missing: list[str] = field(default_factory=list)
    too_small: list[str] = field(default_factory=list)
    error: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "ok": self.ok,
            "name": self.name,
            "elapsed_sec": round(self.elapsed_sec, 3),
            "missing": self.missing,
            "too_small": self.too_small,
            "error": self.error,
            "stdout_tail": self.stdout[-2000:],
            "stderr_tail": self.stderr[-2000:],
        }


def _load_scenario(data: dict[str, Any]) -> Scenario:
    expects: list[Expectation] = []
    for item in data.get("expect_files", data.get("expect", [])):
        if isinstance(item, str):
            expects.append(
                Expectation(path=item, min_bytes=int(data.get("min_file_bytes", 500)))
            )
        else:
            expects.append(
                Expectation(
                    path=item["path"],
                    min_bytes=int(item.get("min_bytes", data.get("min_file_bytes", 500))),
                    must_exist=bool(item.get("must_exist", True)),
                )
            )
    return Scenario(
        name=str(data.get("name", "unnamed")),
        script=data.get("script"),
        inline=data.get("inline"),
        cwd=data.get("cwd"),
        timeout_sec=float(data.get("timeout_sec", 180)),
        expect=expects,
        env={str(k): str(v) for k, v in (data.get("env") or {}).items()},
    )


def load_scenario(path: str | Path) -> Scenario:
    p = Path(path)
    data = json.loads(p.read_text(encoding="utf-8"))
    if "name" not in data:
        data["name"] = p.stem
    return _load_scenario(data)


def _write_inline_script(inline: dict[str, Any]) -> Path:
    w = int(inline.get("width", 320))
    h = int(inline.get("height", 180))
    max_frames = int(inline.get("max_frames", 12))
    shot_at = int(inline.get("screenshot_at", max(1, max_frames // 2)))
    out = inline.get("out", "scratch/verify_inline.png")
    clear = inline.get("cls", [40, 45, 55])
    code = f'''
import os, sys
sys.path.insert(0, {str(ROOT)!r})
import kagra
OUT = {out!r}
os.makedirs(os.path.dirname(OUT) or ".", exist_ok=True)

class S(kagra.Scene):
    def update(self, dt):
        t = kagra.tick_count()
        if t == {shot_at}:
            kagra.screenshot(OUT)
        if t >= {max_frames}:
            kagra.quit()
    def draw(self):
        kagra.cls({clear[0]}, {clear[1]}, {clear[2]})

kagra.init(width={w}, height={h}, title="verify_inline", fps=60, visible=False)
kagra.run(start_scene=S(), max_frames={max_frames + 2}, fixed_dt=1.0/60.0)
print("OK" if os.path.exists(OUT) else "MISSING", OUT)
'''
    fd, path = tempfile.mkstemp(suffix="_kagra_verify.py", text=True)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        f.write(code)
    return Path(path)


def run_scenario(scenario: Scenario, *, python: str | None = None) -> VerifyResult:
    """シナリオをサブプロセス実行（Windows の EventLoop 制約回避）。"""
    py = python or sys.executable
    cwd = Path(scenario.cwd) if scenario.cwd else ROOT
    env = os.environ.copy()
    env.update(scenario.env)
    env.setdefault("PYTHONUTF8", "1")

    tmp_script: Path | None = None
    try:
        if scenario.script:
            script = Path(scenario.script)
            if not script.is_absolute():
                script = cwd / script
            if not script.exists():
                return VerifyResult(
                    ok=False,
                    name=scenario.name,
                    elapsed_sec=0.0,
                    error=f"script not found: {script}",
                )
            cmd = [py, str(script)]
        elif scenario.inline:
            tmp_script = _write_inline_script(scenario.inline)
            # inline の expect が空なら out を自動追加
            if not scenario.expect and scenario.inline.get("out"):
                scenario.expect.append(
                    Expectation(path=str(scenario.inline["out"]), min_bytes=200)
                )
            cmd = [py, str(tmp_script)]
        else:
            return VerifyResult(
                ok=False,
                name=scenario.name,
                elapsed_sec=0.0,
                error="scenario needs 'script' or 'inline'",
            )

        t0 = time.perf_counter()
        try:
            proc = subprocess.run(
                cmd,
                cwd=str(cwd),
                env=env,
                capture_output=True,
                text=True,
                timeout=scenario.timeout_sec,
                encoding="utf-8",
                errors="replace",
            )
        except subprocess.TimeoutExpired as e:
            return VerifyResult(
                ok=False,
                name=scenario.name,
                elapsed_sec=time.perf_counter() - t0,
                stdout=(e.stdout or "") if isinstance(e.stdout, str) else "",
                stderr=(e.stderr or "") if isinstance(e.stderr, str) else "",
                error=f"timeout after {scenario.timeout_sec}s",
            )
        elapsed = time.perf_counter() - t0

        missing: list[str] = []
        too_small: list[str] = []
        for exp in scenario.expect:
            p = Path(exp.path)
            if not p.is_absolute():
                p = cwd / p
            if exp.must_exist and not p.exists():
                missing.append(str(p))
            elif p.exists() and p.stat().st_size < exp.min_bytes:
                too_small.append(f"{p} ({p.stat().st_size} < {exp.min_bytes})")

        ok = proc.returncode == 0 and not missing and not too_small
        return VerifyResult(
            ok=ok,
            name=scenario.name,
            elapsed_sec=elapsed,
            stdout=proc.stdout or "",
            stderr=proc.stderr or "",
            missing=missing,
            too_small=too_small,
            error=None
            if ok
            else (
                f"exit={proc.returncode}"
                + (f"; missing={missing}" if missing else "")
                + (f"; too_small={too_small}" if too_small else "")
            ),
        )
    except Exception as e:
        return VerifyResult(
            ok=False,
            name=scenario.name,
            elapsed_sec=0.0,
            error=f"{type(e).__name__}: {e}\n{traceback.format_exc()}",
        )
    finally:
        if tmp_script is not None:
            try:
                tmp_script.unlink(missing_ok=True)
            except OSError:
                pass


def run_scenario_path(path: str | Path, **kwargs) -> VerifyResult:
    return run_scenario(load_scenario(path), **kwargs)


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__)
        return 0
    path = argv[0]
    result = run_scenario_path(path)
    print(json.dumps(result.to_dict(), ensure_ascii=False, indent=2))
    return 0 if result.ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
