#!/usr/bin/env bash
# A2.0: one measured trial of one attack against one node build.
#
#   run-trial.sh <node-bin-dir> <stress-bin-dir> <scenario> <out-dir>
#
# Everything runs in containers on an *internal* Docker network (no route out),
# with fixed private addresses:
#   .10 producer (honest)        .20 victim: a follower, memory-limited
#   .30 attacker (p2p_stress)    .40 late follower: a new honest peer
#   .50 sampler, outside the victim's cgroup
#
# Phases: WARM s of idle measurement, ATTACK s of traffic (a new honest peer
# joins halfway), RECOVER s after the traffic stops.
#
# Environment (all optional): WARM ATTACK RECOVER MEM CPUS BLOCK_TIME
# IDENTITIES CONCURRENCY IMAGE.
set -euo pipefail
export MSYS_NO_PATHCONV=1

NODE_BIN=$1 STRESS_BIN=$2 SCENARIO=$3 OUT=$4
WARM=${WARM:-20} ATTACK=${ATTACK:-60} RECOVER=${RECOVER:-40}
MEM=${MEM:-512m} CPUS=${CPUS:-2} BLOCK_TIME=${BLOCK_TIME:-1}
IDENTITIES=${IDENTITIES:-4} CONCURRENCY=${CONCURRENCY:-64}
IMAGE=${IMAGE:-rust:1.94.0-bookworm@sha256:4673f78db88b71f09d5451bbc404734807918161241215ba0a50bbbe9b448117}
NET=mbongo-a2 SUBNET=172.31.250
HERE=$(cd "$(dirname "$0")" && pwd -W 2>/dev/null || pwd)
mkdir -p "$OUT"
OUT_W=$(cd "$OUT" && pwd -W 2>/dev/null || pwd)

cleanup() { docker rm -f a2-producer a2-victim a2-attacker a2-late a2-sampler >/dev/null 2>&1 || true; }
cleanup
trap cleanup EXIT

if ! docker network inspect $NET >/dev/null 2>&1; then
  docker network create --internal --subnet $SUBNET.0/24 $NET >/dev/null
fi
[ "$(docker network inspect -f '{{.Internal}}' $NET)" = true ] || { echo "network $NET is not internal" >&2; exit 2; }

node() { # name ip extra-docker-args -- node-args
  local name=$1 ip=$2; shift 2
  local docker_args=()
  while [ "$1" != -- ]; do docker_args+=("$1"); shift; done; shift
  docker run -d --name "$name" --network $NET --ip "$ip" "${docker_args[@]}" \
    -e RUST_LOG=info -v "$NODE_BIN:/opt/m:ro" "$IMAGE" /opt/m/mbongo-node \
    --rpc-host 0.0.0.0 --rpc-port 9944 --rest-port 8080 --p2p-host 0.0.0.0 --p2p-port 30333 \
    --data-dir /tmp/data --no-mdns "$@" >/dev/null
}
rpc_height() { # ip
  docker exec a2-producer curl -s -m 1 -X POST -H 'content-type: application/json' \
    -d '{"jsonrpc":"2.0","method":"get_block_height","id":1}' "http://$1:9944/rpc" 2>/dev/null \
    | sed -n 's/.*"result":\([0-9]*\).*/\1/p'
}

node a2-producer $SUBNET.10 -- --producer --block-time "$BLOCK_TIME"
# VICTIM_ENV (optional, NAME=value) is set for the victim only, for diagnostic
# trials, and recorded in trial.json.
node a2-victim $SUBNET.20 --memory "$MEM" --memory-swap "$MEM" --cpus "$CPUS" \
  ${VICTIM_ENV:+-e "$VICTIM_ENV"} -- \
  --bootnodes /ip4/$SUBNET.10/tcp/30333

# The victim is synced before anything is measured.
for _ in $(seq 120); do
  p=$(rpc_height $SUBNET.10); v=$(rpc_height $SUBNET.20)
  [ -n "$p" ] && [ -n "$v" ] && [ "$p" -ge 2 ] && [ "$v" -ge "$((p - 1))" ] && break
  sleep 1
done
[ -n "${v:-}" ] && [ "$v" -ge 1 ] || { echo "victim never synced" >&2; docker logs a2-victim 2>&1 | tail -20 >&2; exit 3; }

ID=$(docker inspect -f '{{.Id}}' a2-victim)
PID=$(docker inspect -f '{{.State.Pid}}' a2-victim)
CG=/hostcg/docker/$ID
TOTAL=$((WARM + ATTACK + RECOVER))
docker run -d --name a2-sampler --network $NET --ip $SUBNET.50 --pid=host \
  -v /sys/fs/cgroup:/hostcg:ro -v "$HERE:/s:ro" -v "$OUT_W:/out" "$IMAGE" \
  python3 /s/sample.py --cgroup "$CG" --pid "$PID" --out /out/samples.jsonl --seconds "$TOTAL" \
  --rpc producer=http://$SUBNET.10:9944/rpc --rpc victim=http://$SUBNET.20:9944/rpc \
  --rpc late=http://$SUBNET.40:9944/rpc >/dev/null

sleep "$WARM"
date -u +%s > "$OUT/attack_start"
docker run -d --name a2-attacker --network $NET --ip $SUBNET.30 -v "$STRESS_BIN:/opt/s:ro" "$IMAGE" \
  /opt/s/p2p_stress --target /ip4/$SUBNET.20/tcp/30333 --scenario "$SCENARIO" \
  --identities "$IDENTITIES" --concurrency "$CONCURRENCY" --duration-secs "$ATTACK" >/dev/null
sleep $((ATTACK / 2))
node a2-late $SUBNET.40 -- --bootnodes /ip4/$SUBNET.20/tcp/30333
docker wait a2-attacker >/dev/null
date -u +%s > "$OUT/attack_end"
docker wait a2-sampler >/dev/null

docker logs a2-attacker > "$OUT/attacker.jsonl" 2> "$OUT/attacker.err" || true
docker logs -t a2-victim > "$OUT/victim.log" 2>&1 || true
docker inspect -f '{{json .State}}' a2-victim > "$OUT/victim_state.json"
docker logs -t a2-late > "$OUT/late.log" 2>&1 || true
docker logs a2-sampler > "$OUT/sampler.log" 2>&1 || true
printf '{"scenario":"%s","warm":%s,"attack":%s,"recover":%s,"mem":"%s","cpus":%s,"block_time":%s,"identities":%s,"concurrency":%s,"image":"%s","victim_env":"%s"}\n' \
  "$SCENARIO" "$WARM" "$ATTACK" "$RECOVER" "$MEM" "$CPUS" "$BLOCK_TIME" "$IDENTITIES" "$CONCURRENCY" "$IMAGE" "${VICTIM_ENV:-}" > "$OUT/trial.json"
echo "trial done: $SCENARIO -> $OUT"
