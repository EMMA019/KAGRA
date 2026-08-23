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
#   kagra.stage()         load_gltf() / スカイ球
#
# ============================================================
from __future__ import annotations
from typing import Optional
import sys, types
from pathlib import Path

try:
    from kagra.kagra_core import Engine as _Engine
except ImportError as e:
    from kagra.launch import format_core_import_error
    raise ImportError(
        format_core_import_error(e, loaded_from=Path(__file__).resolve().parent)
    ) from e

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
from kagra.color_utils   import clamp_u8 as _clamp_u8
from kagra.color_utils   import norm_color as _norm_color
from kagra.color_utils   import resolve_rgb as _resolve_rgb
assets = _AssetManager()


# ── エンジン状態 ──────────────────────────────────────────────
_engine: _Engine | None = None
_camera: Camera | None  = None

def _check():
    if _engine is None:
        raise RuntimeError("kagra.init() を先に呼んでください")


# ═══════════════════════════════════════════════════════════════
#  公式API: エンジンインスタンスの取得
# ═══════════════════════════════════════════════════════════════

def get_engine() -> _Engine:
    """シングルトンのエンジンインスタンスを返す（公式API）。

    kagra.init() の後に呼ぶこと。

    Example::
        engine = kagra.get_engine()
        engine.set_vrm_bone_rot(vrm_id, "J_Bip_C_Neck", 0.0, 0.1, 0.0, 0.99)
    """
    _check()
    return _engine


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
    # エンジン側の動的解決を優先（あれば）
    if _engine is not None:
        code = _engine.get_key_code(name)
        if code is not None:
            return code
    # 後方互換: keymap.json 由来の keys から探す
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


def init(width=1280, height=720, title="KAGRA Game", fps=60, transparent=False, decorations=True, always_on_top=False, visible=True):
    """エンジンを初期化する。プログラムの最初に1回だけ呼ぶ。

    visible=False でエージェント検証向けの隠れウィンドウになる。
    Windows では DXGI の都合上ウィンドウ自体は作られるが、画面外へ退避する。
    """
    global _engine
    _engine = _Engine(
        width=width,
        height=height,
        title=title,
        fps=fps,
        transparent=transparent,
        decorations=decorations,
        always_on_top=always_on_top,
        visible=visible,
    )
    _update_keys()

def run(update=None, draw=None, start_scene: Scene = None, max_frames=None, fixed_dt=None,
        on_ready=None):
    """ゲームループを開始する（Phase 9 フック付き）。

    Args:
        max_frames: 指定フレーム数だけ描画したら自動終了（エージェント検証向け）
        fixed_dt: 壁時計ではなく固定の dt（秒）を update に渡す。決定論的再生用。
                  max_frames / fixed_dt 指定時は FPS 待ちをせず全速で回る。
        on_ready: レンダラ準備後・最初の update 直前に一度だけ呼ぶ。
                  ``kagra.avatar()`` / ``kagra.font()`` はここで呼ぶ
                  （Windows では run() の外だと Renderer not initialized）。
    """
    _check()
    from kagra.ready import wrap_on_ready

    if start_scene is not None:
        original_scene_update = scene._update

        def _patched_scene_update(dt: float):
            _phase9_frame_hook(dt)
            original_scene_update(dt)

        scene._stack.clear()
        scene._pending.clear()
        scene.change(start_scene)
        _engine.run(
            wrap_on_ready(_patched_scene_update, on_ready),
            scene._draw,
            max_frames,
            fixed_dt,
        )
    else:
        patched_update = _make_phase9_update_wrapper(update) if update else update
        _engine.run(wrap_on_ready(patched_update, on_ready), draw, max_frames, fixed_dt)


def quit():
    """次のフレーム境界でループを終了する。"""
    _check()
    _engine.request_exit()


def screenshot(path: str):
    """次に描画されるフレームを PNG として保存する。

    update() / draw() の中から呼ぶ。実際の書き込みは GPU 描画の直後。
    """
    _check()
    _engine.request_screenshot(str(path))


def set_grab_frames(enabled: bool = True):
    """GPU から毎フレーム RGB を取り出す（仮想カメラ用）。

    720p 推奨。毎フレーム readback するので 1080p は重い。
    ``kagra[stream]`` の VirtualCam が内部で呼ぶ。
    """
    _check()
    _engine.set_grab_frames(bool(enabled))


def grab_frame():
    """直前フレームの ``(width, height, rgb_bytes)``。無ければ None。

    1 回読むと消える。``update()`` の先頭で取る（描画は 1 フレーム遅れ）。
    """
    _check()
    return _engine.grab_frame()


def frame_count() -> int:
    """run() 開始後に完了したフレーム数。"""
    _check()
    return int(_engine.frame_count())


def inject_key(name: str, down: bool = True):
    """次フレームの update 直前にキーイベントを注入する（OS 経由なし）。

    Example::
        if tick_count() == 5:
            inject_key("1")          # フレーム 6 で pressed("1") が True
        if tick_count() == 20:
            inject_key("1", down=False)
    """
    _check()
    code = _key_code(name)
    if down:
        _engine.inject_key_down(code)
    else:
        _engine.inject_key_up(code)


def inject_mouse(x=None, y=None, button=None, down=None):
    """次フレームの update 直前にマウス状態を注入する。

    Args:
        x, y: カーソル位置（どちらか一方でも指定可。片方だけならもう片方は無視）
        button: 1=左, 2=右, 3=中
        down: True=押す / False=離す（button 必須）
    """
    _check()
    if x is not None and y is not None:
        _engine.inject_mouse_move(float(x), float(y))
    if button is not None and down is not None:
        if down:
            _engine.inject_mouse_down(int(button))
        else:
            _engine.inject_mouse_up(int(button))


# ═══════════════════════════════════════════════════════════════
#  低レベル描画 API（既存コードとの後方互換を維持）
# ═══════════════════════════════════════════════════════════════

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

def circle(x: float, y: float, radius: float,
           r: int = 255, g: int = 255, b: int = 255, a: int = 255,
           segments: int = 24):
    """円を描く（スキャンライン rect 方式）。"""
    import math
    _check()
    ri = int(radius)
    for dy in range(-ri, ri+1):
        dx = int(math.sqrt(max(0, radius*radius - dy*dy)))
        if dx <= 0: continue
        rect(x - dx, y + dy, dx*2, 1, r, g, b, a)


