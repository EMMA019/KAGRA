# kagra/ui.py (修正版)
# Phase 5+6: UI系コンポーネント群
#   - MessageWindow  : RPGメッセージボックス（文字送り・選択肢）
#   - Label          : テキスト表示
#   - Button         : クリック/確定ボタン（キーボード操作）
#   - Panel          : 矩形パネル（背景）
#   - EventSystem    : NPC会話・フラグ管理
#   - Tween          : イージングアニメーション
#   - SaveLoad       : JSON セーブ/ロード

from __future__ import annotations
import json, os, math
from typing import Any, Callable

def _kagra():
    import kagra
    return kagra

# ------------------------------ Tween / Easing ---------------------------------
class Easing:
    @staticmethod
    def linear(t):      return t
    @staticmethod
    def in_quad(t):     return t * t
    @staticmethod
    def out_quad(t):    return t * (2 - t)
    @staticmethod
    def in_out_quad(t): return 2*t*t if t < 0.5 else -1+(4-2*t)*t
    @staticmethod
    def in_cubic(t):    return t * t * t
    @staticmethod
    def out_cubic(t):   return (t-1)**3 + 1
    @staticmethod
    def in_out_cubic(t):return 4*t*t*t if t<0.5 else (t-1)*(2*t-2)**2+1
    @staticmethod
    def in_sine(t):     return 1 - math.cos(t * math.pi / 2)
    @staticmethod
    def out_sine(t):    return math.sin(t * math.pi / 2)
    @staticmethod
    def in_out_sine(t): return -(math.cos(math.pi*t)-1)/2
    @staticmethod
    def out_bounce(t):
        if t < 1/2.75:   return 7.5625*t*t
        elif t < 2/2.75: t -= 1.5/2.75;   return 7.5625*t*t + 0.75
        elif t < 2.5/2.75: t -= 2.25/2.75; return 7.5625*t*t + 0.9375
        else:             t -= 2.625/2.75; return 7.5625*t*t + 0.984375
    @staticmethod
    def out_elastic(t):
        if t == 0 or t == 1: return t
        return (2**(-10*t)) * math.sin((t*10-0.75)*(2*math.pi/3)) + 1

class Tween:
    def __init__(self, start: float, end: float, duration: float,
                 easing: Callable = None, on_update: Callable = None,
                 on_complete: Callable = None, loop: bool = False,
                 ping_pong: bool = False):
        self.start = start
        self.end = end
        self.duration = max(duration, 0.0001)
        self.easing = easing or Easing.linear
        self.on_update = on_update
        self.on_complete = on_complete
        self.loop = loop
        self.ping_pong = ping_pong
        self._t = 0.0
        self._done = False
        self._forward = True
        self.value = start

    @property
    def done(self): return self._done

    def update(self, dt: float):
        if self._done: return self.value
        self._t += dt
        progress = min(self._t / self.duration, 1.0)
        t = progress if self._forward else 1.0 - progress
        self.value = self.start + (self.end - self.start) * self.easing(t)
        if self.on_update:
            self.on_update(self.value)
        if progress >= 1.0:
            if self.ping_pong:
                self._forward = not self._forward
                self._t = 0.0
            elif self.loop:
                self._t = 0.0
            else:
                self._done = True
                if self.on_complete:
                    self.on_complete()
        return self.value

    def reset(self):
        self._t = 0.0
        self._done = False
        self._forward = True
        self.value = self.start

class TweenManager:
    def __init__(self):
        self._tweens: list[Tween] = []
    def add(self, tween: Tween) -> Tween:
        self._tweens.append(tween)
        return tween
    def play(self, **kwargs) -> Tween:
        tw = Tween(**kwargs)
        self._tweens.append(tw)
        return tw
    def update(self, dt: float):
        self._tweens = [t for t in self._tweens if not t.done]
        for t in self._tweens:
            t.update(dt)
    def clear(self):
        self._tweens.clear()

