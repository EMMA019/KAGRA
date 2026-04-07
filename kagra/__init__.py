# kagra/__init__.py  ─  KAGRA Python API
# ============================================================
# API レイヤー構造：
#
#   シンプル API（推奨）  低レベル API（上級者向け）
#   ───────────────────   ────────────────────────────
#   kagra.font()          load_font() / draw_text()
#   kagra.text()          draw_text()
#   kagra.image()         draw_texture()
#   kagra.fill()          rect()
#   kagra.button()        draw_ui_button()
#   kagra.bar()           draw_ui_progress_bar()
#   kagra.bgm()           audio.play_bgm()
#   kagra.se()            audio.play_se()
#   kagra.load()          load_texture()
#   kagra.avatar()        load_vrm() + VrmAnimator + ...
#
# ============================================================
from __future__ import annotations
from typing import Optional
import sys, types

try:
    from kagra.kagra_core import Engine as _Engine
except ImportError as e:
    raise ImportError(
        f"kagra_core が見つかりません。maturin develop を実行してください。\n{e}"
    )

# ── 外部モジュール再エクスポート ───────────────────────────────
from kagra.camera        import Camera
from kagra.camera3d      import Camera3D
from kagra.tilemap       import TileSet, TileMap
from kagra.tilemap       import TILE_SOLID, TILE_WATER, TILE_LADDER, TILE_DOOR, TILE_DAMAGE
from kagra.ui            import (
    Tween, TweenManager, Easing,
    Panel, Label, Button,
    MessageWindow, EventFlags, DialogScript,
    SaveLoad, ChoiceMenu, TransitionScene,
    ProgressBar, VBox, HBox, ScrollView, UIGroup,
)
from kagra.entity        import (
    Component, Script, Transform, Sprite, SpriteRenderer, TextRenderer,
    RigRenderer, RectRenderer, AnimatorComponent, Collider, Entity, World, EntityScene,
)
from kagra.skeleton      import (
    Transform2D, Attachment, MeshVertex, MeshAttachment, Bone,
    Keyframe, AnimationTrack, AnimationClip, Skeleton, SkeletonAnimator,
)
from kagra.event_bus     import EventBus, get_global_bus, reset_global_bus
from kagra.components    import TopDownMovement, FourDirAnimator, CameraFollower
from kagra.assets        import AssetManager as _AssetManager
assets = _AssetManager()


# ── エンジン状態 ──────────────────────────────────────────────
_engine: _Engine | None = None
_camera: Camera | None  = None

def _check():
    if _engine is None:
        raise RuntimeError("kagra.init() を先に呼んでください")


# ── キーマップ ────────────────────────────────────────────────
keys = types.SimpleNamespace()

# 後方互換のトップレベル定数（kagra.KEY_Z など）
KEY_UP = KEY_DOWN = KEY_LEFT = KEY_RIGHT = None
KEY_Z = KEY_X = KEY_SPACE = KEY_RETURN = KEY_ESCAPE = None
MOUSE_LEFT = 1; MOUSE_RIGHT = 2; MOUSE_MIDDLE = 3

def _update_keys():
    global KEY_UP,KEY_DOWN,KEY_LEFT,KEY_RIGHT,KEY_Z,KEY_X,KEY_SPACE,KEY_RETURN,KEY_ESCAPE
    if _engine is None: return
    for name, code in _engine.get_keymap().items():
        setattr(keys, name, code)
    keys.MOUSE_LEFT = 1; keys.MOUSE_RIGHT = 2; keys.MOUSE_MIDDLE = 3
    if hasattr(keys,"UP"):     KEY_UP     = keys.UP
    if hasattr(keys,"DOWN"):   KEY_DOWN   = keys.DOWN
    if hasattr(keys,"LEFT"):   KEY_LEFT   = keys.LEFT
    if hasattr(keys,"RIGHT"):  KEY_RIGHT  = keys.RIGHT
    if hasattr(keys,"Z"):      KEY_Z      = keys.Z
    if hasattr(keys,"X"):      KEY_X      = keys.X
    if hasattr(keys,"SPACE"):  KEY_SPACE  = keys.SPACE
    if hasattr(keys,"RETURN"): KEY_RETURN = keys.RETURN
    if hasattr(keys,"ESCAPE"): KEY_ESCAPE = keys.ESCAPE