def draw_polygon(verts: list, r=255, g=255, b=255, a=255, color=None):
    """多角形を塗り潰す（低レベル。シンプルAPIは polygon() を使う）。

    Args:
        verts: 頂点リスト [[x1,y1], [x2,y2], ...]（3点以上）
        color: (r,g,b) タプル（指定時は r,g,b,a より優先）
    """
    _check()
    if color is not None:
        rr,gg,bb,aa = _norm_color(color, a)
    else:
        rr,gg,bb,aa = _clamp_u8(r), _clamp_u8(g), _clamp_u8(b), _clamp_u8(a)
    _engine.draw_polygon(verts, rr, gg, bb, aa)

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

def backspace_pressed():
    """バックスペースキーが押された瞬間だけ True を返す"""
    return _engine.backspace_pressed()

def enter_pressed():
    """Enterキーが押された瞬間だけ True を返す"""
    return _engine.enter_pressed()

def escape_pressed():
    """Escapeキーが押された瞬間だけ True を返す"""
    return _engine.escape_pressed()

def focus_window():
    """ウィンドウにキーボードフォーカスを要求する"""
    _check(); _engine.focus_window()

def drag_window():
    """ウィンドウのドラッグ移動を開始する"""
    _check(); _engine.drag_window()

def set_window_position(x: int, y: int):
    """ウィンドウの位置を設定する"""
    _check(); _engine.set_window_position(x, y)

def set_click_through(enabled: bool):
    """ウィンドウのマウスクリック透過を設定する"""
    _check(); _engine.set_click_through(enabled)

def set_always_on_top(enabled: bool):
    """最前面表示のオンオフを設定する"""
    _check(); _engine.set_always_on_top(enabled)

def set_decorations(enabled: bool):
    """ウィンドウの枠の有無を設定する"""
    _check(); _engine.set_decorations(enabled)

def set_window_title(title: str):
    """ウィンドウタイトルを設定する"""
    _check(); _engine.set_window_title(title)

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

_camera3d: Camera3D | None = None

def get_camera3d() -> Camera3D | None:
    """シングルトンの 3D カメラインスタンスを返す。

    事前に set_camera3d() で設定が必要。

    Example::
        cam = kagra.get_camera3d()
        if cam:
            cam.update(kagra.get_engine())
    """
    return _camera3d

def set_camera3d(cam: Camera3D | None):
    """3D カメラを設定する。

    Example::
        cam = Camera3D(1280, 720)
        cam.use_orbit(radius=2.5, target=(0, 0.9, 0))
        kagra.set_camera3d(cam)
    """
    global _camera3d
    _camera3d = cam

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
def get_screen_size() -> tuple:
    """現在のウィンドウサイズを返す（リサイズ後も正しい値）。"""
    _check()
    return (_engine.screen_width(), _engine.screen_height())

def screen_w() -> int:
    """画面の幅を返す。"""
    _check()
    try:
        return _engine.screen_width()
    except AttributeError:
        return 1280

def screen_h() -> int:
    """画面の高さを返す。"""
    _check()
    try:
        return _engine.screen_height()
    except AttributeError:
        return 720


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
    """3D カメラの view / proj を直接指定する（各 16 要素・行優先）。

    一度呼ぶと組み込みカメラ（engine.orbit_camera 等）は無効になる。
    """
    _check(); _engine.update_camera_3d(view, proj)

def set_light_dir(x: float, y: float, z: float):
    """3D 平行光の方向を設定する（光源へ向かうベクトル）。

    正規化はエンジン側。デフォルトは (0.3, 1.0, 0.5)。
    mesh3d / VRM スキニングの両方に効く。
    """
    _check(); _engine.set_light_dir(x, y, z)

def set_rim(intensity: float = 0.45):
    """グローバルリム（フレネル + 逆光 + 床バウンス）。

    0 でオフ（既定。ゴールデン画像を変えない）。デモは ``apply_live_look``。
    """
    _check(); _engine.set_rim(float(intensity))

def apply_live_look(*, mascot: bool = False):
    """デモ既定の光・トゥーン・ブルーム・リム・フォグ。"""
    from kagra.look import apply_live_look as _apply
    _apply(mascot=mascot)

def draw_vignette(sw: int | None = None, sh: int | None = None, strength: float = 0.42):
    """画面端を落とす。draw() の 3D のあと、HUD の前。"""
    from kagra.look import draw_vignette as _draw
    if sw is None or sh is None:
        sw, sh = get_screen_size()
    _draw(int(sw), int(sh), strength)

def set_shadow_enabled(enabled: bool = True):
    """平行光シャドウマップの有効/無効。"""
    _check(); _engine.set_shadow_enabled(bool(enabled))

def set_bloom(threshold: float = 0.85, intensity: float = 0.35,
              enabled: bool = True):
    """閾値ブルーム。高輝度画素だけを抽出して加算する。

    画面全体をぼかさない。目のハイライト・アウトライン・MToon rimLift
    付近だけが光る。intensity<=0 または enabled=False でオフ。
    """
    _check()
    inten = float(intensity) if enabled else 0.0
    _engine.set_bloom(float(threshold), inten)


def set_ambient(r: float = 0.22, g: float = 0.20, b: float = 0.28,
                strength: float = 0.28):
    """半球アンビエント（簡易 IBL）。strength=0 でオフ。"""
    _check()
    _engine.set_ambient(float(r), float(g), float(b), float(strength))


def set_mesh_cull(enabled: bool = True):
    """ワールド 3D メッシュの視錐台カリング。VRM スキンは対象外。"""
    _check()
    _engine.set_mesh_cull(bool(enabled))


def render_stats() -> dict:
    """直前フレームの 3D 描画統計。

    ``draw_calls`` / ``triangles`` / ``culled``。ワールド箱と VRM の
    カラーパス（アウトライン含む）。影パスは含めない。
    最初のフレームの前は全部 0。
    """
    _check()
    calls, tris, culled = _engine.render_stats()
    return {
        "draw_calls": int(calls),
        "triangles": int(tris),
        "culled": int(culled),
    }


def camera_ray_from_screen(sx: float, sy: float):
    """スクリーン座標からワールドレイ ((ox,oy,oz), (dx,dy,dz))。"""
    _check()
    hit = _engine.camera_ray_from_screen(float(sx), float(sy))
    if hit is None:
        return None
    ox, oy, oz, dx, dy, dz = hit
    return (ox, oy, oz), (dx, dy, dz)


def camera_world_to_screen(wx: float, wy: float, wz: float):
    """ワールド → スクリーン。アクティブ ``Camera3D`` が必要。外れは None。"""
    cam = get_camera3d()
    if cam is None:
        return None
    return cam.world_to_screen(wx, wy, wz)