# ------------------------------ UI プリミティブ ---------------------------------
class Panel:
    def __init__(self, x, y, w, h, r=20, g=20, b=40,
                 br=100, bg=100, bb=140, border=True, alpha=1.0, color=None):
        self.x = x; self.y = y
        self.w = w; self.h = h
        
        # color 引数が指定されたらそれを優先
        if color is not None:
            if isinstance(color, (tuple, list)):
                if len(color) == 3:
                    r, g, b = color
                elif len(color) == 4:
                    r, g, b, a = color
                    alpha = a / 255.0  # 0-255を0.0-1.0に変換
                else:
                    raise ValueError("color must be a tuple of (r,g,b) or (r,g,b,a)")
            else:
                raise ValueError("color must be a tuple or list")
        
        self.r = r; self.g = g; self.b = b
        self.br = br; self.bg = bg; self.bb = bb
        self.border = border
        self.alpha = alpha
        self.visible = True
    def draw(self):
        if not self.visible: return
        # アルファ値を0-255の範囲に変換
        a_int = int(self.alpha * 255)
        _kagra().rect(self.x, self.y, self.w, self.h, self.r, self.g, self.b, a_int)
        if self.border:
            t = 2
            _kagra().rect(self.x,           self.y,           self.w, t,    self.br, self.bg, self.bb)
            _kagra().rect(self.x,           self.y+self.h-t,  self.w, t,    self.br, self.bg, self.bb)
            _kagra().rect(self.x,           self.y,           t, self.h,    self.br, self.bg, self.bb)
            _kagra().rect(self.x+self.w-t,  self.y,           t, self.h,    self.br, self.bg, self.bb)

class Label:
    # 修正: color キーワード引数を受け付ける (r,g,b に変換)
    # 修正: font 引数も受け付ける (font_id のエイリアス)
    def __init__(self, font_id: int = None, text: str = "",
                 x=0, y=0, size=20,
                 r=255, g=255, b=255, color=None, font=None):
        # font 引数が指定されたらそれを優先
        if font is not None:
            self.font_id = font
        elif font_id is not None:
            self.font_id = font_id
        else:
            self.font_id = 0  # デフォルトフォント
        
        self.text = text
        self.x = x; self.y = y
        self.size = size
        # color 引数が指定されたらそれを優先
        if color is not None:
            if isinstance(color, (tuple, list)) and len(color) == 3:
                r, g, b = color
            else:
                raise ValueError("color must be a tuple of (r,g,b)")
        self.r = r; self.g = g; self.b = b
        self.visible = True
    def draw(self):
        if not self.visible: return
        _kagra().draw_text(self.font_id, self.text,
                           self.x, self.y, self.size,
                           self.r, self.g, self.b)

