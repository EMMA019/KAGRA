"""kagra.path（Phase 3 経路探索）の純ロジックテスト。

kagra_core / kagra_shared に依存しない。決定論（同入力 → 同経路）を検証する。
"""
from tests.conftest import load_kagra_submodule

path_mod = load_kagra_submodule("path")


def _grid(rows):
    """'#' 壁・'.' 床の 2D 文字列 → walkable 述語。"""
    cells = {c: r for r, line in enumerate(rows) for c, ch in enumerate(line)}
    w, h = len(rows[0]), len(rows)

    def walkable(x, y):
        if 0 <= x < w and 0 <= y < h:
            return rows[y][x] != "#"
        return False

    return walkable


def test_find_path_straight_line():
    g = _grid(["....."])
    p = path_mod.find_path(g, (0, 0), (4, 0), diagonal=False)
    assert p == [(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]


def test_find_path_around_wall():
    g = _grid(["...#.", "...#.", "....."])
    p = path_mod.find_path(g, (0, 0), (4, 0), diagonal=False)
    assert p is not None
    assert p[0] == (0, 0) and p[-1] == (4, 0)
    for x, y in p:
        assert g(x, y), "壁を通らない"


def test_find_path_unreachable_returns_none():
    g = _grid(["....#", "####.", "....."])
    # (0,0) は (4,0) と壁で隔てられている（(0,2) からも行けない）
    p = path_mod.find_path(g, (0, 0), (4, 2), diagonal=False)
    assert p is None


def test_find_path_start_equals_goal():
    g = _grid(["....."])
    assert path_mod.find_path(g, (2, 0), (2, 0)) == [(2, 0)]


def test_find_path_diagonal_shortcut():
    g = _grid(["....", "....", "....", "...."])
    straight = path_mod.find_path(g, (0, 0), (3, 3), diagonal=False)
    diag = path_mod.find_path(g, (0, 0), (3, 3), diagonal=True)
    assert straight is not None and diag is not None
    assert len(diag) < len(straight), "対角移動で近道できる"


def test_find_path_goal_may_be_blocked():
    # goal 自体は walkable でなくてもよい（敵マスに隣接して止まる用途）
    g = _grid(["....", ".#.."])

    def w(x, y):
        return g(x, y) and not (x == 3 and y == 0)

    p = path_mod.find_path(w, (0, 0), (3, 0), diagonal=False)
    assert p is not None and p[-1] == (3, 0), "ブロックされた goal にも経路を返す"


def test_move_range_ortho():
    g = _grid([".....", ".#...", "....."])
    r = path_mod.move_range(g, (0, 1), 2, diagonal=False)
    assert (0, 1) in r
    assert (0, 0) in r and (0, 2) in r
    assert (1, 0) in r and (1, 2) in r, "移動力 2 で 2 マス先まで届く"
    assert (1, 1) not in r, "壁は通れない"
    assert (2, 0) not in r, "3 歩は移動力 2 では届かない"


def test_move_range_diagonal_counts_more():
    g = _grid(["....", "....", "....", "...."])
    r = path_mod.move_range(g, (0, 0), 2, diagonal=True)
    assert (2, 0) in r and (0, 2) in r
    assert (2, 2) not in r, "対角 2 歩は 2.83 コストで移動力 2 を超える"


def test_move_range_cost_fn():
    g = _grid(["....", "...."])
    # 山（コスト 3）は遠回りさせる
    r = path_mod.move_range(g, (0, 0), 3, diagonal=False, cost_fn=lambda x, y: 3 if (x, y) == (1, 0) else 1)
    assert (1, 0) in r
    assert (2, 0) not in r, "コスト 3 の山を越えると移動力 3 では届かない"


def test_line_of_sight():
    g = _grid(["....", ".#..", "...."])
    assert path_mod.line_of_sight(g, (0, 1), (3, 1)) is False
    assert path_mod.line_of_sight(g, (0, 0), (3, 0)) is True


def test_simplify_drops_redundant_waypoints():
    g = _grid(["............", "............"])
    p = path_mod.find_path(g, (0, 0), (9, 0), diagonal=False)
    assert p is not None and len(p) == 10
    s = path_mod.simplify(p, g)
    assert s == [(0, 0), (9, 0)], "直線は 2 点に縮む"


def test_simplify_keeps_corners():
    # 中央の壁: 直線では通れず、一度下に降りて右へ抜ける必要がある
    g = _grid([".....", ".#...", ".....", "....."])
    p = path_mod.find_path(g, (0, 1), (4, 2), diagonal=False)
    assert p is not None
    s = path_mod.simplify(p, g)
    assert s[0] == p[0] and s[-1] == p[-1]
    assert len(s) >= 3, f"corner must survive, s={s}"
    # 全ウェイポイント間は LOS が通る（簡略化の不変条件）
    for a, b in zip(s, s[1:]):
        assert path_mod.line_of_sight(g, a, b), f"LOS {a}->{b}"


def test_deterministic_repeat():
    g = _grid(["....#....", "....#....", ".........", "#........", "....#...."])
    a = path_mod.find_path(g, (0, 0), (8, 4), diagonal=True)
    b = path_mod.find_path(g, (0, 0), (8, 4), diagonal=True)
    assert a == b, "同入力 → 同経路（決定論）"
    r1 = path_mod.move_range(g, (0, 0), 5)
    r2 = path_mod.move_range(g, (0, 0), 5)
    assert r1 == r2
