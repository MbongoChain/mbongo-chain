#!/usr/bin/env bash
# Reference launcher for a contained provider service (P2). Linux, cgroup v2,
# root. docs/runbooks/PROVIDER_CONTAINMENT_LINUX.md describes what it gives.
#
#   prepare <base>                     accounts, directories, cgroup; idempotent
#   run <base> <service> <config>      starts the service, unprivileged
#
# <base>/keys, state and artifacts belong to the provider account, 0700.
# <base>/work belongs to it too, group runner, 2750. The service checks all of
# this itself and refuses to start otherwise: `run` repairs nothing.
#
# `prepare` also installs the runner's network policy and the host's inbound
# policy. `run` refuses to start without them, unless MBONGO_RUNNER_NETWORK=open.
#
# MBONGO_INBOUND_ALLOW is required by both: the TCP ports this host accepts new
# connections on, comma-separated, or `none`. List everything the host needs,
# such as SSH: every other new inbound connection is dropped, host-wide.
#
# Environment: MBONGO_PROVIDER_USER, MBONGO_RUNNER_USER, MBONGO_CGROUP,
# MBONGO_RUNNER_NETWORK, MBONGO_INBOUND_ALLOW.
set -euo pipefail

PROVIDER_USER=${MBONGO_PROVIDER_USER:-mbongo-provider}
RUNNER_USER=${MBONGO_RUNNER_USER:-mbongo-runner}
CGROUP=${MBONGO_CGROUP:-/sys/fs/cgroup/mbongo-provider}
RUNNER_NETWORK=${MBONGO_RUNNER_NETWORK:-denied}
INBOUND_ALLOW=${MBONGO_INBOUND_ALLOW:-}
NFT_TABLE=mbongo_runner

die() {
  echo "error: $*" >&2
  exit 1
}

[ "$(uname -s)" = Linux ] || die "Linux only"
[ "$(id -u)" = 0 ] || die "run as root"

# An account someone can log in to is not dedicated to this service.
account() {
  if id -u "$1" >/dev/null 2>&1; then
    case "$(getent passwd "$1" | cut -d: -f7)" in
    */nologin | */false) ;;
    *) die "$1 has a login shell: the account must be dedicated" ;;
    esac
  else
    useradd --system --no-create-home --shell /usr/sbin/nologin "$1"
  fi
}

# Fails closed: no list and a malformed list are both refused.
inbound_ports() {
  [ -n "$INBOUND_ALLOW" ] || die "MBONGO_INBOUND_ALLOW is required: a port list, or none"
  [ "$INBOUND_ALLOW" = none ] && return 0
  case "$INBOUND_ALLOW" in
  ,* | *, | *,,*) die "MBONGO_INBOUND_ALLOW is malformed" ;;
  esac
  local port
  for port in ${INBOUND_ALLOW//,/ }; do
    case "$port" in
    '' | *[!0-9]*) die "MBONGO_INBOUND_ALLOW is malformed" ;;
    esac
    [ "${#port}" -le 5 ] && [ "$port" -ge 1 ] && [ "$port" -le 65535 ] ||
      die "MBONGO_INBOUND_ALLOW holds a port out of range"
  done
}

# Everything the runner's account sends is refused, over IPv4 and IPv6,
# loopback included. The rule follows the socket's owner, so it holds for every
# descendant. A reset fails a TCP connection at once.
network_policy() {
  command -v nft >/dev/null || die "nftables is required"
  local uid allow=""
  uid=$(id -u "$RUNNER_USER")
  [ "$INBOUND_ALLOW" = none ] || allow="tcp dport { $INBOUND_ALLOW } ct state new accept"
  # One transaction: a re-run on a live service leaves no moment without the rule.
  nft -f - <<EOF
table inet $NFT_TABLE
delete table inet $NFT_TABLE
table inet $NFT_TABLE {
  chain output {
    type filter hook output priority filter; policy accept;
    meta skuid $uid meta l4proto tcp counter reject with tcp reset
    meta skuid $uid counter reject with icmpx type admin-prohibited
  }
  chain input {
    type filter hook input priority filter; policy drop;
    iif "lo" accept
    ct state established,related accept
    icmpv6 type { nd-neighbor-solicit, nd-neighbor-advert, nd-router-advert } accept
    $allow
  }
}
EOF
}