def pick_vrm_bone(vrm_id: int, ox: float, oy: float, oz: float,
                  dx: float, dy: float, dz: float, max_dist: float = 100.0):
    """レイが当たった humanoid ボーン名。なければ None。"""
    _check()
    return _engine.pick_vrm_bone(
        int(vrm_id), float(ox), float(oy), float(oz),
        float(dx), float(dy), float(dz), float(max_dist),
    )


def set_toon_params(threshold: float = 0.5, softness: float = 1.0,
                    shade: float = 0.55, lit: float = 1.0):
    """VRM スキニング用のトゥーン階調を設定する。

    Args:
        threshold: 明暗境界（half-Lambert 0〜1）
        softness: 0 で硬い2階調。大きいほど柔らかい。
                  ≥0.999 で従来の連続 half-Lambert（デフォルト）
        shade: 影側の明るさ
        lit: 光側の明るさ
    """
    _check(); _engine.set_toon_params(threshold, softness, shade, lit)

def draw_mesh_3d(texture_id: int, verts: list, indices: list):
    _check(); _engine.draw_mesh_3d(texture_id, verts, indices)


def upload_mesh_3d(texture_id: int, verts: list, indices: list) -> int:
    """3D メッシュを GPU に一度載せる。毎フレームは ``draw_mesh_id``。

    ``verts`` は ``[x,y,z,nx,ny,nz,u,v]``。0 は失敗。
    """
    _check()
    return int(_engine.upload_mesh_3d(texture_id, verts, indices))


def draw_mesh_id(mesh_id: int):
    """``upload_mesh_3d`` で載せたメッシュを描く。"""
    _check()
    _engine.draw_mesh_id(int(mesh_id))


def draw_mesh_instances(mesh_id: int, instances: list):
    """保持メッシュをインスタンス描画する。

    各行は ``[x,y,z]`` / ``[x,y,z,scale]`` / ``[x,y,z,sx,sy,sz]`` /
    ``[x,y,z,sx,sy,sz,yaw]``。
    """
    _check()
    _engine.draw_mesh_instances(int(mesh_id), instances)


def unload_mesh_3d(mesh_id: int):
    """保持メッシュを解放する。"""
    _check()
    _engine.unload_mesh_3d(int(mesh_id))


def texture_from_fn(width: int, height: int, pixel_fn, *, name: str | None = None) -> int:
    """手続きテクスチャ。``pixel_fn(x, y) -> (r,g,b) or (r,g,b,a)``。

    Example::
        tex = kagra.texture_from_fn(32, 32, lambda x, y: (255, 200, 40, 255))
    """
    from kagra.gamekit import write_png
    return load(str(write_png(width, height, pixel_fn, name=name)))


def texture_from_pixels(width: int, height: int, pixels: bytes, *, name: str | None = None) -> int:
    """未圧縮 RGBA（上から、1 画素 4 バイト）をテクスチャにする。"""
    from kagra.look import encode_png_rgba
    import tempfile
    from pathlib import Path
    data = encode_png_rgba(width, height, pixels)
    path = Path(tempfile.gettempdir()) / f"kagra_{name or 'pix'}_{width}x{height}.png"
    path.write_bytes(data)
    return load(str(path))


def billboard_mesh(x: float, y: float, z: float, size: float, camera=None, *, yaw: float | None = None):
    """カメラ向き四角の ``(verts, indices)``。``draw_mesh_3d`` に渡す。"""
    from kagra.gamekit import billboard_mesh as _fn
    return _fn(x, y, z, size, camera, yaw=yaw)


def disk_mesh(cx: float, cy: float, cz: float, radius: float, segs: int = 48):
    """Y 上向き円盤の ``(verts, indices)``。床用。"""
    from kagra.gamekit import disk_mesh as _fn
    return _fn(cx, cy, cz, radius, segs)


def quad_y_mesh(cx: float, cy: float, cz: float, size: float):
    """Y 上向き正方形（半辺 ``size``）の ``(verts, indices)``。"""
    from kagra.gamekit import quad_y_mesh as _fn
    return _fn(cx, cy, cz, size)


def box_mesh(cx: float, cy: float, cz: float, w: float, h: float, d: float):
    """軸平行の箱の ``(verts, indices)``。``cy`` は中心。"""
    from kagra.gamekit import box_mesh as _fn
    return _fn(cx, cy, cz, w, h, d)


def sphere_mesh(cx: float = 0.0, cy: float = 0.0, cz: float = 0.0,
                radius: float = 0.5, segs: int = 16):
    """UV 球の ``(verts, indices)``。既定は直径 1。"""
    from kagra.gamekit import sphere_mesh as _fn
    return _fn(cx, cy, cz, radius, segs)


def cylinder_mesh(cx: float = 0.0, cy: float = 0.0, cz: float = 0.0,
                  radius: float = 0.5, height: float = 1.0, segs: int = 16):
    """Y 軸円柱の ``(verts, indices)``。既定は直径 1・高さ 1。"""
    from kagra.gamekit import cylinder_mesh as _fn
    return _fn(cx, cy, cz, radius, height, segs)


def solid_tex(color):
    """1 色テクスチャ。``orange`` または ``(r,g,b)``。"""
    from kagra.play import solid_tex as _fn
    return _fn(color)


def sky(*, radius: float = 18.0, look: bool = True):
    """プロシージャル空。初回は ``apply_live_look``。"""
    from kagra.play import sky as _fn
    return _fn(radius=radius, look=look)


def hovered_prop(cam=None, sx: float | None = None, sy: float | None = None, *, max_dist: float = 80.0):
    """画面上の点から当たった ``Prop``。``cam`` 省略時は ``get_camera3d()``。

    ``sx`` / ``sy`` を省略するとマウス位置。床の ``plane`` は除外。
    レイ計算は ``Camera3D.ray_from_screen``。エンジン無しのテストは
    ``kagra.play.hovered_prop(ox, oy, oz, dx, dy, dz)``。
    """
    from kagra.play import hovered_prop as _pick

    if cam is None:
        cam = get_camera3d()
    if cam is None:
        return None
    if sx is None or sy is None:
        sx, sy = mouse_pos()
    ray = cam.ray_from_screen(float(sx), float(sy))
    if ray is None:
        return None
    (ox, oy, oz), (dx, dy, dz) = ray
    return _pick(ox, oy, oz, dx, dy, dz, max_dist=float(max_dist))


def destroy(prop) -> None:
    """``Prop`` を描画・ホバー・衝突から外す。子も消す。既に消えていても落ちない。"""
    from kagra.play import destroy as _fn
    _fn(prop)


def draw_billboard(tex: int, x: float, y: float, z: float, size: float, camera=None, *, yaw: float | None = None):
    """3D 空間にカメラ向きのスプライトを置く。"""
    verts, idx = billboard_mesh(x, y, z, size, camera, yaw=yaw)
    draw_mesh_3d(tex, verts, idx)


