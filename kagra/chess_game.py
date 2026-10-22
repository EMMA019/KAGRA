"""スタジオ品質 3D チェス（PolyHaven 彫刻駒 + PBR 大理石/黒檀/真鍮）— ロジックは全部 Python。

``assets/library/polyhaven/props/chess_set/chess_set_1k.gltf`` に含まれる
33 個の高精細彫刻メッシュ（キング・クイーン・ルーク・ビショップ・ナイト・ポーン）を
``chess_king`` / ``chess_queen`` 等の別名でロードし、GGX スペキュラ + フレネル環境反射で
艶やかな大理石・黒檀・ゴールドの質感として描画する。

- 矢印キー / WASD: カーソル移動（またはカメラ回転 ``Q`` / ``E``）
- Space / Enter / J / Z: 駒の選択・移動（合法手ハイライト付き）
- 黒番は簡易 AI（駒得 + 中央支配評価）が自動で応手する
"""
from __future__ import annotations

import math
from typing import Any

from kagra.audio import se
from kagra.gameloop import Scene, pressed, render_rgba, was_pressed
from kagra.ui2d import list_lines, merge, panel

W, H = 800, 450
SQ = 0.58  # 1 マスのワールド幅
PIECE_SCALE = 8.8  # PolyHaven 実寸 (約 0.06m) をスタジオ盤面サイズに拡大

PIECE_GLTF = {
    "K": "chess_king",
    "Q": "chess_queen",
    "R": "chess_rook",
    "B": "chess_bishop",
    "N": "chess_knight",
    "P": "chess_pawn",
}

PIECE_VALUE = {"P": 10, "N": 30, "B": 32, "R": 50, "Q": 90, "K": 900}


def initial_board() -> list[list[str]]:
    """8x8 盤面。row 0 が黒陣（奥、8 段目）、row 7 が白陣（手前、1 段目）。"""
    return [
        ["bR", "bN", "bB", "bQ", "bK", "bB", "bN", "bR"],
        ["bP", "bP", "bP", "bP", "bP", "bP", "bP", "bP"],
        ["", "", "", "", "", "", "", ""],
        ["", "", "", "", "", "", "", ""],
        ["", "", "", "", "", "", "", ""],
        ["", "", "", "", "", "", "", ""],
        ["wP", "wP", "wP", "wP", "wP", "wP", "wP", "wP"],
        ["wR", "wN", "wB", "wQ", "wK", "wB", "wN", "wR"],
    ]


def default_state() -> dict[str, Any]:
    return {
        "board": initial_board(),
        "turn": "w",
        "cursor": [6, 4],  # e2
        "selected": None,
        "legal": [],
        "last_move": None,
        "move_count": 0,
        "captured_w": 0,
        "captured_b": 0,
        "cam_angle": 0.32,
        "cam_pitch": 0.62,
        "status": "WHITE TO MOVE — SELECT A PIECE",
    }


def _in_bounds(r: int, c: int) -> bool:
    return 0 <= r < 8 and 0 <= c < 8


def legal_moves_for(board: list[list[str]], r: int, c: int) -> list[tuple[int, int]]:
    piece = board[r][c]
    if not piece:
        return []
    side, kind = piece[0], piece[1]
    out: list[tuple[int, int]] = []

    def add_ray(dr: int, dc: int) -> None:
        nr, nc = r + dr, c + dc
        while _in_bounds(nr, nc):
            target = board[nr][nc]
            if not target:
                out.append((nr, nc))
            else:
                if target[0] != side:
                    out.append((nr, nc))
                break
            nr += dr
            nc += dc

    if kind == "P":
        step = -1 if side == "w" else 1
        start_r = 6 if side == "w" else 1
        if _in_bounds(r + step, c) and not board[r + step][c]:
            out.append((r + step, c))
            if r == start_r and _in_bounds(r + 2 * step, c) and not board[r + 2 * step][c]:
                out.append((r + 2 * step, c))
        for dc in (-1, 1):
            nr, nc = r + step, c + dc
            if _in_bounds(nr, nc) and board[nr][nc] and board[nr][nc][0] != side:
                out.append((nr, nc))
    elif kind == "N":
        for dr, dc in ((-2, -1), (-2, 1), (-1, -2), (-1, 2), (1, -2), (1, 2), (2, -1), (2, 1)):
            nr, nc = r + dr, c + dc
            if _in_bounds(nr, nc) and (not board[nr][nc] or board[nr][nc][0] != side):
                out.append((nr, nc))
    elif kind == "B":
        for dr, dc in ((-1, -1), (-1, 1), (1, -1), (1, 1)):
            add_ray(dr, dc)
    elif kind == "R":
        for dr, dc in ((-1, 0), (1, 0), (0, -1), (0, 1)):
            add_ray(dr, dc)
    elif kind == "Q":
        for dr in (-1, 0, 1):
            for dc in (-1, 0, 1):
                if dr or dc:
                    add_ray(dr, dc)
    elif kind == "K":
        for dr in (-1, 0, 1):
            for dc in (-1, 0, 1):
                if dr or dc:
                    nr, nc = r + dr, c + dc
                    if _in_bounds(nr, nc) and (not board[nr][nc] or board[nr][nc][0] != side):
                        out.append((nr, nc))
    return out


