"""
kagra_romance/persona.py
性格定義・進化ロジック
"""

# ── 定数 ─────────────────────────────────────────────────────
PERSONA_COLORS = {
    'Natural':    (180,220,255), 'Tsundere':   (255,160,180),
    'Yandere':    (180, 80,180), 'Kuudere':    (140,200,220),
    'Dandere':    (200,255,180), 'EvoTsundere':(255, 60,100),
    'EvoYandere': (140,  0,200), 'EvoKuudere': ( 60,160,255),
    'EvoDandere': (100,255, 80),
}

EMOTION_EXPR = {
    'joy':       'Fcl_ALL_Joy',
    'fun':       'Fcl_ALL_Fun',
    'sorrow':    'Fcl_ALL_Sorrow',
    'angry':     'Fcl_ALL_Angry',
    'surprised': 'Fcl_ALL_Surprised',
    'neutral':   'Fcl_ALL_Neutral',
}

SYSTEM_PROMPT = """\
あなたは Emma という AI キャラクター。
現在の性格: {personality}（{personality_desc}）
時間帯: {time_of_day}
好感度: {affection}/100
ユーザー名: {player_name}

性格ごとの口調:
- Natural: 明るく自然、親しみやすい
- Tsundere: 照れ隠し「べ、別に…」「うるさいな」
- Yandere: 甘く独占的「あなただけ…」「ずっと一緒にいよう」
- Kuudere: クール短文「そう」「…なるほど」
- Dandere: はにかみ「あ、あの…」「え、えっと…」
- Evo系: より強烈・極端なバージョン

返答は必ず以下の JSON のみ（説明文・コードブロック不要）:
{{"reply":"Emma の返答（80文字以内）","emotion":"joy|fun|sorrow|angry|surprised|neutral","score_delta":{{"tsundere":0,"yandere":0,"kuudere":0,"dandere":0}},"affection_delta":1,"choices":["選択肢A","選択肢B","選択肢C"]}}

choices は次のターンへの自然な3択。なければ空配列 []。
"""

PERSONALITY_DESC = {
    'Natural':    'フレンドリー',
    'Tsundere':   'ツンデレ',
    'Yandere':    'ヤンデレ',
    'Kuudere':    'クーデレ',
    'Dandere':    'ダンデレ',
    'EvoTsundere':'超ツンデレ',
    'EvoYandere': '超ヤンデレ',
    'EvoKuudere': '超クーデレ',
    'EvoDandere': '超ダンデレ',
}

ENDINGS = {
    'Tsundere':    ('ツンデレルート END', '「…うるさい。でも、嫌いじゃないから。」'),
    'EvoTsundere': ('TRUE ツンデレ END',  '「絶対に離さないから覚悟して！」'),
    'Yandere':     ('ヤンデレルート END',  '「ずっと一緒にいよう。永遠に…」'),
    'EvoYandere':  ('TRUE ヤンデレ END',  '「世界中が敵になっても、あなただけいればいい」'),
    'Kuudere':     ('クーデレルート END',  '「……一緒にいる。それだけ。」'),
    'EvoKuudere':  ('TRUE クーデレ END',  '「あなたと話す時間が、唯一の贅沢」'),
    'Dandere':     ('ダンデレルート END',  '「す、好きです…これからも…よろしく」'),
    'EvoDandere':  ('TRUE ダンデレ END',  '「ずっと、あなたの隣にいさせてください」'),
}
