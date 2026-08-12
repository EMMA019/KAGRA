"""
othello.py - KAGRA オセロ（対人戦 + AI アドバイス）
=====================================================
操作:
  マウスクリック : 石を置く
  H キー        : AI ヒント ON/OFF
  R キー        : リセット
  ESC           : 終了

AI は各マスに点数を表示して「ここに置くといいよ！」を教えてくれる
"""
import kagra
import math

BOARD_SIZE = 8

def _calc_layout():
    sw = kagra.screen_w()
    sh = kagra.screen_h()
    size   = min(sw, sh)
    cell   = (size - 120) // 8
    offset = (size - cell * 8) // 2
    return sw, sh, cell, offset

# 色
C_BG      = (20,  20,  20)
C_BOARD   = (34,  139, 34)
C_LINE    = (0,   100, 0)
C_BLACK   = (20,  20,  20)
C_WHITE   = (245, 245, 245)
C_HINT_G  = (100, 255, 100)   # 良い手（緑）
C_HINT_Y  = (255, 220, 50)    # 普通（黄）
C_HINT_R  = (255, 100, 100)   # 悪い手（赤）
C_VALID   = (255, 255, 255)   # 置ける場所

EMPTY, BLACK, WHITE = 0, 1, 2
DIRS = [(-1,-1),(-1,0),(-1,1),(0,-1),(0,1),(1,-1),(1,0),(1,1)]

# ── 位置評価テーブル ─────────────────────────────────────────
SCORE_TABLE = [
    [100,-20, 10,  5,  5, 10,-20,100],
    [-20,-50, -2, -2, -2, -2,-50,-20],
    [ 10, -2,  5,  1,  1,  5, -2, 10],
    [  5, -2,  1,  1,  1,  1, -2,  5],
    [  5, -2,  1,  1,  1,  1, -2,  5],
    [ 10, -2,  5,  1,  1,  5, -2, 10],
    [-20,-50, -2, -2, -2, -2,-50,-20],
    [100,-20, 10,  5,  5, 10,-20,100],
]

def new_board():
    b = [[EMPTY]*8 for _ in range(8)]
    b[3][3] = WHITE; b[4][4] = WHITE
    b[3][4] = BLACK; b[4][3] = BLACK
    return b

def in_bounds(r, c):
    return 0 <= r < 8 and 0 <= c < 8

def flips(board, r, c, player):
    """(r,c) に player が置いたときにひっくり返るマス一覧"""
    opp = WHITE if player == BLACK else BLACK
    result = []
    for dr, dc in DIRS:
        line = []
        nr, nc = r+dr, c+dc
        while in_bounds(nr, nc) and board[nr][nc] == opp:
            line.append((nr, nc))
            nr += dr; nc += dc
        if line and in_bounds(nr, nc) and board[nr][nc] == player:
            result.extend(line)
    return result

def valid_moves(board, player):
    moves = []
    for r in range(8):
        for c in range(8):
            if board[r][c] == EMPTY and flips(board, r, c, player):
                moves.append((r, c))
    return moves

def apply_move(board, r, c, player):
    import copy
    b = copy.deepcopy(board)
    b[r][c] = player
    for fr, fc in flips(board, r, c, player):
        b[fr][fc] = player
    return b

def count_stones(board):
    black = sum(row.count(BLACK) for row in board)
    white = sum(row.count(WHITE) for row in board)
    return black, white

def evaluate_move(board, r, c, player):
    """手の評価点（位置 + ひっくり返す枚数 + 安定性）"""
    pos_score  = SCORE_TABLE[r][c]
    flip_count = len(flips(board, r, c, player))
    nb = apply_move(board, r, c, player)
    opp = WHITE if player == BLACK else BLACK
    opp_moves_after = len(valid_moves(nb, opp))
    my_moves_after  = len(valid_moves(nb, player))
    mobility = my_moves_after - opp_moves_after * 0.8
    return pos_score + flip_count * 2 + mobility * 1.5

def ai_best_move(board, player):
    moves = valid_moves(board, player)
    if not moves: return None
    return max(moves, key=lambda m: evaluate_move(board, m[0], m[1], player))

def score_label(score):
    """スコアを ★ / ☆ / △ で表示"""
    if score >= 15:  return "★★"
    if score >= 5:   return "★"
    if score >= 0:   return "☆"
    return "△"

