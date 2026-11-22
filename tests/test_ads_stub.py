"""The no-op portal hooks in scripts/preview_site/ads.js."""
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_break_and_rewarded_resolve_without_showing_an_ad():
    node = shutil.which("node")
    if node is None:
        return
    script = r"""
const ads = require("./scripts/preview_site/ads.js");
(async () => {
  const a = await ads.break("level_end");
  const b = await ads.rewarded("continue");
  const c = await ads.ads.break("death");
  console.log(JSON.stringify({a, b, c, n: ads.ads.log.length}));
})();
"""
    done = subprocess.run([node, "-e", script], cwd=ROOT, capture_output=True, text=True, timeout=20)
    assert done.returncode == 0, done.stderr
    got = json.loads(done.stdout)
    assert got["a"] == {"shown": False, "slot": "level_end", "reason": "stub"}
    assert got["b"]["rewarded"] is False and got["b"]["slot"] == "continue"
    assert got["c"]["slot"] == "death" and got["n"] == 3
