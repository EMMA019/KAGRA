# examples/personality_quiz.py
# ドラクエ3風 性格診断ゲーム
# 操作:
#   キーボード: ↑↓選択 / Z or Enter 決定
#   マウス    : 選択肢をクリック

import kagra
import sys
import os

SW, SH = 800, 600
kagra.init(width=SW, height=SH, title="性格診断の旅", fps=60)

FONT_PATH = "C:/Windows/Fonts/meiryo.ttc"

# ── 音声パス（任意） ──────────────────────────────────────────
BGM_TITLE = "assets/audio/title.ogg"
BGM_GODDESS = "assets/audio/goddess.ogg"
BGM_ADVENTURE = "assets/audio/adventure.ogg"
BGM_RESULT = "assets/audio/result.ogg"
SE_CONFIRM = "assets/audio/confirm.wav"
SE_CURSOR = "assets/audio/cursor.wav"

# ── 性格定義 ─────────────────────────────────────────────────
PERSONALITIES = {
    "ゆうかん":      {"job": "勇者",       "img": "chr_yuukan",      "desc": "行動優先・リスク耐性高。迷わず前へ進む勇気の持ち主。"},
    "ごうけつ":      {"job": "戦士",       "img": "chr_gouketsu",    "desc": "力と意志で押し通す。感情より行動、頼れる大黒柱。"},
    "かしこい":      {"job": "魔法使い",   "img": "chr_kashikoi",    "desc": "分析・計画・論理優先。知識こそ最大の武器。"},
    "やさしい":      {"job": "僧侶",       "img": "chr_yasashii",    "desc": "共感・他者優先・調和。仲間を癒す心の支え。"},
    "ぬけめない":    {"job": "商人",       "img": "chr_nukemenai",   "desc": "現実的・利益計算・観察眼鋭い。世渡り上手。"},
    "ようきもの":    {"job": "遊び人",     "img": "chr_yokimono",    "desc": "快楽・直感・自由奔放。人生を楽しむ天才。"},
    "おちつき":      {"job": "賢者",       "img": "chr_ochitsuki",   "desc": "バランス・俯瞰・冷静。全体を見渡す知恵者。"},
    "すばしっこい":  {"job": "盗賊",       "img": "chr_subashikkoi", "desc": "慎重・独立・観察眼。誰も気づかない所を見抜く。"},
    "むっつり":      {"job": "賢者(別)",   "img": "chr_muttsuri",    "desc": "内向き・深い思考・秘密主義。真の実力は底知れず。"},
    "セクシーギャル": {"job": "占い師",     "img": "chr_sexy",        "desc": "魅力・社交・直感。星が告げる運命を読む者。"},
}