def apply_move(state: dict[str, Any], fr: tuple[int, int], to: tuple[int, int]) -> bool:
    board = state["board"]
    r0, c0 = fr
    r1, c1 = to
    piece = board[r0][c0]
    if not piece:
        return False
    target = board[r1][c1]
    if target:
        if piece[0] == "w":
            state["captured_b"] += 1
        else:
            state["captured_w"] += 1
    board[r1][c1] = piece
    board[r0][c0] = ""
    # ポーン昇格
    if piece == "wP" and r1 == 0:
        board[r1][c1] = "wQ"
    elif piece == "bP" and r1 == 7:
        board[r1][c1] = "bQ"
    state["last_move"] = (r0, c0, r1, c1)
    state["move_count"] += 1
    state["selected"] = None
    state["legal"] = []
    files = "abcdefgh"
    move_str = f"{piece[1]}{files[c0]}{8 - r0}->{files[c1]}{8 - r1}"
    state["turn"] = "b" if piece[0] == "w" else "w"
    who = "WHITE" if state["turn"] == "w" else "BLACK"
    state["status"] = f"{move_str}  |  {who} TO MOVE"
    return bool(target)


def step_black_ai(state: dict[str, Any]) -> bool:
    """黒番 AI: 駒得 + 中央進出が最も高い手を選ぶ。"""
    if state["turn"] != "b":
        return False
    board = state["board"]
    best_score = -1e9
    best_move: tuple[tuple[int, int], tuple[int, int]] | None = None
    for r in range(8):
        for c in range(8):
            p = board[r][c]
            if not p or p[0] != "b":
                continue
            for nr, nc in legal_moves_for(board, r, c):
                target = board[nr][nc]
                gain = PIECE_VALUE.get(target[1], 0) if target else 0
                center = 4.0 - (abs(3.5 - nr) + abs(3.5 - nc))
                advance = nr * 0.4
                score = gain * 10.0 + center * 1.5 + advance
                if score > best_score:
                    best_score = score
                    best_move = ((r, c), (nr, nc))
    if best_move is None:
        return False
    return apply_move(state, best_move[0], best_move[1])


def _sq_xz(r: int, c: int) -> tuple[float, float]:
    return ((c - 3.5) * SQ, (r - 3.5) * SQ)