class Button:
    def __init__(self, font_id: int = None, text: str = "",
                 x=0, y=0, w=100, h=40, on_confirm: Callable = None,
                 size=20,
                 normal_bg=(30,30,50), hover_bg=(60,80,120),
                 selected_bg=(60,80,120), normal_fg=(200,200,200),
                 hover_fg=(255,255,255), selected_fg=(255,255,80),
                 # 互換性のための追加引数
                 font=None, label=None, color=None, hover_color=None,
                 on_click=None):
        # font 引数の処理
        if font is not None:
            self.font_id = font
        elif font_id is not None:
            self.font_id = font_id
        else:
            self.font_id = 0  # デフォルトフォント
        
        # text と label 引数の処理
        if label is not None:
            self.text = label
        else:
            self.text = text
        
        self.x = x; self.y = y
        self.w = w; self.h = h
        
        # on_confirm と on_click の処理
        if on_click is not None:
            self.on_confirm = on_click
        else:
            self.on_confirm = on_confirm
        
        self.size = size
        self.selected = False
        self.hovered = False
        self.visible = True
        self.enabled = True
        
        # color と hover_color 引数の処理
        if color is not None:
            if isinstance(color, (tuple, list)) and len(color) >= 3:
                normal_bg = (color[0], color[1], color[2])
                # テキスト色を自動計算（明るい色）
                if color[0] + color[1] + color[2] < 400:
                    normal_fg = (255, 255, 255)  # 明るい背景なら白文字
                else:
                    normal_fg = (30, 30, 30)     # 暗い背景なら黒文字
        
        if hover_color is not None:
            if isinstance(hover_color, (tuple, list)) and len(hover_color) >= 3:
                hover_bg = (hover_color[0], hover_color[1], hover_color[2])
                # ホバー時の文字色を自動計算
                if hover_color[0] + hover_color[1] + hover_color[2] < 400:
                    hover_fg = (255, 255, 255)
                else:
                    hover_fg = (30, 30, 30)
        
        self.normal_bg = normal_bg
        self.hover_bg = hover_bg
        self.selected_bg = selected_bg
        self.normal_fg = normal_fg
        self.hover_fg = hover_fg
        self.selected_fg = selected_fg

    def update(self) -> bool:   # ← dt 引数なし
        if not self.visible or not self.enabled:
            self.hovered = False
            return False
        kg = _kagra()
        mx, my = kg.mouse_pos()
        self.hovered = (self.x <= mx <= self.x + self.w and
                        self.y <= my <= self.y + self.h)
        if self.hovered and kg.mouse_pressed(kg.MOUSE_LEFT):
            self.confirm()
            return True
        return False

    def confirm(self):
        if self.enabled and self.on_confirm:
            self.on_confirm()

    def draw(self):
        if not self.visible: return
        if self.selected or self.hovered:
            bg = self.selected_bg if self.selected else self.hover_bg
            _kagra().rect(self.x, self.y, self.w, self.h, *bg)
            _kagra().rect(self.x, self.y, self.w, 2, 120, 160, 255)
        else:
            _kagra().rect(self.x, self.y, self.w, self.h, *self.normal_bg)
        tw, th = _kagra().measure_text(self.font_id, self.text, self.size)
        tx = self.x + (self.w - tw) / 2
        ty = self.y + (self.h - th) / 2
        if self.selected:
            fg = self.selected_fg
        elif self.hovered:
            fg = self.hover_fg
        else:
            fg = self.normal_fg
        _kagra().draw_text(self.font_id, self.text, tx, ty, self.size, *fg)

