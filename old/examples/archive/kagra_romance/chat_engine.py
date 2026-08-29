"""
kagra_romance/chat_engine.py
API呼び出し・メッセージ管理
"""
import threading, json, logging
from openai import OpenAI

logger = logging.getLogger(__name__)


class Message:
    __slots__ = ('role','text','color')
    def __init__(self, role: str, text: str, color=(200,200,200)):
        self.role  = role
        self.text  = text
        self.color = color


class ChatHistory:
    MAX = 40
    VISIBLE = 20  # 一画面に表示する最大数

    def __init__(self):
        self._msgs: list[Message] = []

    def add(self, role: str, text: str, color=(200,200,200)):
        self._msgs.append(Message(role, text, color))
        if len(self._msgs) > self.MAX:
            # 古いものを削除（system/event メッセージは残す）
            for i, m in enumerate(self._msgs):
                if m.role in ('you', 'emma'):
                    self._msgs.pop(i)
                    break

    def visible(self) -> list[Message]:
        """表示用: 最新 VISIBLE 件"""
        return self._msgs[-self.VISIBLE:]

    def history_for_api(self) -> list[dict]:
        """API 送信用: you/emma のみ、最新10件"""
        result = []
        for m in self._msgs[-20:]:
            if m.role == 'you':
                result.append({'role':'user', 'content': m.text})
            elif m.role == 'emma' and m.text not in ('…','…考え中…'):
                result.append({'role':'assistant', 'content': m.text})
        return result[-10:]

    def replace_last_emma(self, text: str, color=(200,200,200)):
        """「…」プレースホルダを最新の返答で置換"""
        for m in reversed(self._msgs):
            if m.role == 'emma' and m.text == '…':
                m.text  = text
                m.color = color
                return
        # なければ追加
        self.add('emma', text, color)


class ChatEngine:
    """DeepSeek API との非同期通信を管理"""

    def __init__(self, api_key: str):
        self._api_key  = api_key
        self._client   = None
        self._pending  = False
        self._response = None  # スレッドからの結果
        self._error_log: list[str] = []  # デバッグ用エラーログ

        if api_key:
            try:
                self._client = OpenAI(
                    api_key=api_key,
                    base_url="https://api.deepseek.com"
                )
            except Exception as e:
                self._error_log.append(f"Client init error: {e}")

    @property
    def pending(self) -> bool:
        return self._pending

    def send(self, system: str, history: list[dict], user_msg: str):
        if self._pending or not self._client:
            return
        self._pending  = True
        self._response = None
        threading.Thread(
            target=self._call,
            args=(system, history, user_msg),
            daemon=True
        ).start()

    def _call(self, system: str, history: list[dict], user_msg: str):
        try:
            msgs = [{'role':'system','content':system}] + history + \
                   [{'role':'user','content':user_msg}]
            res  = self._client.chat.completions.create(
                model='deepseek-chat',
                messages=msgs,
                max_tokens=400,
                temperature=0.8,
                response_format={"type": "json_object"},  # JSON モード強制
            )
            raw  = res.choices[0].message.content.strip()
            # フェールセーフ: {} を抽出
            if '{' in raw:
                raw = raw[raw.index('{'):raw.rindex('}')+1]
            data = json.loads(raw)
            # 必須フィールドの型チェック
            assert isinstance(data.get('reply',''), str)
            self._error_log.clear()  # 成功したらエラーログをクリア

        except json.JSONDecodeError as e:
            msg = f"JSONDecodeError: {e}\nRaw: {raw[:200]}"
            self._error_log.append(msg)
            logger.warning(msg)
            data = {'reply':'ちょっと混乱しちゃった…もう一度？',
                    'emotion':'sorrow','score_delta':{},'affection_delta':0,'choices':[]}

        except Exception as e:
            msg = f"{type(e).__name__}: {e}"
            self._error_log.append(msg)
            logger.error(msg)
            data = {'reply':f'ごめん…({type(e).__name__})',
                    'emotion':'sorrow','score_delta':{},'affection_delta':0,'choices':[]}

        self._response = data

    def poll(self) -> dict | None:
        """メインスレッドから呼ぶ。レスポンスがあれば返す"""
        if self._response is not None:
            result         = self._response
            self._response = None
            self._pending  = False
            return result
        return None

    def get_error_log(self) -> list[str]:
        return list(self._error_log)
