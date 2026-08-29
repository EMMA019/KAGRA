"""セーブ深化（kagra.save — Phase 6）の純ロジックテスト。

拡張非依存。tmp_path で実ファイルを検証する。
"""
import json

from tests.conftest import load_kagra_submodule

save_mod = load_kagra_submodule("save")


def test_save_load_roundtrip_versioned(tmp_path):
    p = tmp_path / "save.json"
    save_mod.save_data(p, {"day": 3, "money": 120}, version=1)
    raw = json.loads(p.read_text(encoding="utf-8"))
    assert raw["version"] == 1
    assert raw["data"]["money"] == 120
    assert save_mod.load_data(p, version=1) == {"day": 3, "money": 120}


def test_load_missing_returns_default(tmp_path):
    assert save_mod.load_data(tmp_path / "nope.json", default={"x": 1}) == {"x": 1}
    assert save_mod.load_data(tmp_path / "nope.json") is None


def test_load_corrupt_returns_default(tmp_path):
    p = tmp_path / "bad.json"
    p.write_text("{not json", encoding="utf-8")
    assert save_mod.load_data(p, default={}) == {}


def test_legacy_plain_dict_loads_as_version0(tmp_path):
    # 旧形式（version キー無しの生ゲーム dict）は version 0 として読む
    p = tmp_path / "legacy.json"
    p.write_text(json.dumps({"day": 7, "money": 9}), encoding="utf-8")
    assert save_mod.load_data(p, version=1) == {"day": 7, "money": 9}


def test_migration_chain_applies_in_order(tmp_path):
    p = tmp_path / "mig.json"
    # v0 のデータ（旧形式）→ v2 まで引き上げ
    p.write_text(json.dumps({"money_old": 100}), encoding="utf-8")
    migrations = {
        0: lambda d: {"money": d["money_old"], "stock": {}},
        1: lambda d: {**d, "day": 1},
    }
    out = save_mod.load_data(p, version=2, migrations=migrations)
    assert out == {"money": 100, "stock": {}, "day": 1}


def test_migration_missing_step_stops_gracefully(tmp_path):
    p = tmp_path / "mig2.json"
    p.write_text(json.dumps({"v": 0}), encoding="utf-8")
    # v0 → v1 の変換が無い → そのまま返す（壊れたセーブを出さない）
    out = save_mod.load_data(p, version=2, migrations={1: lambda d: d})
    assert out == {"v": 0}


def test_atomic_write_no_tmp_leftover(tmp_path):
    p = tmp_path / "a.json"
    save_mod.atomic_write(p, "hello")
    assert p.read_text(encoding="utf-8") == "hello"
    assert not p.with_name(p.name + ".tmp").exists(), "tmp は残らない"


def test_backup_keeps_previous(tmp_path):
    p = tmp_path / "b.json"
    save_mod.save_data(p, {"n": 1}, version=1)
    save_mod.save_data(p, {"n": 2}, version=1)
    bak = p.with_name(p.name + ".bak")
    assert bak.exists(), "2 回目の保存で .bak ができる"
    assert json.loads(bak.read_text(encoding="utf-8"))["data"]["n"] == 1
    assert save_mod.load_data(p, version=1) == {"n": 2}


def test_slot_store_save_load_latest(tmp_path):
    s = save_mod.SlotStore(tmp_path, name="slot", count=3, version=1)
    assert s.slots() == []
    assert s.latest() is None
    s.save(2, {"x": 20})
    s.save(1, {"x": 10})
    assert s.slots() == [1, 2]
    assert s.latest() == 2
    assert s.load(1) == {"x": 10}
    assert s.load(2) == {"x": 20}
    assert s.load(3) is None


def test_slot_store_clamps_and_deletes(tmp_path):
    s = save_mod.SlotStore(tmp_path, count=3)
    s.save(99, {"a": 1})  # count 3 にクランプ
    assert s.slots() == [3]
    s.delete(3)
    assert s.slots() == []
    assert not (tmp_path / "slot_3.json.bak").exists(), "delete で .bak も消える"


def test_slot_store_migrations(tmp_path):
    s = save_mod.SlotStore(tmp_path, count=1, version=2, migrations={1: lambda d: {**d, "migrated": True}})
    s.save(1, {"v": 1}, backup=False)
    # 保存は current version (2) で行われるので、v1 の旧データを直接書いて読む
    (tmp_path / "slot_1.json").write_text(
        json.dumps({"version": 1, "data": {"old": 1}}), encoding="utf-8"
    )
    assert s.load(1) == {"old": 1, "migrated": True}
