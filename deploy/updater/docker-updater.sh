#!/bin/sh
# PKV Sync Docker updater (opt-in, runs in the `updater` compose profile).
# Applies an upgrade requested by the admin UI for Docker deployments: the
# unprivileged pkv-sync container writes <data_dir>/upgrade-request.json; this
# sidecar pulls the requested pinned image, recreates the pkv-sync service,
# health-checks it, and re-pins the running tag if the new one is unhealthy.
#
# Reaches Docker ONLY through the scoped docker-socket-proxy (DOCKER_HOST);
# the pkv-sync container itself never receives the socket.
#
# TRIGGER MODEL (2026-11-07 report): Docker has no "file appears" primitive
# (no PathExists= equivalent), so a `restart: "no"` oneshot fired once by
# `compose up` consumed at most ONE request and every later click was
# silently dropped. This script therefore STAYS RESIDENT and polls for the
# marker. Idle cost is one stat + sleep per interval (~3s CPU/day at 30s);
# no Docker API call happens unless a marker exists. The systemd deployment
# gets the equivalent via its .path unit; the two paths are now equivalent.
#
# EXIT CODES (0/2/3 — refusing is not failing):
#   0  idle loop or a legitimately refused request (container keeps running)
#   2  configuration error (fix the deployment; restarting cannot help)
#   3  an upgrade attempt failed and was rolled back (one-shot mode only)
# The resident loop continues after refusals and failures; only exit 2
# terminates it (configuration errors cannot be retried into success).
#
# Deployment coordinates come from the environment (set by
# compose.updater.yml from the host-side invocation): the absolute host
# directory of the deployment and the compose project name. This container's
# own filesystem layout (/compose) is NOT the deployment's — rediscovering
# paths or the project from it provisions a duplicate stack instead of
# upgrading the running one (2026-11-01 audit, defects 1+2).
#
# NOTE: runtime behavior must be validated on a Linux + Docker host; it
# cannot be exercised on a host without Docker.
set -eu

DATA_DIR="${PKV_DATA_DIR:-/var/lib/pkv-sync}"
MARKER="$DATA_DIR/upgrade-request.json"
STATUS="$DATA_DIR/upgrade-status.json"
HEALTH_URL="${PKV_HEALTH_URL:-http://pkv-sync:6710/api/health}"
SERVICE="${PKV_TARGET_SERVICE:-pkv-sync}"
# Marker poll interval in seconds (idle cost: one stat per interval).
POLL="${PKV_UPDATER_POLL:-30}"
# A request older than this many seconds is stale and refused, not applied:
# the admin clicked a while ago and the world may have moved on (the field
# was written since 1.4.0 but never validated — 2026-11-07 report, defect 3).
MAX_AGE="${PKV_UPDATER_MAX_AGE:-1800}"
# Compose files (mounted read-only into the updater) used to recreate the service.
COMPOSE_FILE="${PKV_COMPOSE_FILE:-/compose/docker-compose.yml}"
COMPOSE_UPDATER_FILE="${PKV_COMPOSE_UPDATER_FILE:-/compose/deploy/updater/compose.updater.yml}"
# Absolute host path of the deployment directory. All relative bind sources
# in the compose files resolve against the DOCKER DAEMON's view, so the
# updater must export the deployment's own directory; the compose overlay
# requires this variable and passes it through.
HOST_DIR="${PKV_HOST_DIR:-}"
if [ -z "$HOST_DIR" ]; then
  echo "docker-updater: PKV_HOST_DIR is not set (pass the absolute deployment directory); aborting without touching anything" >&2
  exit 2
