"""play.html keeps Corridor/Crest dev links behind ?debug=1."""
from pathlib import Path

PLAY = Path(__file__).resolve().parents[1] / "kagra-shared" / "www" / "play.html"


def test_dev_links_hidden_without_debug_param():
    src = PLAY.read_text(encoding="utf-8")
    assert '<div id="links" hidden>' in src
    assert 'const DEBUG_LINKS = params.get("debug") === "1";' in src
    assert "const showLinks = DEBUG_LINKS && st.phase === \"title\";" in src
