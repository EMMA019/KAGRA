"""
kagra_romance/ui.py
描画ロジック
"""
import math, random
import kagra
from .persona    import PERSONA_COLORS, EMOTION_EXPR, ENDINGS
from .components import PersonalityComp, EmotionComp, ChatComp, EffectComp, TimeComp

SW, SH = 1280, 720
PX, PY_CHAT = 640, 50
PW = SW - PX - 10
PH = SH - 200

BG_THEMES = {
    'morning': [(30,40,80),  (60,80,120)],
    'day':     [(20,30,70),  (80,120,180)],
    'evening': [(60,30,50),  (120,60,80)],
    'night':   [(5,5,20),    (20,15,40)],
}


def draw_background(emma_entity, time_f: float):
    tm  = emma_entity.get(TimeComp)
    tod = tm.time_of_day() if tm else 'day'
    c1, c2 = BG_THEMES.get(tod, BG_THEMES['day'])

    steps = 8
    for i in range(steps):
        t = i / steps
        r = int(c1[0]*(1-t) + c2[0]*t)
        g = int(c1[1]*(1-t) + c2[1]*t)
        b = int(c1[2]*(1-t) + c2[2]*t)
        kagra.rect(0, i*(SH//steps), SW, SH//steps+1, r, g, b, 255)

    if tod == 'night':
        random.seed(42)
        for _ in range(80):
            x = random.randint(0, SW)
            y = random.randint(0, SH//2)
            a = int(128 + 127*math.sin(time_f*2 + x*0.1))
            kagra.rect(x, y, 2, 2, 255,255,255, a)

    kagra.rect(0, SH-80, SW, 80, c2[0]//2, c2[1]//2, c2[2]//2, 180)


def draw_chat(font, emma_entity, history, cursor_f: float, debug=False):
    chat = emma_entity.get(ChatComp)
    p    = emma_entity.get(PersonalityComp)
    tm   = emma_entity.get(TimeComp)
    if not chat: return

    pc  = p.get_color() if p else (180,220,255)
    tod = tm.time_of_day() if tm else 'day'

    # ── 背景パネル ────────────────────────────────────────────
    kagra.rect(PX, PY_CHAT, PW, PH, 8, 6, 20, 200)

    # ── メッセージ一覧 ────────────────────────────────────────
    LINE_H = 22
    y      = PY_CHAT + 10
    for msg in history.visible():
        if y > PY_CHAT + PH - LINE_H: break
        prefix = {'you':'You: ','emma':'Emma: '}.get(msg.role, '')
        full   = prefix + msg.text
        chunk  = 26
        while full and y < PY_CHAT + PH - LINE_H:
            kagra.draw_text(font, full[:chunk], PX+8, y, 16, *msg.color)
            full = full[chunk:]
            y   += LINE_H
        y += 3

    # ── 選択肢 ────────────────────────────────────────────────
    if chat.choices:
        mx, my = kagra.mouse_pos()
        for i, ch in enumerate(chat.choices):
            cy    = SH - 185 - (len(chat.choices)-i-1)*44
            hover = PX < mx < PX+PW and cy < my < cy+36
            bg    = (*pc, 180) if hover else (30,25,60,200)
            kagra.rect(PX, cy, PW, 36, *bg[:3], bg[3])
            tc = (20,15,40) if hover else (220,220,240)
            kagra.draw_text(font, f'{i+1}. {ch}', PX+12, cy+8, 17, *tc)

    # ── 入力欄 ────────────────────────────────────────────────
    IY = SH - 90
    is_pending = hasattr(history, '_pending_flag') and history._pending_flag
    kagra.rect(PX, IY, PW, 40, 20,16,45, 230)
    kagra.set_ime_cursor_pos(PX+8, IY+8)

    if is_pending:
        kagra.draw_text(font, '…考え中…', PX+8, IY+10, 18, 160,160,200)
    else:
        preedit  = kagra.get_preedit_text()
        base     = chat.input_text[-22:]
        kagra.draw_text(font, base, PX+8, IY+10, 18, 220,220,255)
        off_x = len(base) * 10
        if preedit:
            # IME 変換中: 下線 + 黄色
            w = len(preedit) * 10 + 4
            kagra.rect(PX+8+off_x, IY+26, w, 2, 255,220,80, 255)
            kagra.draw_text(font, preedit, PX+8+off_x, IY+10, 18, 255,220,80)
        elif int(cursor_f*2)%2 == 0:
            kagra.draw_text(font, '|', PX+8+off_x, IY+10, 18, 180,180,255)

    hint = 'Enter で送信' if not chat.choices else 'クリック / Enter で選択'
    kagra.draw_text(font, hint, PX+8, IY+44, 12, 100,100,140)

    # ── 性格パネル ────────────────────────────────────────────
    if p:
        _draw_personality(font, p, tod)

    # ── デバッグパネル ────────────────────────────────────────
    if debug:
        _draw_debug(font, emma_entity)


def _draw_personality(font, p: PersonalityComp, tod: str):
    PNX, PNY, PNW = 10, SH-190, 255
    kagra.rect(PNX, PNY, PNW, 135, 10,8,25, 210)

    pc = p.get_color()
    kagra.draw_text(font, f'💫 {p.personality}', PNX+8, PNY+6, 17, *pc)

    bar = PNW - 20
    bw  = int(p.affection / 100 * bar)
    kagra.rect(PNX+8, PNY+30, bar, 10, 30,30,55, 220)
    if bw > 0:
        kagra.rect(PNX+8, PNY+30, bw, 10, *pc, 255)
    kagra.draw_text(font, f'好感度 {p.affection}/100', PNX+8, PNY+44, 13, 160,160,200)

    SC = {'tsundere':(255,140,160),'yandere':(180,60,180),
          'kuudere':(100,200,240),'dandere':(140,220,100)}
    for i,(name,val) in enumerate(p.scores.items()):
        bw2 = min(int(val*10), bar-20)
        c   = SC[name]
        kagra.rect(PNX+8, PNY+62+i*16, bar-20, 10, 25,25,50, 200)
        if bw2 > 0:
            kagra.rect(PNX+8, PNY+62+i*16, bw2, 10, *c, 200)
        kagra.draw_text(font, name[:3].upper(), PNX+bar-8, PNY+63+i*16, 11, *c)

    tod_icon = {'morning':'🌅','day':'☀','evening':'🌇','night':'🌙'}
    kagra.draw_text(font, tod_icon.get(tod,''), PNX+PNW-24, PNY+8, 18, 255,220,100)


def _draw_debug(font, emma_entity):
    """デバッグパネル: エラーログ表示"""
    # ChatInputScript からエンジンを取得
    from .scripts import ChatInputScript
    ci = emma_entity.get(ChatInputScript)
    if not ci: return
    errors = ci.engine.get_error_log()
    if not errors: return

    kagra.rect(0, 0, 500, 20 + len(errors)*18, 40,0,0, 220)
    kagra.draw_text(font, '⚠ DEBUG', 4, 2, 14, 255,100,100)
    for i, e in enumerate(errors[-5:]):
        kagra.draw_text(font, e[:55], 4, 20+i*18, 13, 255,180,180)


def draw_flash(ef: EffectComp):
    if ef and ef.flash_timer > 0:
        a = int(min(1.0, ef.flash_timer) * 110)
        if ef.flash_color:
            kagra.rect(0, 0, SW, SH, *ef.flash_color, a)


def draw_ending(font, emma_entity, time_f: float):
    p = emma_entity.get(PersonalityComp)
    if not p or not p.route: return
    title, msg = ENDINGS.get(p.route, ('END', '…ありがとう。'))
    pc = p.get_color()
    a  = min(200, int(time_f * 30))
    kagra.rect(0, SH//2-90, SW, 180, 5,3,15, a)
    kagra.draw_text(font, title, SW//2-140, SH//2-70, 34, *pc)
    kagra.draw_text(font, msg,   SW//2-220, SH//2-10, 22, 240,240,255)
    kagra.draw_text(font, 'ESC で終了', SW//2-70, SH//2+60, 14, 120,120,160)
