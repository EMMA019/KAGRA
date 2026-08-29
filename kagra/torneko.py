"""トルネコライク・ミニマルローグライク — ゲームロジックは全部 Python。

- ダンジョン: `MapGen.dungeon(cols, rows, seed=...)`（同じ seed → 同じ部屋）
- 決定論: 敵配置・アイテム配置・戦闘ロールも seed + 階数から決まる
  （`random.Random(floor_seed)`）。再現: `--seed 12345 --floor 1` で同じ地図。
- ターン制: プレイヤーが動く/攻撃 → 敵が動く。敵 AI は近接攻撃 / 接近 /
  低速徘徊。
- 在庫: 薬草（回復）/ ちからの種（攻撃+2）。X でメニュー、Z で使う。
- セーブ: JSON スナップショット（seed / 階数 / 位置 / HP / 在庫 / RNG 状態）。
  ロードで完全再現（未来のロールまで一致）。
- ゴール: 各階の下り階段を目指し、3 階を脱出する。
- 世界はデータ: ターン制なので状態変化時だけ dump dict を作り直して
  `draw_world` で描く（毎フレームの描画はしない）。

実行は `examples/torneko_minimal.py`（窓 / ヘッドレス verify）。
"""
from __future__ import annotations

import json
import random
from pathlib import Path

from kagra.audio import se  # noqa: F401
from kagra.gameloop import Scene, draw_world, pressed, was_pressed
from kagra.mapgen import DungeonTiles, MapGen
from kagra.ui2d import bar, choice_menu, list_lines, merge, message

W, H = 480, 300
# MapGen.dungeon は部屋サイズを randint(4, cols//4) で取るため 16 以上必要。
COLS, ROWS = 17, 17
FLOORS_TO_WIN = 3
SAVE_DEFAULT = Path.home() / ".kagra" / "torneko.json"

ITEMS = {
    "薬草": {"heal": 15, "desc": "HP を 15 回復"},
    "ちからの種": {"atk": 2, "desc": "攻撃力 +2"},
}


