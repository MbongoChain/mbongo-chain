#!/bin/sh
# Container readiness for a devnet node.
#
# "Running" is not "ready": the RPC server binds only after storage is open,
# genesis exists and the P2P stack is up, and answering the JSON-RPC `ping`
# method proves the server actually serves requests. That is the same
# readiness primitive mbongo_node::convergence uses.
#
# This is deliberately NOT a convergence check. Height and tip-hash
# comparison lives in exactly one place, the convergence_probe binary.
set -eu

PORT="${MBONGO_RPC_PORT:-9944}"

curl --silent --show-error --fail --max-time 3 \
    --header 'Content-Type: application/json' \
    --data '{"jsonrpc":"2.0","method":"ping","id":1}' \
    "http://127.0.0.1:${PORT}/rpc" \
    | grep -q '"result"'