def score_color(score):
    if score >= 15: return C_HINT_G
    if score >= 5:  return C_HINT_Y
    if score >= 0:  return C_HINT_Y
    return C_HINT_R


class OthelloScene(kagra.Scene):

    def on_enter(self):
        self.font  = kagra.assets.font("meiryo")
        self.board = new_board()
        self.turn  = BLACK
        self.hint  = True
        self.msg   = "黒の番です"
        self.game_over = False
        self.anim  = []
        self._drag = False
        self._confirm_reset = False  # リセット確認フラグ

    def _cell_at(self, mx, my):
        _, _, CELL, BOARD_OFF = _calc_layout()
        c = int((mx - BOARD_OFF) / CELL)
        r = int((my - BOARD_OFF) / CELL)
        if 0 <= r < 8 and 0 <= c < 8:
            return r, c
        return None, None

    def update(self, dt):
        # ESC も2回押しで終了（子供の誤操作防止）
        if kagra.pressed("ESCAPE"):
            if getattr(self, '_confirm_exit', False):
                raise SystemExit
            else:
                self._confirm_exit = True
                self._exit_timer   = 2.0  # 2秒以内に再度押す
                self.msg = "もう一度 ESC で終了"
        if getattr(self, '_confirm_exit', False):
            self._exit_timer = getattr(self, '_exit_timer', 0) - dt
            if self._exit_timer <= 0:
                self._confirm_exit = False
                if not self.game_over:
                    name = "黒" if self.turn == BLACK else "白"
                    self.msg = f"{name}の番です"
        if kagra.pressed("H"):
            self.hint = not self.hint
            self.msg  = f"ヒント {'ON' if self.hint else 'OFF'}"
        # リセット: Z で確認 → もう一度 Z で実行（誤操作防止）
        if kagra.pressed("Z"):
            if self._confirm_reset:
                self._reset()
                return
            else:
                self._confirm_reset = True
                self.msg = "もう一度 Z でリセット  / 他のキーでキャンセル"
        elif self._confirm_reset:
            self._confirm_reset = False
            if not self.game_over:
                b, w = count_stones(self.board)
                name = "黒" if self.turn == BLACK else "白"
                self.msg = f"{name}の番です"

        # アニメ更新
        self.anim = [(r,c,t-dt) for r,c,t in self.anim if t-dt > 0]

        if self.game_over: return

        # クリックで石を置く
        if kagra.mouse_pressed(kagra.MOUSE_LEFT):
            mx, my = kagra.mouse_pos()
            r, c   = self._cell_at(mx, my)
            if r is not None:
                self._try_place(r, c)

    def _reset(self):
        self.board = new_board()
        self.turn  = BLACK
        self.game_over = False
        self.msg   = "黒の番です"
        self.anim  = []

    def _try_place(self, r, c):
        if self.board[r][c] != EMPTY: return
        fl = flips(self.board, r, c, self.turn)
        if not fl: return

        # 石を置く
        self.board = apply_move(self.board, r, c, self.turn)
        for fr, fc in fl:
            self.anim.append((fr, fc, 0.3))

        # 手番交代
        opp  = WHITE if self.turn == BLACK else BLACK
        self.turn = opp
        vm   = valid_moves(self.board, self.turn)
        if not vm:
            # パス
            self.turn = BLACK if self.turn == WHITE else WHITE
            vm2 = valid_moves(self.board, self.turn)
            if not vm2:
                self._end_game()
                return
            name = "黒" if self.turn == BLACK else "白"
            self.msg = f"相手はパス！{name}の番です"
        else:
            name = "黒" if self.turn == BLACK else "白"
            self.msg = f"{name}の番です"

    def _end_game(self):
        self.game_over = True
        b, w = count_stones(self.board)
        if b > w:   self.msg = f"黒の勝ち！  {b} vs {w}"
        elif w > b: self.msg = f"白の勝ち！  {b} vs {w}"
        else:       self.msg = f"引き分け！  {b} vs {w}"

    def draw(self):
        SW, SH, CELL, BOARD_OFF = _calc_layout()
        kagra.cls(*C_BG)

        # ── 盤面の背景 ───────────────────────────────────────
        kagra.rect(BOARD_OFF-4, BOARD_OFF-4,
                   CELL*8+8, CELL*8+8, 0, 80, 0, 255)
        kagra.rect(BOARD_OFF, BOARD_OFF,
                   CELL*8, CELL*8, *C_BOARD, 255)

        # グリッド線
        for i in range(9):
            x = BOARD_OFF + i * CELL
            y = BOARD_OFF + i * CELL
            kagra.rect(x-1, BOARD_OFF, 2, CELL*8, *C_LINE, 200)
            kagra.rect(BOARD_OFF, y-1, CELL*8, 2, *C_LINE, 200)

        # ── AI ヒント ────────────────────────────────────────
        if self.hint and not self.game_over:
            moves = valid_moves(self.board, self.turn)
            scores = {(r,c): evaluate_move(self.board, r, c, self.turn)
                      for r, c in moves}
            best_score = max(scores.values()) if scores else 0

            for (r, c), sc in scores.items():
                x = BOARD_OFF + c * CELL + CELL//2
                y = BOARD_OFF + r * CELL + CELL//2
                cr, cg, cb = score_color(sc)
                # 背景丸
                kagra.rect(BOARD_OFF + c*CELL + 8,
                           BOARD_OFF + r*CELL + 8,
                           CELL-16, CELL-16, cr, cg, cb, 60)
                # スコアラベル
                lbl = score_label(sc)
                if sc == best_score:
                    lbl = "◎"
                    kagra.rect(BOARD_OFF + c*CELL + 4,
                               BOARD_OFF + r*CELL + 4,
                               CELL-8, CELL-8, cr, cg, cb, 100)
                kagra.draw_text(self.font, lbl,
                    x - 12, y - 10, 18, cr, cg, cb)
                # 数値
                kagra.draw_text(self.font, str(int(sc)),
                    x - 10, y + 8, 12, cr, cg, cb)

        # ── 石を描画 ─────────────────────────────────────────
        for r in range(8):
            for c in range(8):
                stone = self.board[r][c]
                if stone == EMPTY: continue
                x  = BOARD_OFF + c * CELL + CELL//2
                y  = BOARD_OFF + r * CELL + CELL//2
                rad = CELL//2 - max(4, CELL//10)

                # アニメ中は揺れる
                anim_t = next((t for ar,ac,t in self.anim if ar==r and ac==c), 0)
                scale  = 1.0 + math.sin(anim_t * 20) * 0.15 if anim_t > 0 else 1.0
                r2     = int(rad * scale)

                if stone == BLACK:
                    kagra.circle(x, y, r2, *C_BLACK)
                    kagra.circle(x - r2//4, y - r2//4, r2//4, 80, 80, 80, 160)
                else:
                    kagra.circle(x, y, r2, *C_WHITE)
                    kagra.circle(x - r2//4, y - r2//4, r2//4, 255, 255, 255, 200)

        # ── UI ──────────────────────────────────────────────
        kagra.rect(0, 0, SW, BOARD_OFF-4, 0, 0, 0, 200)
        b, w = count_stones(self.board)
        kagra.rect(10, 8, 36, 36, *C_BLACK, 255)
        kagra.draw_text(self.font, f"黒 {b}", 52, 14, 22, 220, 220, 220)
        kagra.rect(SW//2 - 60, 8, 36, 36, *C_WHITE, 255)
        kagra.draw_text(self.font, f"白 {w}", SW//2 - 18, 14, 22, 220, 220, 220)
        tc = C_BLACK if self.turn == BLACK else C_WHITE
        if not self.game_over:
            kagra.rect(SW-100, 10, 32, 32, *tc, 255)
        bot = BOARD_OFF + CELL*8 + 4
        kagra.rect(0, bot, SW, SH - bot, 0, 0, 0, 200)
        kagra.draw_text(self.font, self.msg, 20, bot + 8, 22, 255, 230, 100)
        kagra.draw_text(self.font, "H:ヒント  Z→Z:リセット  ESC:終了",
                        20, bot + 34, 15, 140, 140, 160)
        if self.hint:
            kagra.draw_text(self.font, "◎=最善手  ★=良い  ☆=普通  △=悪い",
                SW//2 - 160, bot + 34, 14, 180, 220, 180)


if __name__ == "__main__":
    kagra.init(720, 720, "KAGRA オセロ - AI ヒント付き", 60)
    kagra.run(start_scene=OthelloScene())