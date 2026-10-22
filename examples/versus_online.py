"""Two processes, one local relay, the same fight.

Terminal 1:

    python server/netplay_relay.py

Terminal 2 and 3 (same room, the server assigns slots):

    python examples/versus_online.py --port 8765 --name a --ticks 90 --out scratch/a.json
    python examples/versus_online.py --port 8765 --name b --ticks 90 --out scratch/b.json

``--match`` starts the relay and both clients, then checks the dumps match.
That is the headless check. Input sync lives in Rust (`Lockstep`); this
file only shuttles JSON and ticks `FightSession`.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

DUMP = ROOT / "kagra-shared/tests/fixtures/fight_hitstun_world.json"


def _relay():
    path = ROOT / "server" / "netplay_relay.py"
    spec = importlib.util.spec_from_file_location("kagra_netplay_relay", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _session():
    try:
        from kagra.kagra_shared import FightSession
    except ImportError:
        from kagra_shared import FightSession  # type: ignore[no-redef]
    return FightSession.from_json(DUMP.read_text(encoding="utf-8"))


def _shared_ok() -> bool:
    try:
        import kagra.kagra_shared  # noqa: F401
        return True
    except Exception:
        try:
            import kagra_shared  # noqa: F401
            return True
        except Exception:
            return False


def run_client(port: int, name: str, ticks: int, out: str) -> int:
    from kagra.netplay import RoomClient, scripted_stick

    client = RoomClient("127.0.0.1", port, "ring", name)
    try:
        client.connect(timeout=8.0)
        client.wait_start(timeout=8.0)
        session = _session()
        session.confirm()
        dt = float(client.dt or 0.0)
        if dt <= 0.0:
            raise RuntimeError("missing dt")
        for tick in range(ticks):
            lx, lz, jump, attack, dodge = scripted_stick(int(client.slot), tick)
            client.push_local(tick, lx, lz, jump, attack, dodge)
            waits = 0
            while not client.ready(tick):
                waits += 1
                if waits > 80:
                    raise TimeoutError(f"tick {tick} slot {client.slot}")
                client.pump(0.05)
            rows = client.inputs(tick)
            if not rows:
                raise RuntimeError(f"missing inputs at {tick}")
            for slot, row in enumerate(rows):
                lx, lz, jump, attack, dodge = row
                session.set_player_input(slot, lx, lz, jump, attack, dodge)
            session.tick(dt)
        world = json.loads(session.dump())
        pos = (world.get("player") or {}).get("position") or [0, 0, 0]
        payload = {
            "slot": int(client.slot),
            "ticks": ticks,
            "fingerprint": int(session.fingerprint),
            "hp": int(session.hp),
            "opp_hp": int(session.opp_hp),
            "px": float(pos[0]),
            "dump": session.dump(),
        }
        path = Path(out)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(payload), encoding="utf-8")
        print(
            f"client slot={payload['slot']} fingerprint={payload['fingerprint']} "
            f"hp={payload['hp']} opp={payload['opp_hp']} px={payload['px']:.3f}"
        )
        return 0
    finally:
        client.close()


def run_match(ticks: int = 90) -> int:
    if not _shared_ok():
        print("kagra_shared is not installed", file=sys.stderr)
        return 2
    relay = _relay()
    listener, port = relay.serve_background("127.0.0.1", 0)
    outs = [ROOT / "scratch" / "versus_online_p0.json", ROOT / "scratch" / "versus_online_p1.json"]
    procs = []
    try:
        for index, out in enumerate(outs):
            out.parent.mkdir(parents=True, exist_ok=True)
            if out.exists():
                out.unlink()
            procs.append(
                subprocess.Popen(
                    [
                        sys.executable,
                        str(Path(__file__).resolve()),
                        "--client",
                        "--port",
                        str(port),
                        "--name",
                        f"p{index}",
                        "--ticks",
                        str(ticks),
                        "--out",
                        str(out),
                    ],
                    cwd=str(ROOT),
                )
            )
        codes = [proc.wait(timeout=40) for proc in procs]
        if any(code != 0 for code in codes):
            print(f"client exit {codes}", file=sys.stderr)
            return 1
        rows = [json.loads(path.read_text(encoding="utf-8")) for path in outs]
    finally:
        for proc in procs:
            if proc.poll() is None:
                proc.kill()
        listener.close()
    slots = sorted(row["slot"] for row in rows)
    if slots != [0, 1]:
        print(f"expected slots 0 and 1, got {slots}", file=sys.stderr)
        return 1
    if rows[0]["fingerprint"] != rows[1]["fingerprint"] or rows[0]["dump"] != rows[1]["dump"]:
        print(
            "desync "
            f"{rows[0]['fingerprint']} vs {rows[1]['fingerprint']} "
            f"hp {rows[0]['hp']}/{rows[0]['opp_hp']} vs {rows[1]['hp']}/{rows[1]['opp_hp']}",
            file=sys.stderr,
        )
        return 1
    if abs(rows[0]["px"] - (-2.2)) < 0.05:
        print("players did not move", file=sys.stderr)
        return 1
    print(
        f"MATCH ok ticks={ticks} fingerprint={rows[0]['fingerprint']} "
        f"hp={rows[0]['hp']} opp={rows[0]['opp_hp']} px={rows[0]['px']:.3f}"
    )
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="KAGRA online versus demo")
    parser.add_argument("--match", action="store_true", help="relay plus two local clients")
    parser.add_argument("--client", action="store_true")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--name", default="p")
    parser.add_argument("--ticks", type=int, default=90)
    parser.add_argument("--out", default="scratch/versus_online.json")
    args = parser.parse_args(argv)
    if args.match:
        return run_match(args.ticks)
    if not _shared_ok():
        print("kagra_shared is not installed", file=sys.stderr)
        return 2
    return run_client(args.port, args.name, args.ticks, args.out)


if __name__ == "__main__":
    sys.exit(main())