def _key_code(name: str) -> int:
    code = getattr(keys, name.upper(), None)
    if code is None:
        available = sorted(k for k in dir(keys) if not k.startswith("_"))
        raise ValueError(f"Unknown key: '{name}'. 使えるキー: {available}")
    return code


# ═══════════════════════════════════════════════════════════════
#  初期化 / ループ
# ═══════════════════════════════════════════════════════════════

class Scene:
    def on_enter(self):  pass
    def on_exit(self):   pass
    def on_pause(self):  pass
    def on_resume(self): pass
    def update(self, dt: float): pass
    def draw(self): pass

class _SceneManager:
    def __init__(self):
        self._stack:   list[Scene] = []
        self._pending: list       = []

    @property
    def current(self) -> Optional[Scene]:
        return self._stack[-1] if self._stack else None

    def change(self, s: Scene): self._pending.append(("change", s))
    def push(self, s: Scene):   self._pending.append(("push",   s))
    def pop(self):               self._pending.append(("pop",    None))

    def _flush_pending(self):
        for op, s in self._pending:
            if op == "change":
                if self._stack: self._stack[-1].on_exit(); self._stack.pop()
                reset_global_bus()
                self._stack.append(s); s.on_enter()
            elif op == "push":
                if self._stack: self._stack[-1].on_pause()
                self._stack.append(s); s.on_enter()
            elif op == "pop":
                if self._stack: self._stack[-1].on_exit(); self._stack.pop()
                if self._stack: self._stack[-1].on_resume()
        self._pending.clear()

    def _update(self, dt):
        self._flush_pending()
        if self._stack: self._stack[-1].update(dt)

    def _draw(self):
        if self._stack: self._stack[-1].draw()

scene = _SceneManager()


def init(width=1280, height=720, title="KAGRA Game", fps=60):
    """エンジンを初期化する。プログラムの最初に1回だけ呼ぶ。"""
    global _engine
    _engine = _Engine(width=width, height=height, title=title, fps=fps)
    _update_keys()

def run(update=None, draw=None, start_scene: Scene = None):
    """ゲームループを開始する。"""
    _check()
    if start_scene is not None:
        scene._stack.clear(); scene._pending.clear()
        scene.change(start_scene)
        _engine.run(scene._update, scene._draw)
    else:
        _engine.run(update, draw)


# ═══════════════════════════════════════════════════════════════
#  低レベル描画 API（既存コードとの後方互換を維持）
# ═══════════════════════════════════════════════════════════════

def _clamp_u8(v) -> int:
    iv = int(v); return max(0, min(255, iv))

def _norm_color(value, default_a=255):
    if not isinstance(value, (tuple, list)):
        raise ValueError("color must be (r,g,b) or (r,g,b,a)")
    r,g,b = value[0],value[1],value[2]
    a = value[3] if len(value) > 3 else default_a
    return _clamp_u8(r), _clamp_u8(g), _clamp_u8(b), _clamp_u8(a)

def _resolve_rgb(first, g=None, b=None, a=255):
    """後方互換: rect(x,y,w,h, 255,128,0) 形式と (r,g,b) タプル両対応。"""
    if isinstance(first, (tuple, list)):
        return _norm_color(first, a)
    if g is None and b is None:
        v = _clamp_u8(first); return v, v, v, _clamp_u8(a)
    return _clamp_u8(first), _clamp_u8(g), _clamp_u8(b), _clamp_u8(a)


def cls(r=0, g=0, b=0):
    """画面をクリアする。"""
    _check(); rr,gg,bb,_ = _resolve_rgb(r,g,b,255); _engine.cls(rr,gg,bb)

def rect(x, y, w, h, color=255, g=None, b=None, a=255):
    """矩形を描画する（低レベル。シンプルAPIは fill() を使う）。"""
    _check(); r,g2,b2,a2 = _resolve_rgb(color,g,b,a)
    _engine.rect(x,y,w,h,r,g2,b2,a2)

def load_texture(path: str) -> int:
    """テクスチャを読み込んでIDを返す（低レベル。シンプルAPIは load() を使う）。"""
    _check(); return _engine.load_texture(path)

