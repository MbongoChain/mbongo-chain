#!/usr/bin/env python3
"""A2.0: summarise trials.

    analyze.py <results-root>

Expects <root>/<baseline>/<scenario>/<trial>/ as written by run-trial.sh.
Writes summary.json in each trial directory and prints one Markdown table per
measure, with min / median / max across trials. Values are as measured; nothing
here is extrapolated.
"""
import json
import os
import re
import statistics
import sys

MIB = 1024 * 1024


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def phase(samples, start, end):
    return [s for s in samples if start <= s["t"] < end]


def maxof(rows, key):
    values = [r[key] for r in rows if r.get(key) is not None]
    return max(values) if values else None


def medianof(rows, key):
    values = [r[key] for r in rows if r.get(key) is not None]
    return statistics.median(values) if values else None


def cores(rows):
    rows = [r for r in rows if r.get("cpu_usage_usec") is not None]
    if len(rows) < 2:
        return None
    used = rows[-1]["cpu_usage_usec"] - rows[0]["cpu_usage_usec"]
    span = rows[-1]["t"] - rows[0]["t"]
    return used / 1e6 / span if span > 0 else None


def lag(row):
    p, v = row.get("height_producer"), row.get("height_victim")
    return p - v if p is not None and v is not None else None


def log_rates(path, trial):
    """Victim log lines and bytes per 10 s window, from `docker logs -t`."""
    windows = {}
    stamp = re.compile(r"^(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d)")
    total_lines = total_bytes = 0
    with open(path, errors="replace") as f:
        for line in f:
            m = stamp.match(line)
            if not m:
                continue
            key = m.group(1)[:-1]  # 10 s window
            lines, size = windows.get(key, (0, 0))
            windows[key] = (lines + 1, size + len(line))
            total_lines += 1
            total_bytes += len(line)
    worst = max(windows.values(), default=(0, 0))
    return {"log_lines": total_lines, "log_bytes": total_bytes,
            "log_lines_max_10s": worst[0], "log_bytes_max_10s": worst[1]}


def counters(path):
    """The last P2P counters line the node logged, if it logs them (A1 and later)."""
    last = None
    with open(path, errors="replace") as f:
        for line in f:
            if "P2P counters since start:" in line:
                last = line.split("P2P counters since start:", 1)[1].strip()
    return last


def alloc_stats(path):
    """Live heap bytes from a diagnostic build's `alloc-stats` lines: the
    highest seen, and the last (the end of the trial)."""
    live = re.compile(r"alloc-stats: live_bytes=(\d+) peak_live_bytes=(\d+)")
    readings = []
    with open(path, errors="replace") as f:
        for line in f:
            m = live.search(line)
            if m:
                readings.append((int(m.group(1)), int(m.group(2))))
    if not readings:
        return {"live_max_mib": None, "live_last_mib": None, "peak_live_mib": None}
    return {
        "live_max_mib": max(r[0] for r in readings) / MIB,
        "live_last_mib": readings[-1][0] / MIB,
        "peak_live_mib": readings[-1][1] / MIB,
    }


def malloc_diag(trial_dir):
    """The node's `malloc-diag before/after` lines (one explicit trim) and the
    cgroup read from outside just before and after; empty when not run."""
    out = {}
    pair = re.compile(r"(\w+)=(-?\w+)")
    with open(os.path.join(trial_dir, "victim.log"), errors="replace") as f:
        for line in f:
            for when in ("before", "after"):
                marker = f"malloc-diag {when}: "
                if marker in line:
                    fields = dict(pair.findall(line.split(marker, 1)[1]))
                    out[f"trim_{when}"] = fields
    for when in ("before", "after"):
        path = os.path.join(trial_dir, f"trim_{when}.jsonl")
        if os.path.exists(path):
            rows = load(path)
            if rows:
                out[f"cgroup_{when}_trim"] = {
                    k: rows[-1].get(k) for k in ("memory_current", "memory_anon", "memory_file",
                                                 "vm_rss_kib", "rss_anon_kib")
                }
    return out


