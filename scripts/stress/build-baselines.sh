#!/usr/bin/env bash
# A2.0: build the node at each baseline, and p2p_stress, in one pinned image.
#
#   build-baselines.sh <repo> <work-dir> <stress-rev> <name>=<rev>...
#
# Each revision is cloned with core.autocrlf=false (a Windows checkout would
# otherwise hand Linux a CRLF tree), copied into the container and built there
# with --locked --release. Outputs: <work-dir>/bin/<name>/mbongo-node,
# <work-dir>/bin/stress/p2p_stress, and BUILD.txt with commits and checksums.
set -euo pipefail
export MSYS_NO_PATHCONV=1
REPO=$1 WORK=$2 STRESS_REV=$3; shift 3
IMAGE=${IMAGE:-rust:1.94.0-bookworm@sha256:4673f78db88b71f09d5451bbc404734807918161241215ba0a50bbbe9b448117}
mkdir -p "$WORK/src" "$WORK/bin"
W=$(cd "$WORK" && pwd -W 2>/dev/null || pwd)

build() { # name rev cargo-args out-binary
  local name=$1 rev=$2 cargs=$3 binary=$4
  rm -rf "$WORK/src/$name" "$WORK/bin/$name"; mkdir -p "$WORK/bin/$name"
  git -c core.autocrlf=false clone -q --no-checkout "$REPO" "$WORK/src/$name"
  git -C "$WORK/src/$name" -c core.autocrlf=false checkout -q --detach "$rev"
  docker run --rm -v "$W/src/$name:/src:ro" -v "$W/bin/$name:/out" "$IMAGE" bash -c "
    set -e; apt-get update -qq >/dev/null; apt-get install -y -qq clang libclang-dev >/dev/null
    cp -a /src /build; cd /build
    CARGO_INCREMENTAL=0 cargo build --locked --release $cargs
    cp target/release/$binary /out/"
  echo "$name $(git -C "$WORK/src/$name" rev-parse HEAD) $(sha256sum "$WORK/bin/$name/$binary" | cut -c1-64)" >> "$WORK/bin/BUILD.txt"
}

: > "$WORK/bin/BUILD.txt"
for pair in "$@"; do
  # NODE_FEATURES (optional), e.g. alloc-stats for a diagnostic build.
  build "${pair%%=*}" "${pair#*=}" "-p mbongo-node --bin mbongo-node${NODE_FEATURES:+ --features $NODE_FEATURES}" mbongo-node
done
build stress "$STRESS_REV" "-p mbongo-stress --bin p2p_stress" p2p_stress
echo "image $IMAGE" >> "$WORK/bin/BUILD.txt"
cat "$WORK/bin/BUILD.txt"
