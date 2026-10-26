"""Which browser-page features a game uses: the ``page`` block of ``games/<name>.json``.

``scripts/preview_site/play.html`` is one page for every game. It used to switch features on
by genre (``hopMode``, ``survivorMode``, ``slug === "neon_cat"``), so each new game meant new
branches in the page. The manifest now says what the game wants and the page only reads
``bundle.page``. A game with no ``page`` block gets none of these features (the old default).

    "page": {
      "best": true,            # seed the run's best from the device, save it on game over
      "audio": true,           # world.game.audio drives the page's music and sound
      "lang": true,            # pass the page language (ja / en) to the in-canvas texts
      "pause": true,           # Escape / P, a hidden tab and gamepad Start pause the run
      "fixed_step": true,      # the page steps the sim at a fixed 60 Hz
      "one_button": true,      # Space, Up, W, J, Z press and release the one action
      "music_sync": true,      # the audio-offset stepper and the sim following the music clock
      "revive": true,          # Continue after game over through a rewarded ad
      "direct": true,          # ?play=1 or a portal build starts the run with no title card
      "tagline": true,         # show the title card's tagline line
      "theme": "hop",          # html[data-theme] for the page's per-game CSS
      "blurb": "hop.blurb",    # i18n key of the title-card blurb (swipe games)
      "default_quality": "mobile",                      # world.quality when the link sets none
      "quality_words": {"safe": "low", "mid": "mobile", "max": "high"},  # ?quality= words
      "url_params": {"fx": ["low", "mid", "high"], "skin": "string", "fx_post": "bool"}
    }

``url_params`` copies a link's ``?name=`` into the world before load: a list allows those
values, ``"string"`` any non-empty value, ``"bool"`` ``0`` / ``1`` (as false / true). Like
``platform``, this lives in the manifest layer: it is about the page, not the simulation.
"""
from __future__ import annotations

from typing import Any

FLAGS = (
    "best",
    "audio",
    "lang",
    "pause",
    "fixed_step",
    "one_button",
    "music_sync",
    "revive",
    "direct",
    "tagline",
)
PICTURE = ("low", "mobile", "high")
QUALITY_WORDS = ("safe", "mid", "max")
PARAM_KINDS = ("string", "bool")
KEYS = set(FLAGS) | {"theme", "blurb", "default_quality", "quality_words", "url_params"}


class PageError(ValueError):
    """A ``page`` block that cannot be used as written."""


def _name(value: Any, what: str) -> str:
    if not isinstance(value, str) or not value or not all(c.isalnum() or c in "._-" for c in value):
        raise PageError(f"page.{what} must be a short name (letters, digits, . _ -), got {value!r}")
    return value


def normalize_page(raw: Any) -> dict[str, Any]:
    """Validate ``page``. ``None`` or ``{}`` gives ``{}`` (no page features)."""
    if raw is None:
        return {}
    if not isinstance(raw, dict):
        raise PageError("page must be an object")
    unknown = sorted(set(raw) - KEYS)
    if unknown:
        raise PageError(f"page has unknown keys: {', '.join(unknown)}")
    out: dict[str, Any] = {}
    for flag in FLAGS:
        if flag in raw:
            if not isinstance(raw[flag], bool):
                raise PageError(f"page.{flag} must be true or false")
            if raw[flag]:
                out[flag] = True
    for key in ("theme", "blurb"):
        if key in raw:
            out[key] = _name(raw[key], key)
    if "default_quality" in raw:
        if raw["default_quality"] not in PICTURE:
            raise PageError(f"page.default_quality must be one of {list(PICTURE)}")
        out["default_quality"] = raw["default_quality"]
    if "quality_words" in raw:
        words = raw["quality_words"]
        if not isinstance(words, dict) or not words:
            raise PageError("page.quality_words must be a non-empty object")
        for word, picture in words.items():
            if word not in QUALITY_WORDS:
                raise PageError(f"page.quality_words: {word!r} is not one of {list(QUALITY_WORDS)}")
            if picture not in PICTURE:
                raise PageError(f"page.quality_words.{word} must be one of {list(PICTURE)}")
        out["quality_words"] = dict(words)
    if "url_params" in raw:
        params = raw["url_params"]
        if not isinstance(params, dict):
            raise PageError("page.url_params must be an object")
        clean: dict[str, Any] = {}
        for name, kind in params.items():
            _name(name, "url_params key")
            if isinstance(kind, list):
                if not kind or not all(isinstance(v, str) and v for v in kind):
                    raise PageError(f"page.url_params.{name} must list non-empty strings")
                clean[name] = list(kind)
            elif kind in PARAM_KINDS:
                clean[name] = kind
            else:
                raise PageError(f"page.url_params.{name} must be a list of values, 'string' or 'bool'")
        out["url_params"] = clean
    if out.get("music_sync") and not out.get("fixed_step"):
        raise PageError("page.music_sync needs page.fixed_step (the sim follows the music clock per step)")
    return out