# ── 質問定義 ─────────────────────────────────────────────────
QUESTIONS = [
    {
        "text": "朝、目覚めたとき\nまず何をする？",
        "choices": [
            ("すぐ起きて行動する",      {"ゆうかん": 3, "ごうけつ": 2}),
            ("今日の計画を立てる",      {"かしこい": 3, "おちつき": 2}),
            ("もう少しだけ二度寝…",    {"ようきもの": 3, "むっつり": 1}),
            ("仲間の様子を確認する",    {"やさしい": 3, "おちつき": 1}),
        ]
    },
    {
        "text": "仲間がピンチ！\nあなたはどうする？",
        "choices": [
            ("迷わず助けに飛び込む",    {"ゆうかん": 3, "ごうけつ": 2}),
            ("まず状況を分析する",      {"かしこい": 3, "むっつり": 2}),
            ("作戦を考えてから動く",    {"おちつき": 3, "かしこい": 1}),
            ("声をかけて励ます",        {"やさしい": 3, "セクシーギャル": 1}),
        ]
    },
    {
        "text": "宝箱を発見！\nでも罠かもしれない。",
        "choices": [
            ("即開ける！宝は俺のもの",  {"ゆうかん": 2, "ようきもの": 3}),
            ("慎重に罠を調べる",        {"すばしっこい": 3, "かしこい": 2}),
            ("誰かに開けさせる",        {"ぬけめない": 3, "むっつり": 1}),
            ("開けずに持ち帰る",        {"おちつき": 2, "かしこい": 1}),
        ]
    },
    {
        "text": "あなたが\n大切にするものは？",
        "choices": [
            ("ちから・勇気",            {"ゆうかん": 2, "ごうけつ": 3}),
            ("かね・財産",              {"ぬけめない": 3, "ようきもの": 1}),
            ("なかま・絆",              {"やさしい": 3, "ゆうかん": 1}),
            ("ちえ・知識",              {"かしこい": 3, "おちつき": 2}),
        ]
    },
    {
        "text": "人に秘密を\n打ち明けられた。",
        "choices": [
            ("絶対に守り通す",          {"むっつり": 3, "やさしい": 2}),
            ("必要なら使う",            {"ぬけめない": 3, "すばしっこい": 1}),
            ("相談に乗る",              {"やさしい": 3, "セクシーギャル": 2}),
            ("忘れてしまう",            {"ようきもの": 3, "ごうけつ": 1}),
        ]
    },
    {
        "text": "初対面の人から\nどう思われることが多い？",
        "choices": [
            ("頼りになりそう",          {"ゆうかん": 2, "ごうけつ": 2}),
            ("色っぽい・魅力的",        {"セクシーギャル": 4, "ようきもの": 1}),
            ("物知りそう",              {"かしこい": 2, "おちつき": 2}),
            ("優しそう・穏やか",        {"やさしい": 3, "むっつり": 1}),
        ]
    },
    {
        "text": "お金が\n大量に手に入った！",
        "choices": [
            ("全部使って楽しむ",        {"ようきもの": 3, "セクシーギャル": 2}),
            ("賢く投資・運用する",      {"ぬけめない": 3, "かしこい": 2}),
            ("仲間に分ける",            {"やさしい": 3, "ゆうかん": 1}),
            ("将来のために貯める",      {"おちつき": 3, "むっつり": 2}),
        ]
    },
    {
        "text": "魔王を倒すとしたら\nどんな方法で？",
        "choices": [
            ("正面から力で叩き潰す",    {"ごうけつ": 3, "ゆうかん": 2}),
            ("弱点を研究して確実に",    {"かしこい": 3, "すばしっこい": 2}),
            ("仲間と力を合わせて",      {"やさしい": 2, "おちつき": 2}),
            ("魔王と交渉してみる",      {"ぬけめない": 2, "セクシーギャル": 2}),
        ]
    },
    {
        "text": "旅の一番の\n目的は何？",
        "choices": [
            ("世界を救うため",          {"ゆうかん": 3, "やさしい": 2}),
            ("強くなるため",            {"ごうけつ": 3, "ゆうかん": 1}),
            ("お金・名声を得るため",    {"ぬけめない": 3, "ようきもの": 1}),
            ("自分を知るため",          {"おちつき": 2, "むっつり": 3}),
        ]
    },
    {
        "text": "失敗したとき\n真っ先に思うことは？",
        "choices": [
            ("次こそやってやる！",      {"ゆうかん": 3, "ごうけつ": 2}),
            ("なぜ失敗したか分析する",  {"かしこい": 3, "おちつき": 2}),
            ("誰かのせいにしたくなる",  {"むっつり": 2, "ぬけめない": 1}),
            ("まあいいか笑い飛ばす",   {"ようきもの": 3, "セクシーギャル": 1}),
        ]
    },
    {
        "text": "理想の戦い方は？",
        "choices": [
            ("真っ向勝負！正々堂々",    {"ゆうかん": 2, "ごうけつ": 3}),
            ("策略で相手を翻弄する",    {"かしこい": 2, "すばしっこい": 3}),
            ("仲間を守りながら戦う",    {"やさしい": 3, "おちつき": 1}),
            ("勝てる戦いしかしない",    {"ぬけめない": 3, "むっつり": 2}),
        ]
    },
    {
        "text": "パーティに\n一人だけ誘えるなら？",
        "choices": [
            ("最強の戦士",              {"ごうけつ": 2, "ゆうかん": 1}),
            ("物知りな魔法使い",        {"かしこい": 2, "おちつき": 1}),
            ("癒してくれる僧侶",        {"やさしい": 2, "むっつり": 1}),
            ("楽しい仲間なら誰でも",    {"ようきもの": 2, "セクシーギャル": 2}),
        ]
    },
]

