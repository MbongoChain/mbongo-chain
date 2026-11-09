#!/usr/bin/env bash
# A2.0: every scenario, against every baseline, TRIALS times, same settings.
#
#   run-all.sh <bin-root> <results-root> [scenario...]
#
# <bin-root> holds pre/, post/ (mbongo-node) and stress/ (p2p_stress), as written
# by build-baselines.sh. Trials alternate baselines so that drift on the host
# affects both alike.
set -uo pipefail
BIN=$1 OUT=$2; shift 2
SCENARIOS=${*:-partial full announce sync churn streams}
TRIALS=${TRIALS:-3}
HERE=$(cd "$(dirname "$0")" && pwd)
BIN_W=$(cd "$BIN" && pwd -W 2>/dev/null || pwd)
for trial in $(seq 1 "$TRIALS"); do
  for scenario in $SCENARIOS; do
    for baseline in pre post; do
      dir="$OUT/$baseline/$scenario/$trial"
      mkdir -p "$(dirname "$dir")"
      echo "$(date -u +%H:%M:%S) $baseline $scenario trial $trial"
      bash "$HERE/run-trial.sh" "$BIN_W/$baseline" "$BIN_W/stress" "$scenario" "$dir" \
        > "$dir.run.log" 2>&1 || echo "  trial failed, see $dir.run.log"
    done
  done
done