# ------------------------------ MessageWindow --------------------------------
class MessageWindow:
    CHAR_SPEED = 30.0
    def __init__(self, font_id: int, x=40, y=400, w=720, h=160,
                 text_size=20, char_speed=None):
        self.font_id = font_id
        self.panel = Panel(x, y, w, h, r=10, g=10, b=30)
        self.x = x; self.y = y
        self.w = w; self.h = h
        self.text_size = text_size
        self.char_speed = char_speed or self.CHAR_SPEED
        self._pages: list[str] = []
        self._page_idx = 0
        self._char_pos = 0.0
        self._full_shown = False
        self._active = False
        self._choices: list[str] = []
        self._choice_idx = 0
        self._choosing = False
        self._on_choice: Callable | None = None
        self._on_close: Callable | None = None
        self._blink = 0.0

    @property
    def active(self): return self._active
    @property
    def is_choosing(self): return self._choosing

    def show(self, text: str, choices: list[str] = None,
             on_choice: Callable = None, on_close: Callable = None):
        self._pages = text.split('\f')
        self._page_idx = 0
        self._char_pos = 0.0
        self._full_shown = False
        self._active = True
        self._choosing = False
        self._choices = choices or []
        self._choice_idx = 0
        self._on_choice = on_choice
        self._on_close = on_close

    def advance(self):
        if not self._active: return
        if self._choosing:
            if self._on_choice:
                self._on_choice(self._choice_idx)
            self._close()
            return
        if not self._full_shown:
            self._char_pos = len(self._current_page())
            self._full_shown = True
            return
        if self._page_idx < len(self._pages) - 1:
            self._page_idx += 1
            self._char_pos = 0.0
            self._full_shown = False
            return
        if self._choices:
            self._choosing = True
            return
        self._close()

    def select_up(self):
        if self._choosing:
            self._choice_idx = (self._choice_idx - 1) % len(self._choices)
    def select_down(self):
        if self._choosing:
            self._choice_idx = (self._choice_idx + 1) % len(self._choices)

    def _current_page(self) -> str:
        if not self._pages: return ""
        return self._pages[self._page_idx]
    def _close(self):
        self._active = False
        if self._on_close: self._on_close()

    def update(self, dt: float):
        if not self._active: return
        self._blink = (self._blink + dt) % 1.0
        if self._full_shown or self._choosing: return
        page = self._current_page()
        self._char_pos = min(self._char_pos + self.char_speed * dt, len(page))
        if self._char_pos >= len(page):
            self._full_shown = True

    def draw(self):
        if not self._active: return
        self.panel.draw()
        px = self.x + 16
        py = self.y + 14
        line_h = self.text_size + 6
        page = self._current_page()
        shown = page[:int(self._char_pos)]
        for line in shown.split('\n'):
            _kagra().draw_text(self.font_id, line, px, py, self.text_size)
            py += line_h
        if self._full_shown and not self._choosing and self._blink < 0.5:
            _kagra().draw_text(self.font_id, "▼",
                               self.x + self.w - 28,
                               self.y + self.h - self.text_size - 8,
                               self.text_size, 200, 220, 255)
        if self._choosing:
            cy = self.y + self.h + 8
            for i, choice in enumerate(self._choices):
                bg_r, bg_g, bg_b = (60,80,120) if i == self._choice_idx else (20,20,40)
                _kagra().rect(px - 4, cy, 200, line_h + 4, bg_r, bg_g, bg_b)
                r,g,b = (255,255,80) if i == self._choice_idx else (200,200,200)
                prefix = "▶ " if i == self._choice_idx else "  "
                _kagra().draw_text(self.font_id, prefix + choice, px, cy + 4,
                                   self.text_size, r, g, b)
                cy += line_h + 8

# ------------------------------ EventSystem / NPC -----------------------------
class EventFlags:
    def __init__(self):
        self._data: dict[str, Any] = {}
    def set(self, key: str, value: Any): self._data[key] = value
    def get(self, key: str, default=None): return self._data.get(key, default)
    def inc(self, key: str, amount=1): self._data[key] = self._data.get(key, 0) + amount
    def has(self, key: str) -> bool: return key in self._data
    def clear(self, key: str): self._data.pop(key, None)
    def all(self) -> dict: return dict(self._data)

class DialogScript:
    def __init__(self, entries: list[dict]):
        self.entries = entries
    def run(self, flags: EventFlags, mw: MessageWindow, on_done: Callable = None):
        filtered = [e for e in self.entries if not e.get("cond") or e["cond"](flags)]
        self._run_sequence(filtered, 0, flags, mw, on_done)
    def _run_sequence(self, entries, idx, flags, mw, on_done):
        if idx >= len(entries):
            if on_done: on_done()
            return
        entry = entries[idx]
        for k, v in (entry.get("set") or {}).items():
            flags.set(k, v)
        choices = entry.get("choices")
        on_choice = entry.get("on_choice")
        def next_entry(choice_idx=None):
            if choice_idx is not None and on_choice:
                on_choice(choice_idx, flags)
            self._run_sequence(entries, idx+1, flags, mw, on_done)
        mw.show(entry["text"],
                choices=choices,
                on_choice=(lambda i: next_entry(i)) if choices else None,
                on_close=(lambda: next_entry()) if not choices else None)

