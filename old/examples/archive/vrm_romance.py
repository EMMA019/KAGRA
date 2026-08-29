"""
vrm_romance.py - KAGRA 恋愛シミュレーション
============================================
ECS 設計 + 性格進化 + 選択肢 + イベント + Boids エフェクト + 複数エンディング

必要:
  pip install openai python-dotenv
  assets/Emma.vrm
  .env ファイルに DEEPSEEK_API_KEY=sk-... を記述
"""
import kagra
from kagra.entity import EntityScene
import threading, math, json, os, random, time
from dotenv import load_dotenv
from openai import OpenAI

# .env ファイルを読み込む
load_dotenv()

SW, SH = 1280, 720

# ══════════════════════════════════════════════════════════════
#  定数・定義
# ══════════════════════════════════════════════════════════════

PERSONALITIES = ['Natural','Tsundere','Yandere','Kuudere','Dandere',
                 'EvoTsundere','EvoYandere','EvoKuudere','EvoDandere']

PERSONA_COLORS = {
    'Natural':    (180,220,255), 'Tsundere':   (255,160,180),
    'Yandere':    (180, 80,180), 'Kuudere':    (140,200,220),
    'Dandere':    (200,255,180), 'EvoTsundere':(255, 60,100),
    'EvoYandere': (140,  0,200), 'EvoKuudere': ( 60,160,255),
    'EvoDandere': (100,255, 80),
}

EMOTION_EXPR = {
    'joy':'Fcl_ALL_Joy','fun':'Fcl_ALL_Fun','sorrow':'Fcl_ALL_Sorrow',
    'angry':'Fcl_ALL_Angry','surprised':'Fcl_ALL_Surprised','neutral':'Fcl_ALL_Neutral',
}

# 背景テーマ（時間帯 × 好感度）
BG_THEMES = {
    'morning': [(30,40,80),(60,80,120)],
    'day':     [(20,30,70),(80,120,180)],
    'evening': [(60,30,50),(120,60,80)],
    'night':   [(5,5,20),(20,15,40)],
}

SYSTEM_PROMPT_BASE = """\
あなたは Emma という AI キャラクター。
現在の性格: {personality}
時間帯: {time_of_day}
好感度: {affection}/100

性格ごとの話し方:
- Natural: 明るく自然
- Tsundere: 照れ隠し、でも嬉しそう
- Yandere: 甘く独占的、時々怖い
- Kuudere: クール、短い返答
- Dandere: はにかみ、語尾が小さい
- Evo*: より強烈なバージョン

返答は必ず以下の JSON のみ。余計な説明不要:
{{
  "reply": "Emma の返答（100文字以内）",
  "emotion": "joy|fun|sorrow|angry|surprised|neutral",
  "score_delta": {{"tsundere":0,"yandere":0,"kuudere":0,"dandere":0}},
  "affection_delta": 1,
  "choices": ["選択肢A","選択肢B","選択肢C"]
}}
choices は次のターンに提示する3択。空配列でも可。
"""

# ══════════════════════════════════════════════════════════════
#  ECS コンポーネント
# ══════════════════════════════════════════════════════════════

class PersonalityComp(kagra.Component):
    """性格・好感度・スコアを管理するコンポーネント"""
    EVOLVE_AFF   = 20
    EVOLVE_DIFF  = 4
    CONFESSION_AFF = 60

    def __init__(self):
        super().__init__()
        self.personality = 'Natural'
        self.affection   = 0
        self.scores      = {'tsundere':0,'yandere':0,'kuudere':0,'dandere':0}
        self.evolved     = False
        self.player_name = "あなた"
        self.route       = None   # 確定したルート

    def apply(self, delta: dict, aff_delta: int):
        for k, v in delta.items():
            if k in self.scores:
                self.scores[k] = max(0, self.scores[k] + v)
        self.affection = max(0, min(100, self.affection + aff_delta))

    def check_evolution(self):
        if self.affection < self.EVOLVE_AFF: return None
        if self.personality == 'Natural':
            best = max(self.scores, key=self.scores.get)
            if self.scores[best] >= self.EVOLVE_DIFF:
                return best.capitalize()
        elif not self.evolved and not self.personality.startswith('Evo'):
            if self.affection >= self.EVOLVE_AFF * 2:
                self.evolved = True
                return 'Evo' + self.personality
        return None

    def get_color(self): return PERSONA_COLORS.get(self.personality, (180,220,255))


