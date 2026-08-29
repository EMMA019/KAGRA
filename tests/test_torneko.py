"""トルネコ（kagra.torneko）の純ロジックテスト。

kagra_core / kagra_shared 非依存。保存は tmp_path を使い、ユーザーの
~/.kagra に触らない。決定論（seed → 同じ地図・同じ敵/アイテム配置・
同じロール）を重点的に検証する。
"""
from tests.conftest import load_kagra_submodule

tk = load_kagra_submodule("torneko")


def _game(tmp_path, seed=12345, start_floor=1):
    return tk.Torneko(seed=seed, save_path=tmp_path / "torneko.json", start_floor=start_floor)


# ── 決定論 ────────────────────────────────────────────────────────────────

def test_same_seed_same_layout(tmp_path):
    a = _game(tmp_path, seed=777)
    b = _game(tmp_path, seed=777)
    assert a.grid == b.grid, "同じ seed → 同じダンジョン"
    assert [(e["x"], e["y"]) for e in a.enemies] == [(e["x"], e["y"]) for e in b.enemies]
    assert [(i["x"], i["y"], i["name"]) for i in a.items] == [
        (i["x"], i["y"], i["name"]) for i in b.items
    ]


def test_different_seed_different_layout(tmp_path):
    a = _game(tmp_path, seed=1)
    b = _game(tmp_path, seed=2)
    assert a.grid != b.grid or a.stair_down != b.stair_down


def test_floor_seed_changes_layout(tmp_path):
    a = _game(tmp_path)
    b = _game(tmp_path, start_floor=2)
    assert a.grid != b.grid
    assert a._floor_seed() != b._floor_seed()


def test_same_seed_same_rolls(tmp_path):
    a = _game(tmp_path)
    b = _game(tmp_path)
    assert a.rng.random() == b.rng.random()
    assert a.rng.randint(0, 99) == b.rng.randint(0, 99)


# ── 移動 / 攻撃 ───────────────────────────────────────────────────────────

def test_wall_blocks_movement(tmp_path):
    g = _game(tmp_path)
    # 床に隣接する壁を探して、そこへ進めないこと
    px = py = wall_dir = None
    for r in range(tk.ROWS):
        for c in range(tk.COLS):
            if g.grid[r][c] != tk.DungeonTiles.FLOOR:
                continue
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                if g.tile(c + dx, r + dy) == tk.DungeonTiles.WALL:
                    px, py, wall_dir = c, r, (dx, dy)
                    break
            if wall_dir:
                break
        if wall_dir:
            break
    assert wall_dir is not None, "ダンジョンに壁隣接の床セルがある前提"
    g.player["x"], g.player["y"] = px, py
    g._try_move(*wall_dir)
    assert (g.player["x"], g.player["y"]) == (px, py), "壁には進めない"


def test_move_into_enemy_attacks(tmp_path):
    g = _game(tmp_path)
    foe = g.enemies[0]
    g.player["x"], g.player["y"] = foe["x"] + 1, foe["y"]
    hp0 = foe["hp"]
    g._try_move(-1, 0)
    assert foe["hp"] < hp0, "隣接敵に移動で攻撃"


def test_kill_removes_enemy(tmp_path):
    g = _game(tmp_path)
    foe = g.enemies[0]
    foe["hp"] = 1
    g.player["x"], g.player["y"] = foe["x"] + 1, foe["y"]
    g._try_move(-1, 0)
    assert g.enemy_at(foe["x"], foe["y"]) is None


def test_pickup_adds_inventory(tmp_path):
    g = _game(tmp_path)
    it = g.items[0]
    g.player["x"], g.player["y"] = it["x"] + 1, it["y"]
    g._try_move(-1, 0)
    assert it["name"] in g.inventory
    assert g.item_at(it["x"], it["y"]) is None


