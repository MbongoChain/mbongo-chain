#!/usr/bin/env python3
"""A2.0 sampler: reads one container's cgroup and its node process from outside.

It runs in its own container (host PID namespace, cgroup tree mounted read-only)
so that its own memory never counts against the container it measures. One JSON
object per line, every --interval seconds, for --seconds seconds.
"""
import argparse
import json
import os
import time
import urllib.request


def read(path):
    try:
        with open(path) as f:
            return f.read()
    except OSError:
        return None


def pairs(path):
    text = read(path) or ""
    out = {}
    for line in text.splitlines():
        parts = line.split()
        if len(parts) >= 2:
            out[parts[0].rstrip(":")] = parts[1]
    return out


def as_int(value):
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def tcp_established(pid, port):
    """Established TCP connections on the node's P2P port, in its namespace."""
    want = format(port, "04X")
    count = 0
    for name in ("tcp", "tcp6"):
        text = read(f"/proc/{pid}/net/{name}") or ""
        for line in text.splitlines()[1:]:
            fields = line.split()
            if len(fields) > 3 and fields[1].endswith(":" + want) and fields[3] == "01":
                count += 1
    return count


def height(url):
    body = json.dumps({"jsonrpc": "2.0", "method": "get_block_height", "id": 1}).encode()
    request = urllib.request.Request(url, body, {"content-type": "application/json"})
    try:
        with urllib.request.urlopen(request, timeout=1) as response:
            return json.load(response).get("result")
    except Exception:  # noqa: BLE001 - an unreachable node is a reading, not an error
        return None


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--cgroup", required=True)
    p.add_argument("--pid", type=int, required=True)
    p.add_argument("--port", type=int, default=30333)
    p.add_argument("--out", required=True)
    p.add_argument("--seconds", type=float, required=True)
    p.add_argument("--interval", type=float, default=0.5)
    p.add_argument("--rpc", action="append", default=[], help="name=url")
    args = p.parse_args()
    rpcs = dict(item.split("=", 1) for item in args.rpc)
    if not os.path.isdir(args.cgroup):
        raise SystemExit(f"no cgroup at {args.cgroup}")

    start = time.monotonic()
    with open(args.out, "w") as out:
        while time.monotonic() - start < args.seconds:
            t = time.monotonic() - start
            cg = args.cgroup
            events = pairs(f"{cg}/memory.events")
            status = pairs(f"/proc/{args.pid}/status")
            procs = (read(f"{cg}/cgroup.procs") or "").split()
            sample = {
                "t": round(t, 3),
                "memory_current": as_int((read(f"{cg}/memory.current") or "").strip()),
                "memory_peak": as_int((read(f"{cg}/memory.peak") or "").strip()),
                "memory_max": (read(f"{cg}/memory.max") or "").strip(),
                "oom": as_int(events.get("oom")),
                "oom_kill": as_int(events.get("oom_kill")),
                "cpu_usage_usec": as_int(pairs(f"{cg}/cpu.stat").get("usage_usec")),
                "cgroup_procs": len(procs),
                "alive": bool(status),
                "vm_rss_kib": as_int(status.get("VmRSS")),
                "vm_hwm_kib": as_int(status.get("VmHWM")),
                "vm_size_kib": as_int(status.get("VmSize")),
                "threads": as_int(status.get("Threads")),
                "tcp_established": tcp_established(args.pid, args.port) if status else None,
            }
            for name, url in rpcs.items():
                sample[f"height_{name}"] = height(url)
            out.write(json.dumps(sample) + "\n")
            out.flush()
            time.sleep(max(0.0, args.interval - (time.monotonic() - start - t)))


if __name__ == "__main__":
    main()
