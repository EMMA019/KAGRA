#!/usr/bin/env python3
"""Publish a preview tree onto the ``gh-pages`` branch.

``--dest`` empty replaces the site root and keeps ``pr/``.
``--dest pr/<number>`` replaces only that directory.
``--delete pr/<number>`` removes it.

The token stays in the process environment (``GITHUB_TOKEN``). It is not
written into the preview files and not echoed if git fails.
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def safe_dest(dest: str) -> str:
    """Empty (site root) or ``pr/<digits>``."""
    text = dest.strip().strip("/")
    if not text:
        return ""
    parts = text.split("/")
    if any(part in {"", ".", ".."} for part in parts):
        raise SystemExit(f"bad dest: {dest!r}")
    if len(parts) == 2 and parts[0] == "pr" and parts[1].isdigit():
        return text
    raise SystemExit(f"dest must be empty or pr/<number>, got {dest!r}")


def apply_tree(work: Path, src: Path, dest: str) -> None:
    """Copy ``src`` into ``work`` at ``dest``. Root updates keep ``pr/``."""
    dest = safe_dest(dest)
    if dest:
        target = work.joinpath(*dest.split("/"))
        if target.exists():
            shutil.rmtree(target)
        _copy_into(src, target)
    else:
        for child in list(work.iterdir()):
            if child.name in {".git", "pr"}:
                continue
            if child.is_dir():
                shutil.rmtree(child)
            else:
                child.unlink()
        _copy_into(src, work)
    (work / ".nojekyll").write_bytes(b"")


def delete_tree(work: Path, dest: str) -> None:
    dest = safe_dest(dest)
    if not dest:
        raise SystemExit("refuse to delete the site root")
    target = work.joinpath(*dest.split("/"))
    if target.exists():
        shutil.rmtree(target)
    parent = target.parent
    if parent.name == "pr" and parent.is_dir() and not any(parent.iterdir()):
        parent.rmdir()


def _copy_into(src: Path, dest: Path) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    for child in src.iterdir():
        target = dest / child.name
        if child.is_dir():
            shutil.copytree(child, target, dirs_exist_ok=True)
        else:
            shutil.copy2(child, target)


def stable_root_present(work: Path) -> bool:
    """True when gh-pages root is the stable Lumen shell, not a preview catalog.

    The catalog also has ``play.html`` and wasm. The stable page adds
    ``world.json`` (genre ``neon_run``) next to that wasm.
    """
    return (
        (work / "play.html").is_file()
        and (work / "world.json").is_file()
        and (work / "pkg" / "kagra_shared_bg.wasm").is_file()
    )


def skip_root_seed(work: Path, dest: str, only_if_missing: bool) -> bool:
    """True when a root publish must not replace the stable shell.

    ``pr/<number>`` updates never use this. An empty dest keeps ``pr/`` either way.
    A catalog sitting at ``/`` (no ``world.json``) is not stable, so the seed
    replaces it and leaves ``pr/`` in place.
    """
    dest = safe_dest(dest)
    if only_if_missing and dest:
        raise SystemExit("--only-if-missing only applies to the site root")
    return bool(only_if_missing and stable_root_present(work))


def publish(
    src: Path,
    dest: str,
    *,
    delete: bool,
    repo: str,
    token: str,
    only_if_missing: bool = False,
) -> None:
    dest = safe_dest(dest)
    if only_if_missing and dest:
        raise SystemExit("--only-if-missing only applies to the site root")
    remote = f"https://github.com/{repo}.git"
    for attempt in range(6):
        with tempfile.TemporaryDirectory(prefix="kagra-pages-") as tmp:
            work = Path(tmp) / "site"
            _clone(work, remote, token)
            if skip_root_seed(work, dest, only_if_missing):
                print("gh-pages root already has the stable play.html")
                return
            if delete:
                delete_tree(work, dest)
            else:
                apply_tree(work, src, dest)
            if _commit_and_push(work, remote, token, dest, delete):
                return
        time.sleep(2 * (attempt + 1))
    raise SystemExit("gh-pages push failed after retries")


def authed_remote(remote: str, token: str) -> str:
    """HTTPS remote with the Actions token. Not written into the preview tree."""
    prefix = "https://"
    if not remote.startswith(prefix):
        raise SystemExit(f"remote must be https, got {remote!r}")
    if not token or any(ch in token for ch in "\r\n\x00 @"):
        raise SystemExit("refusing an empty or unsafe token")
    return f"https://x-access-token:{token}@{remote[len(prefix):]}"


def redact(text: str, token: str) -> str:
    if token and text:
        return text.replace(token, "***")
    return text or ""


def _git_env() -> dict[str, str]:
    env = os.environ.copy()
    env["GIT_TERMINAL_PROMPT"] = "0"
    return env


def _git(cwd: Path, *args: str, token: str = "") -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        ["git", *args], cwd=cwd, text=True, capture_output=True, env=_git_env()
    )
    if result.returncode != 0:
        err = redact((result.stderr or "") + (result.stdout or ""), token)
        raise SystemExit(f"git {' '.join(args)} failed:\n{err}")
    return result


def _clone(work: Path, remote: str, token: str) -> None:
    parent = work.parent
    parent.mkdir(parents=True, exist_ok=True)
    # A bearer extraheader is ignored or rejected here (git then prompts).
    # The token stays on the clone/push URL only, and origin is rewritten
    # to the public URL so it is not stored in the gh-pages checkout.
    probe = subprocess.run(
        [
            "git",
            "clone",
            "--branch",
            "gh-pages",
            "--single-branch",
            "--depth",
            "1",
            authed_remote(remote, token),
            str(work),
        ],
        text=True,
        capture_output=True,
        env=_git_env(),
    )
    if probe.returncode == 0:
        subprocess.run(
            ["git", "remote", "set-url", "origin", remote], cwd=work, check=True
        )
        return
    err = redact((probe.stderr or "") + (probe.stdout or ""), token)
    if work.exists():
        shutil.rmtree(work)
    missing = "Remote branch" in err or "not found" in err.lower()
    if not missing:
        raise SystemExit(f"gh-pages clone failed:\n{err}")
    work.mkdir(parents=True, exist_ok=True)
    subprocess.run(["git", "init", "-b", "gh-pages"], cwd=work, check=True, env=_git_env())
    subprocess.run(["git", "remote", "add", "origin", remote], cwd=work, check=True)


def _commit_and_push(work: Path, remote: str, token: str, dest: str, delete: bool) -> bool:
    _git(
        work,
        "config",
        "user.email",
        "41898282+github-actions[bot]@users.noreply.github.com",
    )
    _git(work, "config", "user.name", "github-actions[bot]")
    _git(work, "add", "-A")
    status = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=work,
        text=True,
        capture_output=True,
        check=True,
        env=_git_env(),
    )
    if not status.stdout.strip():
        print("gh-pages already up to date")
        return True
    label = f"delete {dest}" if delete else (dest or "root")
    _git(work, "commit", "-m", f"preview {label}")
    push = subprocess.run(
        ["git", "push", authed_remote(remote, token), "HEAD:gh-pages"],
        cwd=work,
        text=True,
        capture_output=True,
        env=_git_env(),
    )
    if push.returncode == 0:
        print(f"pushed gh-pages ({label})")
        return True
    err = redact((push.stderr or "") + (push.stdout or ""), token)
    print(f"push rejected, retrying:\n{err}", file=sys.stderr)
    return False


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Publish the preview to gh-pages")
    parser.add_argument("--src", default="dist/preview")
    parser.add_argument("--dest", default="", help="empty for /, or pr/<number>")
    parser.add_argument("--delete", action="store_true")
    parser.add_argument(
        "--only-if-missing",
        action="store_true",
        help="root publish: do nothing when play.html, world.json, and wasm are already on gh-pages",
    )
    parser.add_argument("--repo", default=os.environ.get("GITHUB_REPOSITORY", ""))
    args = parser.parse_args(argv)
    token = os.environ.get("GITHUB_TOKEN", "").strip()
    if not token:
        print("GITHUB_TOKEN is required", file=sys.stderr)
        return 2
    if not args.repo:
        print("GITHUB_REPOSITORY or --repo is required", file=sys.stderr)
        return 2
    src = Path(args.src)
    if not args.delete and not src.is_dir():
        print(f"missing preview dir: {src}", file=sys.stderr)
        return 2
    publish(
        src,
        args.dest,
        delete=args.delete,
        repo=args.repo,
        token=token,
        only_if_missing=args.only_if_missing,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