def texture_size(tid: int) -> tuple:
    _check(); return _engine.texture_size(tid)

def draw_texture(tid, x, y, w=None, h=None, sx=0., sy=0., sw=None, sh=None,
                 alpha=1., rotation_deg=0., pivot_x=.5, pivot_y=.5,
                 flip_x=True, flip_y=False, shader_id=0, shader_params=None):
    """テクスチャを描画する（低レベル。シンプルAPIは image() を使う）。"""
    _check()
    _engine.draw_texture(tid,x,y,w,h,sx,sy,sw,sh,alpha,
                         rotation_deg,pivot_x,pivot_y,flip_x,flip_y,
                         shader_id,shader_params)

def load_font(path: str) -> int:
    """フォントを読み込んでIDを返す（低レベル。シンプルAPIは font() を使う）。"""
    _check(); return _engine.load_font(path)

def draw_text(font_id, text_str, x, y, size=24, r=255, g=255, b=255, a=255, color=None):
    """テキストを描画する（低レベル。シンプルAPIは text() を使う）。"""
    _check()
    if color is not None:
        rr,gg,bb,aa = _norm_color(color, a)
    elif isinstance(r, (tuple,list)):
        rr,gg,bb,aa = _norm_color(r, a)
    else:
        rr,gg,bb,aa = _resolve_rgb(r,g,b,a)
    _engine.draw_text(font_id, str(text_str), x, y, size, rr, gg, bb, aa)

def measure_text(font_id, text_str, size=24) -> tuple:
    """テキストの描画サイズを返す（低レベル。シンプルAPIは measure() を使う）。"""
    _check(); return _engine.measure_text(font_id, str(text_str), size)

def draw_mesh(texture_id: int, verts: list,
              shader_id: int = 0, shader_params: list = None):
    _check(); _engine.draw_mesh(texture_id, verts, shader_id, shader_params)

def load_shader(path: str) -> int:
    _check(); return _engine.load_shader(path)

def load_shader_src(wgsl_src: str) -> int:
    _check(); return _engine.load_shader_src(wgsl_src)


# ── UI 低レベル ────────────────────────────────────────────────

def draw_ui_button(x, y, w, h, text,
                   bg_r=70, bg_g=70, bg_b=90,
                   hv_r=100, hv_g=100, hv_b=150,
                   txt_r=255, txt_g=255, txt_b=255,
                   font_size=20,
                   bg_color=None, hover_color=None, text_color=None,
                   font_id=1) -> bool:
    _check()
    if bg_color    is not None: bg_r,bg_g,bg_b,_ = _norm_color(bg_color)
    if hover_color is not None: hv_r,hv_g,hv_b,_ = _norm_color(hover_color)
    if text_color  is not None: txt_r,txt_g,txt_b,_ = _norm_color(text_color)
    return _engine.draw_ui_button(
        x,y,w,h,text,
        _clamp_u8(bg_r),_clamp_u8(bg_g),_clamp_u8(bg_b),
        _clamp_u8(hv_r),_clamp_u8(hv_g),_clamp_u8(hv_b),
        _clamp_u8(txt_r),_clamp_u8(txt_g),_clamp_u8(txt_b),
        font_size, font_id,
    )

def draw_ui_progress_bar(x, y, w, h, max_val, current_val,
                         bg_r=30, bg_g=30, bg_b=30,
                         fl_r=50, fl_g=255, fl_b=50,
                         bg_color=None, fill_color=None):
    _check()
    if bg_color   is not None: bg_r,bg_g,bg_b,_ = _norm_color(bg_color)
    if fill_color is not None: fl_r,fl_g,fl_b,_ = _norm_color(fill_color)
    _engine.draw_ui_progress_bar(x,y,w,h,max_val,current_val,
                                 _clamp_u8(bg_r),_clamp_u8(bg_g),_clamp_u8(bg_b),
                                 _clamp_u8(fl_r),_clamp_u8(fl_g),_clamp_u8(fl_b))


# ── 入力 低レベル ──────────────────────────────────────────────