fi
case "$HOST_DIR" in
  /*) ;;
  *) echo "docker-updater: PKV_HOST_DIR must be absolute (got '$HOST_DIR'); aborting" >&2; exit 2 ;;
esac
case "$POLL" in
  ''|*[!0-9]*) echo "docker-updater: PKV_UPDATER_POLL must be a positive integer (got '$POLL'); aborting" >&2; exit 2 ;;
esac
[ "$POLL" -ge 1 ] || { echo "docker-updater: PKV_UPDATER_POLL must be >= 1s; aborting" >&2; exit 2; }
case "$MAX_AGE" in
  ''|*[!0-9]*) echo "docker-updater: PKV_UPDATER_MAX_AGE must be a non-negative integer (got '$MAX_AGE'); aborting" >&2; exit 2 ;;
esac

compose() {
  # The project name and the deployment's host directory are pinned: compose
  # must reconfigure the RUNNING deployment, not derive a second project from
  # this container's /compose view (which also resolves every relative bind
  # against the wrong root — the daemon then creates empty directories where
  # files belong).
  COMPOSE_PROJECT_NAME="${COMPOSE_PROJECT_NAME:-pkv-sync}" \
  PKV_HOST_DIR="$HOST_DIR" \
  docker compose -f "$COMPOSE_FILE" -f "$COMPOSE_UPDATER_FILE" "$@"
}

# Exact-service lookup for the running container's image. `docker ps
# --filter name=X` is a SUBSTRING match: with this profile up, "pkv-sync"
# also matches pkv-sync-socket-proxy-1 and pkv-sync-updater-1, and taking
# the first row returned the proxy's image — silently disabling the
# downgrade guard. Compose names containers <project>-<service>-<n>, so
# match that full shape for the target service.
running_image() {
  proj="${COMPOSE_PROJECT_NAME:-pkv-sync}"
  # NOTE: the field separator is a plain "|" — docker's Go template takes
  # it literally, and a backslash-escaped "\|" inside the format string is
  # emitted AS a backslash, so the awk split never matches (verified on the
  # docker:27.5.1-cli BusyBox sh).
  docker ps --format '{{.Names}}|{{.Image}}' 2>/dev/null \
    | awk -F'|' -v re="^${proj}-${SERVICE}-[0-9]+$" '$1 ~ re {print $2; exit}'
}

# Strip an image reference down to a comparable version: drop the registry
# and repository, take the tag, tolerate the published "v" prefix.
version_of() {
  t="${1##*:}"
  printf '%s' "$t" | sed 's/^v//'
}

# Refuse downgrades/same-version reinstalls. The marker lives in a directory
# the unprivileged pkv-sync user can write, so the privileged side must not
# trust it: an attacker with app-level code execution could otherwise have
# this updater pin a known-vulnerable old release tag.
version_lte() {
  awk -v a="$1" -v b="$2" 'BEGIN {
    n = split(a, x, "."); m = split(b, y, ".")
    if (n > m) m = n
    for (i = 1; i <= m; i++) {
      if ((x[i] + 0) < (y[i] + 0)) exit 0
      if ((x[i] + 0) > (y[i] + 0)) exit 1
    }
    exit 0
  }'
}

# Record an outcome next to the (removed) marker so /admin can tell a
# dropped click from an applied/refused/failed one. JSON is emitted with
# printf: the docker:*-cli image has no jq, and hand-rolled quoting keeps
# values (versions, reasons) safely inside one JSON string each.
json_escape() {
  # Single-line, script-generated reasons only: escape backslashes and
  # double quotes, drop any stray newlines. No awk acrobatics needed.
  printf '%s' "$1" | tr -d '\r\n' | sed 's/\\/\\\\/g; s/"/\\"/g'
}
write_status() {
  # $1 phase, $2 from, $3 target, $4 reason ("" allowed)
  now=$(date +%s)
  esc_reason=$(json_escape "$4")
  printf '{\n  "phase": "%s",\n  "from": "%s",\n  "target": "%s",\n  "at_unix": %s,\n  "reason": "%s"\n}\n' \
    "$1" "$2" "$3" "$now" "$esc_reason" > "$STATUS.tmp" \
    && mv -f "$STATUS.tmp" "$STATUS" \
    || echo "docker-updater: warning: could not write $STATUS" >&2
}

# Process one marker occurrence. Never exits; echoes a process outcome:
#   applied | refused | failed
process_marker() {
  TARGET="$(sed -n 's/.*"target_version"[[:space:]]*:[[:space:]]*"\([0-9.]*\)".*/\1/p' "$MARKER" | head -n1)"
  REQ_AT="$(sed -n 's/.*"requested_at_unix"[[:space:]]*:[[:space:]]*\([0-9]*\).*/\1/p' "$MARKER" | head -n1)"
  if [ -z "$TARGET" ]; then
    echo "docker-updater: no target_version in $MARKER; clearing" >&2
    write_status refused "" "" "no target_version in marker"
    rm -f "$MARKER"
    echo refused
    return
  fi

  # Stale request guard (2026-11-07 report, defect 3): requested_at_unix has
  # been written since 1.4.0 but never validated. An old marker must not be
  # applied against a world that has moved on. A missing/zero timestamp is
  # treated as stale-safe=unknown: age 0 keeps pre-1.8.1 markers applicable.
  now=$(date +%s)
  age=$(( now - ${REQ_AT:-0} ))
  [ "$age" -lt 0 ] && age=0
  if [ "$MAX_AGE" -gt 0 ] && [ "$age" -gt "$MAX_AGE" ]; then
    echo "docker-updater: refusing stale request (age ${age}s > ${MAX_AGE}s); clearing" >&2
    write_status refused "" "$TARGET" "request older than ${MAX_AGE}s"
    rm -f "$MARKER"
    echo refused
    return
  fi

  # The rollback target is the image the service is RUNNING right now —
  # captured before anything changes it.
  CUR_TAG="$(running_image | { read -r img || true; [ -n "$img" ] && version_of "$img" || true; })"
  PREV_TAG="$CUR_TAG"
  if [ -z "$PREV_TAG" ]; then
    echo "docker-updater: cannot determine the running image for service '$SERVICE'; keeping marker for retry" >&2
    write_status refused "" "$TARGET" "cannot determine running image"
    echo refused
    return
  fi
  if echo "$CUR_TAG" | grep -Eq '^[0-9]+(\.[0-9]+)+$' && version_lte "$TARGET" "$CUR_TAG"; then
    echo "docker-updater: refusing non-upgrade $CUR_TAG -> $TARGET; clearing" >&2
    write_status refused "$CUR_TAG" "$TARGET" "non-upgrade target"
    rm -f "$MARKER"
    echo refused
    return
  fi

  write_status in_progress "$PREV_TAG" "$TARGET" ""
  echo "docker-updater: applying upgrade $PREV_TAG -> $TARGET" >&2

  # Pull the requested pinned image and recreate just the pkv-sync service.
  if ! export PKV_SYNC_TAG="$TARGET"; then :; fi
  if ! compose pull "$SERVICE"; then
    echo "docker-updater: pull failed for $TARGET; keeping marker for retry" >&2
    write_status failed "$PREV_TAG" "$TARGET" "pull failed"
    echo failed
    return
  fi
  if ! compose up -d --no-deps "$SERVICE"; then
    echo "docker-updater: recreate failed for $TARGET; rolling back tag to $PREV_TAG" >&2
    write_status failed "$PREV_TAG" "$TARGET" "recreate failed"
    export PKV_SYNC_TAG="$PREV_TAG"
    compose up -d --no-deps "$SERVICE" >/dev/null 2>&1 || true
    rm -f "$MARKER"
    echo failed
    return
  fi

  # Health window: poll up to ~60s for the recreated service to report ready.
  # wget, not curl: the docker:*-cli image ships busybox wget but NO curl.
  ok=0
  i=0
  while [ "$i" -lt 30 ]; do
    if wget -q -T 3 -O /dev/null "$HEALTH_URL" 2>/dev/null; then
      ok=1
      break
    fi
    i=$((i + 1))
    sleep 2
  done

  if [ "$ok" -ne 1 ]; then
    echo "docker-updater: health check failed; rolling back to $PREV_TAG" >&2
    write_status failed "$PREV_TAG" "$TARGET" "health check failed; rolled back"
    export PKV_SYNC_TAG="$PREV_TAG"
    compose up -d --no-deps "$SERVICE"
    rm -f "$MARKER"
    echo failed
    return
  fi

  rm -f "$MARKER"
  write_status success "$PREV_TAG" "$TARGET" ""
  echo "docker-updater: upgraded $SERVICE to $TARGET (was $PREV_TAG)" >&2
  echo applied
}

# One-shot mode (tests, manual runs): process at most one marker and exit
# with the documented code. The resident loop below is the default.
if [ "${PKV_UPDATER_ONCE:-0}" = "1" ]; then
  if [ ! -f "$MARKER" ]; then
    exit 0
  fi
  OUTCOME="$(process_marker || echo failed)"
  case "$OUTCOME" in
    applied|refused) exit 0 ;;
    *) exit 3 ;;
  esac
fi

# Resident loop: poll for the marker, process it, keep watching. A failure
# inside process_marker records a status and returns to the loop rather
# than killing the watcher — the next click still works (the defect this
# fixes was exactly a watcher that died after its first run).
while :; do
  if [ -f "$MARKER" ]; then
    process_marker || true
  fi
  sleep "$POLL"
done
