#!/usr/bin/env python3
"""同じ ``--game`` でデバッグ署名の APK を作る。

    python scripts/package_android.py --game examples/platformer_minimal.py

やること:

1. ``games/<stem>.json`` を読む
2. ``mobile/android/kagra.game.properties``（表示名、version、applicationId、icon）
3. ``mobile/android/app/src/main/assets/kagra_root/`` にステージ
4. ``drawable-mdpi/ic_launcher.png``
5. 任意で ``scripts/build_android_native.sh``（``--skip-native`` で省略）
6. ``gradle :app:assembleDebug``。APK はデバッグキーで署名される

``--release`` は keystore の環境変数が揃っているときだけ ``assembleRelease`` する。
秘密は作らない。手順は ``docs/distribution.md``。
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from kagra.distribute import (  # noqa: E402
    ManifestError,
    manifest_for_game,
    stage_game,
    write_android_properties,
)

ANDROID = ROOT / "mobile" / "android"
ASSETS = ANDROID / "app" / "src" / "main" / "assets" / "kagra_root"
ICON = ANDROID / "app" / "src" / "main" / "res" / "drawable-mdpi" / "ic_launcher.png"
PROPS = ANDROID / "kagra.game.properties"
RELEASE_ENV = (
    "ANDROID_KEYSTORE_BASE64",
    "ANDROID_KEYSTORE_PASSWORD",
    "ANDROID_KEY_ALIAS",
    "ANDROID_KEY_PASSWORD",
)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Package a KAGRA example as an Android APK")
    parser.add_argument("--game", required=True, help="examples/foo_minimal.py or games/foo.json")
    parser.add_argument("--out", default="dist/android", help="where to copy the apk")
    parser.add_argument("--stage-only", action="store_true", help="write assets and gradle props, do not build")
    parser.add_argument("--skip-native", action="store_true", help="do not build libkagra_shared.so")
    parser.add_argument(
        "--release",
        action="store_true",
        help="assembleRelease using keystore env vars (see docs/distribution.md)",
    )
    args = parser.parse_args(argv)
    try:
        manifest = manifest_for_game(ROOT, args.game)
    except ManifestError as exc:
        print(f"manifest: {exc}", file=sys.stderr)
        return 2

    if ASSETS.exists():
        shutil.rmtree(ASSETS)
    rels = stage_game(ROOT, manifest, ASSETS)
    ICON.parent.mkdir(parents=True, exist_ok=True)
    ICON.write_bytes((ROOT / manifest.icon).read_bytes())
    write_android_properties(manifest, PROPS, icon_ref="@drawable/ic_launcher")
    print(f"staged {len(rels)} files -> {ASSETS}")
    print(f"properties {PROPS}")

    if args.stage_only:
        return 0

    if not args.skip_native:
        script = ROOT / "scripts" / "build_android_native.sh"
        print(f"native {script}")
        subprocess.run(["bash", str(script)], check=True, cwd=ROOT)
        libs = list((ANDROID / "app" / "src" / "main" / "jniLibs").rglob("libkagra_shared.so"))
        if not libs:
            print(
                "libkagra_shared.so was not produced. Install cargo-ndk and NDK r28+ "
                "(ANDROID_NDK_HOME), then re-run. See docs/distribution.md.",
                file=sys.stderr,
            )
            return 1
        for lib in libs:
            print(f"so {lib}")

    task = ":app:assembleRelease" if args.release else ":app:assembleDebug"
    env = os.environ.copy()
    if args.release:
        missing = [name for name in RELEASE_ENV if not env.get(name, "").strip()]
        if missing:
            print(
                "release signing needs these environment variables (do not commit them): "
                + ", ".join(missing),
                file=sys.stderr,
            )
            return 2
        keystore = _materialize_keystore(env, ROOT / "dist" / "android")
        env["ANDROID_KEYSTORE_FILE"] = str(keystore)

    gradle = _gradle()
    print(f"gradle {task}")
    subprocess.run([gradle, task, "--no-daemon", "--stacktrace"], check=True, cwd=ANDROID, env=env)

    kind = "release" if args.release else "debug"
    apks = list((ANDROID / "app" / "build" / "outputs" / "apk" / kind).glob("*.apk"))
    if not apks:
        print(f"no apk under outputs/apk/{kind}", file=sys.stderr)
        return 1
    out = Path(args.out)
    if not out.is_absolute():
        out = ROOT / out
    out.mkdir(parents=True, exist_ok=True)
    dest = out / f"{manifest.slug}-{kind}.apk"
    shutil.copy2(apks[0], dest)
    print(f"apk {dest}")
    return 0


def _materialize_keystore(env: dict[str, str], dest_dir: Path) -> Path:
    import base64

    dest_dir.mkdir(parents=True, exist_ok=True)
    path = dest_dir / "release.keystore"
    path.write_bytes(base64.b64decode(env["ANDROID_KEYSTORE_BASE64"]))
    return path


def _gradle() -> str:
    if os.name == "nt":
        local = ANDROID / "gradlew.bat"
    else:
        local = ANDROID / "gradlew"
    if local.is_file():
        return str(local)
    return "gradle"


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as exc:
        print(f"command failed: {exc}", file=sys.stderr)
        raise SystemExit(exc.returncode or 1) from exc