def key_down(code: int) -> bool:     _check(); return _engine.key_down(code)
def key_pressed(code: int) -> bool:  _check(); return _engine.key_pressed(code)
def key_released(code: int) -> bool: _check(); return _engine.key_released(code)
def mouse_pos() -> tuple:            _check(); return _engine.mouse_pos()
def mouse_down(btn: int) -> bool:    _check(); return _engine.mouse_down(btn)
def mouse_pressed(btn: int) -> bool: _check(); return _engine.mouse_pressed(btn)
def mouse_released(btn: int) -> bool:_check(); return _engine.mouse_released(btn)
def mouse_wheel() -> tuple:          _check(); return _engine.mouse_wheel()

def collide_rect(ax,ay,aw,ah,bx,by,bw,bh):
    _check(); return _engine.collide_rect(ax,ay,aw,ah,bx,by,bw,bh)
def collide_rect_overlap(ax,ay,aw,ah,bx,by,bw,bh):
    _check(); return _engine.collide_rect_overlap(ax,ay,aw,ah,bx,by,bw,bh)
def point_in_rect(px,py,rx,ry,rw,rh):
    _check(); return _engine.point_in_rect(px,py,rx,ry,rw,rh)


# ── オーディオ 低レベル ────────────────────────────────────────

class _Audio:
    """audio.play_bgm() / audio.play_se() など。"""
    def play_bgm(self, path, loop=True, volume=0.8): _check(); _engine.play_bgm(path,loop,volume)
    def stop_bgm(self, fade=0.0):                    _check(); _engine.stop_bgm(fade)
    def pause_bgm(self):                             _check(); _engine.pause_bgm()
    def resume_bgm(self):                            _check(); _engine.resume_bgm()
    def set_bgm_volume(self, v):                     _check(); _engine.set_bgm_volume(v)
    def play_se(self, path, vol=1.0):                _check(); _engine.play_se(path, vol)
    def stop_all_se(self):                           _check(); _engine.stop_all_se()

audio = _Audio()

def play_bgm(path: str, loop_=True, volume=0.8): audio.play_bgm(path, loop_, volume)
def play_se(path: str, volume=1.0):              audio.play_se(path, volume)
def stop_bgm(fade: float = 0.0):                 audio.stop_bgm(fade)


# ── カメラ 低レベル ────────────────────────────────────────────

def set_camera(cam: Camera | None):
    global _camera; _camera = cam

def get_camera() -> Camera | None:
    return _camera

def camera_update(dt: float):
    if _camera is not None: _camera.update(dt)

def screen_to_world(sx: float, sy: float) -> tuple:
    return _camera.to_world(sx, sy) if _camera else (sx, sy)

def world_to_screen(wx: float, wy: float) -> tuple:
    return _camera.to_screen(wx, wy) if _camera else (wx, wy)

def rect_world(wx, wy, w, h, r, g, b, a=255):
    _check()
    if _camera:
        if not _camera.is_visible(wx, wy, w, h): return
        sx,sy = _camera.to_screen(wx, wy)
        w = _camera.scale_to_screen(w); h = _camera.scale_to_screen(h)
    else:
        sx,sy = wx,wy
    _engine.rect(sx,sy,w,h, _clamp_u8(r),_clamp_u8(g),_clamp_u8(b),_clamp_u8(a))

def draw_texture_world(tid, wx, wy, w=None, h=None,
                       sx=0., sy=0., sw=None, sh=None,
                       alpha=1., rotation_deg=0., pivot_x=.5, pivot_y=.5,
                       flip_x=False, flip_y=False,
                       shader_id=0, shader_params=None):
    """ワールド座標でテクスチャを描画（カメラ変換自動適用）。"""
    _check()
    if _camera:
        dw = w or 0; dh = h or 0
        if not _camera.is_visible(wx, wy, dw, dh): return
        sx_s, sy_s = _camera.to_screen(wx, wy)
        sw_s = _camera.scale_to_screen(w) if w else None
        sh_s = _camera.scale_to_screen(h) if h else None
    else:
        sx_s, sy_s, sw_s, sh_s = wx, wy, w, h
    _engine.draw_texture(tid, sx_s, sy_s, sw_s, sh_s,
                         sx, sy, sw, sh, alpha, rotation_deg,
                         pivot_x, pivot_y, flip_x, flip_y,
                         shader_id, shader_params)


# ── システム情報 ───────────────────────────────────────────────

def get_fps() -> float:         _check(); return _engine.fps
def get_screen_size() -> tuple: _check(); return (_engine.width, _engine.height)