class EmotionComp(kagra.Component):
    """表情・感情状態"""
    def __init__(self):
        super().__init__()
        self.current  = 'neutral'
        self.timer    = 0.0
        self.duration = 3.0


class ChatComp(kagra.Component):
    """チャット履歴・入力状態"""
    MAX_HISTORY = 30

    def __init__(self):
        super().__init__()
        self.messages   = []   # (role, text, color)
        self.input_text = ""
        self.pending    = False
        self.choices    = []   # 現在表示中の選択肢
        self.selected   = -1

    def add(self, role, text, color=(200,200,200)):
        self.messages.append((role, text, color))
        if len(self.messages) > self.MAX_HISTORY:
            self.messages.pop(0)


class EventComp(kagra.Component):
    """イベント管理"""
    def __init__(self):
        super().__init__()
        self.triggered   = set()   # 発火済みイベント
        self.current_evt = None    # 実行中イベント名
        self.evt_phase   = 0
        self.evt_timer   = 0.0


class TimeComp(kagra.Component):
    """ゲーム内時間"""
    def __init__(self):
        super().__init__()
        self.real_elapsed = 0.0    # 実時間（秒）
        self.period_secs  = 120.0  # 1時間帯の長さ

    def time_of_day(self):
        t = (self.real_elapsed % (self.period_secs * 4)) / self.period_secs
        return ['morning','day','evening','night'][int(t) % 4]


class EffectComp(kagra.Component):
    """Boids エフェクト"""
    def __init__(self):
        super().__init__()
        self.boid_id     = None
        self.active      = False
        self.effect_type = 'none'  # sakura / spark / firework
        self.timer       = 0.0
        self.flash_color = None
        self.flash_timer = 0.0


# ══════════════════════════════════════════════════════════════
#  ECS スクリプト
# ══════════════════════════════════════════════════════════════

class PersonalityScript(kagra.Script):
    """性格進化・イベントトリガーを管理"""

    def start(self):
        self._scene = None  # シーンへの参照は後でセット

    def update(self, dt):
        p  = self.entity.get(PersonalityComp)
        ev = self.entity.get(EventComp)
        if not p or not ev: return

        # 進化チェック
        new_p = p.check_evolution()
        if new_p and new_p != p.personality:
            old = p.personality
            p.personality = new_p
            ef = self.entity.get(EffectComp)
            if ef:
                ef.effect_type = 'firework'
                ef.timer       = 3.0
                ef.flash_color = p.get_color()
                ef.flash_timer = 1.5
            chat = self.entity.get(ChatComp)
            if chat:
                chat.add('system',
                    f'✨ 性格が {old} → {new_p} に進化！',
                    (255,220,80))

        # イベントチェック
        aff = p.affection
        if aff >= 10 and 'name' not in ev.triggered:
            ev.triggered.add('name')
            ev.current_evt = 'name'
            ev.evt_phase   = 0

        if aff >= 30 and 'date' not in ev.triggered:
            ev.triggered.add('date')
            ev.current_evt = 'date'
            ev.evt_phase   = 0

        if aff >= p.CONFESSION_AFF and 'confession' not in ev.triggered:
            ev.triggered.add('confession')
            ev.current_evt = 'confession'
            ev.evt_phase   = 0


class ExpressionScript(kagra.Script):
    """表情を VRM に適用"""

    def update(self, dt):
        em = self.entity.get(EmotionComp)
        if not em: return
        if em.timer > 0:
            em.timer -= dt
        else:
            # フェードアウト
            em.current = 'neutral'