_billboard_unit: dict[int, int] = {}


def draw_billboard_instances(tex: int, items, camera=None, *, yaw: float | None = None):
    """複数ビルボードを 1 ドロー。``items`` は ``(x,y,z,size)``。"""
    from kagra.gamekit import _yaw_of

    mid = _billboard_unit.get(int(tex))
    if not mid:
        verts, idx = billboard_mesh(0.0, 0.0, 0.0, 1.0, yaw=0.0)
        mid = upload_mesh_3d(int(tex), verts, idx)
        if mid:
            _billboard_unit[int(tex)] = mid
    if not mid:
        return
    theta = _yaw_of(camera, yaw)
    inst = []
    for it in items:
        if len(it) < 3:
            continue
        s = float(it[3]) if len(it) > 3 else 1.0
        inst.append([float(it[0]), float(it[1]), float(it[2]), s, s, s, theta])
    if inst:
        draw_mesh_instances(mid, inst)


def load_gltf(path: str) -> int:
    """glTF / GLB を読み込む。通常は kagra.stage() を使う。

    ``.gltf`` は隣の ``.bin`` と画像 URI を読む。Windows では
    ``on_ready`` / ``run()`` のあと（Renderer 作成後）に呼ぶ。
    """
    _check()
    return _engine.load_gltf(str(path))


def draw_gltf(model_id: int):
    """読み込んだ glTF を描く。draw() の中で呼ぶ。"""
    _check()
    _engine.draw_gltf(int(model_id))


def unload_gltf(model_id: int):
    """glTF を解放する。"""
    _check()
    _engine.unload_gltf(int(model_id))


# ── GPU Boids API ────────────────────────────────────────────

def create_boid_system_gpu(count: int, width: float = 1280.0, height: float = 720.0) -> int:
    """GPU Compute Shader によるボイドシステムを作成する。

    Args:
        count:  最大ボイド数（バッファは一度だけ確保）
        width:  シミュレーション幅
        height: シミュレーション高さ

    Returns:
        boid_id: update_boids_gpu / draw_boids_gpu で使う ID

    Example::
        boid_id = kagra.create_boid_system_gpu(1_000_000)
        kagra.update_boids_gpu(boid_id, dt)
        kagra.draw_boids_gpu(boid_id)
    """
    _check(); return _engine.create_boid_system_gpu(count, width, height)


def set_boid_active_count(boid_id: int, count: int):
    """アクティブなボイド数を変更する（バッファ再確保なし）。"""
    _check(); _engine.set_boid_active_count(boid_id, count)


def update_boids_gpu(boid_id: int, dt: float):
    """GPU でボイドを1フレーム更新する（CPU 転送ゼロ）。"""
    _check(); _engine.update_boids_gpu(boid_id, dt)


def draw_boids_gpu(boid_id: int):
    """GPU ボイドを描画する。draw() の中で呼ぶ。"""
    _check(); _engine.draw_boids_gpu(boid_id)


def create_boid_system(count: int, width: float = 1280.0, height: float = 720.0) -> int:
    """CPU（Rust + rayon）によるボイドシステムを作成する。"""
    _check(); return _engine.create_boid_system(count, width, height)


def update_boids(boid_id: int, dt: float):
    """CPU でボイドを1フレーム更新する。"""
    _check(); _engine.update_boids(boid_id, dt)


def draw_boids(boid_id: int, batch_id: int, sprite_w: float = 6.0, sprite_h: float = 3.0):
    """CPU ボイドをバッチに転送して描画する。"""
    _check(); _engine.draw_boids(boid_id, batch_id, sprite_w, sprite_h)


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

def set_vrm_first_person(vrm_id: int, enabled: bool = True):
    """一人称視点レイヤー。True で頭メッシュ（Auto / ThirdPersonOnly）を隠す。"""
    _check(); _engine.set_vrm_first_person(vrm_id, bool(enabled))

def vrm_spring_info(vrm_id: int) -> tuple:
    """SpringBone の (chains, joints, colliders)。未ロードは (0,0,0)。"""
    _check(); return _engine.vrm_spring_info(vrm_id)

def step_vrm_spring(vrm_id: int, dt: float):
    _check(); _engine.step_vrm_spring(vrm_id, float(dt))

def reset_vrm_spring(vrm_id: int):
    _check(); _engine.reset_vrm_spring(vrm_id)

def set_vrm_spring_wind(vrm_id: int, x: float = 0.0, y: float = 0.0, z: float = 0.0):
    _check(); _engine.set_vrm_spring_wind(vrm_id, float(x), float(y), float(z))

def set_vrm_spring_enabled(vrm_id: int, enabled: bool = True):
    _check(); _engine.set_vrm_spring_enabled(vrm_id, bool(enabled))

def set_vrm_pose(vrm_id: int, bones: list):
    """ライブモーキャプ。``[(name, qx, qy, qz, qw), ...]`` をまとめて書く。"""
    _check()
    packed = []
    for item in bones:
        if len(item) < 5:
            continue
        packed.append((str(item[0]), float(item[1]), float(item[2]), float(item[3]), float(item[4])))
    _engine.set_vrm_pose(vrm_id, packed)

def list_human_bones(vrm_id: int) -> list[str]:
    """VRM humanoid 標準ボーン名の一覧を返す（hips, head, leftUpperArm, …）。"""
    _check(); return _engine.list_human_bones(vrm_id)

def resolve_vrm_bone(vrm_id: int, name: str) -> int | None:
    """ボーン名をノード index に解決する。

    実ノード名 / VRM 標準名（head） / VRoid 名（J_Bip_C_Head）のいずれでも可。
    見つからなければ None。
    """
    _check(); return _engine.resolve_vrm_bone(vrm_id, name)

def has_vrm_bone(vrm_id: int, name: str) -> bool:
    """ボーン名がこの VRM で使えるか。"""
    _check(); return _engine.has_vrm_bone(vrm_id, name)

def get_vrm_look_at(vrm_id: int) -> dict | None:
    """VRM LookAt メタデータを dict で返す。未定義なら None。

    Returns:
        {
          "type": "bone" | "expression",
          "offsetFromHeadBone": [x, y, z],
          "rangeMapHorizontalInner": {"inputMaxValue": f, "outputScale": f},
          "rangeMapHorizontalOuter": {...},
          "rangeMapVerticalDown": {...},
          "rangeMapVerticalUp": {...},
        }
    """
    _check()
    raw = _engine.get_vrm_look_at(vrm_id)
    if raw is None:
        return None
    (typ, ox, oy, oz,
     hi_in, hi_out, ho_in, ho_out,
     vd_in, vd_out, vu_in, vu_out) = raw
    def _rm(inp, out):
        return {"inputMaxValue": float(inp), "outputScale": float(out)}
    return {
        "type": typ,
        "offsetFromHeadBone": [float(ox), float(oy), float(oz)],
        "rangeMapHorizontalInner": _rm(hi_in, hi_out),
        "rangeMapHorizontalOuter": _rm(ho_in, ho_out),
        "rangeMapVerticalDown": _rm(vd_in, vd_out),
        "rangeMapVerticalUp": _rm(vu_in, vu_out),
    }

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

