"""
kagra_romance/components.py
ECS コンポーネント定義
"""
import kagra
from .persona import PERSONA_COLORS, PERSONALITY_DESC, SYSTEM_PROMPT


class EmotionComp(kagra.Component):
    def __init__(self):
        super().__init__()
        self.current  = 'neutral'
        self.timer    = 0.0


class ChatComp(kagra.Component):
    def __init__(self):
        super().__init__()
        self.input_text  = ""
        self.choices     = []
        self._ime_active = False  # IME 変換中フラグ

    @property
    def ime_active(self) -> bool:
        return bool(kagra.get_preedit_text())


class EventComp(kagra.Component):
    def __init__(self):
        super().__init__()
        self.triggered   = set()
        self.current_evt = None
        self.evt_phase   = 0


class TimeComp(kagra.Component):
    PERIOD = 120.0  # 1時間帯 = 120秒

    def __init__(self):
        super().__init__()
        self.elapsed = 0.0

    def update(self, dt: float):
        self.elapsed += dt

    def time_of_day(self) -> str:
        t = (self.elapsed % (self.PERIOD * 4)) / self.PERIOD
        return ['morning','day','evening','night'][int(t) % 4]


class EffectComp(kagra.Component):
    def __init__(self):
        super().__init__()
        self.boid_id     = None
        self.active      = False
        self.timer       = 0.0
        self.flash_color = None
        self.flash_timer = 0.0

    def trigger(self, effect_type: str, color, duration=2.5):
        self.active      = True
        self.timer       = duration
        self.flash_color = color
        self.flash_timer = 1.2



# ── コンポーネント ────────────────────────────────────────────

class PersonalityComp(kagra.Component):
    EVOLVE_AFF     = 20
    EVOLVE_DIFF    = 4
    CONFESSION_AFF = 60

    def __init__(self):
        super().__init__()
        self.personality = 'Natural'
        self.affection   = 0
        self.scores      = {'tsundere':0,'yandere':0,'kuudere':0,'dandere':0}
        self.evolved     = False
        self.player_name = 'あなた'
        self.route       = None

    def apply(self, delta: dict, aff_delta: int):
        for k, v in delta.items():
            if k in self.scores:
                self.scores[k] = max(0, self.scores[k] + int(v))
        self.affection = max(0, min(100, self.affection + int(aff_delta)))

    def check_evolution(self) -> str | None:
        if self.affection < self.EVOLVE_AFF:
            return None
        if self.personality == 'Natural':
            best = max(self.scores, key=self.scores.get)
            if self.scores[best] >= self.EVOLVE_DIFF:
                return best.capitalize()
        elif not self.evolved and not self.personality.startswith('Evo'):
            if self.affection >= self.EVOLVE_AFF * 2:
                self.evolved = True
                return 'Evo' + self.personality
        return None

    def evolve_to(self, new_p: str):
        self.personality = new_p
        self.affection   = max(0, self.affection - self.EVOLVE_AFF)

    def get_color(self):
        return PERSONA_COLORS.get(self.personality, (180,220,255))

    def get_system_prompt(self, tod: str) -> str:
        return SYSTEM_PROMPT.format(
            personality      = self.personality,
            personality_desc = PERSONALITY_DESC.get(self.personality,''),
            time_of_day      = tod,
            affection        = self.affection,
            player_name      = self.player_name,
        )