class EffectScript(kagra.Script):
    """Boids エフェクト管理"""

    def update(self, dt):
        ef = self.entity.get(EffectComp)
        if not ef: return

        if ef.flash_timer > 0:
            ef.flash_timer -= dt

        if ef.timer > 0:
            ef.timer -= dt
            if ef.boid_id is not None and ef.active:
                t = ef.timer
                count = max(1, int((1.0 - t/3.0) * 80000))
                kagra.set_boid_active_count(ef.boid_id, min(count, 100000))
                kagra.update_boids_gpu(ef.boid_id, dt)  # ← 追加: Boids を動かす
        elif ef.boid_id is not None and ef.active:
            kagra.set_boid_active_count(ef.boid_id, 0)
            ef.active      = False
            ef.effect_type = 'none'


class ChatInputScript(kagra.Script):
    """キー入力・APIコール処理（DeepSeek 対応）"""

    def start(self):
        self._response = None
        self._api_key = os.environ.get('DEEPSEEK_API_KEY', '')
        if not self._api_key:
            # .env から読み込めなかった場合のフォールバック
            self._api_key = os.environ.get('ANTHROPIC_API_KEY', '')

        chat = self.entity.get(ChatComp)
        p    = self.entity.get(PersonalityComp)

        if chat:
            if not self._api_key:
                chat.add('system','⚠ DEEPSEEK_API_KEY が未設定です',(255,180,80))
            else:
                chat.add('emma','こんにちは！私は Emma。よろしくね♪',
                         p.get_color() if p else (180,220,255))

    def update(self, dt):
        chat = self.entity.get(ChatComp)
        ev   = self.entity.get(EventComp)
        p    = self.entity.get(PersonalityComp)
        if not chat: return

        # イベント処理
        if ev and ev.current_evt and not chat.pending:
            self._handle_event(ev, chat, p)
            return

        # 選択肢表示中
        if chat.choices:
            # クリック判定
            mx, my = kagra.mouse_pos()
            if kagra.mouse_pressed(kagra.MOUSE_LEFT):
                for i, choice in enumerate(chat.choices):
                    cy = SH - 185 - (len(chat.choices) - i - 1) * 44
                    PX_UI = 640  # チャット欄のX座標
                    PW_UI = SW - 650
                    if cy < my < cy + 36 and PX_UI < mx < PX_UI + PW_UI:
                        self._send_message(choice, chat, p)
                        chat.choices = []
                        return

            # バックスペース処理（kagra.backspace_pressed() を使用）
            if kagra.backspace_pressed() and chat.input_text:
                chat.input_text = chat.input_text[:-1]

            # 文字入力（IME確定文字 / 通常キー）
            chars = kagra.get_typed_chars()
            for c in chars:
                # 制御文字はスキップ（Enterは別途処理）
                if c >= ' ' or c == '\t':
                    if len(chat.input_text) < 60:
                        chat.input_text += c

            # Enterで送信（kagra.enter_pressed() を使用）
            if kagra.enter_pressed() and chat.input_text.strip() and not chat.pending:
                msg = chat.input_text.strip()
                chat.input_text = ""
                chat.choices = []
                self._send_message(msg, chat, p)
                return

            return  # 選択肢表示中はここで終了

        # 通常テキスト入力モード
        # バックスペース処理
        if kagra.backspace_pressed() and chat.input_text:
            chat.input_text = chat.input_text[:-1]

        # 文字入力
        chars = kagra.get_typed_chars()
        for c in chars:
            if c >= ' ' or c == '\t':
                if len(chat.input_text) < 60:
                    chat.input_text += c

        # Enterで送信
        if kagra.enter_pressed() and chat.input_text.strip() and not chat.pending:
            self._send_message(chat.input_text.strip(), chat, p)
            chat.input_text = ""

        # レスポンス受取
        if self._response is not None:
            self._apply_response(self._response, chat, p)
            self._response  = None
            chat.pending    = False

    def _handle_event(self, ev, chat, p):
        evt = ev.current_evt
        if evt == 'name' and ev.evt_phase == 0:
            chat.add('emma', f'ねえ、{p.player_name}って呼んでもいい？', p.get_color())
            chat.choices = ['うん、いいよ','なんで急に？','……']
            ev.evt_phase = 1
        elif evt == 'name' and ev.evt_phase == 1:
            ev.current_evt = None
        elif evt == 'date' and ev.evt_phase == 0:
            chat.add('emma', '今度、二人でどこか行かない…？', p.get_color())
            chat.choices = ['カフェに行こう！','公園がいいな','いつでもいいよ']
            ev.evt_phase = 1
        elif evt == 'date' and ev.evt_phase == 1:
            ev.current_evt = None
        elif evt == 'confession' and ev.evt_phase == 0:
            if p.personality.startswith('Evo'):
                msg = self._confession_msg(p.personality)
            else:
                msg = '…あなたのことが、好きです。'
            chat.add('emma', msg, p.get_color())
            chat.choices = ['私も好きだよ','…ごめん、友達として見てた','考えさせて']
            ev.evt_phase = 1
        elif evt == 'confession' and ev.evt_phase == 1:
            ev.current_evt = None

    def _confession_msg(self, personality):
        msgs = {
            'EvoTsundere': 'べ、別に好きとかじゃ…うそ、大好き！ばか！',
            'EvoYandere':  'あなただけ。永遠に私だけのもの…ね？',
            'EvoKuudere':  '…好き。それだけ。',
            'EvoDandere':  'す、好きで…す…聞こえた…？',
        }
        return msgs.get(personality, '…好きです。')

    def _send_message(self, msg, chat, p):
        chat.add('you', msg, (180,230,255))
        chat.pending = True
        chat.add('emma', '…', p.get_color() if p else (200,200,200))
        tm = self.entity.get(TimeComp)
        tod = tm.time_of_day() if tm else 'day'
        system = SYSTEM_PROMPT_BASE.format(
            personality=p.personality if p else 'Natural',
            time_of_day=tod,
            affection=p.affection if p else 0,
        )
        # 会話履歴（直近8件）
        history = []
        for role, text, _ in chat.messages[-10:]:
            if role in ('you','emma') and '…' != text:
                r = 'user' if role == 'you' else 'assistant'
                history.append({'role': r, 'content': text})

        threading.Thread(
            target=self._call_api_deepseek,
            args=(system, history, msg),
            daemon=True
        ).start()

    def _call_api_deepseek(self, system, history, user_msg):
        try:
            client = OpenAI(
                api_key=self._api_key,
                base_url="https://api.deepseek.com"
            )
            messages = [
                {"role": "system", "content": system}
            ] + history + [
                {"role": "user", "content": user_msg}
            ]
            res = client.chat.completions.create(
                model="deepseek-chat",
                messages=messages,
                max_tokens=400,
                temperature=0.8,
            )
            raw = res.choices[0].message.content.strip()
            raw = raw.replace('```json','').replace('```','').strip()
            if '{' in raw:
                raw = raw[raw.index('{'):raw.rindex('}')+1]
            data = json.loads(raw)
        except json.JSONDecodeError:
            data = {'reply': 'ごめん、少し混乱しちゃった…もう一度話しかけて？',
                    'emotion':'sorrow','score_delta':{},'affection_delta':0,'choices':[]}
        except Exception as e:
            data = {'reply': f'ごめん…({type(e).__name__})',
                    'emotion':'sorrow','score_delta':{},'affection_delta':0,'choices':[]}
        self._response = data

    def _apply_response(self, data, chat, p):
        reply   = data.get('reply','...')
        emotion = data.get('emotion','neutral')
        delta   = data.get('score_delta',{})
        aff     = data.get('affection_delta',1)
        choices = data.get('choices',[])

        # 「…」のプレースホルダを置換
        for i in range(len(chat.messages)-1,-1,-1):
            if chat.messages[i][0]=='emma' and chat.messages[i][1]=='…':
                col = p.get_color() if p else (200,200,200)
                chat.messages[i] = ('emma', reply, col)
                break

        if p: p.apply(delta, aff)

        # 表情
        em = self.entity.get(EmotionComp)
        if em:
            em.current  = emotion
            em.timer    = 3.5

        # エフェクト
        ef = self.entity.get(EffectComp)
        if ef and emotion in ('joy','fun') and ef.boid_id is not None:
            ef.effect_type = 'sakura'
            ef.timer       = 2.0
            ef.active      = True
            kagra.set_boid_active_count(ef.boid_id, 30000)

        # 選択肢
        if choices: chat.choices = choices[:3]

        # エンディング判定
        if p and p.affection >= p.CONFESSION_AFF:
            ev = self.entity.get(EventComp)
            if ev and 'confession' in ev.triggered:
                p.route = p.personality  # ルート確定