# ------------------------------ Save / Load -----------------------------------
class SaveLoad:
    def __init__(self, save_dir: str = "saves"):
        self.save_dir = save_dir
        os.makedirs(save_dir, exist_ok=True)
    def _path(self, slot: int) -> str:
        return os.path.join(self.save_dir, f"save_{slot:02d}.json")
    def save(self, slot: int, data: dict, meta: dict = None) -> bool:
        import datetime
        payload = {
            "_meta": {
                "slot": slot,
                "timestamp": datetime.datetime.now().isoformat(),
                **(meta or {}),
            },
            "data": data,
        }
        try:
            with open(self._path(slot), "w", encoding="utf-8") as f:
                json.dump(payload, f, ensure_ascii=False, indent=2)
            return True
        except Exception as e:
            print(f"[SaveLoad] save failed: {e}")
            return False
    def load(self, slot: int) -> dict | None:
        path = self._path(slot)
        if not os.path.exists(path):
            return None
        try:
            with open(path, encoding="utf-8") as f:
                payload = json.load(f)
            return payload.get("data")
        except Exception as e:
            print(f"[SaveLoad] load failed: {e}")
            return None
    def list_slots(self) -> list[dict]:
        result = []
        for fname in sorted(os.listdir(self.save_dir)):
            if not fname.startswith("save_") or not fname.endswith(".json"):
                continue
            try:
                with open(os.path.join(self.save_dir, fname), encoding="utf-8") as f:
                    payload = json.load(f)
                result.append(payload.get("_meta", {}))
            except Exception:
                pass
        return result
    def delete(self, slot: int) -> bool:
        path = self._path(slot)
        if os.path.exists(path):
            os.remove(path)
            return True
        return False
    def exists(self, slot: int) -> bool:
        return os.path.exists(self._path(slot))

# ------------------------------ ChoiceMenu / TransitionScene ------------------
class ChoiceMenu:
    def __init__(self, font_id: int, items: list[str], x: float, y: float, w: float,
                 item_h: float = 44, gap: float = 8, text_size: int = 20,
                 normal_bg=(20,25,50), hover_bg=(35,45,90), selected_bg=(50,70,130),
                 normal_fg=(180,190,220), hover_fg=(220,230,255), selected_fg=(255,255,80)):
        self.font_id = font_id
        self.items = list(items)
        self.x = x; self.y = y
        self.w = w
        self.item_h = item_h
        self.gap = gap
        self.text_size = text_size
        self.normal_bg = normal_bg
        self.hover_bg = hover_bg
        self.selected_bg = selected_bg
        self.normal_fg = normal_fg
        self.hover_fg = hover_fg
        self.selected_fg = selected_fg
        self.cursor = 0
        self.hover_index = -1
        self.confirmed = False
        self.selected_index = -1
        self.selected_text = None
        self.visible = True
        self.enabled = True
    def _item_rect(self, i: int):
        iy = self.y + i * (self.item_h + self.gap)
        return self.x, iy, self.w, self.item_h
    def _mouse_in(self, rx, ry, rw, rh):
        mx, my = _kagra().mouse_pos()
        return rx <= mx <= rx + rw and ry <= my <= ry + rh
    def update(self):
        if not self.visible or not self.enabled or not self.items:
            self.confirmed = False
            return
        kg = _kagra()
        self.confirmed = False
        self.hover_index = -1
        if kg.key_pressed(kg.KEY_UP):
            self.cursor = (self.cursor - 1) % len(self.items)
        if kg.key_pressed(kg.KEY_DOWN):
            self.cursor = (self.cursor + 1) % len(self.items)
        for i in range(len(self.items)):
            rx, ry, rw, rh = self._item_rect(i)
            if self._mouse_in(rx, ry, rw, rh):
                self.hover_index = i
                self.cursor = i
                if kg.mouse_pressed(kg.MOUSE_LEFT):
                    self.confirmed = True
                    self.selected_index = i
                    self.selected_text = self.items[i]
                    return
        if kg.key_pressed(kg.KEY_Z) or kg.key_pressed(kg.KEY_RETURN):
            self.confirmed = True
            self.selected_index = self.cursor
            self.selected_text = self.items[self.cursor]
    def draw(self):
        if not self.visible: return
        kg = _kagra()
        for i, text in enumerate(self.items):
            x, y, w, h = self._item_rect(i)
            if i == self.cursor:
                bg = self.selected_bg
                fg = self.selected_fg
                prefix = "▶ "
            elif i == self.hover_index:
                bg = self.hover_bg
                fg = self.hover_fg
                prefix = "  "
            else:
                bg = self.normal_bg
                fg = self.normal_fg
                prefix = "  "
            kg.rect(x, y, w, h, *bg)
            if i == self.cursor:
                kg.rect(x, y, w, 2, 120, 160, 255)
            if self.font_id:
                kg.draw_text(self.font_id, prefix + text,
                             x + 20, y + 6, self.text_size, *fg)