def make_world(state: dict[str, Any]) -> dict[str, Any]:
    """スタジオ照明 + 光沢マホガニー外枠 + 大理石/黒檀マスのチェス盤と PolyHaven 彫刻駒。"""
    props: list[dict[str, Any]] = []

    # スタジオテーブル＆真鍮フレーム外枠
    props.extend(
        [
            {
                "id": "prop:studio_floor",
                "type": "prop",
                "name": "studio_floor",
                "model": "box",
                "position": [0.0, -0.28, 0.0],
                "scale": [18.0, 0.20, 18.0],
                "color": [22, 26, 34],
                "metallic": 0.15,
                "roughness": 0.28,
                "enabled": True,
            },
            {
                "id": "prop:board_frame",
                "type": "prop",
                "name": "board_frame",
                "model": "box",
                "position": [0.0, -0.09, 0.0],
                "scale": [SQ * 8.9, 0.16, SQ * 8.9],
                "color": [74, 38, 22],
                "metallic": 0.08,
                "roughness": 0.18,
                "enabled": True,
            },
            {
                "id": "prop:brass_trim",
                "type": "prop",
                "name": "brass_trim",
                "model": "box",
                "position": [0.0, -0.06, 0.0],
                "scale": [SQ * 8.32, 0.14, SQ * 8.32],
                "color": [232, 184, 78],
                "metallic": 0.92,
                "roughness": 0.14,
                "enabled": True,
            },
        ]
    )

    cur_r, cur_c = state["cursor"]
    sel = state["selected"]
    legal_set = set(state.get("legal", []))
    last = state.get("last_move")

    # 8x8 の大理石＆ウォルナット盤面タイル（ツヤのある低ラフネス）
    for r in range(8):
        for c in range(8):
            x, z = _sq_xz(r, c)
            light_sq = (r + c) % 2 == 0
            if (r, c) == (cur_r, cur_c):
                col = [96, 210, 255]
                rough = 0.10
                met = 0.35
            elif sel and (r, c) == tuple(sel):
                col = [255, 214, 76]
                rough = 0.12
                met = 0.45
            elif (r, c) in legal_set:
                col = [92, 232, 148]
                rough = 0.14
                met = 0.30
            elif last and ((r, c) == (last[0], last[1]) or (r, c) == (last[2], last[3])):
                col = [238, 198, 118] if light_sq else [132, 92, 54]
                rough = 0.14
                met = 0.12
            elif light_sq:
                col = [238, 228, 210]
                rough = 0.14
                met = 0.05
            else:
                col = [68, 44, 32]
                rough = 0.16
                met = 0.08

            props.append(
                {
                    "id": f"prop:tile_{r}_{c}",
                    "type": "prop",
                    "name": "tile",
                    "model": "box",
                    "position": [x, 0.0, z],
                    "scale": [SQ * 0.985, 0.06, SQ * 0.985],
                    "color": col,
                    "metallic": met,
                    "roughness": rough,
                    "enabled": True,
                }
            )

    # 32 個の彫刻チェス駒（白 = アイボリー大理石、黒 = 黒檀＆ブロンズ光沢）
    board = state["board"]
    for r in range(8):
        for c in range(8):
            piece = board[r][c]
            if not piece:
                continue
            side, kind = piece[0], piece[1]
            x, z = _sq_xz(r, c)
            is_white = side == "w"
            col = [248, 240, 222] if is_white else [46, 52, 66]
            met = 0.12 if is_white else 0.58
            rough = 0.14 if is_white else 0.16
            yaw = math.pi if is_white else 0.0
            lift = 0.12 if (sel and (r, c) == tuple(sel)) else 0.03
            props.append(
                {
                    "id": f"prop:piece_{r}_{c}",
                    "type": "prop",
                    "name": f"piece_{side}_{kind}",
                    "model": "box",
                    "gltf": PIECE_GLTF[kind],
                    "position": [x, lift, z],
                    "yaw": yaw,
                    "scale": [PIECE_SCALE, PIECE_SCALE, PIECE_SCALE],
                    "color": col,
                    "metallic": met,
                    "roughness": rough,
                    "enabled": True,
                }
            )

    angle = state.get("cam_angle", 0.32)
    pitch = state.get("cam_pitch", 0.62)
    dist = 5.4
    cam_x = math.sin(angle) * math.cos(pitch) * dist
    cam_y = math.sin(pitch) * dist + 0.4
    cam_z = math.cos(angle) * math.cos(pitch) * dist

    return {
        "version": 1,
        "half": 16.0,
        "floor_y": -0.38,
        "ground": False,
        "sky": [20, 25, 36, 255],
        "ibl": 0.48,
        "exposure": 1.12,
        "tonemap": True,
        "props": props,
        "lights": [
            {
                "id": "light:0",
                "type": "light",
                "name": "key_warm",
                "position": [4.5, 6.5, 4.2],
                "kind": "point",
                "slot": 0,
                "intensity": 3.2,
                "radius": 24.0,
                "color": [1.0, 0.95, 0.86],
            },
            {
                "id": "light:1",
                "type": "light",
                "name": "rim_cool",
                "position": [-5.0, 4.8, -4.5],
                "kind": "point",
                "slot": 1,
                "intensity": 1.8,
                "radius": 22.0,
                "color": [0.62, 0.80, 1.0],
            },
            {
                "id": "light:2",
                "type": "light",
                "name": "gold_accent",
                "position": [0.0, 3.8, -5.2],
                "kind": "point",
                "slot": 2,
                "intensity": 1.2,
                "radius": 18.0,
                "color": [1.0, 0.82, 0.55],
            },
        ],
        "cameras": [
            {
                "id": "camera:main",
                "type": "camera",
                "name": "studio",
                "position": [cam_x, cam_y, cam_z],
                "target": [0.0, 0.18, 0.15],
                "fov": 40,
            }
        ],
    }


