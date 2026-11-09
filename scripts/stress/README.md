# A2.0 resource-exhaustion harness

Test tooling for measuring how one `mbongo-node` uses memory, CPU and
connections under hostile P2P traffic. It changes no node code and is not
part of any release.

## Safety boundaries

- **Isolated network.** Every container joins `mbongo-a2`, a Docker network
  created with `--internal`, so it has no route outside the host. `run-trial.sh`
  refuses to run if that network exists without `--internal`.
- **Fixed allowlist.** The harness only creates, uses and removes five
  containers: `a2-producer`, `a2-victim`, `a2-attacker`, `a2-late` and
  `a2-sampler`, at fixed addresses in `172.31.250.0/24`.
- **Local targets only.** `p2p_stress` refuses any target that is not a
  loopback or private address (RFC 1918, IPv6 loopback or unique-local), and
  refuses DNS names, before opening a socket.
- **No weakening.** No validation is disabled and no limit is lifted in the
  nodes under test; they are the release builds of the named commits.

## Pieces

| File | Role |
|---|---|
| `crates/mbongo-stress` (`p2p_stress`) | Sends traffic in the frozen wire format through a raw codec, so every node version receives the same bytes. Scenarios: `partial`, `full`, `announce`, `sync`, `churn`, `streams` |
| `build-baselines.sh` | Builds each node revision and `p2p_stress` in one digest-pinned image, from `core.autocrlf=false` clones, and records commits and SHA-256 sums in `BUILD.txt` |
| `run-trial.sh` | One trial: producer, memory-limited victim, sampler; WARM s idle, ATTACK s of traffic (a new honest follower joins halfway), RECOVER s after |
| `sample.py` | Reads the victim's cgroup v2 files and `/proc/<pid>` from **outside** the victim's cgroup, so it never counts against what it measures |
| `run-all.sh` | Every scenario × every baseline × `TRIALS`, alternating baselines |
| `analyze.py` | Per-trial `summary.json` and a min / median / max table across trials |

## Running

```bash
bash scripts/stress/build-baselines.sh C:/Dev/mbongo-chain C:/Dev/a2-work <stress-rev> pre=<rev> post=<rev>
TRIALS=3 bash scripts/stress/run-all.sh C:/Dev/a2-work/bin C:/Dev/a2-work/results
```

Then run `analyze.py` in the same image, with the results mounted.

Defaults: `WARM=20 ATTACK=60 RECOVER=40 MEM=512m CPUS=2 BLOCK_TIME=1 IDENTITIES=4 CONCURRENCY=64`.

## What it measures, and what it does not

Measured: `memory.current`, `memory.peak`, `memory.events`, `cpu.stat`, VmRSS,
VmHWM, VmSize, threads, established P2P connections, heights of producer,
victim and late follower, victim log volume, node counters (A1 and later),
and what the sending side saw.

Not measured:
- queue occupancy inside the node (no gauge is exposed);
- stream counts inside yamux;
- RocksDB memory beyond what the cgroup reports as a whole.

A trial on one host is an observation of that host; it is not a bound.

## Reading results

- **`answered` changed meaning after the first campaign.**
  - In output from `p2p_stress` built at `99fa6a07` (the 2026-11-09 campaign), `answered` also counts streams the node closed with no reply, which are refusals.
  - Later builds count those separately as `closed_unanswered`.
  - For block announcements, whose acknowledgement is a 4-byte frame, the acknowledgements in older output are `bytes_in / 4`.
- **`memory_anon` and `memory_file`** (from `memory.stat`) were added after that campaign; its samples do not have them.
- **A node killed by the OOM killer takes its cgroup with it.** The last sample before the kill can be far below the real peak. The kill itself is in `victim_state.json` (`OOMKilled`, exit code 137).
- **The `local` Docker log driver rotates files.** A very talkative node loses its earliest lines, so total line counts are a lower bound. The maximum per 10-second window is still a measurement.
- **`summary.json` and the printed table are derived** by `analyze.py`; every other file in a trial directory is a raw observation.
