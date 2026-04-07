"""
迷路探索ゲーム - KAGRA版
操作: カーソルキー長押しでスムーズ移動、アイテムを集めてゴールへ！
"""
import math, random, os, struct, tempfile, zlib
import kagra
from kagra.tilemap import TileSet, TileMap, TILE_SOLID
from kagra.effects import EffectManager
from kagra.camera import Camera
from kagra.physics import Rigidbody, BoxCollider, TopDownPhysicsSystem

SW, SH = 1280, 720
TW = TH = 48

TILE_FLOOR, TILE_WALL, TILE_GOAL, TILE_ITEM = 0, 1, 2, 3
ATTRS = {TILE_WALL: TILE_SOLID}

# ── 迷路生成 ──────────────────────────────────────────────────

def generate_maze(width, height):
    width  = width  if width  % 2 else width  + 1
    height = height if height % 2 else height + 1
    maze   = [[TILE_WALL] * width for _ in range(height)]
    stack  = [(1, 1)]
    maze[1][1] = TILE_FLOOR
    dirs = [(0,-2),(0,2),(-2,0),(2,0)]
    while stack:
        if random.random() < 0.2:
            i = random.randint(0, len(stack)-1)
            stack.append(stack.pop(i))
        x, y = stack[-1]
        neighbors = [(x+dx, y+dy, dx//2, dy//2) for dx,dy in dirs
                     if 0<x+dx<width-1 and 0<y+dy<height-1 and maze[y+dy][x+dx]==TILE_WALL]
        if neighbors:
            nx,ny,mx,my = random.choice(neighbors)
            maze[y+my][x+mx] = maze[ny][nx] = TILE_FLOOR
            stack.append((nx, ny))
        else:
            stack.pop()
    maze[height-2][width-2] = TILE_GOAL
    placed = 0
    while placed < max(3, width*height//25):
        x,y = random.randint(1,width-2), random.randint(1,height-2)
        if maze[y][x] == TILE_FLOOR and (x,y) not in ((1,1),(width-2,height-2)):
            maze[y][x] = TILE_ITEM
            placed += 1
    return maze, (1,1), (width-2, height-2)

# ── テクスチャ生成 ─────────────────────────────────────────────

def make_tile_texture():
    try:
        from PIL import Image, ImageDraw
        img  = Image.new("RGBA", (TW*4, TH), (0,0,0,0))
        draw = ImageDraw.Draw(img)
        draw.rectangle((0,0,TW-1,TH-1),       fill=(60,180,80,255))
        draw.rectangle((TW,0,TW*2-1,TH-1),    fill=(120,80,40,255))
        draw.rectangle((TW*2,0,TW*3-1,TH-1),  fill=(255,215,0,255))
        draw.rectangle((TW*3,0,TW*4-1,TH-1),  fill=(60,180,80,255))
        draw.ellipse((TW*3+8,8,TW*4-9,TH-9),  fill=(255,60,60,255))
        path = os.path.join(tempfile.gettempdir(), "maze_tiles.png")
        img.save(path)
    except ImportError:
        W, H = TW*4, TH
        colors = [(60,180,80,255),(120,80,40,255),(255,215,0,255),(255,60,60,255)]
        data   = b"".join(b"\x00" + b"".join(bytes(colors[x//TW]) for x in range(W)) for _ in range(H))
        def chunk(t,d): c=zlib.crc32(t+d)&0xFFFFFFFF; return struct.pack(">I",len(d))+t+d+struct.pack(">I",c)
        png = (b"\x89PNG\r\n\x1a\n"
               + chunk(b"IHDR", struct.pack(">IIBBBBB",W,H,8,6,0,0,0))
               + chunk(b"IDAT", zlib.compress(data))
               + chunk(b"IEND", b""))
        path = os.path.join(tempfile.gettempdir(), "maze_tiles.png")
        open(path,"wb").write(png)
    return kagra.load(path)

def make_player_texture():
    try:
        from PIL import Image, ImageDraw
        img  = Image.new("RGBA", (TW, TH), (0,0,0,0))
        draw = ImageDraw.Draw(img)
        p = 8
        draw.ellipse((p,p,TW-1-p,TH-1-p), fill=(255,220,80,255))
        ew,eh = TW*0.15, TH*0.15
        draw.rectangle((TW*0.30, TH*0.30, TW*0.30+ew, TH*0.30+eh), fill=(0,0,0,255))
        draw.rectangle((TW*0.55, TH*0.30, TW*0.55+ew, TH*0.30+eh), fill=(0,0,0,255))
        path = os.path.join(tempfile.gettempdir(), "maze_player.png")
        img.save(path)
    except ImportError:
        W, H = TW, TH
        data = b"".join(
            b"\x00" + b"".join(
                (b"\xff\xdc\x50\xff" if 8<=x<W-8 and 8<=y<H-8 else b"\x00\x00\x00\x00")
                for x in range(W))
            for y in range(H))
        def chunk(t,d): c=zlib.crc32(t+d)&0xFFFFFFFF; return struct.pack(">I",len(d))+t+d+struct.pack(">I",c)
        png = (b"\x89PNG\r\n\x1a\n"
               + chunk(b"IHDR", struct.pack(">IIBBBBB",W,H,8,6,0,0,0))
               + chunk(b"IDAT", zlib.compress(data))
               + chunk(b"IEND", b""))
        path = os.path.join(tempfile.gettempdir(), "maze_player.png")
        open(path,"wb").write(png)
    return kagra.load(path)

# ── プレイヤースクリプト ──────────────────────────────────────

class PlayerScript(kagra.Script):
    def start(self):
        self.rb    = self.entity.get(Rigidbody)
        self.speed = 320.0
        self.game  = None

    def update(self, dt):
        if self.game.game_clear:
            self.rb.vx = self.rb.vy = 0
            return
        dt = min(dt, 0.05)
        dx = (-1 if kagra.key("LEFT") else 0) + (1 if kagra.key("RIGHT") else 0)
        dy = (-1 if kagra.key("UP")   else 0) + (1 if kagra.key("DOWN")  else 0)
        if dx or dy:
            ln = math.hypot(dx, dy)
            dx /= ln; dy /= ln
        self.rb.vx += (dx*self.speed - self.rb.vx) * 15 * dt
        self.rb.vy += (dy*self.speed - self.rb.vy) * 15 * dt
        self._check_tiles()

    def _check_tiles(self):
        tx = int(self.entity.transform.x // TW)
        ty = int(self.entity.transform.y // TH)
        if 0 <= tx < self.game.maze_w and 0 <= ty < self.game.maze_h:
            tile = self.game.maze_data[ty][tx]
            if tile == TILE_ITEM: self.game.pick_item(tx, ty)
            elif tile == TILE_GOAL: self.game.reach_goal()

# ── メインシーン ──────────────────────────────────────────────

class MazeGame(kagra.Scene):
    def on_enter(self):
        kagra.font("C:/Windows/Fonts/meiryo.ttc")  # デフォルトフォント設定

        self.tileset    = TileSet(make_tile_texture(), TW, TH)
        self.player_tex = make_player_texture()
        self.fx         = EffectManager()

        self.level = self.score = self.total_score = 0
        self.level = 1

        self.cam   = Camera(screen_w=SW, screen_h=SH, world_w=1, world_h=1)
        kagra.set_camera(self.cam)

        self.world   = kagra.World()
        self.physics = TopDownPhysicsSystem()
        self.player  = self.world.create("Player")
        self.player.add(Rigidbody(gravity=0.0, mass=1.0, bounce=0.0))
        col = self.player.add(BoxCollider(w=24, h=24, offset_x=-12, offset_y=-12))
        col.layer = "player"; col.mask = []

        self.ps      = self.player.add(PlayerScript())
        self.ps.game = self

        self.game_clear  = False
        self.clear_timer = 0.0
        self.start_new_maze()

    def start_new_maze(self):
        self.maze_w = min(81, 21 + (self.level-1)*3)
        self.maze_h = min(61, 11 + (self.level-1)*2)
        self.maze_data, self.start_pos, self.goal_pos = generate_maze(self.maze_w, self.maze_h)

        self.tilemap = TileMap(self.tileset, self.maze_data, ATTRS, TW, TH)
        self.physics.set_tilemap(self.tilemap)
        self.cam.world_w = self.maze_w * TW
        self.cam.world_h = self.maze_h * TH

        px = self.start_pos[0]*TW + TW/2
        py = self.start_pos[1]*TH + TH/2
        self.player.transform.x, self.player.transform.y = px, py

        self.items_left = sum(row.count(TILE_ITEM) for row in self.maze_data)
        self.game_clear = False
        self.cam.follow(px, py, lerp=1.0)
        self.cam.update(0.0)

    def pick_item(self, tx, ty):
        self.maze_data[ty][tx] = TILE_FLOOR
        self.tilemap    = TileMap(self.tileset, self.maze_data, ATTRS, TW, TH)
        self.total_score += 10
        self.items_left  -= 1
        self.fx.heal(tx*TW + TW/2, ty*TH + TH/2, 10)

    def reach_goal(self):
        if not self.game_clear:
            self.game_clear  = True
            self.clear_timer = 0.0
            self.total_score += 100

    def update(self, dt):
        dt = min(dt, 0.05)
        if kagra.pressed("ESCAPE"):
            raise SystemExit
        self.cam.update(dt)
        if self.game_clear:
            self.clear_timer += dt
            if self.clear_timer > 1.5:
                self.level += 1
                self.start_new_maze()
            return
        self.world.update(dt)
        self.physics.update(dt, self.world)
        kagra.flush_events()
        px, py = self.player.transform.x, self.player.transform.y
        self.cam.follow(px, py, lerp=0.15)
        self.fx.update(dt)

    def draw(self):
        kagra.cls(40, 50, 70)
        self.tilemap.draw(self.cam)

        px = self.player.transform.x - TW/2
        py = self.player.transform.y - TH/2
        kagra.image_world(self.player_tex, px, py, TW, TH)

        kagra.text(f"レベル {self.level}",           20,  20, 28, (255,255,100))
        kagra.text(f"スコア {self.total_score}",      20,  60, 24, (255,220, 80))
        if self.items_left:
            kagra.text(f"のこりアイテム {self.items_left} 個", 20, 100, 20, (200,200,255))
        kagra.text("長押しでスムーズ移動", SW-220, SH-40, 18, (150,150,150))
        self.fx.draw()

        if self.game_clear:
            alpha = min(180, int(180 * self.clear_timer / 0.5))
            if alpha > 0:
                kagra.fill(0, 0, SW, SH, (0,0,0), alpha)
            w, _ = kagra.measure("CLEAR!", 80)
            kagra.text("CLEAR!", (SW-w)//2, SH//2-50, 80, (255,215,0))

# ── タイトル ──────────────────────────────────────────────────

class TitleScene(kagra.Scene):
    def on_enter(self):
        kagra.font("C:/Windows/Fonts/meiryo.ttc")
        self.t = 0.0

    def update(self, dt):
        self.t += dt
        if kagra.pressed("Z"):
            kagra.go(MazeGame())

    def draw(self):
        kagra.cls(20, 30, 50)
        w, _ = kagra.measure("まいぞう たんけん", 72)
        kagra.text("まいぞう たんけん", (SW-w)//2, 200, 72, (255,220,80))
        w, _ = kagra.measure("自分のペースで アイテムを集めて ゴールを目指そう！", 28)
        kagra.text("自分のペースで アイテムを集めて ゴールを目指そう！", (SW-w)//2, 320, 28, (200,200,255))
        if int(self.t*2) % 2 == 0:
            w, _ = kagra.measure("Z キーで スタート", 28)
            kagra.text("Z キーで スタート", (SW-w)//2, 500, 28, (150,150,150))

if __name__ == "__main__":
    kagra.init(width=SW, height=SH, title="Maze Explorer", fps=60)
    kagra.run(start_scene=TitleScene())