# ── リグ ──────────────────────────────────────────────────────

def load_rig(path: str) -> int:         _check(); return _engine.load_rig(path)
def draw_rig(rig_id: int, x, y):        _check(); _engine.draw_rig(rig_id, x, y)


# ── シェーダー定数 ─────────────────────────────────────────────

SHADER_DEFAULT   = 0
SHADER_GRAYSCALE = 1
SHADER_FLASH     = 2
SHADER_SPOTLIGHT = 3
SHADER_GLOW      = 4
SHADER_TINT      = 5

def draw_texture_spotlight(tid, x, y, w=None, h=None,
                           spot_x=.5, spot_y=.5, radius=.4, intensity=1.,
                           alpha=1., rotation_deg=0., flip_x=False):
    _check()
    _engine.draw_texture(tid,x,y,w,h,0,0,None,None,alpha,rotation_deg,.5,.5,
                         flip_x,False,SHADER_SPOTLIGHT,[spot_x,spot_y,radius,intensity])

def draw_texture_glow(tid, x, y, w=None, h=None,
                      r=1., g=.8, b=1., intensity=1.,
                      alpha=1., rotation_deg=0., flip_x=False):
    _check()
    _engine.draw_texture(tid,x,y,w,h,0,0,None,None,alpha,rotation_deg,.5,.5,
                         flip_x,False,SHADER_GLOW,[r,g,b,intensity])

def draw_texture_tint(tid, x, y, w=None, h=None,
                      r=1., g=1., b=1., intensity=1.,
                      alpha=1., rotation_deg=0., flip_x=False):
    _check()
    _engine.draw_texture(tid,x,y,w,h,0,0,None,None,alpha,rotation_deg,.5,.5,
                         flip_x,False,SHADER_TINT,[r,g,b,intensity])


# ── 3D ────────────────────────────────────────────────────────

def update_camera_3d(view: list, proj: list):
    _check(); _engine.update_camera_3d(view, proj)

def draw_mesh_3d(texture_id: int, verts: list, indices: list):
    _check(); _engine.draw_mesh_3d(texture_id, verts, indices)


# ── VRM 低レベル ──────────────────────────────────────────────

def load_vrm(path: str) -> int:
    """VRM ファイルを読み込む。通常は kagra.avatar() を使う。"""
    _check(); return _engine.load_vrm(path)

def draw_vrm(vrm_id: int):
    """VRM を GPU スキニングで描画する。draw() の中で呼ぶ。"""
    _check(); _engine.draw_vrm(vrm_id)

def set_vrm_bone_euler(vrm_id: int, bone: str, rx=0., ry=0., rz=0.):
    _check(); _engine.set_vrm_bone_euler(vrm_id, bone, rx, ry, rz)

def reset_vrm_pose(vrm_id: int):
    _check(); _engine.reset_vrm_pose(vrm_id)

def set_vrm_offset(vrm_id: int, x: float = 0., y: float = 0., z: float = 0.):
    """VRM のルート位置オフセットを設定する（BVH の Root 移動に使用）。"""
    _check(); _engine.set_vrm_offset(vrm_id, float(x), float(y), float(z))

def set_vrm_bone_trans(vrm_id: int, bone: str, tx: float = 0., ty: float = 0., tz: float = 0.):
    """VRM ボーンの並進（位置）を設定する。腰の上下移動などに使用。"""
    _check(); _engine.set_vrm_bone_trans(vrm_id, bone, float(tx), float(ty), float(tz))

def set_vrm_bone_scale(vrm_id: int, bone: str, sx: float = 1., sy: float = 1., sz: float = 1.):
    """VRM ボーンのスケールを設定する。"""
    _check(); _engine.set_vrm_bone_scale(vrm_id, bone, float(sx), float(sy), float(sz))

def set_blend_shape(vrm_id: int, name: str, weight: float):
    """VRM のブレンドシェイプウェイトを設定する。通常は avatar.set_expression() を使う。"""
    _check(); _engine.set_blend_shape(vrm_id, name, float(weight))

def reset_blend_shapes(vrm_id: int):
    """VRM の全ブレンドシェイプをリセット。通常は avatar.reset_expressions() を使う。"""
    _check(); _engine.reset_blend_shapes(vrm_id)