def summarise(trial_dir):
    trial = json.load(open(os.path.join(trial_dir, "trial.json")))
    samples = load(os.path.join(trial_dir, "samples.jsonl"))
    warm, attack, recover = trial["warm"], trial["attack"], trial["recover"]
    idle = phase(samples, 2, warm)
    during = phase(samples, warm, warm + attack)
    after = phase(samples, warm + attack, warm + attack + recover)
    state = json.load(open(os.path.join(trial_dir, "victim_state.json")))

    lags = [lag(s) for s in during if lag(s) is not None]
    start_h = next((s["height_victim"] for s in during if s.get("height_victim") is not None), None)
    end_h = next((s["height_victim"] for s in reversed(during) if s.get("height_victim") is not None), None)
    recovered_at = next((s["t"] - (warm + attack) for s in after
                         if lag(s) is not None and lag(s) <= 1), None)
    late_end = next((s.get("height_late") for s in reversed(samples) if s.get("height_late") is not None), None)
    producer_end = next((s.get("height_producer") for s in reversed(samples) if s.get("height_producer") is not None), None)
    attacker = load(os.path.join(trial_dir, "attacker.jsonl"))
    out = {
        "idle_memory_current_mib": (medianof(idle, "memory_current") or 0) / MIB,
        "idle_rss_mib": (medianof(idle, "vm_rss_kib") or 0) / 1024,
        "peak_memory_peak_mib": (maxof(samples, "memory_peak") or 0) / MIB,
        "attack_max_memory_current_mib": (maxof(during, "memory_current") or 0) / MIB,
        "attack_max_rss_mib": (maxof(during, "vm_rss_kib") or 0) / 1024,
        "max_vm_hwm_mib": (maxof(samples, "vm_hwm_kib") or 0) / 1024,
        "attack_max_vm_size_mib": (maxof(during, "vm_size_kib") or 0) / 1024,
        "after_memory_current_mib": ((after[-1].get("memory_current") if after else None) or 0) / MIB,
        "attack_cpu_cores": cores(during),
        "attack_max_tcp": maxof(during, "tcp_established"),
        "attack_max_threads": maxof(during, "threads"),
        "oom_kill": maxof(samples, "oom_kill"),
        "victim_oom_killed": state.get("OOMKilled"),
        "victim_running_at_end": state.get("Running"),
        "victim_exit_code": state.get("ExitCode"),
        "attack_max_lag": max(lags) if lags else None,
        "attack_progress_blocks": (end_h - start_h) if start_h is not None and end_h is not None else None,
        "recovered_after_s": recovered_at,
        "late_follower_height_end": late_end,
        "producer_height_end": producer_end,
        "attacker_final": attacker[-1] if attacker else None,
        "victim_counters": counters(os.path.join(trial_dir, "victim.log")),
        # memory.stat split, present only in samples taken after the first campaign.
        "attack_max_anon_mib": (maxof(during, "memory_anon") or 0) / MIB if maxof(during, "memory_anon") is not None else None,
        "after_anon_mib": (after[-1].get("memory_anon") / MIB) if after and after[-1].get("memory_anon") is not None else None,
        "after_file_mib": (after[-1].get("memory_file") / MIB) if after and after[-1].get("memory_file") is not None else None,
        "victim_env": trial.get("victim_env"),
        # Diagnostic builds (`alloc-stats`) only; None otherwise.
        **alloc_stats(os.path.join(trial_dir, "victim.log")),
        **malloc_diag(trial_dir),
    }
    out.update(log_rates(os.path.join(trial_dir, "victim.log"), trial))
    json.dump(out, open(os.path.join(trial_dir, "summary.json"), "w"), indent=2)
    return out


def fmt(values):
    values = [v for v in values if isinstance(v, (int, float)) and not isinstance(v, bool)]
    if not values:
        return "n/a"
    lo, mid, hi = min(values), statistics.median(values), max(values)
    f = (lambda x: f"{x:.1f}") if any(isinstance(v, float) for v in values) else str
    return f"{f(mid)} [{f(lo)}–{f(hi)}]" if lo != hi else f(mid)


def main():
    root = sys.argv[1]
    table = {}
    for baseline in sorted(os.listdir(root)):
        bdir = os.path.join(root, baseline)
        if not os.path.isdir(bdir):
            continue
        for scenario in sorted(os.listdir(bdir)):
            sdir = os.path.join(bdir, scenario)
            trials = []
            for trial in sorted(os.listdir(sdir)):
                tdir = os.path.join(sdir, trial)
                if os.path.exists(os.path.join(tdir, "samples.jsonl")):
                    try:
                        trials.append(summarise(tdir))
                    except Exception as e:  # noqa: BLE001 - report, do not hide
                        print(f"<!-- {tdir}: {e} -->")
            table[(scenario, baseline)] = trials
    keys = ["n", "peak_memory_peak_mib", "attack_max_rss_mib", "attack_max_vm_size_mib",
            "idle_memory_current_mib", "after_memory_current_mib", "attack_cpu_cores",
            "attack_max_tcp", "oom_kill", "attack_max_lag", "attack_progress_blocks",
            "recovered_after_s", "late_follower_height_end", "log_lines_max_10s"]
    print("| scenario | baseline | " + " | ".join(keys) + " |")
    print("|" + "---|" * (len(keys) + 2))
    for (scenario, baseline), trials in sorted(table.items()):
        cells = [str(len(trials))] + [fmt([t.get(k) for t in trials]) for k in keys[1:]]
        print(f"| {scenario} | {baseline} | " + " | ".join(cells) + " |")
    print()
    for (scenario, baseline), trials in sorted(table.items()):
        for i, t in enumerate(trials):
            print(f"- {scenario}/{baseline}/{i}: oom_killed={t['victim_oom_killed']} "
                  f"running={t['victim_running_at_end']} counters={t['victim_counters']} "
                  f"attacker={json.dumps(t['attacker_final'])}")


if __name__ == "__main__":
    main()
