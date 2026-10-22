"""セーブ先の規則。拡張モジュールは読まない。"""
from tests.conftest import load_kagra_submodule

flow = load_kagra_submodule("flow")


def test_windows_appdata(monkeypatch):
    monkeypatch.delenv("KAGRA_SAVE_DIR", raising=False)
    monkeypatch.setenv("APPDATA", r"C:\Users\a\AppData\Roaming")
    monkeypatch.setattr(flow.sys, "platform", "win32")
    text = str(flow.save_dir()).replace("\\", "/")
    assert text.endswith("AppData/Roaming/kagra")


def test_linux_xdg_and_override(monkeypatch):
    monkeypatch.setenv("KAGRA_SAVE_DIR", "/tmp/kagra-save")
    assert flow.save_dir().as_posix() == "/tmp/kagra-save"
    monkeypatch.delenv("KAGRA_SAVE_DIR", raising=False)
    monkeypatch.delenv("XDG_DATA_HOME", raising=False)
    monkeypatch.setenv("HOME", "/home/player")
    monkeypatch.setattr(flow.sys, "platform", "linux")
    assert flow.save_dir().as_posix() == "/home/player/.local/share/kagra"


def test_android_waits_for_the_shell(monkeypatch):
    monkeypatch.delenv("KAGRA_SAVE_DIR", raising=False)
    monkeypatch.setattr(flow.sys, "getandroidapilevel", lambda: 30, raising=False)
    monkeypatch.setattr(flow.sys, "platform", "linux")
    assert flow.save_dir().as_posix() == "kagra-save"