import os as _os
from kagra.fonts import find_system_font as _find_system_font


def font(path: str = None) -> int:
    """フォントを読み込み、デフォルト登録してIDを返す。

    path を省略するとシステムフォントを自動検出する。
    指定したパスが存在しなければ自動フォールバック。

    Example::
        kagra.font()  # システムフォントを自動選択
        kagra.font()  # システムフォントを自動選択
        kagra.text("スコア", 20, 20, 28)
    """
    global _default_font

    if path is None:
        found = _find_system_font()
        if found:
            path = found
        else:
            raise RuntimeError(
                "システムフォントが見つかりません。"
                "kagra.font('path/to/font.ttf') で明示的に指定してください。"
            )
    elif not _os.path.isabs(path) and not _os.path.splitext(path)[1]:
        # 拡張子なし → assets.font 経由の名前解決
        from kagra import assets
        try:
            fid = assets.font(path)
            _default_font = fid
            return fid
        except Exception:
            pass

    # フォント読み込み（失敗時はフォールバック）
    try:
        fid = load_font(path)
    except Exception as e:
        fallback = _find_system_font()
        if fallback and fallback != path:
            try:
                fid = load_font(fallback)
            except Exception as e2:
                raise RuntimeError(
                    f"フォント読み込み失敗（フォールバックも不可）:\n"
                    f"  指定: {path} -> {e}\n"
                    f"  フォールバック: {fallback} -> {e2}"
                )
        else:
            raise RuntimeError(f"フォント読み込み失敗: {path} ({e})")

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


# ── 2D 描画基本図形（ライン・ポリゴン・ラウンド矩形）────────────
# これらの関数は既存の rect() / draw_mesh() を使って実装しているため、
# Rust コア側に変更を加えずに純 Python で動作する。

import math as _math


def line(x1: float, y1: float, x2: float, y2: float,
         color=(255,255,255), width: float = 1, alpha: int = 255):
    """線分を描画する（矩形の連続で近似）。

    Example::
        kagra.line(100, 100, 500, 300, (255,100,100), width=3)
    """
    from kagra.geom2d import line_rects
    r,g,b,a = _c(color, alpha)
    for rx, ry, rw, rh in line_rects(x1, y1, x2, y2, width):
        rect(rx, ry, rw, rh, r, g, b, a)



def line_h(x: float, y: float, length: float,
           color=(255,255,255), width: float = 1, alpha: int = 255):
    """水平線を描画する（line より高速）。

    Example::
        kagra.line_h(100, 200, 400, (200,200,200), width=2)
    """
    r,g,b,a = _c(color, alpha)
    rect(x, y - width / 2, length, width, r, g, b, a)


def line_v(x: float, y: float, length: float,
           color=(255,255,255), width: float = 1, alpha: int = 255):
    """垂直線を描画する（line より高速）。

    Example::
        kagra.line_v(300, 100, 500, (200,200,200), width=2)
    """
    r,g,b,a = _c(color, alpha)
    rect(x - width / 2, y, width, length, r, g, b, a)


def polygon(pts: list, color=(255,255,255), alpha: int = 255):
    """凸多角形を塗り潰す（三角形ファン分割）。

    Args:
        pts:  頂点リスト [(x1,y1), (x2,y2), ...]（3点以上）
        color: (r,g,b) タプル

    Example::
        kagra.polygon([(400,100), (500,300), (300,300)], (100,200,255))
    """
    if len(pts) < 3:
        return
    r,g,b,a = _c(color, alpha)

    # 三角形ファン: 最初の頂点を固定して扇状に三角形を描く
    x0, y0 = pts[0]
    for i in range(1, len(pts) - 1):
        x1, y1 = pts[i]
        x2, y2 = pts[i + 1]

        # 三角形のバウンディングボックス
        min_x = min(x0, x1, x2)
        max_x = max(x0, x1, x2)
        min_y = min(y0, y1, y2)
        max_y = max(y0, y1, y2)

        w = max_x - min_x
        h = max_y - min_y
        if w < 0.5 or h < 0.5:
            continue

        # スキャンライン方式で塗り潰し
        area2 = abs((x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0))
        if area2 < 0.001:
            continue

        for py in range(int(min_y), int(max_y) + 1):
            # 走査線と辺の交差を求める
            intersects = []
            edges = [(x0, y0, x1, y1), (x1, y1, x2, y2), (x2, y2, x0, y0)]
            for ex1, ey1, ex2, ey2 in edges:
                if (ey1 <= py < ey2) or (ey2 <= py < ey1):
                    t = (py - ey1) / (ey2 - ey1)
                    ix = ex1 + t * (ex2 - ex1)
                    intersects.append(ix)

            if len(intersects) >= 2:
                lx = min(intersects)
                rx = max(intersects)
                rect(lx, py, rx - lx, 1, r, g, b, a)


def polygon_outline(pts: list, color=(255,255,255), width: float = 1, alpha: int = 255):
    """多角形の輪郭線を描画する。

    Args:
        pts: 頂点リスト [(x1,y1), (x2,y2), ...]
        color: (r,g,b) タプル
        width: 線の太さ

    Example::
        kagra.polygon_outline([(100,100), (200,100), (200,200)],
                              (255,0,0), width=2)
    """
    n = len(pts)
    for i in range(n):
        x1, y1 = pts[i]
        x2, y2 = pts[(i + 1) % n]
        line(x1, y1, x2, y2, color, width, alpha)


