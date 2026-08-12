# kagra/http_client.py
"""
Phase 9f: ノンブロッキング HTTP クライアント

ゲームループをブロックせずに HTTP リクエストを送る。
AI API 呼び出し・スコア送信・マルチプレイヤーシグナリングに使う。

DragonRuby の GTK.http_get / GTK.http_post に相当する機能。

【使い方】
    from kagra.http_client import http_get, http_post, HttpClient

    # 一発リクエスト（コールバック版）
    http_get("https://api.example.com/score",
             on_done=lambda r: print(r.json()))

    # 毎フレームチェック版（ポーリング）
    req = http_get("https://api.example.com/data")

    def update(dt):
        if req.done:
            if req.ok:
                data = req.json()
            else:
                print(f"エラー: {req.status}")

    # OpenAI / VOICEVOX 等との連携
    from kagra.http_client import openai_chat, voicevox_speak

    # 非同期で LLM に問い合わせ
    req = openai_chat("こんにちは！",
                      api_key=os.environ["OPENAI_API_KEY"])

    def update(dt):
        if req.done and req.ok:
            response = req.json()["choices"][0]["message"]["content"]
            avatar.lipsync_text(response, duration=2.0)
"""
from __future__ import annotations

import json
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Callable, Optional


class HttpResponse:
    """HTTP レスポンス。"""

    def __init__(self):
        self.done:    bool           = False
        self.ok:      bool           = False
        self.status:  int            = 0
        self.headers: dict           = {}
        self._body:   Optional[bytes] = None
        self.error:   Optional[str]   = None
        self._start_time: float      = time.time()

    @property
    def elapsed(self) -> float:
        """リクエスト開始からの経過秒数。"""
        return time.time() - self._start_time

    @property
    def body(self) -> Optional[bytes]:
        return self._body

    @property
    def text(self) -> Optional[str]:
        if self._body is None:
            return None
        return self._body.decode("utf-8", errors="replace")

    def json(self) -> Any:
        """レスポンスボディを JSON デコードして返す。"""
        if self._body is None:
            return None
        return json.loads(self._body)

    def __repr__(self):
        if self.done:
            return f"<HttpResponse {self.status} {'OK' if self.ok else 'ERROR'}>"
        return "<HttpResponse pending...>"


class HttpClient:
    """ノンブロッキング HTTP クライアント。

    毎フレーム tick() を呼ぶことで、完了したリクエストのコールバックを処理する。

    Example::
        client = HttpClient()

        # GET リクエスト（非同期）
        def on_score(resp):
            if resp.ok:
                print(f"スコア: {resp.json()['score']}")

        client.get("https://api.example.com/score", on_done=on_score)

        def update(dt):
            client.tick()   # コールバックを処理
    """

    def __init__(self, timeout: float = 10.0):
        self.timeout    = timeout
        self._pending:  list[tuple[HttpResponse, Callable]] = []
        self._lock      = threading.Lock()

    def get(
        self,
        url:      str,
        headers:  dict = None,
        on_done:  Callable = None,
        timeout:  float = None,
    ) -> HttpResponse:
        """GET リクエストを送る（ノンブロッキング）。

        Args:
            url:     リクエスト先 URL
            headers: 追加ヘッダー
            on_done: 完了時コールバック (HttpResponse) → None
            timeout: タイムアウト秒数

        Returns:
            HttpResponse（done=False の状態）
        """
        resp = HttpResponse()
        t = threading.Thread(
            target=self._do_request,
            args=("GET", url, None, headers or {}, timeout or self.timeout, resp, on_done),
            daemon=True,
        )
        t.start()
        return resp

    def post(
        self,
        url:         str,
        body:        Any  = None,
        json_body:   Any  = None,
        headers:     dict = None,
        on_done:     Callable = None,
        timeout:     float = None,
    ) -> HttpResponse:
        """POST リクエストを送る（ノンブロッキング）。

        Args:
            url:       リクエスト先 URL
            body:      bytes のボディ
            json_body: dict → 自動で JSON エンコード + Content-Type 設定
            headers:   追加ヘッダー
            on_done:   完了時コールバック
            timeout:   タイムアウト秒数

        Returns:
            HttpResponse（done=False の状態）
        """
        h = dict(headers or {})
        if json_body is not None:
            body = json.dumps(json_body).encode("utf-8")
            h.setdefault("Content-Type", "application/json")

        resp = HttpResponse()
        t = threading.Thread(
            target=self._do_request,
            args=("POST", url, body, h, timeout or self.timeout, resp, on_done),
            daemon=True,
        )
        t.start()
        return resp

    def tick(self):
        """毎フレーム呼ぶ。完了したリクエストのコールバックを処理する。

        コールバックはメインスレッドで呼ばれるため、
        kagra の描画関数を安全に呼び出せる。
        """
        with self._lock:
            pending = list(self._pending)
            self._pending.clear()

        for resp, cb in pending:
            try:
                cb(resp)
            except Exception as e:
                print(f"[HttpClient] コールバックエラー: {e}")

    def _do_request(
        self,
        method:  str,
        url:     str,
        body:    Optional[bytes],
        headers: dict,
        timeout: float,
        resp:    HttpResponse,
        on_done: Optional[Callable],
    ):
        """別スレッドでリクエストを実行する。"""
        try:
            req = urllib.request.Request(url, data=body, method=method)
            for k, v in headers.items():
                req.add_header(k, v)

            with urllib.request.urlopen(req, timeout=timeout) as r:
                resp._body   = r.read()
                resp.status  = r.status
                resp.headers = dict(r.headers)
                resp.ok      = 200 <= r.status < 300

        except urllib.error.HTTPError as e:
            resp.status = e.code
            resp.error  = str(e)
            resp.ok     = False
            try:
                resp._body = e.read()
            except Exception:
                pass
        except Exception as e:
            resp.error = str(e)
            resp.ok    = False
            resp.status = 0
        finally:
            resp.done = True

        if on_done:
            with self._lock:
                self._pending.append((resp, on_done))


