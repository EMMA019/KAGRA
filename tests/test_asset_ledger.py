"""Asset ledger: the real repo passes, and each rule fails when it should."""

from __future__ import annotations

import copy
import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location("asset_ledger", ROOT / "tools" / "asset_ledger.py")
assert _spec is not None and _spec.loader is not None
al = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(al)


def good_pack(**kw):
    pack = {
        "id": "p1", "name": "P", "source": "s", "author": "a", "url": "https://x.test",
        "license": "cc0", "license_spdx": "CC0-1.0", "license_url": "https://creativecommons.org/publicdomain/zero/1.0/",
        "obtained": "2026-01-01", "attribution_required": False, "redistributable_in_public_repo": True,
        "allowed_uses": ["commercial"], "paths": ["assets/p1/**"], "verified": "checked", "verified_how": "read it",
    }
    pack.update(kw)
    return pack


def ledger(*packs):
    return {"private_dir": "assets-private/", "exempt_paths": ["docs/**"], "extra_required_paths": [], "packs": list(packs)}


def run(led, files, credits=None):
    return al.check(led, files, credits, manifest_dir=None)


def has(errors, text):
    return any(text in e for e in errors)


def test_real_repo_ledger_passes():
    led = al.load_ledger()
    credits = (ROOT / "CREDITS.md").read_text(encoding="utf-8")
    errors, _ = al.check(led, al.tracked_files(), credits)
    assert errors == []


def test_every_real_entry_has_license_and_known_enum():
    for pack in al.load_ledger()["packs"]:
        assert pack["license"] in al.LICENSES
        assert pack["redistributable_in_public_repo"] in (True, False)


def test_uncovered_asset_fails():
    errors, _ = run(ledger(good_pack()), ["assets/p1/a.glb", "assets/other/b.glb"])
    assert has(errors, "assets/other/b.glb: asset file has no ledger entry")
    assert not has(errors, "assets/p1/a.glb")


def test_exempt_and_non_asset_files_are_ignored():
    errors, _ = run(ledger(good_pack()), ["assets/p1/a.glb", "docs/shot.png", "tools/x.py"])
    assert errors == []


def test_non_redistributable_pack_committed_fails():
    pack = good_pack(license="restricted", license_spdx=None, redistributable_in_public_repo=False,
                     paths=["assets-private/paid/**"])
    errors, _ = run(ledger(pack), ["assets-private/paid/a.glb"])
    assert has(errors, "tracked file under the private asset dir")
    assert has(errors, "not public-redistributable")


def test_non_redistributable_pack_may_not_point_at_public_paths():
    pack = good_pack(redistributable_in_public_repo=False, paths=["assets/paid/**"])
    errors, _ = run(ledger(pack), [])
    assert has(errors, "paths must be under assets-private/")


def test_eula_cannot_be_marked_public():
    errors, _ = run(ledger(good_pack(license="standard-eula", redistributable_in_public_repo=True)), [])
    assert has(errors, "cannot be redistributable_in_public_repo")


def test_missing_or_bad_license_fails():
    bad = good_pack(license="")
    errors, _ = run(ledger(bad), [])
    assert has(errors, "license")
    no_key = good_pack()
    del no_key["license"]
    errors, _ = run(ledger(no_key), [])
    assert has(errors, "missing field license")
    errors, _ = run(ledger(good_pack(license="mit-ish")), [])
    assert has(errors, "is not one of")


def test_cc_by_needs_attribution_text_and_credits_entry():
    pack = good_pack(license="cc-by", license_spdx="CC-BY-4.0", attribution_required=True, attribution_text="")
    errors, _ = run(ledger(pack), [])
    assert has(errors, "attribution_text is empty")
    pack = good_pack(license="cc-by", license_spdx="CC-BY-4.0", attribution_required=False)
    errors, _ = run(ledger(pack), [])
    assert has(errors, "cc-by requires attribution_required")
    pack = good_pack(license="cc-by", license_spdx="CC-BY-4.0", attribution_required=True, attribution_text="Model by Ann")
    led = ledger(pack)
    errors, _ = run(led, [], credits="nothing here")
    assert has(errors, "attribution text is not in CREDITS.md")
    errors, _ = run(led, [], credits=al.render_credits(led))
    assert errors == []


def test_credits_drift_fails():
    led = ledger(good_pack())
    errors, _ = run(led, [], credits=al.render_credits(led) + "\nhand edit\n")
    assert has(errors, "CREDITS.md is out of date")


def test_receipt_ref_must_not_be_a_file():
    errors, _ = run(ledger(good_pack(receipt_ref="receipts/order-123.pdf")), [])
    assert has(errors, "receipt_ref must be an opaque id")
    errors, _ = run(ledger(good_pack(receipt_ref="itch-order-123")), [])
    assert errors == []


def test_private_dir_file_is_always_an_error():
    errors, _ = run(ledger(good_pack()), ["assets-private/x.txt"])
    assert has(errors, "tracked file under the private asset dir")


def test_duplicate_ids_fail():
    errors, _ = run(ledger(good_pack(), good_pack(paths=["assets/q/**"])), [])
    assert has(errors, "duplicate ledger id")


def test_unconfirmed_entries_warn_only():
    pack = good_pack(verified="owner_confirmation_needed", notes="who made this?")
    errors, warnings = run(ledger(pack), ["assets/p1/a.glb"])
    assert errors == []
    assert has(warnings, "awaiting owner confirmation")


def test_manifest_without_ledger_entry_fails(tmp_path):
    (tmp_path / "m.json").write_text('{"packs":[{"id":"z","license":"CC0-1.0","dest":"assets/z"}]}')
    errors, _ = al.check(ledger(good_pack()), [], None, manifest_dir=tmp_path)
    assert has(errors, "has no ledger entry")
    (tmp_path / "m.json").write_text('{"packs":[{"id":"z","license":"CC-BY-4.0","dest":"assets/p1"}]}')
    errors, _ = al.check(ledger(good_pack()), [], None, manifest_dir=tmp_path)
    assert has(errors, "license CC-BY-4.0 != ledger CC0-1.0")


def test_glob_matching():
    assert al.matches("a/b/c.glb", ["a/**"])
    assert al.matches("a/c.glb", ["a/*.glb"])
    assert not al.matches("a/b/c.glb", ["a/*.glb"])
    assert al.matches("x.glb", ["**/x.glb"])


def test_private_dir_is_gitignored():
    ignore = (ROOT / ".gitignore").read_bytes().decode("utf-8", "replace").splitlines()
    assert "assets-private/" in ignore


def test_mutating_a_real_pack_is_caught():
    led = copy.deepcopy(al.load_ledger())
    led["packs"][0]["license"] = "unknown"
    errors, _ = al.check(led, al.tracked_files(), None)
    assert errors