# ══════════════════════════════════════════════════════════════
#  メインシーン（EntityScene を継承）
# ══════════════════════════════════════════════════════════════

class RomanceScene(EntityScene):

    def on_enter(self):
        self.font    = kagra.assets.font("meiryo")
        self._time   = 0.0
        self._cursor = 0.0
        self._ending = None

        # VRM
        self.av  = kagra.avatar("assets/Emma.vrm")
        self.av.play("idle")
        self.cam = kagra.Camera3D(SW, SH, fov_deg=40.0)
        # カメラ距離を調整（キャラクターを小さく）
        self.cam.use_orbit(radius=2.0, theta=0.0, phi=0.0,
                           target=(1.0, 0.5, 1.0))  # 固定・正面向き

        # GPU Boids（エフェクト用）
        self.boid_id = kagra.create_boid_system_gpu(200_000, float(SW), float(SH))
        kagra.set_boid_active_count(self.boid_id, 0)

        # ── ECS エンティティ生成 ────────────────────────────────
        self.emma = self.world.create("emma", tag="emma")

        # コンポーネント追加
        self.emma.add(PersonalityComp())
        self.emma.add(EmotionComp())
        self.emma.add(ChatComp())
        self.emma.add(EventComp())
        self.emma.add(TimeComp())

        ef = EffectComp()
        ef.boid_id = self.boid_id
        self.emma.add(ef)

        # スクリプト追加
        ps = PersonalityScript(); ps._scene = self
        self.emma.add(ps)
        self.emma.add(ExpressionScript())
        self.emma.add(EffectScript())
        ci = ChatInputScript()
        self.emma.add(ci)

    def on_exit(self):
        pass

    def update(self, dt):
        if kagra.pressed("ESCAPE"): raise SystemExit

        super().update(dt)   # ECS world.update

        self._time   += dt
        self._cursor += dt

        # 時間コンポーネント更新
        tm = self.emma.get(TimeComp)
        if tm: tm.real_elapsed += dt

        # VRM 更新
        em = self.emma.get(EmotionComp)
        if em and em.timer > 0:
            expr = EMOTION_EXPR.get(em.current,'Fcl_ALL_Neutral')
            self.av.set_expression(expr, 1.0)
        else:
            self.av.reset_expressions()
        self.av.update(dt)

        # Boids 更新
        ef = self.emma.get(EffectComp)
        if ef and ef.active:
            kagra.update_boids_gpu(self.boid_id, dt)

        # カメラ更新
        self.cam.update(kagra.get_engine())

        # エンディング判定
        p = self.emma.get(PersonalityComp)
        if p and p.route and not self._ending:
            self._ending = p.route

    def draw(self):
        # ── 背景 ────────────────────────────────────────────────
        self._draw_background()

        # ── VRM ─────────────────────────────────────────────────
        kagra.draw_vrm(self.av.vrm_id)

        # ── Boids エフェクト ─────────────────────────────────────
        ef = self.emma.get(EffectComp)
        if ef and ef.active:
            kagra.draw_boids_gpu(self.boid_id)

        # ── チャット UI ──────────────────────────────────────────
        self._draw_chat_ui()

        # ── フラッシュ ───────────────────────────────────────────
        if ef and ef.flash_timer > 0:
            a = int(min(1.0, ef.flash_timer) * 120)
            if ef.flash_color:
                kagra.rect(0, 0, SW, SH, *ef.flash_color, a)

        # ── エンディング ─────────────────────────────────────────
        if self._ending:
            self._draw_ending()

    def _draw_background(self):
        tm = self.emma.get(TimeComp)
        tod = tm.time_of_day() if tm else 'day'
        c1, c2 = BG_THEMES.get(tod, BG_THEMES['day'])

        steps = 8
        for i in range(steps):
            t  = i / steps
            r  = int(c1[0]*(1-t) + c2[0]*t)
            g  = int(c1[1]*(1-t) + c2[1]*t)
            b  = int(c1[2]*(1-t) + c2[2]*t)
            h  = SH // steps
            kagra.rect(0, i*h, SW, h+1, r, g, b, 255)

        if tod == 'night':
            random.seed(42)
            for _ in range(80):
                x = random.randint(0,SW)
                y = random.randint(0,SH//2)
                a = int(128 + 127*math.sin(self._time*2 + x*0.1))
                kagra.rect(x, y, 2, 2, 255, 255, 255, a)

        kagra.rect(0, SH-80, SW, 80, c2[0]//2, c2[1]//2, c2[2]//2, 180)

    def _draw_chat_ui(self):
        chat = self.emma.get(ChatComp)
        p    = self.emma.get(PersonalityComp)
        tm   = self.emma.get(TimeComp)
        if not chat: return

        pc   = p.get_color() if p else (180,220,255)
        tod  = tm.time_of_day() if tm else 'day'

        PX, PY, PW, PH = 640, 50, SW-650, SH-200
        kagra.rect(PX, PY, PW, PH, 8, 6, 20, 200)

        LINE_H = 22
        y = PY + 10
        for role, text, color in chat.messages[-18:]:
            if y > PY + PH - LINE_H: break
            prefix = {'you':'You: ','emma':'Emma: ','system':'','event':'📌 '}
            full   = prefix.get(role,'') + text
            chunk = 26
            while full and y < PY + PH - LINE_H:
                kagra.draw_text(self.font, full[:chunk], PX+8, y, 16, *color)
                full = full[chunk:]
                y   += LINE_H
            y += 3

        if chat.choices:
            for i, ch in enumerate(chat.choices):
                cy   = SH - 185 - (len(chat.choices)-i-1)*44
                mx,my = kagra.mouse_pos()
                hover = PX < mx < PX+PW and cy < my < cy+36
                bg    = (*pc, 180) if hover else (30,25,60,200)
                kagra.rect(PX, cy, PW, 36, *bg[:3], bg[3])
                tc = (20,15,40) if hover else (220,220,240)
                kagra.draw_text(self.font, f'{i+1}. {ch}', PX+12, cy+8, 17, *tc)

        IY = SH - 90
        kagra.rect(PX, IY, PW, 40, 20,16,45, 230)
        kagra.set_ime_cursor_pos(PX + 8, IY + 10)

        if chat.pending:
            kagra.draw_text(self.font, '…考え中…', PX+8, IY+10, 18, 160,160,200)
        else:
            preedit = kagra.get_preedit_text()
            base    = chat.input_text[-20:]
            kagra.draw_text(self.font, base, PX+8, IY+10, 18, 220,220,255)
            px_per_char = 10
            offset_x    = len(base) * px_per_char
            if preedit:
                w = len(preedit) * px_per_char + 4
                kagra.rect(PX+8+offset_x, IY+26, w, 2, 255,220,80,255)
                kagra.draw_text(self.font, preedit,
                                PX+8+offset_x, IY+10, 18, 255,220,80)
            else:
                if int(self._cursor*2)%2==0:
                    kagra.draw_text(self.font, '|',
                                    PX+8+offset_x, IY+10, 18, 180,180,255)

        hint = 'Enter で送信' if not chat.choices else 'クリック or Enter で送信'
        kagra.draw_text(self.font, hint, PX+8, IY+44, 12, 100,100,140)

        if p:
            self._draw_personality_panel(p, tod)

    def _draw_personality_panel(self, p, tod):
        PNX, PNY, PNW, PNH = 10, SH-185, 250, 130
        kagra.rect(PNX, PNY, PNW, PNH, 10,8,25, 200)

        pc = p.get_color()
        kagra.draw_text(self.font, f'💫 {p.personality}',
                        PNX+8, PNY+6, 17, *pc)

        bar_max = PNW - 20
        bar_w   = int(p.affection / 100 * bar_max)
        kagra.rect(PNX+8, PNY+30, bar_max, 10, 30,30,55, 220)
        if bar_w > 0:
            kagra.rect(PNX+8, PNY+30, bar_w, 10, *pc, 255)
        kagra.draw_text(self.font, f'好感度 {p.affection}/100',
                        PNX+8, PNY+44, 13, 160,160,200)

        SC_COLORS = {'tsundere':(255,140,160),'yandere':(180,60,180),
                     'kuudere':(100,200,240),'dandere':(140,220,100)}
        for i,(name,val) in enumerate(p.scores.items()):
            bw = min(int(val*10), bar_max-20)
            c  = SC_COLORS[name]
            kagra.rect(PNX+8, PNY+60+i*16, bar_max-20, 10, 25,25,50, 200)
            if bw > 0:
                kagra.rect(PNX+8, PNY+60+i*16, bw, 10, *c, 200)
            kagra.draw_text(self.font, name[:3].upper(),
                            PNX+bar_max-8, PNY+61+i*16, 11, *c)

        tod_icon = {'morning':'🌅','day':'☀️','evening':'🌇','night':'🌙'}
        kagra.draw_text(self.font, tod_icon.get(tod,''),
                        PNX+PNW-28, PNY+8, 18, 255,220,100)

    def _draw_ending(self):
        p = self.emma.get(PersonalityComp)
        ENDINGS = {
            'Tsundere':    ('ツンデレルート', '「…うるさい。でも、嫌いじゃないから。」'),
            'EvoTsundere': ('TRUE ツンデレ END', '「絶対に離さないから覚悟して！」'),
            'Yandere':     ('ヤンデレルート', '「ずっと一緒にいよう。永遠に…」'),
            'EvoYandere':  ('TRUE ヤンデレ END', '「世界中が敵になっても、あなただけいればいい」'),
            'Kuudere':     ('クーデレルート', '「……一緒にいる。それだけ。」'),
            'EvoKuudere':  ('TRUE クーデレ END', '「あなたと話す時間が、唯一の贅沢」'),
            'Dandere':     ('ダンデレルート', '「す、好きです…これからも…よろしく」'),
            'EvoDandere':  ('TRUE ダンデレ END', '「ずっと、あなたの隣にいさせてください」'),
        }
        route = self._ending
        title, msg = ENDINGS.get(route, ('エンディング', '…ありがとう。'))
        pc = p.get_color() if p else (180,220,255)

        a = min(160, int(self._time * 20))
        kagra.rect(0, SH//2-80, SW, 160, 5,3,15, a)
        kagra.draw_text(self.font, title,
                        SW//2-120, SH//2-60, 32, *pc)
        kagra.draw_text(self.font, msg,
                        SW//2-200, SH//2, 20, 240,240,255)
        kagra.draw_text(self.font, 'ESC で終了',
                        SW//2-60, SH//2+50, 14, 120,120,160)


# ══════════════════════════════════════════════════════════════
if __name__ == "__main__":
    kagra.init(SW, SH, "KAGRA - Emma Romance / 性格進化 AI", 60)
    kagra.run(start_scene=RomanceScene())