# What `run` will not start without. It checks what is installed against what
# was asked for, not only that something is there.
check_policies() {
  local table listed wanted
  table=$(nft list table inet "$NFT_TABLE" 2>/dev/null) ||
    die "the runner's network policy is not installed: run prepare"
  grep -q "meta skuid $(id -u "$RUNNER_USER") " <<<"$table" ||
    die "the runner's network policy is not installed: run prepare"
  grep -q "hook input priority filter; policy drop;" <<<"$table" ||
    die "the inbound policy is not installed: run prepare"
  listed=$(grep -o 'tcp dport [^c]*' <<<"$table" | grep -o '[0-9]\+' | sort -un | paste -sd, || true)
  wanted=""
  [ "$INBOUND_ALLOW" = none ] || wanted=$(tr , '\n' <<<"$INBOUND_ALLOW" | sort -un | paste -sd,)
  [ "$listed" = "$wanted" ] || die "the inbound policy does not match MBONGO_INBOUND_ALLOW"
}

prepare() {
  local base=$1
  inbound_ports
  [ "$(stat -f -c %T /sys/fs/cgroup)" = cgroup2fs ] || die "cgroup v2 is required"
  account "$PROVIDER_USER"
  account "$RUNNER_USER"

  install -d -o "$PROVIDER_USER" -g "$PROVIDER_USER" -m 0700 "$base/keys" "$base/state" "$base/artifacts"
  chown -R "$PROVIDER_USER:$PROVIDER_USER" "$base/keys" "$base/state" "$base/artifacts"
  chmod -R go-rwx "$base/keys" "$base/state" "$base/artifacts"
  find "$base/keys" -type f -exec chmod 0600 {} +
  install -d -o "$PROVIDER_USER" -g "$RUNNER_USER" -m 2750 "$base/work"

  # The service sits in a leaf: a cgroup with processes cannot hand controllers
  # down. The provider account owns the rest, so it can create and remove job
  # cgroups and move a new runner into one, and nothing else.
  mkdir -p "$CGROUP/service"
  echo "+memory +pids" >"$CGROUP/cgroup.subtree_control"
  chown "$PROVIDER_USER:$PROVIDER_USER" "$CGROUP" "$CGROUP/cgroup.procs" \
    "$CGROUP/cgroup.threads" "$CGROUP/cgroup.subtree_control"

  network_policy

  echo "RUNNER_UID=$(id -u "$RUNNER_USER")"
  echo "RUNNER_GID=$(id -g "$RUNNER_USER")"
  echo "PROVIDER_UID=$(id -u "$PROVIDER_USER")"
  echo "CGROUP_ROOT=$CGROUP"
}

run() {
  local service=$2 config=$3
  if [ "$RUNNER_NETWORK" = denied ]; then
    inbound_ports
    check_policies
  fi
  echo $$ >"$CGROUP/service/cgroup.procs"
  # Job directories inherit group runner from work/; 027 lets it read them.
  umask 027
  # CAP_SETUID and CAP_SETGID are what it takes to start a runner under another
  # account. The bounding set allows nothing else, and the runner keeps neither.
  exec setpriv --reuid="$PROVIDER_USER" --regid="$PROVIDER_USER" --clear-groups \
    --bounding-set=-all,+setuid,+setgid \
    --inh-caps=-all,+setuid,+setgid \
    --ambient-caps=-all,+setuid,+setgid \
    --no-new-privs \
    "$service" --config "$config"
}

case "${1:-}" in
prepare)
  [ $# = 2 ] || die "usage: $0 prepare <base>"
  prepare "$2"
  ;;
run)
  [ $# = 4 ] || die "usage: $0 run <base> <service> <config>"
  run "$2" "$3" "$4"
  ;;
*)
  die "usage: $0 prepare|run ..."
  ;;
esac
