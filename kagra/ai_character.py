# kagra/ai_character.py
"""
KAGRA AI キャラクター SDK (Phase 8)

LLM・TTS・感情分析を VrmAvatar と統合するハイレベル API。
Python の AI エコシステムをそのまま使えるのが KAGRA の最大の強み。

【最小構成】
    char = AiCharacter("assets/Emma.vrm")
    response = char.chat("こんにちは！")
    # → LLM 応答 + 自動感情 + 自動口パク

【VOICEVOX 連携】
    char = AiCharacter("assets/Emma.vrm", tts="voicevox", voice_id=3)
    char.speak("よろしくお願いします！")
    # → 合成音声 + リップシンク同期

【OpenAI 連携】
    char = AiCharacter("assets/Emma.vrm",
                        llm="openai", llm_model="gpt-4o",
                        tts="voicevox", voice_id=3)
    char.chat("今日の天気は？")

【カスタム LLM / TTS】
    char = AiCharacter("assets/Emma.vrm")
    char.set_llm_func(my_llm_function)    # def f(text: str) -> str
    char.set_tts_func(my_tts_function)    # def f(text: str) -> str (wav_path)

Example::
    import kagra
    from kagra.ai_character import AiCharacter

    class TalkScene(kagra.Scene):
        def on_enter(self):
            self.char = AiCharacter(
                "assets/Emma.vrm",
                system_prompt="あなたは明るいアシスタントです。",
                tts="voicevox",
                voice_id=3,
            )
            self.font = kagra.font("meiryo")
            self.message = ""

        def update(self, dt):
            self.char.update(dt)

            if kagra.pressed("RETURN"):
                # ユーザー入力を取得してキャラに話させる
                response = self.char.chat("こんにちは！")
                self.message = response

        def draw(self):
            kagra.cls(20, 20, 40)
            kagra.draw_vrm(self.char.avatar.vrm_id)
            self.char.draw_state(self.font)
            if self.message:
                kagra.text(40, 600, self.message, font=self.font, size=20)
"""
from __future__ import annotations

import os
import math
import threading
import time
from typing import Optional, Callable, TYPE_CHECKING

if TYPE_CHECKING:
    from kagra.vrm_avatar import VrmAvatar


# ── 状態定数 ──────────────────────────────────────────────────

class CharState:
    IDLE      = "idle"       # 待機中
    THINKING  = "thinking"   # LLM 応答待ち
    SPEAKING  = "speaking"   # 発話中
    LISTENING = "listening"  # 入力待ち（将来）


# ══════════════════════════════════════════════════════════════
#  AiCharacter
# ══════════════════════════════════════════════════════════════