def rounded_rect(x: float, y: float, w: float, h: float,
                 radius: float = 8, color=(255,255,255), alpha: int = 255):
    """角丸矩形を塗り潰す。

    Example::
        kagra.rounded_rect(100, 100, 300, 200, 16, (60,60,80))
    """
    r,g,b,a = _c(color, alpha)
    radius = max(0, min(radius, min(w, h) / 2))

    # 中央の矩形
    rect(x + radius, y, w - radius * 2, h, r, g, b, a)
    # 上下の矩形
    rect(x, y + radius, radius, h - radius * 2, r, g, b, a)
    rect(x + w - radius, y + radius, radius, h - radius * 2, r, g, b, a)

    # 4つの角（扇形→スキャンライン）
    def fill_corner(cx, cy, quad):
        for dy in range(-radius, 0):
            for dx in range(-radius, 0):
                if dx*dx + dy*dy <= radius*radius:
                    if quad == 0:
                        rect(cx + dx, cy + dy, 1, 1, r, g, b, a)
                    elif quad == 1:
                        rect(cx - dx - 1, cy + dy, 1, 1, r, g, b, a)
                    elif quad == 2:
                        rect(cx + dx, cy - dy - 1, 1, 1, r, g, b, a)
                    elif quad == 3:
                        rect(cx - dx - 1, cy - dy - 1, 1, 1, r, g, b, a)

    fill_corner(x + radius, y + radius, 0)       # 左上
    fill_corner(x + w - radius, y + radius, 1)    # 右上
    fill_corner(x + radius, y + h - radius, 2)    # 左下
    fill_corner(x + w - radius, y + h - radius, 3)  # 右下


def rounded_rect_outline(x: float, y: float, w: float, h: float,
                          radius: float = 8, color=(255,255,255),
                          width: float = 1, alpha: int = 255):
    """角丸矩形の輪郭線を描画する。

    Example::
        kagra.rounded_rect_outline(50, 50, 200, 100, 12, (255,200,100), width=2)
    """
    r,g,b,a = _c(color, alpha)
    radius = max(0, min(radius, min(w, h) / 2))

    # 4辺の直線部分
    rect(x + radius, y, w - radius * 2, width, r, g, b, a)            # 上
    rect(x + radius, y + h - width, w - radius * 2, width, r, g, b, a)  # 下
    rect(x, y + radius, width, h - radius * 2, r, g, b, a)            # 左
    rect(x + w - width, y + radius, width, h - radius * 2, r, g, b, a)  # 右

    # 4つの角（弧): ピクセル単位でドット打ち
    def arc_pixel(cx, cy, start_angle, end_angle):
        for a_deg in range(int(start_angle), int(end_angle), 1):
            a = _math.radians(a_deg)
            px = cx + radius * _math.cos(a)
            py = cy + radius * _math.sin(a)
            rect(px, py, width, width, r, g, b, a)

    arc_pixel(x + radius, y + radius, 180, 270)     # 左上
    arc_pixel(x + w - radius, y + radius, 270, 360)  # 右上
    arc_pixel(x + w - radius, y + h - radius, 0, 90) # 右下
    arc_pixel(x + radius, y + h - radius, 90, 180)   # 左下


def circle_fill(x: float, y: float, radius: float,
                color=(255,255,255), alpha: int = 255):
    """塗り潰し円を描画する（スキャンライン）。

    Example::
        kagra.circle_fill(400, 300, 80, (255,100,100))
    """
    r,g,b,a = _c(color, alpha)
    ri = int(radius)
    for dy in range(-ri, ri + 1):
        dx = int(_math.sqrt(max(0, radius*radius - dy*dy)))
        if dx > 0:
            rect(x - dx, y + dy, dx * 2, 1, r, g, b, a)


def circle_outline(x: float, y: float, radius: float,
                   color=(255,255,255), width: float = 1, alpha: int = 255):
    """円の輪郭線を描画する。

    Example::
        kagra.circle_outline(400, 300, 80, (255,255,255), width=2)
    """
    r, g, b, a = _c(color, alpha)
    # 多角形近似: 半径に応じて頂点数を決める
    segments = max(12, int(radius * 0.5))
    pts = []
    for i in range(segments):
        ang = 2 * _math.pi * i / segments
        px = x + radius * _math.cos(ang)
        py = y + radius * _math.sin(ang)
        pts.append((px, py))
    for i in range(segments):
        x1, y1 = pts[i]
        x2, y2 = pts[(i + 1) % segments]
        # 短い線は小さな矩形で
        mx, my = (x1 + x2) / 2, (y1 + y2) / 2
        dx, dy = x2 - x1, y2 - y1
        seg_len = _math.sqrt(dx*dx + dy*dy)
        if seg_len < 0.5:
            continue
        rect(mx - seg_len/2, my - width/2, seg_len, width, r, g, b, a)


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

def get_typed_chars() -> str:
    """このフレームで確定入力された文字列を返す。

    日本語 IME で変換確定した文字、ASCII の直接入力、
    バックスペース（\x08）が含まれる。

    Example::
        chars = kagra.get_typed_chars()
        for c in chars:
            if c == '\x08':
                text = text[:-1]
            else:
                text += c
    """
    _check()
    return _engine.get_typed_chars()


def get_preedit_text() -> str:
    """IME 変換中のテキストを返す（確定前）。

    日本語入力中に変換候補を選んでいる間の「よみがな」。
    入力欄に下線付きで表示するのに使う。

    Example::
        preedit = kagra.get_preedit_text()
        display = committed_text + preedit  # 変換中を末尾に表示
    """
    _check()
    return _engine.get_preedit_text()


def set_ime_cursor_pos(x: float, y: float):
    """IME 候補ウィンドウの表示位置を設定する。

    テキスト入力カーソルの位置（スクリーン座標）を渡す。
    これにより変換候補が入力位置の近くに表示される。

    Example::
        # 入力欄の位置に合わせる
        kagra.set_ime_cursor_pos(input_x, input_y)
    """
    _check()
    _engine.set_ime_cursor_pos(float(x), float(y))

def down(name: str) -> bool:
    """キーが押し続けられているか。"""
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


def tone(
    name: str,
    freqs,
    duration: float = 0.12,
    volume: float = 0.35,
    decay: bool = True,
) -> str:
    """合成トーンの WAV を書いてパスを返す。``kagra.se(path)`` で鳴らす。

    Example::
        coin = kagra.tone("coin", (880, 1320), duration=0.1)
        kagra.se(coin)
    """
    from kagra.gamekit import write_tone
    return str(write_tone(name, freqs, duration=duration, volume=volume, decay=decay))


def save_json(name: str, data: dict, *, directory: str | None = None):
    """小さな dict を JSON で残す（ハイスコア等）。``~/.kagra/saves`` か ``KAGRA_DATA``。

    アセット用の ``load_data`` とは別。こちらはゲーム進行の永続化。
    """
    from kagra.gamekit import save_json as _fn
    return _fn(name, data, directory=directory)


def load_json(name: str, default=None, *, directory: str | None = None):
    """``save_json`` の対。無ければ ``default``。"""
    from kagra.gamekit import load_json as _fn
    return _fn(name, default, directory=directory)


# ── VRM / 3D シンプル ─────────────────────────────────────────