def list_blend_shapes(vrm_id: int) -> list[str]:
    """VRM のブレンドシェイプ名一覧を返す。"""
    _check(); return _engine.list_blend_shapes(vrm_id)

def set_fog(start: float = 5., end: float = 20.,
            color: tuple = (110,180,230), *, enabled: bool = True):
    """3D フォグを設定する。

    Example::
        kagra.set_fog(start=3.0, end=12.0, color=(35,25,20))
        kagra.set_fog(enabled=False)
    """
    _check()
    _engine.set_fog(float(start), float(end),
                    int(color[0]), int(color[1]), int(color[2]), enabled)


# ── Event Bus ─────────────────────────────────────────────────

def on(event, callback, priority=0, once=False):
    return get_global_bus().on(event, callback, priority=priority, once=once)
def once(event, callback, priority=0):
    return get_global_bus().once(event, callback, priority=priority)
def off(event, callback):       get_global_bus().off(event, callback)
def off_all(event):             get_global_bus().off_all(event)
def emit(event, data=None, deferred=False):
    get_global_bus().emit(event, data, deferred=deferred)
def flush_events():             get_global_bus().flush()


# ── Scriptable ────────────────────────────────────────────────

def load_data(key: str, force_reload=False) -> "DataObject":
    return get_data_registry().load(key, force_reload=force_reload)
def preload_data(subdir="", recursive=True) -> list:
    return get_data_registry().preload_dir(subdir, recursive=recursive)


# ═══════════════════════════════════════════════════════════════
#  シンプル API（推奨）
#  命名規則：動詞なし短縮名。フォント・色はデフォルト適用。
# ═══════════════════════════════════════════════════════════════

_default_font: int = 0

def _c(color, default_a=255):
    r,g,b = int(color[0]),int(color[1]),int(color[2])
    a = int(color[3]) if len(color) > 3 else default_a
    return r,g,b,a


# ── フォント・テキスト ─────────────────────────────────────────

def font(path: str) -> int:
    """フォントを読み込み、デフォルト登録してIDを返す。

    Example::
        kagra.font("C:/Windows/Fonts/meiryo.ttc")
        kagra.text("スコア", 20, 20, 28)
    """
    global _default_font
    fid = load_font(path)
    _default_font = fid
    return fid

def set_font(font_id: int):
    """デフォルトフォントを変更する。"""
    global _default_font
    _default_font = font_id

def text(s, x: float, y: float, size: int = 24,
         color=(255,255,255), font: int = None, alpha: int = 255):
    """テキストを描画する。

    Example::
        kagra.text("こんにちは", 100, 50, 28, (255,220,80))
        kagra.text(f"スコア {score}", 20, 20)
    """
    f = font if font is not None else _default_font
    r,g,b,a = _c(color, alpha)
    draw_text(f, str(s), x, y, size, r, g, b, a)

def measure(s, size: int = 24, font: int = None) -> tuple:
    """テキストの描画幅・高さを返す。

    Example::
        w,_ = kagra.measure("CLEAR!", 80)
        kagra.text("CLEAR!", (SW-w)//2, 300, 80)
    """
    f = font if font is not None else _default_font
    return measure_text(f, str(s), size)


# ── 図形・テクスチャ ──────────────────────────────────────────

def fill(x: float, y: float, w: float, h: float,
         color=(255,255,255), alpha: int = 255):
    """矩形を塗り潰す。

    Example::
        kagra.fill(0, 0, SW, 60, (20,22,38))
        kagra.fill(0, 0, SW, SH, (0,0,0), alpha=128)
    """
    r,g,b,a = _c(color, alpha)
    rect(x, y, w, h, r, g, b, a)

def load(path: str) -> int:
    """テクスチャを読み込んでIDを返す。

    Example::
        player_tex = kagra.load("assets/player.png")
    """
    return load_texture(path)

def image(tex: int, x: float, y: float, w: float = None, h: float = None,
          *, alpha: float = 1., rotation: float = 0.,
          flip_x: bool = False, flip_y: bool = False,
          sx: float = 0., sy: float = 0., sw: float = None, sh: float = None):
    """テクスチャを描画する。

    Example::
        kagra.image(player_tex, x, y, 64, 64)
        kagra.image(tex, x, y, flip_x=True, alpha=0.8)
    """
    draw_texture(tex, x, y, w, h, sx, sy, sw, sh, alpha, rotation, .5, .5, flip_x, flip_y)

