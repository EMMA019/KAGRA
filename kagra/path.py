"""kagra/path.py — 汎用経路探索 (Phase 3).

ゲームロジックは Python のみ。トルネコの敵追跡・階段探索、SLG の移動範囲
（move_range）が共有する、GPU 不要・決定論的な 2D グリッド経路探索。

- ``find_path``    A*。障害物を避けて start→goal の最短経路を返す。
- ``move_range``   SLG の移動範囲。移動力 budget 内で到達できるマス集合。
- ``line_of_sight`` 2 点間が直線で通れるか（射線 / 経路簡略化）。
- ``simplify``     LOS で冗長なウェイポイントを削る（string pulling 簡易版）。

全て純関数（乱数なし）。同じ入力 → 同じ出力。
"""
from __future__ import annotations

from heapq import heappop, heappush
from typing import Callable, Iterable, Optional

Cell = tuple[int, int]

# 4 近傍
ORTHO = ((1, 0), (-1, 0), (0, 1), (0, -1))
# 8 近傍（対角込み）
DIAG = ORTHO + ((1, 1), (1, -1), (-1, 1), (-1, -1))

Walkable = Callable[[int, int], bool]


def neighbors(x: int, y: int, *, diagonal: bool = True) -> Iterable[Cell]:
    """(x, y) の近傍セル。``diagonal`` で対角 4 マスを含める。"""
    dirs = DIAG if diagonal else ORTHO
    for dx, dy in dirs:
        yield (x + dx, y + dy)


def _in_bounds(cell: Cell, w: int, h: int) -> bool:
    return 0 <= cell[0] < w and 0 <= cell[1] < h


def find_path(
    walkable: Walkable,
    start: Cell,
    goal: Cell,
    *,
    diagonal: bool = True,
    max_steps: int = 10_000,
) -> Optional[list[Cell]]:
    """A* で start..goal の最短経路（両端含む）。到達不可は None。

    ``walkable(x, y)`` が False のセルは通れない。start と goal 自体は
    walkable でなくてもよい（敵マスに隣接して止まる等、呼び出し側が
    goal を「歩けないマス」に置く用途のため）。4/8 近傍のコストは 1。
    決定論: 同 f 値は挿入順（辞書順）で安定。
    """
    if start == goal:
        return [start]
    open_heap: list[tuple[float, int, Cell]] = []
    counter = 0
    heappush(open_heap, (0.0, counter, start))
    g_score: dict[Cell, float] = {start: 0.0}
    came_from: dict[Cell, Cell] = {}

    def h(c: Cell) -> float:
        dx = abs(c[0] - goal[0])
        dy = abs(c[1] - goal[1])
        if diagonal:
            return max(dx, dy) + 0.41421 * min(dx, dy)
        return float(dx + dy)

    while open_heap:
        if len(g_score) > max_steps:
            return None
        _, _, cur = heappop(open_heap)
        if cur == goal:
            break
        for n in neighbors(*cur, diagonal=diagonal):
            if n != goal and not walkable(*n):
                continue
            step = 1.41421 if diagonal and n[0] != cur[0] and n[1] != cur[1] else 1.0
            tentative = g_score[cur] + step
            if tentative < g_score.get(n, float("inf")):
                g_score[n] = tentative
                came_from[n] = cur
                counter += 1
                heappush(open_heap, (tentative + h(n), counter, n))
    if goal not in came_from:
        return None
    path = [goal]
    cur = goal
    while cur != start:
        cur = came_from[cur]
        path.append(cur)
    path.reverse()
    return path


def move_range(
    walkable: Walkable,
    start: Cell,
    budget: int,
    *,
    diagonal: bool = True,
    cost_fn: Optional[Callable[[int, int], float]] = None,
) -> set[Cell]:
    """SLG の移動範囲。移動力 ``budget`` で到達できるマス集合（start 含む）。

    ``cost_fn(x, y)`` は地形コスト（デフォルト 1）。0 以下のマスは通れない。
    Dijkstra 的 BFS。決定論的。
    """
    cost_of = cost_fn or (lambda x, y: 1.0)
    if budget <= 0:
        return {start}
    reachable: set[Cell] = {start}
    frontier: list[tuple[float, int, Cell]] = []
    counter = 0
    heappush(frontier, (0.0, counter, start))
    dist: dict[Cell, float] = {start: 0.0}
    while frontier:
        _, _, cur = heappop(frontier)
        if dist[cur] >= budget:
            continue
        for n in neighbors(*cur, diagonal=diagonal):
            if n in dist:
                continue
            if not walkable(*n):
                continue
            step = 1.41421 if diagonal and n[0] != cur[0] and n[1] != cur[1] else 1.0
            c = cost_of(*n)
            if c <= 0:
                continue
            nd = dist[cur] + step * c
            if nd > budget:
                continue
            dist[n] = nd
            reachable.add(n)
            counter += 1
            heappush(frontier, (nd, counter, n))
    return reachable


def line_of_sight(walkable: Walkable, a: Cell, b: Cell) -> bool:
    """Bresenham で a→b の直線上の全セルが walkable か。両端は判定しない。

    SLG の射線・攻撃範囲、``simplify`` の直線化チェックに使う。
    """
    x0, y0 = a
    x1, y1 = b
    dx = abs(x1 - x0)
    dy = -abs(y1 - y0)
    sx = 1 if x0 < x1 else -1
    sy = 1 if y0 < y1 else -1
    err = dx + dy
    while True:
        if (x0, y0) == b:
            return True
        if (x0, y0) != a and not walkable(x0, y0):
            return False
        e2 = 2 * err
        if e2 >= dy:
            err += dy
            x0 += sx
        if e2 <= dx:
            err += dx
            y0 += sy


def simplify(path: list[Cell], walkable: Walkable) -> list[Cell]:
    """LOS で通れる部分を飛ばし、ウェイポイントを減らす。

    先頭から貪欲に「直線で到達できる最遠点」へジャンプする（string pulling
    の簡易版）。隣接 2 点は必ず残る。経路が 2 点以下ならそのまま。
    """
    if len(path) <= 2:
        return list(path)
    out: list[Cell] = [path[0]]
    i = 0
    while i < len(path) - 1:
        j = len(path) - 1
        while j > i + 1 and not line_of_sight(walkable, path[i], path[j]):
            j -= 1
        out.append(path[j])
        i = j
    return out