def avatar(vrm_path: str) -> "VrmAvatar":
    """VRM キャラクターを読み込んで VrmAvatar を返す。

    アニメーション・スプリングボーン・ブレンドシェイプを1オブジェクトで管理。

    Example::
        # Scene.on_enter か run(on_ready=...) の中で一度だけ
        # （Windows では kagra.run() の外で呼ぶと Renderer not initialized）
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
    from pathlib import Path as _Path
    from kagra.contracts import KagraContractError
    from kagra.samples import ensure_vrm
    from kagra.vrm_avatar import VrmAvatar
    p = _Path(vrm_path)
    if not p.is_file():
        try:
            # ローカル assets → キャッシュ済みサンプル。ダウンロードはしない
            # （明示的な python -m kagra.demo / ensure_vrm(download=True) に任せる）
            vrm_path = str(ensure_vrm(vrm_path, download=False))
        except KagraContractError:
            raise
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


def stage(path: str = "stage", *, radius: float = 12.0) -> "Stage":
    """会場を読み込む。ダンスと同じくファイルを落とすだけ。

    Sketchfab のホール ``.glb`` / ``.gltf``、または空の PNG/JPEG を
    内側から貼るスカイ球。エンジン内でブルームを組まない。

    Example::
        hall = kagra.stage("assets/venue.glb")   # on_ready の中
        hall.draw()                               # draw() の中
    """
    _check()
    from kagra.stage import Stage
    return Stage.load(path, radius=radius)


def load_vrma(path: str, *, sample_fps: float = 30.0) -> "VrmaMotion":
    """VRM Animation (``.vrma``) を読み込んで VrmaMotion を返す。

    glTF + ``VRMC_vrm_animation``。どの VRM ヒューマノイドにも載せられる。
    エンジン未初期化でも読める（ファイルパーサ）。

    Example::
        motion = kagra.load_vrma("assets/wave.vrma")
        print(f"{motion.fps:.0f}fps  {motion.duration:.1f}sec")
        avatar.add_motion("wave", motion)
        avatar.play("wave")
    """
    from kagra.vrma_player import load_vrma as _load
    return _load(path, sample_fps=sample_fps)

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
from kagra.world3d       import World3D
from kagra.play          import Prop, Walk  # hovered_prop is the wrapper above
from kagra.instances     import InstanceBatch
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

# ─────────────────────────────────────────────────────────────
# Phase 7: VRM 高品質化
# ─────────────────────────────────────────────────────────────

from kagra.stage import Stage, backdrop_sphere, classify_stage_file, resolve_stage_path
from kagra.vrm_lookat  import LookAtController
from kagra.vrm_lipsync import LipSyncController, LipSyncTimeline, timeline_from_audio_query
from kagra.vrm_ik      import ArmIK, TwoBoneIK
from kagra.vrm_emotion import EmotionController
from kagra.vrm_action import ActionController

# ─────────────────────────────────────────────────────────────
# Phase 8: AI キャラクター SDK
# ─────────────────────────────────────────────────────────────

from kagra.ai_character import AiCharacter, CharState

# ─────────────────────────────────────────────────────────────
# avatar() 関数のシグネチャ変更不要。
# kagra.avatar() は引き続き VrmAvatar を返す。
# Phase 7 機能は VrmAvatar のメソッドとして追加済み。
#
# 使い方:
#   av = kagra.avatar("Emma.vrm")
#   av.enable_lookat()
#   av.enable_lipsync()
#   av.enable_emotion()
#   av.feel("joy")
#
# AI キャラ:
#   char = kagra.AiCharacter("Emma.vrm", tts="voicevox")
#   char.chat("こんにちは！")
# ─────────────────────────────────────────────────────────────
# ─────────────────────────────────────────────────────────────
# Phase 9 追記パッチ
# 既存の kagra/__init__.py の末尾（ `get_blend_shape_names = ...` の後）に追加する
# ─────────────────────────────────────────────────────────────


# ═══════════════════════════════════════════════════════════════
#  Phase 9a: ホットリロード
# ═══════════════════════════════════════════════════════════════

from kagra.hot_reload import HotReloader, make_hot_scene


# ═══════════════════════════════════════════════════════════════
#  Phase 9b: インゲームコンソール
# ═══════════════════════════════════════════════════════════════

from kagra.console import DevConsole, get_console

# ═══════════════════════════════════════════════════════════════
#  Agent contracts / verify / touch（モバイル入口）
# ═══════════════════════════════════════════════════════════════

from kagra.contracts import (
    AssetKind,
    KagraContractError,
    describe_environment,
    resolve_asset,
)
from kagra.samples import ensure_vrm
from kagra.verify import run_scenario, run_scenario_path, load_scenario
from kagra.touch import VirtualPad, PointerEvent, PointerPhase, apply_pad, inject_pointer


# ═══════════════════════════════════════════════════════════════
#  Phase 9d: tick_count / frame_index（DragonRuby 風）
# ═══════════════════════════════════════════════════════════════

_tick_count: int   = 0
_fps_target: int   = 60
_real_dt:    float = 0.0

def _phase9_frame_hook(dt: float):
    """ゲームループから毎フレーム呼ばれる内部フック。"""
    global _tick_count, _real_dt
    _tick_count += 1
    _real_dt = dt
    # HTTP コールバックのフラッシュ
    from kagra.http_client import http_tick as _http_tick
    _http_tick()


def _make_phase9_update_wrapper(original_update):
    """update 関数に Phase 9 フックを注入するラッパー。"""
    def wrapped(dt: float):
        _phase9_frame_hook(dt)
        original_update(dt)
    return wrapped


def tick_count() -> int:
    """ゲーム開始からのフレーム数を返す（DragonRuby の args.tick_count に相当）。

    60fps なら 1秒 = 60、5秒 = 300。
    dt を計算する代わりにフレーム数で時間を表現できる。

    Example::
        # 120フレーム（2秒）後に何かする
        if kagra.tick_count() == 120:
            spawn_enemy()

        # 2秒周期の波
        phase = (kagra.tick_count() % 120) / 120.0  # 0.0〜1.0
        y = math.sin(phase * math.pi * 2) * 50
    """
    return _tick_count


def frame_index(count: int, hold_for: int = 4, repeat: bool = True,
                offset: int = 0) -> int:
    """スプライトアニメのフレームインデックスを1行で計算する。

    DragonRuby の tick_count.frame_index() に相当。

    Args:
        count:    フレーム枚数（例: 4 枚アニメなら 4）
        hold_for: 1枚を何フレーム表示するか（デフォルト: 4）
        repeat:   ループするか（False なら最終フレームで止まる）
        offset:   開始オフセット（フレーム単位）

    Returns:
        現在のフレームインデックス（0 〜 count-1）

    Example::
        # 4枚・1枚4フレーム表示のアニメを1行で
        frame = kagra.frame_index(count=4, hold_for=4)
        # tileset[frame] を使って描画

        # 8枚・60fpsで0.2秒ずつ表示（= 12フレーム）
        frame = kagra.frame_index(count=8, hold_for=12)

        # ループしない（死亡アニメ等）
        frame = kagra.frame_index(count=6, hold_for=5, repeat=False)
    """
    tc = _tick_count + offset
    total = count * hold_for
    if repeat:
        tc = tc % total
    else:
        tc = min(tc, total - 1)
    return tc // hold_for


def every(frames: int) -> bool:
    """N フレームに 1 度 True を返す。

    Example::
        if kagra.every(30):        # 0.5秒に1回（60fps時）
            spawn_particle()
        if kagra.every(120):       # 2秒に1回
            spawn_enemy()
    """
    if frames <= 0:
        return True
    return _tick_count % frames == 0


def after(frames: int, from_tick: int = 0) -> bool:
    """開始から N フレーム後に True になる（1フレームだけ）。

    Example::
        start = kagra.tick_count()

        def update(dt):
            if kagra.after(60, from_tick=start):  # 1秒後に一度だけ
                show_hint()
    """
    return _tick_count == from_tick + frames


# ═══════════════════════════════════════════════════════════════
#  Phase 9e: ジオメトリヘルパー（DragonRuby 風）
# ═══════════════════════════════════════════════════════════════

def intersect_rect(
    ax: float, ay: float, aw: float, ah: float,
    bx: float, by: float, bw: float, bh: float,
) -> bool:
    """2つの矩形が重なっているか判定する。

    Args:
        ax,ay,aw,ah: 矩形 A の x,y,幅,高さ
        bx,by,bw,bh: 矩形 B の x,y,幅,高さ

    Returns:
        重なっていれば True

    Example::
        if kagra.intersect_rect(player.x, player.y, 32, 32,
                                 coin.x,   coin.y,   16, 16):
            collect_coin()
    """
    return (ax < bx + bw and ax + aw > bx and
            ay < by + bh and ay + ah > by)


def inside_rect(
    px: float, py: float,
    rx: float, ry: float, rw: float, rh: float,
) -> bool:
    """点が矩形の中にあるか判定する。

    Example::
        mx, my = kagra.mouse()
        if kagra.inside_rect(mx, my, btn_x, btn_y, btn_w, btn_h):
            # ホバー中
    """
    return rx <= px <= rx + rw and ry <= py <= ry + rh


def inside_circle(
    px: float, py: float,
    cx: float, cy: float, radius: float,
) -> bool:
    """点が円の中にあるか判定する。

    Example::
        if kagra.inside_circle(px, py, enemy.x, enemy.y, 64):
            take_damage()
    """
    dx = px - cx
    dy = py - cy
    return dx * dx + dy * dy <= radius * radius


def intersect_circle_rect(
    cx: float, cy: float, cr: float,
    rx: float, ry: float, rw: float, rh: float,
) -> bool:
    """円と矩形が重なっているか判定する。

    Example::
        if kagra.intersect_circle_rect(
                ball.x, ball.y, ball.radius,
                wall.x, wall.y, wall.w, wall.h):
            bounce()
    """
    nearest_x = max(rx, min(cx, rx + rw))
    nearest_y = max(ry, min(cy, ry + rh))
    dx = cx - nearest_x
    dy = cy - nearest_y
    return dx * dx + dy * dy <= cr * cr


def distance(x1: float, y1: float, x2: float, y2: float) -> float:
    """2点間の距離を返す。

    Example::
        d = kagra.distance(player.x, player.y, enemy.x, enemy.y)
        if d < 100:
            attack()
    """
    import math
    return math.sqrt((x2 - x1) ** 2 + (y2 - y1) ** 2)


def distance_sq(x1: float, y1: float, x2: float, y2: float) -> float:
    """2点間の距離の2乗を返す（sqrt なしで高速）。

    Example::
        # distance() より速い（比較だけなら sqrt 不要）
        if kagra.distance_sq(px, py, ex, ey) < 100 * 100:
            attack()
    """
    return (x2 - x1) ** 2 + (y2 - y1) ** 2


def angle_to(x1: float, y1: float, x2: float, y2: float) -> float:
    """点1から点2への角度（ラジアン）を返す。

    Example::
        angle = kagra.angle_to(bullet.x, bullet.y, target.x, target.y)
        bullet.vx = math.cos(angle) * speed
        bullet.vy = math.sin(angle) * speed
    """
    import math
    return math.atan2(y2 - y1, x2 - x1)


def lerp(a: float, b: float, t: float) -> float:
    """線形補間。t=0 で a、t=1 で b。

    Example::
        camera_x = kagra.lerp(camera_x, player.x, 0.1)  # 滑らかな追従
    """
    return a + (b - a) * t


def clamp(value: float, lo: float, hi: float) -> float:
    """値を [lo, hi] の範囲にクランプする。

    Example::
        player.hp = kagra.clamp(player.hp + heal, 0, 100)
    """
    return max(lo, min(hi, value))


def sign(value: float) -> int:
    """値の符号を返す（正: 1、負: -1、0: 0）。

    Example::
        direction = kagra.sign(target_x - player_x)
    """
    if value > 0:
        return 1
    elif value < 0:
        return -1
    return 0


def screen_to_world(sx: float, sy: float) -> tuple[float, float]:
    """スクリーン座標をワールド座標に変換する（アクティブカメラを使用）。

    Example::
        mx, my = kagra.mouse()
        wx, wy = kagra.screen_to_world(mx, my)
        # wx, wy はワールド空間のマウス位置
    """
    if _camera:
        return _camera.to_world(sx, sy)
    return sx, sy


def world_to_screen(wx: float, wy: float) -> tuple[float, float]:
    """ワールド座標をスクリーン座標に変換する。

    Example::
        sx, sy = kagra.world_to_screen(enemy.x, enemy.y)
        kagra.text("!", sx, sy - 20)
    """
    if _camera:
        return _camera.to_screen(wx, wy)
    return wx, wy


# ═══════════════════════════════════════════════════════════════
#  Phase 9f: HTTP クライアント
# ═══════════════════════════════════════════════════════════════

from kagra.http_client import (
    HttpClient, HttpResponse,
    http_get, http_post, http_tick,
    openai_chat, voicevox_speak,
)
from kagra.stream import StreamHud, ChatInbox, ChatMessage, VirtualCam
from kagra.voicevox import VoicevoxError
from kagra.mic import MicLipsync

