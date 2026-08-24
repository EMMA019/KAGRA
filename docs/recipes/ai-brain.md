# 頭脳をつなぐ / Give the avatar a brain

English below. 日本語は後半。

KAGRA does not ship a model. The official hook is `kagra.brain(...)`:
text in, text out. Plug it into lipsync, or into shelf `AiCharacter.set_llm_func`.

The recommended brain is **[kairi](https://github.com/EMMA019/kairi)**
(BYOK chat with a grounding layer). The live instance is
**https://kairi.onrender.com** — that is the default. KAGRA talks HTTP only.
Do not vendor FastAPI / SQLite / keys into the wheel.

## kairi (Render is the default)

Hosted `/api/ping` is public. `/api/chat` returns **401** without a token.

```bash
export KAIRI_API_TOKEN=…          # required for kairi.onrender.com
python examples/vrm_kairi_chat.py
```

```python
import kagra

kagra.init()
av = kagra.avatar(str(kagra.ensure_vrm()))
av.enable_lipsync()
mind = kagra.brain("kairi")          # https://kairi.onrender.com
reply = mind.ask("こんにちは。自己紹介して。")
av.lipsync_text(reply, duration=min(8.0, 0.06 * max(8, len(reply))))
```

Render Free can sleep. The first `ask` may take up to ~a minute.

Local override (no token if kairi is in dev mode):

```bash
export KAIRI_URL=http://127.0.0.1:8000
```

`KAIRI_DEMO=1` on a local server is a fixture, not a real LLM.

Shelf path (same `ask`):

```python
from kagra.ai_character import AiCharacter

char = AiCharacter(str(kagra.ensure_vrm()), tts="voicevox")
char.set_llm_func(kagra.KairiBrain().ask)
char.chat("今日の気分は？")
```

Env: `KAIRI_URL` (default `https://kairi.onrender.com`), `KAIRI_API_TOKEN`, `KAIRI_SESSION`.

Smoke: `KAGRA_SMOKE=1 python examples/vrm_kairi_chat.py` skips HTTP.

## Ollama / OpenAI-compatible

```python
mind = kagra.brain("ollama")          # http://127.0.0.1:11434/v1
# mind = kagra.brain("openai")        # OPENAI_API_KEY
reply = mind.ask("hello")
```

No torch in the KAGRA core. Keys stay in the environment.

---

# 日本語

モデルは wheel に入れない。公式面は `kagra.brain`（`ask(text) -> str`）。

推奨は **[kairi](https://github.com/EMMA019/kairi)**。本命は Render の
**https://kairi.onrender.com**（既定）。KAGRA は `POST /api/chat` の SSE を読むだけ。
チャットには `KAIRI_API_TOKEN` が要る。`/api/ping` はトークン無しで生きている。

手元で動かすときだけ `KAIRI_URL=http://127.0.0.1:8000`。
Free プランは寝ていることがある。最初の `ask` は待つ。
