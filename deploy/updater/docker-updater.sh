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
# Deployment coordinates come from the environment (set by
# compose.updater.yml from the host-side invocation): the absolute host
# directory of the deployment and the compose project name. This container's
# own filesystem layout (/compose) is NOT the deployment's — rediscovering
# paths or the project from it provisions a duplicate stack instead of
# upgrading the running one (2026-11-01 audit, defects 1+2).
#
# NOTE: runtime behavior must be validated on a Linux + Docker host (see the
# plan's integration check); it cannot be exercised on a host without Docker.
set -eu

DATA_DIR="${PKV_DATA_DIR:-/var/lib/pkv-sync}"
MARKER="$DATA_DIR/upgrade-request.json"
HEALTH_URL="${PKV_HEALTH_URL:-http://pkv-sync:6710/api/health}"
SERVICE="${PKV_TARGET_SERVICE:-pkv-sync}"
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
  exit 1
fi
case "$HOST_DIR" in
  /*) ;;
  *) echo "docker-updater: PKV_HOST_DIR must be absolute (got '$HOST_DIR'); aborting" >&2; exit 1 ;;
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
# match that full shape for the target service (the report's bare
# `$1 == service` never matches: the container NAME carries the project
# prefix and index).
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
#   ghcr.io/x/y:1.7.1 -> 1.7.1     ghcr.io/x/y:v1.7.1 -> 1.7.1
version_of() {
  t="${1##*:}"
  printf '%s' "$t" | sed 's/^v//'
}

[ -f "$MARKER" ] || exit 0

TARGET="$(sed -n 's/.*"target_version"[[:space:]]*:[[:space:]]*"\([0-9.]*\)".*/\1/p' "$MARKER" | head -n1)"
if [ -z "$TARGET" ]; then
  echo "docker-updater: no target_version in $MARKER; clearing"
  rm -f "$MARKER"
  exit 1
fi

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
# The rollback target is the image the service is RUNNING right now —
# captured before anything changes it. The previous code re-derived it from
# PKV_SYNC_TAG (unset here, so "latest"), which could roll "back" to a
# NEWER image than the one that just failed its health check.
CUR_TAG="$(running_image | { read -r img || true; [ -n "$img" ] && version_of "$img" || true; })"
PREV_TAG="$CUR_TAG"
if echo "$CUR_TAG" | grep -Eq '^[0-9]+(\.[0-9]+)+$' && version_lte "$TARGET" "$CUR_TAG"; then
  echo "docker-updater: refusing non-upgrade $CUR_TAG -> $TARGET; clearing marker"
  rm -f "$MARKER"
  exit 1
fi
if [ -z "$PREV_TAG" ]; then
  echo "docker-updater: cannot determine the running image for service '$SERVICE'; aborting without touching anything" >&2
  exit 1
fi

# Pull the requested pinned image and recreate just the pkv-sync service.
export PKV_SYNC_TAG="$TARGET"
compose pull "$SERVICE"
compose up -d --no-deps "$SERVICE"

# Health window: poll up to ~60s for the recreated service to report ready.
# wget, not curl: the docker:*-cli image ships busybox wget but NO curl, so
# the previous curl invocation always failed with 127 (command not found) —
# the health check could never succeed and every upgrade was rolled back
# (found in the 2026-11-01 end-to-end pass; earlier repros never reached
# this line because they died at port binding first).
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
  echo "docker-updater: health check failed; rolling back to $PREV_TAG"
  export PKV_SYNC_TAG="$PREV_TAG"
  compose up -d --no-deps "$SERVICE"
  rm -f "$MARKER"
  exit 1
fi

rm -f "$MARKER"
echo "docker-updater: upgraded $SERVICE to $TARGET (was $PREV_TAG)"