def test_enemy_turn_damages_player_when_adjacent(tmp_path):
    g = _game(tmp_path)
    g.player["x"], g.player["y"] = g.enemies[0]["x"] + 1, g.enemies[0]["y"]
    hp0 = g.player["hp"]
    g._enemy_turn()
    assert g.player["hp"] < hp0 or g.state == "dead"


def test_death_on_zero_hp(tmp_path):
    g = _game(tmp_path)
    g.player["hp"] = 1
    g.player["x"], g.player["y"] = g.enemies[0]["x"] + 1, g.enemies[0]["y"]
    for _ in range(20):
        g._enemy_turn()
        if g.state == "dead":
            break
    assert g.state == "dead"
    assert g.player["hp"] == 0


# ── アイテム ──────────────────────────────────────────────────────────────

def test_heal_item_restores_hp(tmp_path):
    g = _game(tmp_path)
    g.inventory = ["薬草"]
    g.player["hp"] = 5
    g._use_item(0)
    assert g.player["hp"] == 20
    assert g.inventory == []


def test_heal_refused_at_full_hp(tmp_path):
    g = _game(tmp_path)
    g.inventory = ["薬草"]
    g.player["hp"] = g.player["max_hp"]
    g._use_item(0)
    assert g.inventory == ["薬草"], "満タンなら消費しない"


def test_atk_seed_raises_attack(tmp_path):
    g = _game(tmp_path)
    g.inventory = ["ちからの種"]
    atk0 = g.player["atk"]
    g._use_item(0)
    assert g.player["atk"] == atk0 + 2


# ── 階 / ゴール ───────────────────────────────────────────────────────────

def test_descend_increments_floor(tmp_path):
    g = _game(tmp_path)
    f0 = g.floor
    g._descend()
    assert g.floor == f0 + 1
    assert g.player["x"], g.player["y"] == g.stair_up


def test_win_on_last_floor(tmp_path):
    g = _game(tmp_path)
    g.floor = tk.FLOORS_TO_WIN
    g._descend()
    assert g.state == "win"


# ── セーブ / ロード ───────────────────────────────────────────────────────

def test_save_load_roundtrip(tmp_path):
    g = _game(tmp_path)
    g.player["hp"] = 13
    g.inventory = ["薬草", "ちからの種"]
    # 数ターン敵を動かして状態を散らす
    for e in g.enemies:
        e["hp"] = max(1, e["hp"] - 2)
    g._save()
    h = _game(tmp_path)  # 同じ save パス → ロード
    assert h.seed == g.seed
    assert h.floor == g.floor
    assert h.player == g.player
    assert h.inventory == g.inventory
    assert [(e["x"], e["y"], e["hp"]) for e in h.enemies] == [
        (e["x"], e["y"], e["hp"]) for e in g.enemies
    ]


def test_reload_keeps_future_rolls(tmp_path):
    g = _game(tmp_path)
    g._save()
    h = _game(tmp_path)
    roll_a = g.rng.randint(0, 9999)
    roll_b = h.rng.randint(0, 9999)
    assert roll_a == roll_b, "RNG 状態も保存され、未来のロールが一致する"


def test_scripted_policy_progresses(tmp_path):
    g = _game(tmp_path)
    steps = tk.scripted_policy(g, 60)
    assert steps, "60 ターンで何かしら動く"
    assert len(g.log_lines) > 0


def test_esc_saves_and_quits(tmp_path):
    from tests.conftest import load_kagra_submodule

    gl = load_kagra_submodule("gameloop")
    g = _game(tmp_path)
    g.player["hp"] = 14
    gl._just.clear()
    gl._just.add("escape")
    g.update(1 / 60)
    assert not g.running, "ESC で終了"
    assert (tmp_path / "torneko.json").exists(), "ESC でセーブされる"
    h = _game(tmp_path)
    assert h.player["hp"] == 14, "再開で状態が復元される"


def test_on_close_saves(tmp_path):
    g = _game(tmp_path)
    g.player["hp"] = 9
    g.on_close()
    assert not g.running
    assert (tmp_path / "torneko.json").exists(), "窓を閉じてもセーブされる"