class AiCharacter:
    """VRM キャラクターに AI（LLM / TTS）を統合する高レベルクラス。

    Args:
        vrm_path:      VRM ファイルパス
        system_prompt: キャラクターの人格・設定（LLM に渡すシステムプロンプト）
        llm:           LLM プロバイダー ("openai" / "ollama" / None)
        llm_model:     LLM モデル名 (例: "gpt-4o", "llama3")
        tts:           TTS プロバイダー ("voicevox" / "coeiroink" / "gtts" / None)
        voice_id:      TTS 話者 ID（VOICEVOX: 3=ずんだもん等）
        tts_url:       TTS サーバー URL（ローカル VOICEVOX: "http://localhost:50021"）
        eye_height:    視線追従の目の高さ

    Example::
        char = AiCharacter(
            "assets/Emma.vrm",
            system_prompt="あなたは親切なアシスタントです。",
            tts="voicevox",
            voice_id=3,
        )
    """

    def __init__(
        self,
        vrm_path:      str,
        system_prompt: str = "あなたは親切で元気なアシスタントキャラクターです。",
        llm:           Optional[str] = None,
        llm_model:     str = "gpt-4o-mini",
        tts:           Optional[str] = None,
        voice_id:      int = 3,
        tts_url:       str = "http://localhost:50021",
        eye_height:    float = 1.5,
    ):
        # VRM キャラクター
        import kagra
        self.avatar: VrmAvatar = kagra.avatar(vrm_path)

        # Phase 7 機能を全部有効化
        self._lookat  = self.avatar.enable_lookat(eye_height=eye_height)
        self._lipsync = self.avatar.enable_lipsync()
        self._emotion = self.avatar.enable_emotion()
        self._ik      = self.avatar.enable_ik()

        # 設定
        self.system_prompt = system_prompt
        self._llm_type     = llm
        self._llm_model    = llm_model
        self._tts_type     = tts
        self._voice_id     = voice_id
        self._tts_url      = tts_url

        # 状態
        self.state:    str  = CharState.IDLE
        self._history: list = []  # LLM 会話履歴

        # 非同期処理
        self._pending_response: Optional[str] = None
        self._pending_wav:      Optional[str] = None
        self._pending_query:    Optional[dict] = None
        self._last_audio_query: Optional[dict] = None
        self._thread:           Optional[threading.Thread] = None
        self._lock = threading.Lock()

        # カスタム関数（ユーザーが差し替え可能）
        self._llm_func: Optional[Callable[[str], str]]  = None
        self._tts_func: Optional[Callable[[str], str]]  = None

        # テキスト表示用バッファ
        self.last_user_text:    str = ""
        self.last_char_text:    str = ""
        self.last_emotion:      str = "neutral"
        self.last_speak_time:   float = 0.0

        # アイドルアニメ
        self._idle_t:    float = 0.0
        self._idle_clip: str   = "idle"

        self.avatar.play("idle")
        print(f"[AiCharacter] 初期化完了: {vrm_path}")
        if tts:
            print(f"[AiCharacter] TTS: {tts} (voice_id={voice_id}, url={tts_url})")
        if llm:
            print(f"[AiCharacter] LLM: {llm} ({llm_model})")

    # ── カスタム関数の設定 ────────────────────────────────────

    def set_llm_func(self, func: Callable[[str], str]):
        """カスタム LLM 関数を設定する。

        Args:
            func: text (str) → response (str) を返す関数

        Example::
            import openai
            client = openai.OpenAI()

            def my_llm(text: str) -> str:
                res = client.chat.completions.create(
                    model="gpt-4o",
                    messages=[{"role": "user", "content": text}]
                )
                return res.choices[0].message.content

            char.set_llm_func(my_llm)
        """
        self._llm_func = func

    def set_tts_func(self, func: Callable[[str], str]):
        """カスタム TTS 関数を設定する。

        Args:
            func: text (str) → wav_path (str) を返す関数

        Example::
            def my_tts(text: str) -> str:
                # 任意の TTS エンジンで音声生成
                wav_path = f"/tmp/speech_{hash(text)}.wav"
                my_tts_engine.synthesize(text, wav_path)
                return wav_path

            char.set_tts_func(my_tts)
        """
        self._tts_func = func

    # ── メイン API ────────────────────────────────────────────

    def chat(self, user_text: str, async_: bool = True) -> Optional[str]:
        """LLM にテキストを送り、キャラクターに返答させる。

        Args:
            user_text: ユーザーの発言
            async_:    True = 非同期実行（update() で結果を処理）
                       False = 同期実行（ブロッキング、デバッグ向け）

        Returns:
            async_=False の場合: LLM 応答テキスト
            async_=True  の場合: None（update() で処理される）

        Example::
            # 非同期（推奨 - ゲームループをブロックしない）
            char.chat("こんにちは！")

            # 同期（デバッグ用）
            response = char.chat("今日の天気は？", async_=False)
            print(response)
        """
        self.last_user_text = user_text
        self.state          = CharState.THINKING
        self.avatar.play("think", loop=True)
        self._emotion.set("neutral")

        if async_:
            self._thread = threading.Thread(
                target=self._async_chat_worker,
                args=(user_text,),
                daemon=True,
            )
            self._thread.start()
            return None
        else:
            response = self._run_llm(user_text)
            self._start_speaking(response)
            return response

    def speak(self, text: str, async_: bool = True) -> Optional[str]:
        """LLM を介さず、直接テキストをキャラクターに喋らせる。

        Args:
            text:   喋らせるテキスト
            async_: True = 非同期 TTS

        Example::
            char.speak("こんにちは！よろしくお願いします。")
        """
        if async_:
            self._thread = threading.Thread(
                target=self._async_speak_worker,
                args=(text,),
                daemon=True,
            )
            self._thread.start()
            return None
        else:
            self._start_speaking(text)
            return text

    def react(self, text: str):
        """テキストから感情を推定してキャラクターに反応させる（発話なし）。

        Example::
            char.react("やった！！！")  # → joy 表情 + wave モーション
        """
        emotion = self._emotion.from_text(text, intensity=0.9)
        self.last_emotion = emotion
        # 感情に合わせたリアクションモーション
        _EMOTION_MOTION = {
            "joy":       "wave",
            "excited":   "arm_up",
            "surprised": "shake_head",  # 代替
            "sorrow":    "bow",
            "loving":    "wave",
        }
        motion = _EMOTION_MOTION.get(emotion, "idle")
        self.avatar.play(motion, loop=(motion == "idle"),
                         on_finish=lambda: self.avatar.play("idle"))

    # ── 毎フレーム更新 ────────────────────────────────────────

    def update(self, dt: float):
        """毎フレーム呼ぶ。非同期処理の結果を適用する。"""
        self.last_speak_time += dt

        # 非同期チャット結果の処理
        with self._lock:
            if self._pending_response is not None:
                self._start_speaking(self._pending_response)
                self._pending_response = None
            if self._pending_wav is not None:
                self._last_audio_query = self._pending_query
                self._play_wav_with_lipsync(self._pending_wav)
                self._pending_wav = None
                self._pending_query = None

        # アバター更新（マウスを見る）
        import kagra
        mx, my = kagra.mouse()
        sw, sh = kagra.screen_w(), kagra.screen_h()
        self.avatar.look_at_screen(mx, my, sw, sh)
        self.avatar.update(dt)

        # アイドル中の息遣いアニメ（ゆっくりとした揺れ）
        if self.state == CharState.IDLE:
            self._update_idle_breath(dt)

    def _update_idle_breath(self, dt: float):
        """アイドル時の微細な体の揺れ（息遣い表現）。"""
        self._idle_t += dt
        # 4秒周期でゆっくり揺れる
        breath = math.sin(self._idle_t * math.pi / 2) * 0.015
        import kagra
        kagra.get_engine().set_vrm_bone_rot(
            self.avatar.vrm_id, "J_Bip_C_Spine",
            breath, 0, 0, math.sqrt(max(0, 1 - breath*breath))
        )

    # ── 描画ヘルパー ──────────────────────────────────────────

    def draw_state(self, font_id: int = 0, x: float = 40, y: float = 40):
        import kagra

        # 防御的変換（文字列対策）
        if isinstance(x, str):
            x = float(x)
        if isinstance(y, str):
            y = float(y)

        state_color = {
            CharState.IDLE:      (150, 150, 150),
            CharState.THINKING:  (100, 180, 255),
            CharState.SPEAKING:  (100, 255, 150),
            CharState.LISTENING: (255, 220, 80),
        }.get(self.state, (200, 200, 200))

        kagra.fill(x, y, 12, 12, color=state_color)

        # ✅ 修正: テキストを第一引数に
        kagra.text(self.state, x + 18, y - 2, font=font_id, size=14, color=state_color)

        # 感情
        if self.last_emotion and self.last_emotion != "neutral":
            # ✅ 修正: テキストを第一引数に
            kagra.text(f"感情: {self.last_emotion}", x, y + 18, font=font_id,size=13, color=(255, 220, 100))

        # 発話テキスト（最新）
        if self.last_char_text:
            max_len = 40
            disp = self.last_char_text[:max_len] + ("..." if len(self.last_char_text) > max_len else "")
            # ✅ 修正: テキストを第一引数に
            kagra.text(disp, x, y + 36, font=font_id, size=15, color=(220, 220, 220))

    def draw_bubble(self, font_id: int = 0,
                    x: float = 200, y: float = 80, w: float = 500):
        """発話テキストをふきだし風に表示する。

        Example::
            char.draw_bubble(font, x=200, y=80, w=500)
        """
        if not self.last_char_text:
            return
        import kagra

        # 背景
        lines = _wrap_text(self.last_char_text, max_chars=22)
        h = len(lines) * 28 + 24
        kagra.fill(x - 10, y - 10, w + 20, h + 20,
                  color=(240, 240, 255, 210))
        kagra.fill(x - 8, y - 8, w + 16, h + 16,
                  color=(100, 130, 200, 100))

        # テキスト
        for i, line in enumerate(lines):
            kagra.text(line, x, y + i * 28, font=font_id,
              size=20, color=(30, 30, 60))

    # ── LLM バックエンド ──────────────────────────────────────

    def _run_llm(self, user_text: str) -> str:
        """LLM を実行してテキストを返す（同期）。"""
        # カスタム関数が設定されていれば優先
        if self._llm_func:
            return self._llm_func(user_text)

        if self._llm_type == "openai":
            return self._llm_openai(user_text)
        elif self._llm_type == "ollama":
            return self._llm_ollama(user_text)
        else:
            # LLM なし: エコーで返す
            return f"（LLM 未設定）「{user_text}」と言いましたね？"

    def _llm_openai(self, user_text: str) -> str:
        """OpenAI API を呼び出す。"""
        try:
            import openai
            client = openai.OpenAI()
            messages = [{"role": "system", "content": self.system_prompt}]
            messages.extend(self._history[-10:])   # 直近 10 ターンを履歴として渡す
            messages.append({"role": "user", "content": user_text})

            res = client.chat.completions.create(
                model=self._llm_model,
                messages=messages,
                max_tokens=256,
            )
            response = res.choices[0].message.content or ""
            # 履歴に追加
            self._history.append({"role": "user",      "content": user_text})
            self._history.append({"role": "assistant",  "content": response})
            return response
        except ImportError:
            return "openai ライブラリが必要です: pip install openai"
        except Exception as e:
            return f"OpenAI エラー: {e}"

    def _llm_ollama(self, user_text: str) -> str:
        """Ollama (ローカル LLM) を呼び出す。"""
        try:
            import urllib.request, json
            url     = "http://localhost:11434/api/generate"
            payload = json.dumps({
                "model":  self._llm_model,
                "prompt": f"{self.system_prompt}\n\nUser: {user_text}\nAssistant:",
                "stream": False,
            }).encode()
            req = urllib.request.Request(url, data=payload,
                                         headers={"Content-Type": "application/json"})
            with urllib.request.urlopen(req, timeout=30) as r:
                data = json.loads(r.read())
            return data.get("response", "").strip()
        except Exception as e:
            return f"Ollama エラー: {e}"

    # ── TTS バックエンド ──────────────────────────────────────

    def _run_tts(self, text: str) -> Optional[str]:
        """TTS を実行して WAV ファイルパスを返す（同期）。"""
        if self._tts_func:
            return self._tts_func(text)

        if self._tts_type == "voicevox":
            return self._tts_voicevox(text)
        elif self._tts_type == "coeiroink":
            return self._tts_coeiroink(text)
        elif self._tts_type == "gtts":
            return self._tts_gtts(text)
        return None

    def _tts_voicevox(self, text: str) -> Optional[str]:
        """VOICEVOX HTTP API を呼び出す。"""
        try:
            import urllib.request, urllib.parse, json, tempfile
            base = self._tts_url

            # 1. audio_query（mora 長をリップシンクに渡す）
            query_url = f"{base}/audio_query?text={urllib.parse.quote(text)}&speaker={self._voice_id}"
            with urllib.request.urlopen(query_url, timeout=10) as r:
                query = r.read()
            try:
                self._last_audio_query = json.loads(query)
            except Exception:
                self._last_audio_query = None

            # 2. synthesis
            synth_url = f"{base}/synthesis?speaker={self._voice_id}"
            req = urllib.request.Request(
                synth_url, data=query,
                headers={"Content-Type": "application/json"},
            )
            with urllib.request.urlopen(req, timeout=30) as r:
                wav_bytes = r.read()

            # 3. 一時ファイルに保存
            tmp = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
            tmp.write(wav_bytes)
            tmp.close()
            return tmp.name

        except Exception as e:
            print(f"[AiCharacter] VOICEVOX エラー: {e}")
            print("  → VOICEVOX が起動しているか確認してください: http://localhost:50021")
            return None

    def _tts_coeiroink(self, text: str) -> Optional[str]:
        """COEIROINK HTTP API を呼び出す（VOICEVOX 互換）。"""
        old_url = self._tts_url
        self._tts_url = "http://localhost:50031"
        result = self._tts_voicevox(text)
        self._tts_url = old_url
        return result

    def _tts_gtts(self, text: str) -> Optional[str]:
        """Google Text-to-Speech（gTTS）を使う。要: pip install gtts pydub"""
        try:
            from gtts import gTTS
            import tempfile
            tts = gTTS(text=text, lang="ja")
            tmp = tempfile.NamedTemporaryFile(suffix=".mp3", delete=False)
            tts.save(tmp.name)
            tmp.close()
            return tmp.name
        except ImportError:
            print("[AiCharacter] gTTS が必要です: pip install gtts")
            return None
        except Exception as e:
            print(f"[AiCharacter] gTTS エラー: {e}")
            return None

    # ── 非同期ワーカー ────────────────────────────────────────

    def _async_chat_worker(self, user_text: str):
        """別スレッドで LLM → TTS を実行する。"""
        response = self._run_llm(user_text)
        wav_path = self._run_tts(response) if response else None

        with self._lock:
            self._pending_response = response
            if wav_path:
                self._pending_wav = wav_path
                self._pending_query = self._last_audio_query

    def _async_speak_worker(self, text: str):
        """別スレッドで TTS を実行する。"""
        wav_path = self._run_tts(text)
        with self._lock:
            self._pending_response = text
            if wav_path:
                self._pending_wav = wav_path
                self._pending_query = self._last_audio_query

    def _start_speaking(self, text: str):
        """発話を開始する（メインスレッドから呼ぶ）。"""
        self.last_char_text  = text
        self.last_speak_time = 0.0
        self.state           = CharState.SPEAKING

        # 感情推定
        em = self._emotion.from_text(text, intensity=0.85)
        self.last_emotion = em

        # モーション選択（感情ベース）
        _SPEAK_MOTION = {
            "joy":       "wave",
            "excited":   "arm_up",
            "loving":    "wave",
            "sorrow":    "bow",
            "surprised": "idle",
        }
        motion = _SPEAK_MOTION.get(em, "idle")
        self.avatar.play(motion, loop=True)

        # テキストリップシンク（WAV がない場合の fallback）
        duration = max(1.0, len(text) * 0.08)
        self._lipsync.play_text(text, duration=duration)

        # 一定時間後に IDLE に戻る
        def _finish():
            self.state = CharState.IDLE
            self.avatar.play("idle")
            self._emotion.set("neutral")

        t = threading.Timer(duration + 0.5, _finish)
        t.daemon = True
        t.start()

    def _play_wav_with_lipsync(self, wav_path: str):
        """WAV 再生 + リップシンク同期（メインスレッドから呼ぶ）。"""
        import kagra
        try:
            from kagra.vrm_lipsync import timeline_from_audio_query

            query = self._last_audio_query
            if query:
                timeline = timeline_from_audio_query(query, max_open=self._lipsync.max_open)
            else:
                timeline = self._lipsync.analyze_wav(wav_path)
            if len(timeline) > 0:
                self._lipsync.play_timeline(timeline)

            # 音声再生
            kagra.play_se(wav_path, volume=1.0)

            # 一時ファイルを遅延削除（再生が終わってから）
            duration = timeline.duration if len(timeline) > 0 else 2.0

            def _cleanup():
                time.sleep(duration + 1.0)
                try:
                    os.remove(wav_path)
                except OSError:
                    pass

            t = threading.Thread(target=_cleanup, daemon=True)
            t.start()

        except Exception as e:
            print(f"[AiCharacter] WAV 再生エラー: {e}")

    # ── 会話履歴管理 ─────────────────────────────────────────

    def clear_history(self):
        """会話履歴をクリアする。"""
        self._history.clear()

    def set_personality(self, system_prompt: str):
        """キャラクターの人格設定を変更する。

        Example::
            char.set_personality("あなたはツンデレな女の子です。")
        """
        self.system_prompt = system_prompt


# ── テキスト折り返しユーティリティ ───────────────────────────

def _wrap_text(text: str, max_chars: int = 22) -> list[str]:
    """テキストを指定文字数で折り返す。"""
    lines = []
    current = ""
    for ch in text:
        current += ch
        if len(current) >= max_chars or ch in "。！？\n":
            lines.append(current.rstrip())
            current = ""
    if current.strip():
        lines.append(current.strip())
    return lines or [""]
