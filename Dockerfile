# Devnet image for the deterministic 3-node Docker devnet (issue #53).
#
# Multi-stage: the builder carries the C/C++ toolchain RocksDB needs, the
# runtime carries only the two binaries and their shared libraries. Nothing
# in this file changes how the node behaves -- it only packages it.

# Exact toolchain pin. The repository declares no canonical rust-toolchain
# file, and the workspace rust-version = "1.75" is the MSRV, not the version
# the project is validated on; 1.94.0 is what the check suite (fmt, clippy
# -D warnings, tests, replay and devnet harnesses) is currently green on.
# A floating tag drifts: rust:1-bookworm already resolves to a different
# compiler. Keep in sync with RUST_VERSION in .env.base, which is what the
# Compose build actually passes. Dependency reproducibility comes from
# Cargo.lock plus --locked below.
ARG RUST_VERSION=1.94.0

# ── builder ─────────────────────────────────────────────────────────────
FROM rust:${RUST_VERSION}-bookworm AS builder

# librocksdb-sys compiles RocksDB from source and generates bindings with
# bindgen (clang/libclang). reqwest pulls in native-tls, hence libssl-dev
# and pkg-config.
RUN apt-get update && apt-get install -y --no-install-recommends \
        clang \
        libclang-dev \
        pkg-config \
        libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Only the workspace manifests and crates: everything else is excluded by
# .dockerignore, so an unrelated edit does not invalidate this layer.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# --locked: fail rather than silently resolve different dependency versions
# than the committed Cargo.lock.
RUN cargo build --locked --release \
        -p mbongo-node \
        --bin mbongo-node \
        --bin convergence_probe

# ── runtime ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# libssl3: runtime half of native-tls. curl: used only by the container
# healthcheck to issue the JSON-RPC ping (see docker/healthcheck.sh); the
# node itself does not need it.
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        curl \
    && rm -rf /var/lib/apt/lists/*

# Unprivileged runtime user: the devnet never needs root.
RUN useradd --create-home --uid 10001 --shell /usr/sbin/nologin mbongo

COPY --from=builder /src/target/release/mbongo-node /usr/local/bin/mbongo-node
COPY --from=builder /src/target/release/convergence_probe /usr/local/bin/convergence_probe
COPY docker/healthcheck.sh /usr/local/bin/devnet-healthcheck

RUN chmod +x /usr/local/bin/devnet-healthcheck \
    && mkdir -p /data \
    && chown mbongo:mbongo /data

USER mbongo
WORKDIR /data

# Node by default; the convergence-check service overrides the entrypoint
# with convergence_probe.
ENTRYPOINT ["mbongo-node"]
