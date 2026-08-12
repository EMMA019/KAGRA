"""
desktop_mascot.py - KAGRA AI デスクトップマスコット (V1.0 決定版)
================================================================
・[設定外部化] config.json の自動生成と読み込み
・[安定化] TTSワーカースレッドの常駐化、チャット履歴のスレッドセーフ化
・[万能化] Skillクラスを継承する拡張システム (正規表現対応)
"""
import kagra
import threading
import os
import math
import time
import queue
import requests
import datetime
import json
import psutil
import re

from dotenv import load_dotenv
from openai import OpenAI
from kagra.vrm_lookat import LookAtController
from kagra.vrm_lipsync import LipSyncController

load_dotenv()

# ══════════════════════════════════════════════════════════════
#  [1] コンフィグ管理 (config.json)
# ══════════════════════════════════════════════════════════════
DEFAULT_CONFIG = {
    "window": {
        "width": 220,
        "height": 480,
        "fps": 60,
        "font": "meiryo"
    },
    "avatar": {
        "model_path": "assets/Emma.vrm",
        "target_y": 0.75,
        "camera_distance": 2.9
    },
    "ai": {
        "system_prompt": "あなたは優秀なデスクトップ秘書のEmmaです。可愛く、30文字以内で短く話して。",
        "weather_area_code": "110000"  # 埼玉
    }
}