# ── 冒険シーン定義 ───────────────────────────────────────────
ADVENTURE_SCENES = [
    {
        "bg": "bg_castle",
        "location": "王城",
        "narration": "王城に到着した。\n謁見の間で王様が待っている。",
        "question": "無礼な貴族があなたを\n見下した態度で話しかけてきた。",
        "choices": [
            ("きっぱり言い返す",        {"ゆうかん": 2, "ごうけつ": 2}),
            ("無視して王様の元へ",      {"おちつき": 2, "すばしっこい": 1}),
            ("愛想よく話を合わせる",    {"セクシーギャル": 2, "ぬけめない": 2}),
            ("あとで仕返しを考える",    {"むっつり": 3, "すばしっこい": 1}),
        ]
    },
    {
        "bg": "bg_village",
        "location": "村",
        "narration": "小さな村にたどり着いた。\n村人たちが困った顔をしている。",
        "question": "老人が重い荷物を\n持って苦しんでいる。",
        "choices": [
            ("迷わず助ける",            {"やさしい": 3, "ゆうかん": 1}),
            ("報酬を確認してから",      {"ぬけめない": 3, "むっつり": 1}),
            ("村人に頼む",              {"おちつき": 2, "かしこい": 1}),
            ("見なかったことにする",    {"むっつり": 2, "すばしっこい": 1}),
        ]
    },
    {
        "bg": "bg_field",
        "location": "フィールド",
        "narration": "広大なフィールドに出た。\n突然、魔物が現れた！",
        "question": "強そうな魔物と\n遭遇してしまった！",
        "choices": [
            ("正面から戦う！",          {"ゆうかん": 2, "ごうけつ": 3}),
            ("逃げ道を探す",            {"すばしっこい": 3, "かしこい": 1}),
            ("弱点を見極めてから",      {"かしこい": 3, "おちつき": 1}),
            ("魔物に話しかける",        {"やさしい": 2, "セクシーギャル": 2}),
        ]
    },
    {
        "bg": "bg_cave",
        "location": "洞窟",
        "narration": "暗い洞窟に入った。\n奥から声が聞こえる…",
        "question": "仲間が助けを\n呼んでいる声がする！",
        "choices": [
            ("すぐ声のする方へ向かう",  {"ゆうかん": 2, "やさしい": 2}),
            ("罠かもしれず慎重に進む",  {"すばしっこい": 3, "むっつり": 2}),
            ("周りを確認してから行く",  {"おちつき": 3, "かしこい": 1}),
            ("大声で返事をする",        {"ごうけつ": 2, "ようきもの": 1}),
        ]
    },
]

# ── アセット管理 ──────────────────────────────────────────────
_assets = {}

def safe_play_bgm(path, loop=True, volume=0.7):
    if os.path.exists(path):
        try:
            kagra.audio.play_bgm(path, loop=loop, volume=volume)
        except Exception:
            pass

def safe_play_se(path, vol=0.9):
    if os.path.exists(path):
        try:
            kagra.audio.play_se(path, vol)
        except Exception:
            pass

def load_assets():
    if _assets:
        return

    _assets["font"] = 0
    if os.path.exists(FONT_PATH):
        try:
            _assets["font"] = kagra.load_font(FONT_PATH)
        except Exception:
            _assets["font"] = 0

    for key in ["bg_title", "bg_goddess", "bg_castle", "bg_village", "bg_field", "bg_cave"]:
        path = f"assets/img/{key}.png"
        if os.path.exists(path):
            try:
                _assets[key] = kagra.load_texture(path)
            except Exception:
                pass

    for key in [
        "chr_yuukan", "chr_gouketsu", "chr_kashikoi", "chr_yasashii", "chr_nukemenai",
        "chr_yokimono", "chr_ochitsuki", "chr_subashikkoi", "chr_muttsuri", "chr_sexy"
    ]:
        path = f"assets/img/{key}.png"
        if os.path.exists(path):
            try:
                _assets[key] = kagra.load_texture(path)
            except Exception:
                pass

def fnt():
    return _assets.get("font", 0)

def tex(key):
    return _assets.get(key)

# ── 共通描画ヘルパー ──────────────────────────────────────────
def draw_bg(key):
    tid = tex(key)
    if tid:
        kagra.draw_texture(tid, 0, 0, SW, SH)
    else:
        colors = {
            "bg_title":   (20, 10, 40),
            "bg_goddess": (40, 30, 80),
            "bg_castle":  (30, 20, 50),
            "bg_village": (20, 40, 20),
            "bg_field":   (30, 50, 20),
            "bg_cave":    (10, 10, 15),
        }
        r, g, b = colors.get(key, (20, 20, 30))
        kagra.cls(r, g, b)

