"""
kagra_romance/scripts.py
ECS スクリプト（ロジック）
"""
import kagra
from .persona    import EMOTION_EXPR, ENDINGS
from .components import PersonalityComp, EmotionComp, ChatComp, EventComp, TimeComp, EffectComp
from .chat_engine import ChatHistory, ChatEngine

PX, PY_CHAT = 640, 50   # チャット欄の X 座標


class PersonalityScript(kagra.Script):
    """性格進化・イベントトリガー"""

    def update(self, dt):
        p  = self.entity.get(PersonalityComp)
        ev = self.entity.get(EventComp)
        if not p or not ev: return

        # 進化チェック
        new_p = p.check_evolution()
        if new_p and new_p != p.personality:
            old_p = p.personality
            p.evolve_to(new_p)
            chat = self.entity.get_component('chat_history')
            if chat:
                chat.add('system', f'✨ {old_p} → {new_p} に進化！', (255,220,80))
            ef = self.entity.get(EffectComp)
            if ef:
                ef.trigger('firework', p.get_color(), duration=3.0)

        # イベントトリガー
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
    """表情タイマー管理"""

    def update(self, dt):
        em = self.entity.get(EmotionComp)
        if not em: return
        if em.timer > 0:
            em.timer -= dt


class EffectScript(kagra.Script):
    """Boids エフェクト"""

    def update(self, dt):
        ef = self.entity.get(EffectComp)
        if not ef or not ef.active: return

        if ef.flash_timer > 0:
            ef.flash_timer -= dt

        if ef.timer > 0:
            ef.timer -= dt
            if ef.boid_id is not None:
                count = max(1, int((1.0 - ef.timer/3.0) * 80000))
                kagra.set_boid_active_count(ef.boid_id, min(count, 100000))
                kagra.update_boids_gpu(ef.boid_id, dt)
        else:
            if ef.boid_id is not None:
                kagra.set_boid_active_count(ef.boid_id, 0)
            ef.active = False


class ChatInputScript(kagra.Script):
    """入力・API・レスポンス処理"""

    def start(self):
        import os
        from dotenv import load_dotenv
        load_dotenv()
        api_key = os.environ.get('DEEPSEEK_API_KEY','') or \
                  os.environ.get('ANTHROPIC_API_KEY','')
        self.engine  = ChatEngine(api_key)
        self.history = ChatHistory()

        p = self.entity.get(PersonalityComp)
        color = p.get_color() if p else (180,220,255)
        if not api_key:
            self.history.add('system', '⚠ DEEPSEEK_API_KEY が未設定です', (255,180,80))
        else:
            self.history.add('emma', 'こんにちは！私は Emma。よろしくね♪', color)

    def update(self, dt):
        chat = self.entity.get(ChatComp)
        p    = self.entity.get(PersonalityComp)
        ev   = self.entity.get(EventComp)
        if not chat or not p: return

        # API レスポンス受取（最優先）
        result = self.engine.poll()
        if result is not None:
            self._apply(result, chat, p)

        # イベント優先
        if ev and ev.current_evt and not self.engine.pending:
            self._handle_event(ev, chat, p)
            return

        # バックスペース（IME変換中は無視）
        if kagra.backspace_pressed() and not chat.ime_active:
            chat.input_text = chat.input_text[:-1]

        # 文字入力（IME確定文字）
        for c in kagra.get_typed_chars():
            if c >= ' ' and len(chat.input_text) < 80:
                chat.input_text += c

        # Enter で送信（IME 変換中は送信しない）
        if kagra.enter_pressed() and not chat.ime_active:
            if chat.choices:
                # 選択肢がある時は Enter で1番目を選択
                self._choose(0, chat, p)
            elif chat.input_text.strip() and not self.engine.pending:
                self._send(chat.input_text.strip(), chat, p)
                chat.input_text = ""

        # 選択肢クリック
        if chat.choices and kagra.mouse_pressed(kagra.MOUSE_LEFT):
            mx, my = kagra.mouse_pos()
            PW = 1280 - 650
            for i, ch in enumerate(chat.choices):
                cy = 720 - 185 - (len(chat.choices) - i - 1) * 44
                if cy < my < cy + 36 and PX < mx < PX + PW:
                    self._choose(i, chat, p)
                    break

    def _choose(self, idx: int, chat: ChatComp, p: PersonalityComp):
        if idx < len(chat.choices):
            msg = chat.choices[idx]
            chat.choices = []
            self._send(msg, chat, p)

    def _send(self, msg: str, chat: ChatComp, p: PersonalityComp):
        tm  = self.entity.get(TimeComp)
        tod = tm.time_of_day() if tm else 'day'
        self.history.add('you',  msg, (180,230,255))
        self.history.add('emma', '…', p.get_color())
        self.engine.send(
            system   = p.get_system_prompt(tod),
            history  = self.history.history_for_api(),
            user_msg = msg,
        )

    def _apply(self, data: dict, chat: ChatComp, p: PersonalityComp):
        reply   = data.get('reply',   '…')
        emotion = data.get('emotion', 'neutral')
        delta   = data.get('score_delta', {})
        aff     = data.get('affection_delta', 1)
        choices = data.get('choices', [])

        self.history.replace_last_emma(reply, p.get_color())
        p.apply(delta, aff)

        em = self.entity.get(EmotionComp)
        if em:
            em.current = emotion
            em.timer   = 3.5

        ef = self.entity.get(EffectComp)
        if ef and emotion in ('joy','fun') and ef.boid_id is not None:
            ef.trigger('sakura', p.get_color(), duration=2.5)

        if choices:
            chat.choices = [c for c in choices[:3] if c]

        if p.affection >= p.CONFESSION_AFF:
            ev = self.entity.get(EventComp)
            if ev and 'confession' in ev.triggered:
                p.route = p.personality

    def _handle_event(self, ev: EventComp, chat: ChatComp, p: PersonalityComp):
        EVENTS = {
            ('name', 0):       ('ねえ、名前で呼んでもいい？',
                                ['うん、いいよ','なんで急に？','……']),
            ('date', 0):       ('今度、二人でどこか行かない…？',
                                ['カフェに行こう！','公園がいいな','いつでもいいよ']),
            ('confession', 0): (self._confession_msg(p.personality),
                                ['私も好きだよ','…ごめん、友達として見てた','考えさせて']),
        }
        key = (ev.current_evt, ev.evt_phase)
        if key in EVENTS:
            msg, choices = EVENTS[key]
            self.history.add('emma', msg, p.get_color())
            chat.choices = choices
            ev.evt_phase = 1
        elif ev.evt_phase == 1:
            ev.current_evt = None

    def _confession_msg(self, p: str) -> str:
        return {
            'EvoTsundere': 'べ、別に好きとかじゃ…うそ、大好き！ばか！',
            'EvoYandere':  'あなただけ。永遠に私だけのもの…ね？',
            'EvoKuudere':  '…好き。それだけ。',
            'EvoDandere':  'す、好きで…す…聞こえた…？',
        }.get(p, '…あなたのことが、好きです。')