def load_config():
    if not os.path.exists("config.json"):
        with open("config.json", "w", encoding="utf-8") as f:
            json.dump(DEFAULT_CONFIG, f, indent=4, ensure_ascii=False)
        return DEFAULT_CONFIG
    try:
        with open("config.json", "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception as e:
        print(f"Config load error: {e}")
        return DEFAULT_CONFIG

CONFIG = load_config()
SW, SH = CONFIG["window"]["width"], CONFIG["window"]["height"]

# ══════════════════════════════════════════════════════════════
#  [2] スキル（万能プラグイン）システム
# ══════════════════════════════════════════════════════════════
class Skill:
    """すべてのスキルのベースクラス。これを継承して機能を追加する"""
    def match(self, text: str) -> bool:
        return False
    def execute(self, text: str) -> str:
        return ""

class WeatherSkill(Skill):
    """天気予報を取得するスキル"""
    def __init__(self, area_code):
        self.area_code = area_code
        
    def match(self, text):
        return bool(re.search(r'(天気|晴れ|雨|傘)', text))
        
    def execute(self, text):
        try:
            sess = requests.Session()
            sess.headers.update({"User-Agent": "Mozilla/5.0", "Referer": "https://www.jma.go.jp/"})
            url = f"https://www.jma.go.jp/bosai/forecast/data/forecast/{self.area_code}.json"
            r = sess.get(url, timeout=5)
            r.raise_for_status()
            w = r.json()[0]["timeSeries"][0]["areas"][0]["weathers"][0].replace("　", " ")
            return f"\n[システム情報: 気象庁によると今日の天気は「{w}」です]"
        except Exception as e:
            return f"\n[システム情報: 天気取得失敗({e})。窓から空が見えないと謝って]"

class SystemInfoSkill(Skill):
    """PCのリソース状況をチェックするスキル"""
    def match(self, text):
        return bool(re.search(r'(cpu|メモリ|重い|遅い|調子)', text, re.IGNORECASE))
        
    def execute(self, text):
        cpu = psutil.cpu_percent()
        mem = psutil.virtual_memory().percent
        return f"\n[システム情報: 現在のCPU使用率は{cpu}%、メモリ使用率は{mem}%です。これに基づいてPCの調子を答えて]"

class SkillManager:
    """登録されたスキルを順に評価し、AIへのカンペ（コンテキスト）を作る"""
    def __init__(self):
        self.skills = []
    def register(self, skill: Skill):
        self.skills.append(skill)
    def process(self, text: str, reply_queue: queue.Queue, lipsync) -> str:
        context = ""
        for skill in self.skills:
            if skill.match(text):
                reply_queue.put("調べてるよ...")
                lipsync.play_text("ん", 2.0)
                context += skill.execute(text)
        return context

# ══════════════════════════════════════════════════════════════
#  [3] TTS (音声合成) ワーカースレッド化による安定化
# ══════════════════════════════════════════════════════════════
class MascotTTS:
    def __init__(self):
        self.task_queue = queue.Queue()
        self.thread = threading.Thread(target=self._worker, daemon=True)
        self.thread.start()

    def _worker(self):
        """スレッド内で一度だけエンジンを初期化し、メモリリークと遅延を防ぐ"""
        try:
            # Windows COMの初期化（別スレッドでpyttsx3を動かすため）
            import pythoncom
            pythoncom.CoInitialize()
        except ImportError:
            pass # Mac/Linux等の場合は無視

        try:
            import pyttsx3
            engine = pyttsx3.init()
            engine.setProperty('rate', 185)
            for voice in engine.getProperty('voices'):
                if "JA" in voice.id or "Japan" in voice.name:
                    engine.setProperty('voice', voice.id)
                    break
        except Exception as e:
            print(f"[TTS Worker] 初期化エラー: {e}")
            engine = None

        while True:
            text, lipsync_ctrl = self.task_queue.get()
            duration = max(1.0, len(text) * 0.16)
            lipsync_ctrl.play_text(text, duration)
            if engine:
                try:
                    engine.say(text)
                    engine.runAndWait()
                except Exception as e:
                    print(f"[TTS Worker] 発声エラー: {e}")
            self.task_queue.task_done()

    def speak(self, text, lipsync_ctrl):
        self.task_queue.put((text, lipsync_ctrl))

# ══════════════════════════════════════════════════════════════
#  メインシーン
# ══════════════════════════════════════════════════════════════
class MascotScene(kagra.Scene):
    def on_enter(self):
        self.font = kagra.assets.font(CONFIG["window"]["font"])

        # VRM読み込み (config優先、なければ探索)
        vrm_path = CONFIG["avatar"]["model_path"]
        if not os.path.exists(vrm_path):
            for f in os.listdir("."):
                if f.endswith(".vrm"):
                    vrm_path = f
                    break

        self.avatar = kagra.avatar(vrm_path)
        self.avatar.play("idle")

        # カメラ設定
        self.base_target_y = CONFIG["avatar"]["target_y"]
        self.cam = kagra.Camera3D(SW, SH, fov_deg=35.0)
        self.cam.use_orbit(radius=CONFIG["avatar"]["camera_distance"], theta=0.0, phi=0.05, target=(0, self.base_target_y, 0))

        self.lookat = LookAtController(self.avatar, eye_height=1.4)
        self.lipsync = LipSyncController(self.avatar, smoothing=0.35)
        self.tts = MascotTTS()

        # チャットと状態管理
        self.chat_history = []
        self.history_lock = threading.Lock() # スレッドセーフ化
        self.current_reply = "システム起動完了！"
        self.input_text = ""
        self.pending = False
        self.reply_timer = 5.0
        self.ime_active = False
        self.always_on_top = True
        self.click_through = False
        self.pomodoro_timer = 0.0

        # スキルシステムの初期化
        self.skill_manager = SkillManager()
        self.skill_manager.register(WeatherSkill(CONFIG["ai"]["weather_area_code"]))
        self.skill_manager.register(SystemInfoSkill())

        # AI クライアント (フォールバック対応)
        deepseek_key = os.environ.get("DEEPSEEK_API_KEY")
        openai_key = os.environ.get("OPENAI_API_KEY")
        self.api_key = deepseek_key or openai_key
        self.use_deepseek = bool(deepseek_key)

        self.client = None
        if self.api_key:
            base_url = "https://api.deepseek.com/v1" if self.use_deepseek else None
            self.client = OpenAI(api_key=self.api_key, base_url=base_url)
        else:
            self.current_reply = ".envにAPIキーがないからおしゃべりできないよ"

        self.reply_queue = queue.Queue()
        self.tts.speak(self.current_reply, self.lipsync)

        if hasattr(kagra, 'focus_window'):
            kagra.focus_window()
        self._started = time.monotonic()
        self._last_focus_request = 0.0

    def update(self, dt):
        try:
            while True:
                reply = self.reply_queue.get_nowait()
                self.current_reply = reply
                self.pending = False
                self.reply_timer = max(5.0, len(reply) * 0.2)
                self.tts.speak(reply, self.lipsync)
        except queue.Empty:
            pass

        if kagra.pressed("ESCAPE"):
            raise SystemExit

        now = time.monotonic()
        if hasattr(kagra, 'focus_window'):
            if now - self._started < 3.0 and now - self._last_focus_request > 0.5:
                kagra.focus_window()
                self._last_focus_request = now

        mx, my = kagra.mouse()
        IY = SH - 50
        
        if kagra.mouse_click(kagra.MOUSE_LEFT):
            if 70 < my < IY:
                kagra.drag_window()
            elif my >= IY:
                if hasattr(kagra, 'focus_window'):
                    kagra.focus_window()

        # ── 文字入力処理 ──
        preedit = kagra.get_preedit_text()
        self.ime_active = len(preedit) > 0
        if kagra.backspace_pressed() and not self.ime_active:
            self.input_text = self.input_text[:-1]
        for c in kagra.get_typed_chars():
            if c >= ' ' and len(self.input_text) < 50:
                self.input_text += c
        if kagra.enter_pressed() and not self.ime_active:
            if self.input_text.strip() and not self.pending:
                msg = self.input_text.strip()
                self.input_text = ""
                self.send_chat_message(msg)

        # ── ショートカット ──
        if not self.ime_active and len(self.input_text) == 0:
            if kagra.pressed("R"):
                self.click_through = not self.click_through
                kagra.set_click_through(self.click_through)
            if kagra.pressed("T"):
                self.always_on_top = not self.always_on_top
                kagra.set_always_on_top(self.always_on_top)
            if kagra.pressed("S") and self.avatar.spring_bone:
                self.avatar.spring_bone.enabled = not self.avatar.spring_bone.enabled
            if kagra.pressed("P"):
                if self.pomodoro_timer > 0:
                    self.pomodoro_timer = 0.0
                    self.reply_queue.put("タイマーをキャンセルしたよ。")
                else:
                    self.pomodoro_timer = 25 * 60.0
                    self.reply_queue.put("25分のタイマーをセットしたよ。集中してね！")

        if self.pomodoro_timer > 0:
            self.pomodoro_timer -= dt
            if self.pomodoro_timer <= 0:
                self.pomodoro_timer = 0.0
                self.reply_queue.put("お疲れ様！25分経ったよ。少し休憩しよう！")

        breath = math.sin(time.time() * (math.pi * 2 / 3.0)) * 0.005
        self.cam.use_orbit(radius=CONFIG["avatar"]["camera_distance"], theta=0.0, phi=0.05, target=(0, self.base_target_y + breath, 0))
        self.lookat.look_at_screen(mx, my, SW, SH, 40)
        self.lookat.update(dt)
        self.lipsync.update(dt)
        self.avatar.update(dt)
        
        # 将来的に kagra.get_engine() 等の公開APIが実装された場合に備えた安全なフォールバック
        engine = getattr(kagra, 'get_engine', lambda: getattr(kagra, '_engine', None))()
        if engine:
            self.cam.update(engine)

        if self.reply_timer > 0:
            self.reply_timer -= dt

    def draw(self):
        kagra.cls(0, 0, 0)
        kagra.draw_vrm(self.avatar.vrm_id)

        if self.pomodoro_timer > 0:
            m = int(self.pomodoro_timer // 60)
            s = int(self.pomodoro_timer % 60)
            kagra.rect(5, 5, 65, 25, 200, 50, 50, 200)
            kagra.draw_text(self.font, f"🍅 {m:02d}:{s:02d}", 10, 10, 12, 255, 255, 255, 255)

        if self.reply_timer > 0 or self.pending:
            y_base = 35 if self.pomodoro_timer > 0 else 10
            bx, by, bw, bh = 5, y_base, SW - 10, 65
            kagra.rect(bx, by, bw, bh, 20, 20, 35, 210)
            
            # 新しく追加された kagra.polygon API を使用して吹き出しのしっぽをネイティブ描画
            tri_x, tri_y = SW // 2, by + bh
            kagra.polygon([(tri_x - 6, tri_y), (tri_x + 6, tri_y), (tri_x, tri_y + 6)], color=(20, 20, 35), alpha=210)
            
            y = by + 5
            for line in [self.current_reply[i:i+13] for i in range(0, len(self.current_reply), 13)][:3]:
                kagra.draw_text(self.font, line, bx + 5, y, 13, 255, 255, 255, 255)
                y += 18

        IY = SH - 50
        kagra.rect(5, IY, SW - 10, 26, 10, 8, 24, 220)
        kagra.rect(5, IY, SW - 10, 2, 100, 150, 255, 255)
        kagra.rect(5, IY + 24, SW - 10, 2, 100, 150, 255, 255)
        kagra.rect(5, IY, 2, 26, 100, 150, 255, 255)
        kagra.rect(SW - 7, IY, 2, 26, 100, 150, 255, 255)
        kagra.set_ime_cursor_pos(10, IY + 4)

        preedit = kagra.get_preedit_text()
        base = self.input_text[-14:]
        kagra.draw_text(self.font, base, 10, IY + 5, 13, 220, 220, 255, 255)

        px_per_char = 8
        offset_x = 10 + len(base) * px_per_char
        if preedit:
            w = len(preedit) * px_per_char + 2
            kagra.rect(offset_x, IY + 20, w, 2, 255, 220, 80, 255)
            kagra.draw_text(self.font, preedit, offset_x, IY + 5, 13, 255, 220, 80, 255)
        else:
            if int(time.time() * 2) % 2 == 0:
                kagra.draw_text(self.font, "|", offset_x, IY + 5, 13, 180, 180, 255, 255)

        if len(self.input_text) == 0 and not preedit:
            status = "話しかけてね" if self.api_key else "APIキー未設定(オフライン)"
            status += " [R:解除]" if self.click_through else " [R:透過 T:前面]"
            kagra.draw_text(self.font, status, 10, IY + 28, 10, 140, 140, 160, 255)

    def send_chat_message(self, text):
        self.pending = True
        self.current_reply = "考え中..."
        self.reply_timer = 10.0
        self.lipsync.play_text("あ", 3.0)
        
        with self.history_lock:
            self.chat_history.append({"role": "user", "content": text})
            
        threading.Thread(target=self._ai_task, args=(text,), daemon=True).start()

    def _ai_task(self, user_text):
        if not self.client:
            reply = f"「{user_text}」だね！.envにAPIキーを入れてね。"
            with self.history_lock:
                self.chat_history.append({"role": "assistant", "content": reply})
            self.reply_queue.put(reply)
            return

        # スキルシステムによるコンテキスト（カンペ）の自動構築
        added_context = self.skill_manager.process(user_text, self.reply_queue, self.lipsync)

        now_str = datetime.datetime.now().strftime("%Y年%m月%d日 %H時%M分")
        system_prompt = f"{CONFIG['ai']['system_prompt']} 現在日時は{now_str}です。{added_context}"

        try:
            with self.history_lock:
                history_copy = self.chat_history[-7:-1] # 直近のやりとりのみコピー
                
            msgs = [{"role": "system", "content": system_prompt}] + history_copy
            msgs.append({"role": "user", "content": user_text})
            
            resp = self.client.chat.completions.create(
                model="deepseek-chat" if self.use_deepseek else "gpt-4o-mini",
                messages=msgs, max_tokens=150, temperature=0.9
            )
            reply = resp.choices[0].message.content.strip()
        except Exception as e:
            print(f"AIエラー: {e}")
            reply = f"通信エラーだよ: {str(e)[:30]}..."

        with self.history_lock:
            self.chat_history.append({"role": "assistant", "content": reply})
        self.reply_queue.put(reply)

if __name__ == "__main__":
    kagra.init(
        width=SW, height=SH, title="KAGRA Mascot",
        fps=CONFIG["window"]["fps"],
        transparent=True, decorations=False, always_on_top=True
    )
    kagra.run(start_scene=MascotScene())