def image_world(tex: int, wx: float, wy: float, w: float, h: float,
                *, alpha: float = 1., flip_x: bool = False, flip_y: bool = False):
    """ワールド座標でテクスチャを描画（アクティブカメラで自動変換）。

    Example::
        kagra.image_world(tex, player.x-24, player.y-24, 48, 48)
    """
    if _camera:
        sx,sy = _camera.to_screen(wx, wy)
        draw_texture(tex, sx, sy, w*_camera.zoom, h*_camera.zoom,
                     0,0,None,None, alpha,0,0.,0.,flip_x,flip_y)
    else:
        draw_texture(tex, wx, wy, w, h, 0,0,None,None, alpha,0,0.,0.,flip_x,flip_y)


# ── UI ────────────────────────────────────────────────────────

def button(x: float, y: float, w: float, h: float, label: str = "",
           *, bg=(70,70,90), hover=(100,100,150),
           color=(255,255,255), size: int = 20, font: int = None) -> bool:
    """UIボタン。クリックされたとき True を返す。

    Example::
        if kagra.button(400, 300, 200, 60, "スタート"):
            kagra.go(GameScene())
        if kagra.button(x, y, 200, 50, "削除", bg=(150,30,30)):
            ...
    """
    f = font if font is not None else _default_font
    return bool(draw_ui_button(x,y,w,h,label,
                               font_size=size,
                               bg_color=bg, hover_color=hover, text_color=color,
                               font_id=f))

def bar(x: float, y: float, w: float, h: float,
        value: float, max_value: float = 100,
        *, bg=(25,25,35), fill=(50,220,80)):
    """プログレスバーを描画する。

    Example::
        kagra.bar(40, y, 160, 10, hp, 100, fill=(60,220,80))
    """
    draw_ui_progress_bar(x,y,w,h, float(max_value), float(value),
                         bg_color=bg, fill_color=fill)


# ── 入力 ──────────────────────────────────────────────────────

def key(name: str) -> bool:
    """キーが押し続けられているか。

    Example::
        if kagra.key("LEFT"): player.x -= speed * dt
    """
    return key_down(_key_code(name))

def pressed(name: str) -> bool:
    """このフレームでキーが押されたか（1フレームだけ True）。

    Example::
        if kagra.pressed("Z"): jump()
    """
    return key_pressed(_key_code(name))

def released(name: str) -> bool:
    """このフレームでキーが離されたか。"""
    return key_released(_key_code(name))

def mouse() -> tuple:
    """マウス座標を (x, y) で返す。"""
    return mouse_pos()

def mouse_btn(button_id: int = 1) -> bool:
    """マウスボタンが押し続けられているか（1=左, 2=右, 3=中）。"""
    return mouse_down(button_id)

def mouse_click(button_id: int = 1) -> bool:
    """このフレームでマウスボタンが押されたか。"""
    return mouse_pressed(button_id)


# ── シーン管理 ────────────────────────────────────────────────

def go(next_scene: Scene) -> None:
    """シーンを切り替える。

    Example::
        if kagra.button(400, 300, 200, 60, "スタート"):
            kagra.go(GameScene())
    """
    scene.change(next_scene)

def push(next_scene: Scene) -> None:
    """現在シーンを残したまま新しいシーンをスタックに積む。"""
    scene.push(next_scene)

def pop() -> None:
    """スタックから直前のシーンに戻る。"""
    scene.pop()


# ── オーディオ ────────────────────────────────────────────────

def bgm(path: str, loop: bool = True, vol: float = 0.8) -> None:
    """BGM を再生する。

    Example::
        kagra.bgm("assets/bgm/title.ogg")
        kagra.bgm("assets/bgm/race.ogg", loop=False, vol=0.6)
    """
    play_bgm(path, loop_=loop, volume=vol)

def se(path: str, vol: float = 1.0) -> None:
    """効果音を再生する。

    Example::
        kagra.se("assets/se/coin.wav")
    """
    play_se(path, volume=vol)


# ── VRM / 3D シンプル ─────────────────────────────────────────