class Torneko(Scene):
    """ターン制ローグライク。state: play | menu | dead | win"""

    def __init__(
        self,
        seed: int = 12345,
        save_path: Path | None = None,
        start_floor: int = 1,
    ) -> None:
        super().__init__()
        self.width, self.height = W, H
        self.seed = int(seed)
        self.save_path = Path(save_path) if save_path else SAVE_DEFAULT
        self.floor = max(1, start_floor)
        self.player = {"x": 0, "y": 0, "hp": 20, "max_hp": 20, "atk": 4}
        self.inventory: list[str] = []
        self.log_lines: list[str] = []
        self.state = "play"
        self.sel = 0
        self.facing = (1, 0)
        self.world: dict = {}
        self._dirty = True
        saved = self._load() if start_floor <= 1 and self.save_path.exists() else None
        if saved:
            self._restore(saved)
        else:
            self._gen_floor()
            self._push_log("B1F に降り立った")

    # ── セーブ / ロード ───────────────────────────────────────────────────

    def _save_dict(self) -> dict:
        return {
            "seed": self.seed,
            "floor": self.floor,
            "player": self.player,
            "inventory": self.inventory,
            "enemies": self.enemies,
            "items": self.items,
            "rng_state": list(self.rng.getstate()[1]),
        }

    def _save(self) -> None:
        self.save_path.parent.mkdir(parents=True, exist_ok=True)
        self.save_path.write_text(
            json.dumps(self._save_dict(), ensure_ascii=False, indent=1), encoding="utf-8"
        )

    def _load(self) -> dict | None:
        try:
            return json.loads(self.save_path.read_text(encoding="utf-8"))
        except Exception:
            return None

    def _restore(self, d: dict) -> None:
        self.seed = int(d["seed"])
        self.floor = int(d["floor"])
        self.player = dict(d["player"])
        self.inventory = list(d["inventory"])
        self.rng = random.Random()
        self.rng.setstate((3, tuple(d["rng_state"]), None))
        self._gen_layout()
        self.enemies = [dict(e) for e in d["enemies"]]
        self.items = [dict(i) for i in d["items"]]
        self._push_log(f"B{self.floor}F から再開")

    # ── フロア生成（決定論） ──────────────────────────────────────────────

    def _floor_seed(self) -> int:
        return (self.seed * 1009 + self.floor * 7919) & 0x7FFFFFFF

    def _gen_floor(self) -> None:
        self.rng = random.Random(self._floor_seed())
        self._gen_layout()
        # 敵: 6 + 階数
        pool = [
            (c, r)
            for r in range(ROWS)
            for c in range(COLS)
            if self.grid[r][c] == DungeonTiles.FLOOR
            and (c, r) not in (self.stair_up, self.stair_down)
        ]
        rng = self.rng
        rng.shuffle(pool)
        self.enemies = []
        for _ in range(min(6 + self.floor, len(pool))):
            c, r = pool.pop()
            self.enemies.append(
                {"x": c, "y": r, "hp": 6 + self.floor, "atk": 2 + self.floor // 2, "name": "スライム"}
            )
        # アイテム: 薬草 x3、種 x1
        self.items = []
        for name, n in (("薬草", 3), ("ちからの種", 1)):
            for _ in range(n):
                if pool:
                    c, r = pool.pop()
                    self.items.append({"x": c, "y": r, "name": name})
        self._push_log(f"B{self.floor}F のダンジョン（seed {self._floor_seed()}）")

    def _gen_layout(self) -> None:
        """地図だけ再生成（ロード時: 敵/アイテムは保存値で復元）。"""
        g, centers, up, down = MapGen.dungeon(
            COLS, ROWS, seed=self._floor_seed(), min_rooms=4, max_rooms=6
        )
        self.grid = g
        self.stair_up, self.stair_down = up, down
        self.player["x"], self.player["y"] = up
        self._dirty = True

    # ── 地図ヘルパ ────────────────────────────────────────────────────────

    def tile(self, c: int, r: int) -> int:
        if 0 <= c < COLS and 0 <= r < ROWS:
            return self.grid[r][c]
        return DungeonTiles.WALL

    def is_walkable(self, c: int, r: int) -> bool:
        return self.tile(c, r) in (
            DungeonTiles.FLOOR,
            DungeonTiles.DOOR,
            DungeonTiles.STAIR_UP,
            DungeonTiles.STAIR_DOWN,
            DungeonTiles.CHEST,
        )

    def enemy_at(self, c: int, r: int):
        for e in self.enemies:
            if e["hp"] > 0 and e["x"] == c and e["y"] == r:
                return e
        return None

    def item_at(self, c: int, r: int):
        for i in self.items:
            if i["x"] == c and i["y"] == r:
                return i
        return None

    def _push_log(self, line: str) -> None:
        self.log_lines.append(line)
        self.log_lines = self.log_lines[-5:]
        self._dirty = True

    # ── プレイヤーターン ──────────────────────────────────────────────────

    def _try_move(self, dx: int, dy: int) -> None:
        self.facing = (dx, dy)
        nx, ny = self.player["x"] + dx, self.player["y"] + dy
        if not self.is_walkable(nx, ny):
            self._push_log("壁にぶつかった")
            return
        foe = self.enemy_at(nx, ny)
        if foe:
            self._attack_foe(foe)
            self._enemy_turn()
            return
        it = self.item_at(nx, ny)
        if it:
            self._pickup(it)
        self.player["x"], self.player["y"] = nx, ny
        if (nx, ny) == self.stair_down:
            self._descend()
        elif (nx, ny) == self.stair_up and self.floor > 1:
            self.floor -= 1
            self._gen_floor()
            self._push_log("階段を上った")
        else:
            if self.tile(nx, ny) == DungeonTiles.CHEST:
                self._open_chest(nx, ny)
            self._enemy_turn()
        self._dirty = True

    def _attack_foe(self, foe: dict) -> None:
        dmg = max(1, self.player["atk"] + self.rng.randint(-1, 1))
        foe["hp"] -= dmg
        se("hit")
        self._push_log(f"{foe['name']}に {dmg} ダメージ！")
        if foe["hp"] <= 0:
            self._push_log(f"{foe['name']}を倒した！")
            se("coin")
            self.enemies = [e for e in self.enemies if e is not foe]

    def _pickup(self, it: dict) -> None:
        self.inventory.append(it["name"])
        self.items = [i for i in self.items if i is not it]
        se("ok")
        self._push_log(f"{it['name']}を手に入れた！（X で使う）")

    def _open_chest(self, c: int, r: int) -> None:
        name = self.rng.choice(["薬草", "薬草", "ちからの種"])
        self.inventory.append(name)
        self.grid[r][c] = DungeonTiles.FLOOR
        se("coin")
        self._push_log(f"宝箱を開けた！ {name}を手に入れた！")

    def _descend(self) -> None:
        if self.floor >= FLOORS_TO_WIN:
            self.state = "win"
            se("cast")
            self._push_log("ダンジョンを脱出した！ 大成功！")
            self._save()
            return
        self.floor += 1
        self._gen_floor()
        se("cast")
        self._push_log(f"階段を降りた。B{self.floor}F")

    # ── 敵ターン ──────────────────────────────────────────────────────────

    def _enemy_turn(self) -> None:
        for e in self.enemies:
            if e["hp"] <= 0:
                continue
            dx = self.player["x"] - e["x"]
            dy = self.player["y"] - e["y"]
            dist = abs(dx) + abs(dy)
            if dist == 1:
                dmg = max(1, e["atk"] + self.rng.randint(-1, 1))
                self.player["hp"] -= dmg
                se("hurt")
                self._push_log(f"{e['name']}の攻撃！ {dmg} ダメージ")
                if self.player["hp"] <= 0:
                    self.player["hp"] = 0
                    self.state = "dead"
                    self._push_log("トルネコは倒れた…（Z でやり直し）")
                    return
            elif dist <= 6:
                sx = 1 if dx > 0 else (-1 if dx < 0 else 0)
                sy = 1 if dy > 0 else (-1 if dy < 0 else 0)
                for cand in ((sx, 0), (0, sy), (0, sy), (sx, 0)):
                    nx, ny = e["x"] + cand[0], e["y"] + cand[1]
                    if (
                        self.is_walkable(nx, ny)
                        and self.enemy_at(nx, ny) is None
                        and (nx, ny) != (self.player["x"], self.player["y"])
                        and self.item_at(nx, ny) is None
                        and (nx, ny) not in (self.stair_up, self.stair_down)
                    ):
                        e["x"], e["y"] = nx, ny
                        break
            elif self.rng.random() < 0.4:
                d = self.rng.choice([(1, 0), (-1, 0), (0, 1), (0, -1)])
                nx, ny = e["x"] + d[0], e["y"] + d[1]
                if self.is_walkable(nx, ny) and self.enemy_at(nx, ny) is None:
                    e["x"], e["y"] = nx, ny
        self._dirty = True

    # ── アイテム使用 ──────────────────────────────────────────────────────

    def _use_item(self, idx: int) -> None:
        name = self.inventory[idx]
        spec = ITEMS[name]
        se("ok")
        if "heal" in spec:
            if self.player["hp"] >= self.player["max_hp"]:
                self._push_log("HP は満タンだ")
                return
            self.player["hp"] = min(self.player["max_hp"], self.player["hp"] + spec["heal"])
            self._push_log(f"{name}を使った！ HP {spec['heal']} 回復")
        else:
            self.player["atk"] += spec["atk"]
            self._push_log(f"{name}を使った！ 攻撃力 +{spec['atk']}")
        self.inventory.pop(idx)

    # ── 入力 ──────────────────────────────────────────────────────────────

    def update(self, dt: float) -> None:
        if was_pressed("escape") and self.state != "dead":
            # セーブして終了（窓の × も同じ）
            self._save()
            self.quit()
            return
        if self.state == "dead":
            if was_pressed("z") or was_pressed("j"):
                self.save_path.unlink(missing_ok=True)
                self.__init__(seed=self.seed, save_path=self.save_path, start_floor=1)
                self._push_log("やり直し！")
            return
        if self.state == "win":
            if was_pressed("z") or was_pressed("j"):
                self.save_path.unlink(missing_ok=True)
                self.__init__(seed=self.seed, save_path=self.save_path, start_floor=1)
            return
        if self.state == "menu":
            self._menu_input()
            return
        # play
        dx = dy = 0
        if pressed("left") or pressed("a"):
            dx = -1
        elif pressed("right") or pressed("d"):
            dx = 1
        if pressed("up") or pressed("w"):
            dy = -1
        elif pressed("down") or pressed("s"):
            dy = 1
        if dx or dy:
            self._try_move(dx, dy)
            return
        if was_pressed("x"):
            self.state = "menu"
            self.sel = 0
            se("ok")
        if was_pressed("z") or was_pressed("j"):
            # 向いている方向の敵を攻撃
            fx, fy = self.facing
            foe = self.enemy_at(self.player["x"] + fx, self.player["y"] + fy)
            if foe:
                self._attack_foe(foe)
                self._enemy_turn()
            else:
                self._push_log("誰もいない…")

    def on_close(self) -> None:
        """窓を閉じたらセーブして終了（ESC も同じ）。"""
        self._save()
        self.quit()

    def _menu_input(self) -> None:
        items = self.inventory + ["閉じる"]
        if was_pressed("up") or was_pressed("w"):
            self.sel = (self.sel - 1) % len(items)
        elif was_pressed("down") or was_pressed("s"):
            self.sel = (self.sel + 1) % len(items)
        if was_pressed("z") or was_pressed("j") or was_pressed("return"):
            if self.sel < len(self.inventory):
                self._use_item(self.sel)
            else:
                self.state = "play"
            se("ok")
        if was_pressed("x"):
            self.state = "play"

    # ── 世界と描画（ターン制: 状態変化時のみ再描画） ─────────────────────

    def _wx(self, c: int) -> float:
        return c - COLS / 2 + 0.5

    def _wz(self, r: int) -> float:
        return r - ROWS / 2 + 0.5

    def _build_world(self) -> dict:
        props = [
            {"id": "prop:floor", "type": "prop", "name": "floor", "position": [0, -0.35, 0],
             "model": "box", "scale": [COLS + 1.0, 0.5, ROWS + 1.0], "enabled": True,
             "color": [34, 30, 26], "roughness": 0.95},
        ]
        for r in range(ROWS):
            for c in range(COLS):
                t = self.grid[r][c]
                wx, wz = self._wx(c), self._wz(r)
                if t == DungeonTiles.WALL:
                    props.append({"id": f"prop:w{c}x{r}", "type": "prop", "name": "wall",
                                  "position": [wx, 0.5, wz], "model": "box",
                                  "scale": [1.0, 1.0, 1.0], "enabled": True,
                                  "color": [96, 90, 84], "roughness": 0.9})
                elif t == DungeonTiles.DOOR:
                    props.append({"id": f"prop:d{c}x{r}", "type": "prop", "name": "door",
                                  "position": [wx, 0.25, wz], "model": "box",
                                  "scale": [1.0, 0.5, 1.0], "enabled": True,
                                  "color": [120, 84, 50], "roughness": 0.7})
                elif t == DungeonTiles.STAIR_DOWN:
                    props.append({"id": f"prop:sd{c}x{r}", "type": "prop", "name": "stair_down",
                                  "position": [wx, 0.08, wz], "model": "box",
                                  "scale": [0.7, 0.16, 0.7], "enabled": True,
                                  "color": [255, 200, 60], "metallic": 0.6, "roughness": 0.3})
                elif t == DungeonTiles.STAIR_UP:
                    props.append({"id": f"prop:su{c}x{r}", "type": "prop", "name": "stair_up",
                                  "position": [wx, 0.08, wz], "model": "box",
                                  "scale": [0.7, 0.16, 0.7], "enabled": True,
                                  "color": [90, 170, 255], "metallic": 0.5, "roughness": 0.3})
                elif t == DungeonTiles.CHEST:
                    props.append({"id": f"prop:ch{c}x{r}", "type": "prop", "name": "chest",
                                  "position": [wx, 0.25, wz], "model": "box",
                                  "scale": [0.8, 0.5, 0.8], "enabled": True,
                                  "color": [150, 110, 60], "roughness": 0.6})
        for i in self.items:
            props.append({"id": f"prop:item{i}", "type": "prop", "name": i["name"],
                          "position": [self._wx(i["x"]), 0.22, self._wz(i["y"])],
                          "model": "sphere",
                          "scale": [0.3, 0.3, 0.3], "enabled": True,
                          "color": [80, 200, 90] if i["name"] == "薬草" else [255, 160, 60],
                          "metallic": 0.2, "roughness": 0.5})
        for e in self.enemies:
            if e["hp"] <= 0:
                continue
            props.append({"id": f"prop:e{id(e)}", "type": "prop", "name": e["name"],
                          "position": [self._wx(e["x"]), 0.45, self._wz(e["y"])],
                          "model": "sphere",
                          "scale": [0.45, 0.45, 0.45], "enabled": True,
                          "color": [220, 70, 70], "roughness": 0.4})
        px, pz = self._wx(self.player["x"]), self._wz(self.player["y"])
        return {
            "version": 1,
            "half": 12.0,
            "floor_y": 0.0,
            "gravity": 9.8,
            "water_y": None,
            "coins": 0,
            "player": None,
            "props": props,
            "walkers": [
                {"id": "walker:hero", "type": "walker", "name": "hero",
                 "position": [px, 0, pz], "yaw": 0.0, "face": 0.0, "on_ground": True,
                 "model": "capsule", "clip": 0.0, "anim": "idle"},
            ],
            "lights": [
                {"id": "light:top", "type": "light", "name": "top", "position": [0, 8.0, 0],
                 "kind": "point", "slot": 0, "intensity": 3.0, "radius": 14.0,
                 "color": [1.0, 0.95, 0.85]},
            ],
            "cameras": [
                {"id": "camera:main", "type": "camera", "name": "main",
                 "position": [px, 10.5, pz + 2.0], "target": [px, 0, pz], "fov": 88},
            ],
            "heightfield": None,
        }

    def draw(self) -> None:
        if self._dirty:
            self.world = self._build_world()
            self._dirty = False
        p = self.player
        parts = [
            message(f"B{self.floor}F   HP {p['hp']}/{p['max_hp']}  攻 {p['atk']}", 10, 10, 210, size=13,
                    color=[240, 235, 220, 255]),
            bar(230, 12, 100, 8, ratio=p["hp"] / p["max_hp"], color=[240, 110, 100, 255]),
            list_lines(
                [f"WASD/矢印 移動 / Z 攻撃 / X 道具", f"ESC セーブして終了   seed {self.seed}  B{self.floor}F"],
                x=340, y=10, size=10, color=[180, 180, 168, 255],
            ),
        ]
        if self.state == "menu":
            parts.append(
                choice_menu(
                    self.inventory + ["閉じる"], selected=self.sel, x=200, y=80, w=152,
                    size=12,
                )
            )
        # メッセージログ（下 3 行）
        log = " / ".join(self.log_lines[-3:])
        parts.append(message(log, 8, H - 56, 344, size=11))
        if self.state == "dead":
            parts.append(message("トルネコは倒れた…（Z でやり直し）", 40, 80, 280, size=14,
                                 color=[255, 200, 190, 255]))
        elif self.state == "win":
            parts.append(message("ダンジョンを脱出した！ 大成功！", 40, 80, 280, size=14,
                                 color=[255, 235, 160, 255]))
        self._canvas_png = draw_world(self.world, self.width, self.height, hud=merge(*parts))


def _path_to(game: "Torneko", target: tuple[int, int]) -> list[tuple[int, int]]:
    """BFS で (敵を避けつつ) 目標タイルへの最短歩行経路。到達不可は []。"""
    from collections import deque

    start = (game.player["x"], game.player["y"])
    if start == target:
        return []
    q: deque = deque([start])
    prev = {start: None}
    while q:
        cur = q.popleft()
        if cur == target:
            break
        for d in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            n = (cur[0] + d[0], cur[1] + d[1])
            if n in prev:
                continue
            cx, cy = n
            if not game.is_walkable(cx, cy):
                continue
            if game.enemy_at(cx, cy) is not None:
                continue
            prev[n] = cur
            q.append(n)
    if target not in prev:
        return []
    path = []
    cur = target
    while prev[cur] is not None:
        path.append(cur)
        cur = prev[cur]
    path.reverse()
    return path


def scripted_policy(game: "Torneko", turns: int) -> list[str]:
    """ヘッドレス verify 用: BFS で下り階段を目指し、隣接敵を攻撃、
    HP が減ったら薬草を使う。"""
    steps = []
    for _ in range(turns):
        if game.state in ("dead", "win"):
            break
        if game.state == "menu":
            game.state = "play"
            continue
        # 危険なら薬草
        if game.player["hp"] < 8 and "薬草" in game.inventory:
            game._use_item(game.inventory.index("薬草"))
            steps.append("heal")
            continue
        # 隣接敵を攻撃
        foe = None
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            e = game.enemy_at(game.player["x"] + dx, game.player["y"] + dy)
            if e:
                foe = e
                break
        if foe:
            game._attack_foe(foe)
            game._enemy_turn()
            steps.append("attack")
            continue
        # 階段へ BFS
        path = _path_to(game, game.stair_down)
        if path:
            nx, ny = path[0]
            game._try_move(nx - game.player["x"], ny - game.player["y"])
            steps.append("move")
            continue
        # 詰まったら彷徨う
        for d in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            nx, ny = game.player["x"] + d[0], game.player["y"] + d[1]
            if game.is_walkable(nx, ny) and game.enemy_at(nx, ny) is None:
                game._try_move(*d)
                steps.append("wander")
                break
        else:
            steps.append("stuck")
    return steps
