# 頭脳をつなぐ / Give the avatar a brain

English below. 日本語は後半。

KAGRA does not ship a model. The official hook is `kagra.brain(...)`:
text in, text out. Plug it into lipsync, or into shelf `AiCharacter.set_llm_func`.

The recommended brain for this repo is **[kairi](https://github.com/EMMA019/kairi)**
(local BYOK chat with a grounding layer). KAGRA talks to it over HTTP.
Do not vendor FastAPI / SQLite / keys into the wheel.

## kairi (recommended)

1. Start kairi (`docker compose up` or `uvicorn` on `http://127.0.0.1:8000`)
2. Optional: `KAIRI_API_TOKEN` if you set one. `KAIRI_DEMO=1` is a fixture, not a real LLM
3. Python:

```python
import kagra

kagra.init()
av = kagra.avatar(str(kagra.ensure_vrm()))
av.enable_lipsync()
mind = kagra.brain("kairi")          # or kagra.KairiBrain()
reply = mind.ask("こんにちは。自己紹介して。")
av.lipsync_text(reply, duration=min(8.0, 0.06 * max(8, len(reply))))
```

Shelf path (same `ask`):

```python
from kagra.ai_character import AiCharacter

char = AiCharacter(str(kagra.ensure_vrm()), tts="voicevox")
char.set_llm_func(kagra.KairiBrain().ask)
char.chat("今日の気分は？")
```

Env: `KAIRI_URL` (default `http://127.0.0.1:8000`), `KAIRI_API_TOKEN`, `KAIRI_SESSION`.

Demo: `python examples/vrm_kairi_chat.py` (needs kairi). `KAGRA_SMOKE=1` skips HTTP.

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

推奨は自作の **[kairi](https://github.com/EMMA019/kairi)**。グラウンディング付きのローカル BYOK サーバー。KAGRA は `POST /api/chat` の SSE を読むだけ。

```python
mind = kagra.brain("kairi")
reply = mind.ask("こんにちは。自己紹介して。")
```

Ollama は `kagra.brain("ollama")`。OpenAI 互換は `kagra.brain("openai")`。
`AiCharacter` は棚。新しい面に `set_llm_func(mind.ask)` で繋ぐ。
