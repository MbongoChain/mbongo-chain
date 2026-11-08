#!/usr/bin/env bash
# Build the unpublished development binaries, protocol v0.4, and describe them.
#
# Runs inside rust:1.94.0-bookworm pinned by digest. The workflow
# .github/workflows/dev-binaries.yml and a manual build run this same script,
# so the two cannot drift apart.
#
#   build.sh <out-dir> <image-reference-with-digest>
#
# Writes into <out-dir>: the binaries, metadata.json and SHA256SUMS.
#
# One build proves nothing about reproducibility. Two independent builds with
# matching checksums would; until then, metadata.json says "not established".
set -euo pipefail

OUT=${1:?usage: build.sh <out-dir> <image-reference>}
IMAGE=${2:?usage: build.sh <out-dir> <image-reference>}
TARGET=x86_64-unknown-linux-gnu

# Package -> binaries. Each pair is built as `-p <package> --bin <name>`, so
# cargo itself refuses a binary that the package does not declare.
NODE_PKG=mbongo-node
NODE_BINS=(mbongo-node convergence_probe)
COMPUTE_PKG=mbongo-compute
COMPUTE_BINS=(provider_service job_client)

# The system packages the Dockerfile builder stage installs: RocksDB bindings
# need clang and libclang, native-tls needs OpenSSL headers. Versions come from
# the image's Debian snapshot, not pinned individually.
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends clang libclang-dev pkg-config libssl-dev >/dev/null

git config --global --add safe.directory "$PWD"
COMMIT=$(git rev-parse HEAD)
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then DIRTY=true; else DIRTY=false; fi

HOST=$(rustc -vV | sed -n 's/^host: //p')
if [ "$HOST" != "$TARGET" ]; then
  echo "error: host $HOST is not the target $TARGET" >&2
  exit 1
fi

# No shared build cache, and no machine paths in the binaries.
export CARGO_INCREMENTAL=0
export RUSTFLAGS="--remap-path-prefix=$PWD=/build --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo"
SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)
export SOURCE_DATE_EPOCH

node_args=(); for b in "${NODE_BINS[@]}"; do node_args+=(--bin "$b"); done
compute_args=(); for b in "${COMPUTE_BINS[@]}"; do compute_args+=(--bin "$b"); done
cargo build --locked --release --target "$TARGET" -p "$NODE_PKG" "${node_args[@]}"
cargo build --locked --release --target "$TARGET" -p "$COMPUTE_PKG" "${compute_args[@]}"

mkdir -p "$OUT"
for b in "${NODE_BINS[@]}" "${COMPUTE_BINS[@]}"; do
  src="target/$TARGET/release/$b"
  [ -x "$src" ] || { echo "error: $src was not produced" >&2; exit 1; }
  # An ELF executable, not a script or a leftover.
  [ "$(head -c 4 "$src" | od -An -tx1 | tr -d ' \n')" = "7f454c46" ] \
    || { echo "error: $src is not an ELF file" >&2; exit 1; }
  cp "$src" "$OUT/$b"
done
"$OUT/mbongo-node" --version

json_list() { local first=1; printf '['; for x in "$@"; do
  [ $first -eq 1 ] || printf ', '; printf '"%s"' "$x"; first=0; done; printf ']'; }

cat > "$OUT/metadata.json" <<EOF
{
  "schema": "mbongo-dev-binaries:v1",
  "published": false,
  "reproducible": "not established",
  "protocol": "v0.4",
  "source_commit": "$COMMIT",
  "source_ref": "${GITHUB_REF:-manual}",
  "source_tree_modified": $DIRTY,
  "workflow_run": "${GITHUB_RUN_ID:-manual}",
  "container_image": "$IMAGE",
  "target": "$TARGET",
  "profile": "release",
  "rustc": "$(rustc -V)",
  "rustc_commit_hash": "$(rustc -vV | sed -n 's/^commit-hash: //p')",
  "llvm_version": "$(rustc -vV | sed -n 's/^LLVM version: //p')",
  "cargo": "$(cargo -V)",
  "cargo_lock_sha256": "$(sha256sum Cargo.lock | cut -d' ' -f1)",
  "rustflags": "--remap-path-prefix=<source>=/build --remap-path-prefix=<cargo home>=/cargo",
  "source_date_epoch": $SOURCE_DATE_EPOCH,
  "packages": {
    "$NODE_PKG": $(json_list "${NODE_BINS[@]}"),
    "$COMPUTE_PKG": $(json_list "${COMPUTE_BINS[@]}")
  }
}
EOF

( cd "$OUT" && sha256sum "${NODE_BINS[@]}" "${COMPUTE_BINS[@]}" metadata.json > SHA256SUMS && cat SHA256SUMS )
