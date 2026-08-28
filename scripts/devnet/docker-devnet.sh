#!/bin/sh
# Host-side orchestration for the deterministic Docker devnet (issue #53).
#
# This is the single implementation of the bootstrap: `make devnet-up` and
# `make devnet-down` delegate to it, and CI can call it directly, so the
# local and automated paths cannot drift apart.
#
# It orchestrates containers only. Convergence is decided exclusively by the
# convergence_probe binary running in the convergence-check service.
#
# Usage:
#   docker-devnet.sh up      boot the 3-node devnet and verify convergence
#   docker-devnet.sh down    stop it and remove its containers/network
#   docker-devnet.sh config  print the fully resolved Compose configuration
#   docker-devnet.sh status  show container state
#
# Environment:
#   DEVNET_ENV=local (default) layers .env.local on top of .env.base when
#                              that file exists.
#   DEVNET_ENV=ci              layers .env.ci instead.
set -eu

REPO_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$REPO_ROOT"

COMPOSE_FILE=docker-compose.yml
NODES="producer follower-a follower-b"

# ── Environment layering ────────────────────────────────────────────────
# .env.base is always applied; the second file, when present, overrides it.
# Compose applies --env-file left to right, last one wins.
env_args() {
    printf '%s' "--env-file .env.base"
    case "${DEVNET_ENV:-local}" in
        ci)
            printf ' %s' "--env-file .env.ci"
            ;;
        local)
            if [ -f .env.local ]; then
                printf ' %s' "--env-file .env.local"
            fi
            ;;
        *)
            echo "unknown DEVNET_ENV: ${DEVNET_ENV}" >&2
            exit 2
            ;;
    esac
}

compose() {
    # shellcheck disable=SC2046 # word splitting of the env-file list is intended
    docker compose $(env_args) -f "$COMPOSE_FILE" "$@"
}

# ── Diagnostics ─────────────────────────────────────────────────────────
# Printed on any failure so a red run is diagnosable without re-running it.
dump_diagnostics() {
    echo ""
    echo "----- devnet container state -----"
    compose ps --all || true
    for node in $NODES; do
        echo ""
        echo "----- last 60 log lines: ${node} -----"
        compose logs --tail=60 "$node" || true
    done
    echo ""
    echo "----- health status -----"
    for node in $NODES; do
        cid=$(compose ps -q "$node" 2>/dev/null || true)
        if [ -n "$cid" ]; then
            state=$(docker inspect --format '{{.State.Status}} health={{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$cid" 2>/dev/null || echo unknown)
            echo "  ${node}: ${state}"
        else
            echo "  ${node}: no container"
        fi
    done
}

cmd_up() {
    echo "=== Building image and starting ${MBONGO_IMAGE:-devnet} nodes ==="
    # --wait blocks until every started service reports healthy, so no
    # arbitrary sleep is needed anywhere in this script.
    # shellcheck disable=SC2086 # NODES is an intentional word list
    if ! compose up --detach --build --wait --wait-timeout "${DEVNET_WAIT_TIMEOUT:-420}" $NODES; then
        echo ""
        echo "FAILED: the three nodes did not all become healthy." >&2
        dump_diagnostics
        exit 1
    fi

    echo ""
    echo "=== All 3 nodes healthy; running convergence_probe ==="
    if ! compose run --rm convergence-check; then
        echo ""
        echo "FAILED: convergence check did not pass." >&2
        dump_diagnostics
        exit 1
    fi

    echo ""
    echo "devnet-up: PASS (3 nodes healthy, convergence verified)"
}

cmd_down() {
    # Safe to run when nothing is up: compose down on an absent project is a
    # no-op and still exits 0.
    compose down --volumes --remove-orphans --timeout 15
    echo "devnet-down: cleaned up"
}

case "${1:-}" in
    up)     cmd_up ;;
    down)   cmd_down ;;
    config) compose config ;;
    status) compose ps --all ;;
    *)
        echo "usage: $0 {up|down|config|status}" >&2
        exit 2
        ;;
esac
