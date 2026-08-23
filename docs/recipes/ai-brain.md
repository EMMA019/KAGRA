# Give your LLM a body / LLM に体を与える

English first. 日本語は後半。

KAGRA never bundles an LLM. Anything that turns text into text can be the
brain; KAGRA is the 3D VRM body (voice, lipsync, expressions, dance, OBS
virtual camera).

## Any OpenAI-compatible API

```python
from kagra.ai_character import AiCharacter

char = AiCharacter("me.vrm", tts="voicevox")

def my_llm(text: str) -> str:
    import openai                       # or any client
    r = openai.OpenAI().chat.completions.create(
        model="gpt-4o-mini",
        messages=[{"role": "user", "content": text}],
    )
    return r.choices[0].message.content or ""

char.set_llm_func(my_llm)
char.chat("hi!")        # reply is spoken with lipsync + emotion
```

Ollama works out of the box: `AiCharacter("me.vrm", llm="ollama", llm_model="llama3")`.

## kairi — a grounded brain (recommended)

[kairi](https://github.com/EMMA019/kairi) is a local BYOK chat backend with a
hard grounding layer (citation contracts, numeric defense, offline evals).
Answers that survive its filters are exactly what you want an unattended
avatar to say out loud.

```bash
git clone https://github.com/EMMA019/kairi && cd kairi
docker compose up --build     # KAIRI_DEMO=1: works without any API key
```

```python
from kagra.brain import KairiBrain
char.set_llm_func(KairiBrain("http://127.0.0.1:8000").ask)
```

Full example — typed chat + JSONL viewer comments + VOICEVOX voice:

```bash
python examples/vrm_kairi_chat.py
```

Viewers (or another script) append to `kagra-chat.jsonl`; the avatar answers
each message. That is the skeleton of an unattended AI VTuber: kairi decides
*what* is safe to say, KAGRA decides *how* it looks and sounds.

---

# 日本語

KAGRA は LLM を同梱しない。「テキスト → テキスト」ができれば何でも頭脳になる。
KAGRA は 3D VRM の体（声・リップシンク・表情・ダンス・OBS 仮想カメラ）を受け持つ。

## OpenAI 互換 API

上のコードと同じ。`AiCharacter.set_llm_func` に関数を渡すだけ。
Ollama は `AiCharacter("me.vrm", llm="ollama", llm_model="llama3")` で直結。

## kairi — 接地済みの頭脳（推奨）

[kairi](https://github.com/EMMA019/kairi) は幻覚対策レイヤー
（引用契約・数値防衛・オフライン評価）を持つローカル BYOK チャット。
フィルタを通った返答だけが返るので、無人アバターに喋らせる用途に向く。

```bash
git clone https://github.com/EMMA019/kairi && cd kairi
docker compose up --build     # KAIRI_DEMO=1 なら API キー不要
```

```python
from kagra.brain import KairiBrain
char.set_llm_func(KairiBrain("http://127.0.0.1:8000").ask)
```

実行例（キーボード入力 + JSONL 視聴者コメント + VOICEVOX）:

```bash
python examples/vrm_kairi_chat.py
```

`kagra-chat.jsonl` への追記がそのまま視聴者コメントとして届く。
**何を喋ってよいかは kairi、どう見せるかは KAGRA** — これが無人 AI VTuber の骨格。