class TransitionScene:
    def __init__(self):
        self._ready = False
        self._t = 0.0
        self._fade_alpha = 0.0
        self._tweens = None
        self._bgm_started = False
    def on_enter(self): self._ready = False
    def on_exit(self): pass
    def on_pause(self): pass
    def on_resume(self): pass
    def _late_init(self): pass
    def _start_bgm(self): pass
    def update(self, dt: float):
        if not self._ready:
            self._late_init()
            self._tweens = TweenManager()
            self._ready = True
            self._fade_alpha = 255.0
            self._tweens.add(Tween(255, 0, 0.5, easing=Easing.out_quad,
                                   on_update=lambda v: setattr(self, "_fade_alpha", v)))
            if not self._bgm_started:
                self._start_bgm()
                self._bgm_started = True
            return
        self._t += dt
        if self._tweens:
            self._tweens.update(dt)
    def draw(self):
        self._draw_content()
        self.draw_fade()
    def _draw_content(self): pass
    def draw_fade(self):
        a = int(self._fade_alpha)
        if a <= 0: return
        sw, sh = _kagra().get_screen_size()
        v = max(0, min(255, a))
        _kagra().rect(0, 0, sw, sh, v // 3, v // 3, v // 3)
    def go(self, scene, fade_time=0.4):
        if self._tweens:
            self._tweens.add(Tween(0, 255, fade_time, easing=Easing.in_quad,
                                   on_update=lambda v: setattr(self, "_fade_alpha", v),
                                   on_complete=lambda: _kagra().scene.change(scene)))
        else:
            _kagra().scene.change(scene)

# ------------------------------ ProgressBar -----------------------------------
class ProgressBar:
    def __init__(self, x: float, y: float, w: float, h: float,
                 max_val: float = 100, value: float = 100,
                 bg_color=(20,20,20), fg_color=(80,220,80),
                 border_color=(80,80,80), border: bool = True,
                 label_font: int = 0, label_fmt: str = "",
                 label_size: int = 14, label_color=(255,255,255),
                 smooth: bool = False, smooth_speed: float = 5.0):
        self.x = x; self.y = y
        self.w = w; self.h = h
        self.max_val = max(max_val, 1)
        self.value = value
        self._display = float(value)
        self.bg_color = bg_color
        self.fg_color = fg_color
        self.border_color = border_color
        self.border = border
        self.label_font = label_font
        self.label_fmt = label_fmt
        self.label_size = label_size
        self.label_color = label_color
        self.smooth = smooth
        self.smooth_speed = smooth_speed
        self.visible = True

    def update(self, dt: float):
        if self.smooth:
            diff = self.value - self._display
            self._display += diff * min(1.0, self.smooth_speed * dt)
        else:
            self._display = float(self.value)

    def draw(self):
        if not self.visible: return
        kg = _kagra()
        kg.rect(self.x, self.y, self.w, self.h, *self.bg_color)
        ratio = max(0.0, min(1.0, self._display / self.max_val))
        fill_w = self.w * ratio
        if fill_w > 0:
            kg.rect(self.x, self.y, fill_w, self.h, *self.fg_color)
        if self.border:
            t = 2
            kg.rect(self.x,             self.y,             self.w, t, *self.border_color)
            kg.rect(self.x,             self.y + self.h - t, self.w, t, *self.border_color)
            kg.rect(self.x,             self.y,             t, self.h, *self.border_color)
            kg.rect(self.x + self.w - t, self.y,            t, self.h, *self.border_color)
        if self.label_fmt and self.label_font:
            text = self.label_fmt.format(value=int(self._display),
                                         max=int(self.max_val),
                                         pct=int(ratio*100))
            tw, th = kg.measure_text(self.label_font, text, self.label_size)
            kg.draw_text(self.label_font, text,
                         self.x + (self.w - tw) / 2,
                         self.y + (self.h - th) / 2,
                         self.label_size, *self.label_color)

# ------------------------------ VBox / HBox -----------------------------------
# 修正: update(dt) で子の update を呼ぶ際、引数の数をチェックする
def _safe_update(child, dt):
    if hasattr(child, 'update'):
        try:
            # まず dt ありで試す
            child.update(dt)
        except TypeError:
            # dt を受け取らない場合は引数なしで呼ぶ
            child.update()
    # それ以外は無視

class VBox:
    def __init__(self, x: float, y: float, w: float,
                 gap: float = 8, padding: float = 0, align: str = "left"):
        self.x = x; self.y = y
        self.w = w
        self.gap = gap
        self.padding = padding
        self.align = align
        self.visible = True
        self._children: list = []

    def add(self, widget) -> "VBox":
        self._children.append(widget)
        return self

    def layout(self):
        cy = self.y + self.padding
        for child in self._children:
            cw = getattr(child, "w", self.w - self.padding * 2)
            if self.align == "center":
                child.x = self.x + (self.w - cw) / 2
            elif self.align == "right":
                child.x = self.x + self.w - cw - self.padding
            else:
                child.x = self.x + self.padding
            child.y = cy
            cy += getattr(child, "h", 0) + self.gap

    @property
    def total_height(self) -> float:
        h = self.padding * 2
        for i, child in enumerate(self._children):
            h += getattr(child, "h", 0)
            if i < len(self._children) - 1:
                h += self.gap
        return h

    def move_to(self, x: float, y: float):
        self.x = x; self.y = y
        self.layout()

    def update(self, dt: float = 0):
        if not self.visible: return
        for child in self._children:
            if getattr(child, "visible", True):
                _safe_update(child, dt)

    def draw(self):
        if not self.visible: return
        for child in self._children:
            if getattr(child, "visible", True):
                child.draw()

    @property
    def children(self):
        """外部から子要素リストを参照するための互換性プロパティ"""
        return self._children


class HBox:
    def __init__(self, x: float, y: float, h: float,
                 gap: float = 8, padding: float = 0, align: str = "top"):
        self.x = x; self.y = y
        self.h = h
        self.gap = gap
        self.padding = padding
        self.align = align
        self.visible = True
        self._children: list = []

    def add(self, widget) -> "HBox":
        self._children.append(widget)
        return self

    def layout(self):
        cx = self.x + self.padding
        for child in self._children:
            ch = getattr(child, "h", self.h - self.padding * 2)
            if self.align == "center":
                child.y = self.y + (self.h - ch) / 2
            elif self.align == "bottom":
                child.y = self.y + self.h - ch - self.padding
            else:
                child.y = self.y + self.padding
            child.x = cx
            cx += getattr(child, "w", 0) + self.gap

    @property
    def total_width(self) -> float:
        w = self.padding * 2
        for i, child in enumerate(self._children):
            w += getattr(child, "w", 0)
            if i < len(self._children) - 1:
                w += self.gap
        return w

    def move_to(self, x: float, y: float):
        self.x = x; self.y = y
        self.layout()

    def update(self, dt: float = 0):
        if not self.visible: return
        for child in self._children:
            if getattr(child, "visible", True):
                _safe_update(child, dt)

    def draw(self):
        if not self.visible: return
        for child in self._children:
            if getattr(child, "visible", True):
                child.draw()

    @property
    def children(self):
        """外部から子要素リストを参照するための互換性プロパティ"""
        return self._children

# ------------------------------ ScrollView ------------------------------------
class ScrollView:
    def __init__(self, x: float, y: float, w: float, h: float,
                 content, scroll_speed: float = 30.0,
                 bg_color=(15,15,25), show_scrollbar: bool = True,
                 scrollbar_color=(80,100,160)):
        self.x = x; self.y = y
        self.w = w; self.h = h
        self.content = content
        self.scroll_speed = scroll_speed
        self.bg_color = bg_color
        self.show_scrollbar = show_scrollbar
        self.scrollbar_color = scrollbar_color
        self.visible = True
        self._scroll_y = 0.0
        self._max_scroll = 0.0
        self.content.x = x
        self.content.y = y
        if hasattr(self.content, "layout"):
            self.content.layout()
    def _content_height(self) -> float:
        if hasattr(self.content, "total_height"):
            return self.content.total_height
        return getattr(self.content, "h", self.h)
    def _mouse_in_view(self) -> bool:
        mx, my = _kagra().mouse_pos()
        return self.x <= mx <= self.x + self.w and self.y <= my <= self.y + self.h
    def scroll_to(self, y: float):
        content_h = self._content_height()
        self._max_scroll = max(0.0, content_h - self.h)
        self._scroll_y = max(0.0, min(y, self._max_scroll))
    def scroll_by(self, delta: float):
        self.scroll_to(self._scroll_y + delta)
    def update(self, dt: float = 0):
        if not self.visible: return
        if self._mouse_in_view():
            wheel = _kagra().mouse_wheel_y()
            if wheel != 0:
                self.scroll_by(-wheel * self.scroll_speed)
        self.content.y = self.y - self._scroll_y
        if hasattr(self.content, "layout"):
            self.content.layout()
        _safe_update(self.content, dt)
    def draw(self):
        if not self.visible: return
        kg = _kagra()
        kg.rect(self.x, self.y, self.w, self.h, *self.bg_color)
        self.content.draw()
        if self.show_scrollbar:
            content_h = self._content_height()
            if content_h > self.h:
                bar_w = 6
                bar_x = self.x + self.w - bar_w - 2
                ratio = self.h / content_h
                bar_h = max(20, self.h * ratio)
                scroll_r = self._scroll_y / max(1, self._max_scroll)
                bar_y = self.y + (self.h - bar_h) * scroll_r
                kg.rect(bar_x, self.y,  bar_w, self.h,  30, 30, 40)
                kg.rect(bar_x, bar_y,   bar_w, bar_h,   *self.scrollbar_color)

# ------------------------------ UIGroup ---------------------------------------
class UIGroup:
    def __init__(self, visible: bool = True):
        self.visible = visible
        self._widgets: list = []
    def add(self, widget) -> "UIGroup":
        self._widgets.append(widget)
        return self
    def remove(self, widget):
        self._widgets = [w for w in self._widgets if w is not widget]
    def clear(self):
        self._widgets.clear()
    def show(self):
        self.visible = True
        for w in self._widgets:
            w.visible = True
    def hide(self):
        self.visible = False
        for w in self._widgets:
            w.visible = False
    def update(self, dt: float = 0):
        if not self.visible: return
        for w in self._widgets:
            if getattr(w, "visible", True):
                _safe_update(w, dt)
    def draw(self):
        if not self.visible: return
        for w in self._widgets:
            if getattr(w, "visible", True):
                w.draw()