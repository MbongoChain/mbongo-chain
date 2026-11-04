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
# Environment: MBONGO_PROVIDER_USER, MBONGO_RUNNER_USER, MBONGO_CGROUP.
set -euo pipefail

PROVIDER_USER=${MBONGO_PROVIDER_USER:-mbongo-provider}
RUNNER_USER=${MBONGO_RUNNER_USER:-mbongo-runner}
CGROUP=${MBONGO_CGROUP:-/sys/fs/cgroup/mbongo-provider}

die() {
  echo "error: $*" >&2
  exit 1
}

[ "$(uname -s)" = Linux ] || die "Linux only"
[ "$(id -u)" = 0 ] || die "run as root"

account() {
  id -u "$1" >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin "$1"
}

prepare() {
  local base=$1
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

  echo "RUNNER_UID=$(id -u "$RUNNER_USER")"
  echo "RUNNER_GID=$(id -g "$RUNNER_USER")"
  echo "PROVIDER_UID=$(id -u "$PROVIDER_USER")"
  echo "CGROUP_ROOT=$CGROUP"
}

run() {
  local service=$2 config=$3
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