def avatar(vrm_path: str) -> "VrmAvatar":
    """VRM キャラクターを読み込んで VrmAvatar を返す。

    アニメーション・スプリングボーン・ブレンドシェイプを1オブジェクトで管理。

    Example::
        # on_enter で一度だけ
        self.av = kagra.avatar("assets/Emma.vrm")
        self.av.load_motion("dance", "assets/dance.bvh")

        # 毎フレーム
        self.av.play("walk")
        self.av.update(dt)
        kagra.draw_vrm(self.av.vrm_id)

        # 表情
        self.av.set_expression("Fcl_ALL_Joy", 0.8)
        self.av.reset_expressions()

        # 利用可能クリップ
        print(self.av.clips)
    """
    _check()
    from kagra.vrm_avatar import VrmAvatar
    return VrmAvatar(vrm_path)

def load_fbx(path: str, clip_name: str = None) -> "FbxMotion":
    """FBX ファイルを直接読み込む（Blender 変換不要）。

    Args:
        path:      FBX ファイルのパス（Mixamo / DeepMotion 等）
        clip_name: 使用するクリップ名（省略時は最初のクリップ）

    Example::
        # シンプル版（推奨）
        avatar.load_motion("dance", "assets/hiphop.fbx")

        # クリップを確認してから使う
        motion = kagra.load_fbx("assets/hiphop.fbx")
        print(motion.clip_names)
        avatar.add_motion("dance", motion)
        avatar.play("dance")
    """
    _check()
    from kagra.fbx_player import load_fbx as _load_fbx
    return _load_fbx(path, clip_name=clip_name)


def load_bvh(path: str, extra_map: dict = None) -> "BvhMotion":
    """BVH ファイルを読み込んで BvhMotion を返す。

    通常は avatar.load_motion() で1行で済む。

    Example::
        motion = kagra.load_bvh("assets/walk.bvh")
        print(f"{motion.fps:.0f}fps  {motion.duration:.1f}sec")
        avatar.add_motion("walk", motion)
    """
    _check()
    from kagra.bvh_player import load_bvh as _lbvh
    return _lbvh(path, extra_map=extra_map)

def spring_bone(vrm_path: str, vrm_id: int) -> "SpringBone":
    """スプリングボーンシミュレーターを生成する。
    通常は kagra.avatar() が内部で自動生成する。
    """
    return SpringBone(vrm_path, vrm_id)



# ── 遅延インポート（kagra.Scene が定義された後に行う）──────────
from kagra.scene_io      import serialize_entity, serialize_transform, serialize_component, save_scene
from kagra.scene_loader  import load_entity, load_scene
from kagra.scenegraph    import SceneGraph
from kagra.prefab        import Prefab
from kagra.asset_scan    import scan_assets
from kagra.asset_manifest import AssetManifest
from kagra.asset_db      import AssetDatabase
from kagra.debug_tools   import debug_info as asset_debug_info
from kagra.scene_runtime import SceneRuntime
from kagra.scriptable    import (
    DataObject, DataRegistry, register_spawn_rule, spawn_rule,
    spawn_from, get_data_registry, set_data_dir,
)
from kagra.timeline      import (
    Timeline, TimelinePlayer, Track,
    EntityAnimTrack, CameraTrack, EventTrack,
)
from kagra.anim_io       import (
    save_timeline, load_timeline,
    save_state_machine, load_state_machine,
    save_clips, load_clips_into,
    save_all, list_saved,
)
from kagra.physics       import BoxCollider, Rigidbody, PhysicsSystem, TopDownPhysicsSystem
from kagra.physics3d     import Physics3D, RigidBody3D, AABB
from kagra.bgm_sync      import BgmSync, BgmCue, RhythmJudge, LiveScore
from kagra.vrm_loader    import VrmModel
from kagra.vrm_anim      import VrmAnimator, PoseKeyframe
from kagra.vrm_spring    import SpringBone

# ── 後方互換エイリアス ─────────────────────────────────────────
# 古い API 名で呼んでも動くようにする

get_blend_shape_names = list_blend_shapes   # 旧名 → 新名

try:
    from kagra.vrm_avatar import VrmAvatar, PRESETS as avatar_presets
except ImportError:
    pass