# ── モジュールレベルのシングルトン ────────────────────────────

_default_client: Optional[HttpClient] = None

def _get_client() -> HttpClient:
    global _default_client
    if _default_client is None:
        _default_client = HttpClient()
    return _default_client


def http_tick():
    """毎フレーム呼ぶ。モジュールレベルのコールバックを処理する。

    Example::
        def update(dt):
            kagra.http_tick()   # または from kagra.http_client import http_tick
    """
    _get_client().tick()


def http_get(
    url:     str,
    headers: dict = None,
    on_done: Callable = None,
    timeout: float = 10.0,
) -> HttpResponse:
    """GET リクエスト（シンプル版）。

    Example::
        req = http_get("https://api.example.com/ranking")

        def update(dt):
            http_tick()
            if req.done and req.ok:
                data = req.json()
    """
    return _get_client().get(url, headers=headers, on_done=on_done, timeout=timeout)


def http_post(
    url:       str,
    json_body: Any  = None,
    body:      bytes = None,
    headers:   dict = None,
    on_done:   Callable = None,
    timeout:   float = 10.0,
) -> HttpResponse:
    """POST リクエスト（シンプル版）。

    Example::
        http_post("https://api.example.com/score",
                  json_body={"score": 1234, "name": "Player"},
                  on_done=lambda r: print("送信完了" if r.ok else "失敗"))
    """
    return _get_client().post(url, body=body, json_body=json_body,
                              headers=headers, on_done=on_done, timeout=timeout)


# ── AI API ヘルパー ───────────────────────────────────────────

def openai_chat(
    message:    str,
    api_key:    str  = None,
    model:      str  = "gpt-4o-mini",
    system:     str  = "You are a helpful assistant.",
    on_done:    Callable = None,
    timeout:    float = 30.0,
) -> HttpResponse:
    """OpenAI Chat API を非同期で呼ぶ。

    Args:
        message:  ユーザーメッセージ
        api_key:  OpenAI API キー（省略時は環境変数 OPENAI_API_KEY を使う）
        model:    モデル名
        system:   システムプロンプト
        on_done:  完了コールバック（HttpResponse）

    Returns:
        HttpResponse。done になったら .json()["choices"][0]["message"]["content"] で取得。

    Example::
        import os
        req = openai_chat(
            "こんにちは！",
            api_key=os.environ["OPENAI_API_KEY"],
            on_done=lambda r: avatar.lipsync_text(
                r.json()["choices"][0]["message"]["content"], duration=2.0
            )
        )
    """
    import os
    key = api_key or os.environ.get("OPENAI_API_KEY", "")
    return _get_client().post(
        "https://api.openai.com/v1/chat/completions",
        json_body={
            "model": model,
            "messages": [
                {"role": "system",  "content": system},
                {"role": "user",    "content": message},
            ],
            "max_tokens": 256,
        },
        headers={"Authorization": f"Bearer {key}"},
        on_done=on_done,
        timeout=timeout,
    )


def voicevox_speak(
    text:     str,
    speaker:  int = 3,
    url:      str = "http://localhost:50021",
    on_done:  Callable = None,
) -> HttpResponse:
    """VOICEVOX に音声合成を依頼する（2段階: query → synthesis）。

    on_done コールバックの HttpResponse.body が WAV バイナリになる。

    Example::
        import tempfile, kagra

        def on_voice(resp):
            if resp.ok:
                tmp = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
                tmp.write(resp.body); tmp.close()
                kagra.se(tmp.name)
                avatar.lipsync_wav(tmp.name)

        voicevox_speak("こんにちは！", speaker=3, on_done=on_voice)
    """
    client = _get_client()

    def _on_query(qresp: HttpResponse):
        if not qresp.ok:
            if on_done:
                with client._lock:
                    client._pending.append((qresp, on_done))
            return
        # Step 2: synthesis
        client.post(
            f"{url}/synthesis?speaker={speaker}",
            body=qresp.body,
            headers={"Content-Type": "application/json"},
            on_done=on_done,
            timeout=30.0,
        )

    encoded = urllib.parse.quote(text)
    return client.get(
        f"{url}/audio_query?text={encoded}&speaker={speaker}",
        on_done=_on_query,
        timeout=10.0,
    )