def draw_text_center(text, y, size=24, r=255, g=255, b=255):
    if not fnt():
        return
    w, _ = kagra.measure_text(fnt(), text, size)
    kagra.draw_text(fnt(), text, (SW - w) // 2, y, size, r, g, b)

def draw_multiline(text, x, y, size=20, r=220, g=230, b=255, gap=8):
    if not fnt():
        return
    for i, line in enumerate(text.split("\n")):
        kagra.draw_text(fnt(), line, x, y + i * (size + gap), size, r, g, b)

class TypeWriter:
    def __init__(self, text: str, speed: float = 40.0):
        self.text = text
        self.speed = speed
        self.visible = 0
        self.acc = 0.0
        self.finished = False

    def update(self, dt: float):
        if self.finished:
            return
        self.acc += dt * self.speed
        self.visible = min(len(self.text), int(self.acc))
        if self.visible >= len(self.text):
            self.finished = True

    def skip(self):
        self.visible = len(self.text)
        self.finished = True

    def current_text(self):
        return self.text[:self.visible]

# ── シーン ────────────────────────────────────────────────────
class TitleScene(kagra.TransitionScene):
    def _late_init(self):
        load_assets()
        self.menu = kagra.ChoiceMenu(
            font_id=fnt(),
            items=["はじめから", "やめる"],
            x=220, y=390, w=360, item_h=48, text_size=20,
        )

    def _start_bgm(self):
        safe_play_bgm(BGM_TITLE, loop=True, volume=0.6)

    def update(self, dt):
        super().update(dt)
        if not self._ready:
            return

        prev = self.menu.cursor
        self.menu.update()
        if self.menu.cursor != prev:
            safe_play_se(SE_CURSOR, 0.5)

        if self.menu.confirmed:
            safe_play_se(SE_CONFIRM)
            if self.menu.selected_index == 0:
                self.go(QuizScene())
            else:
                sys.exit(0)

    def _draw_content(self):
        draw_bg("bg_title")
        kagra.Panel(150, 140, 500, 90, 10, 5, 30).draw()
        draw_text_center("性格診断の旅", 158, 42, 255, 220, 80)

        kagra.Panel(200, 280, 400, 60, 15, 15, 50).draw()
        draw_text_center("〜 あなたはどんな冒険者？ 〜", 297, 18, 180, 200, 255)

        self.menu.draw()
        draw_text_center("全12問＋冒険パートで性格決定！", 500, 16, 100, 110, 150)

class QuizScene(kagra.TransitionScene):
    def __init__(self, scores=None, q_idx=0):
        super().__init__()
        self.scores = scores or {k: 0 for k in PERSONALITIES}
        self.q_idx = q_idx
        self._answered = False

    def _late_init(self):
        load_assets()
        q = QUESTIONS[self.q_idx]
        self.menu = kagra.ChoiceMenu(
            font_id=fnt(),
            items=[text for text, _ in q["choices"]],
            x=80, y=350, w=640, item_h=44, text_size=20,
        )

    def _commit_choice(self, idx):
        if self._answered:
            return
        self._answered = True
        safe_play_se(SE_CONFIRM)

        _, delta = QUESTIONS[self.q_idx]["choices"][idx]
        for k, v in delta.items():
            self.scores[k] = self.scores.get(k, 0) + v

        next_idx = self.q_idx + 1
        if next_idx >= len(QUESTIONS):
            self.go(GoddessScene(self.scores))
        else:
            self.go(QuizScene(self.scores, next_idx))

    def update(self, dt):
        super().update(dt)
        if not self._ready or self._answered:
            return

        prev = self.menu.cursor
        self.menu.update()
        if self.menu.cursor != prev:
            safe_play_se(SE_CURSOR, 0.5)

        if self.menu.confirmed:
            self._commit_choice(self.menu.selected_index)

    def _draw_content(self):
        draw_bg("bg_goddess")

        total = len(QUESTIONS)
        bar_w = int((SW - 100) * (self.q_idx / total))
        kagra.rect(50, 20, SW - 100, 12, 20, 20, 50)
        kagra.rect(50, 20, bar_w, 12, 100, 150, 255)

        if fnt():
            kagra.draw_text(fnt(), f"Q{self.q_idx + 1} / {total}", SW - 100, 16, 14, 150, 170, 220)

        q = QUESTIONS[self.q_idx]
        kagra.Panel(60, 260, 680, 70, 15, 15, 40).draw()
        draw_multiline(q["text"], 80, 270, 20, 220, 230, 255)

        self.menu.draw()

        if fnt():
            kagra.draw_text(fnt(), "↑↓選択 / Click   Z/Enter決定", 240, 565, 14, 100, 110, 150)

class GoddessScene(kagra.TransitionScene):
    def __init__(self, scores):
        super().__init__()
        self.scores = scores
        self.msg_idx = 0
        self.messages = [
            "よく来た、旅人よ。",
            "汝の魂の輝きを\n私は見守っておった。",
            "だが…性格とは\n行動によって定まるもの。",
            "これより汝に\n試練の旅を与えよう。",
            "城、村、野、洞窟…\nそれぞれの場所で汝の心が試される。",
            "さあ、旅立ちなさい。",
        ]
        self.writer = None

    def _late_init(self):
        load_assets()
        self.writer = TypeWriter(self.messages[0], speed=34)

    def _start_bgm(self):
        safe_play_bgm(BGM_GODDESS, loop=True, volume=0.6)

    def _next_message(self):
        if self.msg_idx < len(self.messages) - 1:
            self.msg_idx += 1
            self.writer = TypeWriter(self.messages[self.msg_idx], speed=34)
        else:
            self.go(AdventureScene(self.scores, 0))

    def update(self, dt):
        super().update(dt)
        if not self._ready:
            return

        self.writer.update(dt)

        clicked = kagra.mouse_pressed(kagra.MOUSE_LEFT)
        key_ok = kagra.key_pressed(kagra.KEY_Z) or kagra.key_pressed(kagra.KEY_RETURN)

        if clicked or key_ok:
            safe_play_se(SE_CONFIRM, 0.6)
            if not self.writer.finished:
                self.writer.skip()
            else:
                self._next_message()

    def _draw_content(self):
        draw_bg("bg_goddess")
        kagra.Panel(60, 420, 680, 140, 5, 5, 25).draw()

        if fnt():
            draw_multiline(self.writer.current_text(), 85, 440, 22, 220, 230, 255, gap=10)
            if self.writer.finished and int(self._t * 3) % 2 == 0:
                kagra.draw_text(fnt(), "▼", 710, 530, 20, 180, 200, 255)
            kagra.draw_text(fnt(), f"（{self.msg_idx + 1}/{len(self.messages)}）", 620, 425, 13, 80, 90, 120)

class AdventureScene(kagra.TransitionScene):
    def __init__(self, scores, scene_idx):
        super().__init__()
        self.scores = scores
        self.scene_idx = scene_idx
        self.phase = 0
        self.answered = False
        self.writer = None

    def _scene_data(self):
        return ADVENTURE_SCENES[self.scene_idx]

    def _late_init(self):
        load_assets()
        sd = self._scene_data()
        self.writer = TypeWriter(sd["narration"], speed=34)
        self.menu = kagra.ChoiceMenu(
            font_id=fnt(),
            items=[text for text, _ in sd["choices"]],
            x=80, y=350, w=640, item_h=44, text_size=20,
        )

    def _start_bgm(self):
        safe_play_bgm(BGM_ADVENTURE, loop=True, volume=0.6)

    def _commit_choice(self, idx):
        if self.answered:
            return
        self.answered = True
        safe_play_se(SE_CONFIRM)

        _, delta = self._scene_data()["choices"][idx]
        for k, v in delta.items():
            self.scores[k] = self.scores.get(k, 0) + v

        next_idx = self.scene_idx + 1
        if next_idx >= len(ADVENTURE_SCENES):
            self.go(ResultScene(self.scores))
        else:
            self.go(AdventureScene(self.scores, next_idx))

    def update(self, dt):
        super().update(dt)
        if not self._ready or self.answered:
            return

        if self.phase == 0:
            self.writer.update(dt)
            clicked = kagra.mouse_pressed(kagra.MOUSE_LEFT)
            key_ok = kagra.key_pressed(kagra.KEY_Z) or kagra.key_pressed(kagra.KEY_RETURN)

            if clicked or key_ok:
                safe_play_se(SE_CONFIRM, 0.6)
                if not self.writer.finished:
                    self.writer.skip()
                else:
                    self.phase = 1
            return

        prev = self.menu.cursor
        self.menu.update()
        if self.menu.cursor != prev:
            safe_play_se(SE_CURSOR, 0.5)

        if self.menu.confirmed:
            self._commit_choice(self.menu.selected_index)

    def _draw_content(self):
        sd = self._scene_data()
        draw_bg(sd["bg"])

        kagra.Panel(20, 20, 220, 36, 10, 10, 30).draw()
        if fnt():
            kagra.draw_text(fnt(), f"Scene {self.scene_idx + 1}: {sd['location']}", 30, 28, 18, 200, 220, 255)

        if self.phase == 0:
            kagra.Panel(60, 420, 680, 150, 5, 5, 25).draw()
            if fnt():
                draw_multiline(self.writer.current_text(), 85, 440, 22, 220, 230, 255, gap=10)
                if self.writer.finished and int(self._t * 3) % 2 == 0:
                    kagra.draw_text(fnt(), "▼ Z/Enter/Clickで続ける", 430, 540, 14, 120, 140, 200)
        else:
            kagra.Panel(60, 265, 680, 70, 15, 15, 40).draw()
            draw_multiline(sd["question"], 80, 275, 20, 220, 230, 255)
            self.menu.draw()

            if fnt():
                kagra.draw_text(fnt(), "↑↓選択 / Click   Z/Enter決定", 240, 565, 14, 100, 110, 150)

class ResultScene(kagra.TransitionScene):
    def __init__(self, scores):
        super().__init__()
        self.scores = scores
        self.result_key = max(scores, key=lambda k: scores[k])
        self.result = PERSONALITIES[self.result_key]
        self.phase = 0
        self.reveal_t = 0.0

    def _late_init(self):
        load_assets()
        self.menu = kagra.ChoiceMenu(
            font_id=fnt(),
            items=["タイトルへ戻る", "もう一度占う"],
            x=260, y=520, w=280, item_h=28, gap=6, text_size=18,
        )

    def _start_bgm(self):
        safe_play_bgm(BGM_RESULT, loop=True, volume=0.6)

    def update(self, dt):
        super().update(dt)
        if not self._ready:
            return

        self.reveal_t += dt
        if self.reveal_t > 1.5:
            self.phase = 1

        if self.phase != 1:
            return

        prev = self.menu.cursor
        self.menu.update()
        if self.menu.cursor != prev:
            safe_play_se(SE_CURSOR, 0.5)

        if self.menu.confirmed:
            safe_play_se(SE_CONFIRM)
            if self.menu.selected_index == 0:
                self.go(TitleScene())
            else:
                self.go(QuizScene())

    def _draw_content(self):
        draw_bg("bg_goddess")

        if self.phase == 0 and self.reveal_t < 0.8:
            kagra.Panel(100, 240, 600, 120, 10, 5, 30).draw()
            draw_text_center("性格が　決定した！", 285, 32, 255, 220, 80)
            return

        tid = tex(self.result["img"])
        if tid:
            kagra.draw_texture(tid, SW // 2 - 96, 60, 192, 192)
        else:
            kagra.rect(SW // 2 - 60, 60, 120, 180, 60, 80, 120)

        kagra.Panel(80, 270, 640, 200, 8, 8, 28).draw()

        if fnt():
            name_w, _ = kagra.measure_text(fnt(), self.result_key, 38)
            kagra.draw_text(fnt(), self.result_key, (SW - name_w) // 2, 282, 38, 255, 220, 80)
            kagra.draw_text(fnt(), f"職業：{self.result['job']}", 110, 334, 22, 180, 220, 255)

            desc = self.result["desc"]
            if len(desc) <= 20:
                kagra.draw_text(fnt(), desc, 110, 368, 18, 200, 210, 230)
            else:
                kagra.draw_text(fnt(), desc[:20], 110, 368, 18, 200, 210, 230)
                kagra.draw_text(fnt(), desc[20:], 110, 392, 18, 200, 210, 230)

            sorted_scores = sorted(self.scores.items(), key=lambda x: x[1], reverse=True)[:3]
            kagra.draw_text(fnt(), "スコア上位:", 110, 425, 14, 120, 130, 160)

            for i, (k, v) in enumerate(sorted_scores):
                bar_w = min(int(v * 8), 200)
                kagra.rect(250 + i * 160, 422, bar_w, 16, 100 + i * 30, 150 - i * 20, 200 - i * 10)
                kagra.draw_text(fnt(), f"{k}:{v}", 250 + i * 160, 440, 13, 150, 160, 190)

        if self.phase == 1:
            self.menu.draw()

# ── 起動 ──────────────────────────────────────────────────────
import traceback

try:
    kagra.run(start_scene=TitleScene())
except Exception:
    traceback.print_exc()
    input("Press Enter...")