class StudioChess(Scene):
    def __init__(self) -> None:
        super().__init__()
        self.width = W
        self.height = H
        self.state = default_state()
        self.world = make_world(self.state)

    def update(self, dt: float) -> None:
        if was_pressed("escape"):
            self.quit()
            return
        if pressed("a"):
            self.state["cam_angle"] -= 0.9 * dt
        if pressed("d"):
            self.state["cam_angle"] += 0.9 * dt
        if pressed("w"):
            self.state["cam_pitch"] = min(1.15, self.state["cam_pitch"] + 0.6 * dt)
        if pressed("s"):
            self.state["cam_pitch"] = max(0.28, self.state["cam_pitch"] - 0.6 * dt)

        r, c = self.state["cursor"]
        if was_pressed("up"):
            r = (r - 1) % 8
        elif was_pressed("down"):
            r = (r + 1) % 8
        elif was_pressed("left"):
            c = (c - 1) % 8
        elif was_pressed("right"):
            c = (c + 1) % 8
        self.state["cursor"] = [r, c]

        if any(was_pressed(k) for k in ("space", "enter", "j", "z")):
            self._activate_square(r, c)

        self.world = make_world(self.state)

    def _activate_square(self, r: int, c: int) -> None:
        board = self.state["board"]
        sel = self.state["selected"]
        if sel is not None and (r, c) in self.state["legal"]:
            captured = apply_move(self.state, tuple(sel), (r, c))
            se("hit" if captured else "ok")
            if self.state["turn"] == "b":
                step_black_ai(self.state)
            return
        piece = board[r][c]
        if piece and piece[0] == "w":
            self.state["selected"] = [r, c]
            self.state["legal"] = legal_moves_for(board, r, c)
            se("ok")
        else:
            self.state["selected"] = None
            self.state["legal"] = []

    def draw(self) -> None:
        hud = merge(
            panel(14, 14, 380, 48, color=(14, 18, 28, 220), border=(228, 182, 78, 255)),
            list_lines(
                [
                    f"KAGRA STUDIO CHESS   |   MOVE {self.state['move_count']}",
                    self.state["status"],
                ],
                x=26,
                y=20,
                size=13,
                color=[245, 240, 225, 255],
            ),
            panel(14, self.height - 34, 430, 24, color=(14, 18, 28, 200), border=(90, 120, 165, 220)),
            list_lines(
                ["Arrows: Square   Space: Move   WASD: Orbit Camera"],
                x=24,
                y=self.height - 29,
                size=12,
                color=[215, 228, 245, 240],
            ),
        )
        self._frame_rgba = render_rgba(self.world, self.width, self.height, hud=hud)


def scripted_opening(state: dict[str, Any] | None = None) -> dict[str, Any]:
    """イタリアン・ゲーム風の数手を進めて白ナイト f3 を選択状態にする。"""
    state = default_state() if state is None else state
    # 1. e4 (e2 -> e4) + 黒 AI 応手
    apply_move(state, (6, 4), (4, 4))
    step_black_ai(state)
    # 2. Nf3 (g1 -> f3) + 黒 AI 応手
    apply_move(state, (7, 6), (5, 5))
    step_black_ai(state)
    # 3. Bc4 (f1 -> c4) + 黒 AI 応手
    apply_move(state, (7, 5), (4, 2))
    step_black_ai(state)
    # 次の手で白クイーン d1 またはナイト f3 を選択して合法手を光らせる
    state["selected"] = [5, 5]
    state["cursor"] = [3, 4]
    state["legal"] = legal_moves_for(state["board"], 5, 5)
    state["status"] = "KNIGHT f3 SELECTED — 5 LEGAL MOVES"
    return state
