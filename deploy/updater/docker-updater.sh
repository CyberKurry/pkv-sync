#!/bin/sh
# PKV Sync Docker updater (opt-in, runs in the `updater` compose profile).
# Applies an upgrade requested by the admin UI for Docker deployments: the
# unprivileged pkv-sync container writes <data_dir>/upgrade-request.json; this
# sidecar pulls the requested pinned image, recreates the pkv-sync service,
# health-checks it, and re-pins the previous tag if the new one is unhealthy.
#
# Reaches Docker ONLY through the scoped docker-socket-proxy (DOCKER_HOST);
# the pkv-sync container itself never receives the socket.
#
# NOTE: runtime behavior must be validated on a Linux + Docker host (see the
# plan's integration check); it cannot be exercised on a host without Docker.
set -eu

DATA_DIR="${PKV_DATA_DIR:-/var/lib/pkv-sync}"
MARKER="$DATA_DIR/upgrade-request.json"
# Container-local (writable, ephemeral) state: the data dir is mounted :ro, and
# a data-dir location would let the unprivileged server user pre-seed the
# rollback target tag.
PREV_TAG_FILE="/tmp/pkv-sync-upgrade-previous-tag"
HEALTH_URL="${PKV_HEALTH_URL:-http://pkv-sync:6710/api/health}"
SERVICE="${PKV_TARGET_SERVICE:-pkv-sync}"
# Compose files (mounted read-only into the updater) used to recreate the service.
COMPOSE_FILE="${PKV_COMPOSE_FILE:-/compose/docker-compose.yml}"
COMPOSE_UPDATER_FILE="${PKV_COMPOSE_UPDATER_FILE:-/compose/deploy/updater/compose.updater.yml}"

compose() {
  docker compose -f "$COMPOSE_FILE" -f "$COMPOSE_UPDATER_FILE" "$@"
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
RUNNING_IMAGE="$(docker ps --filter "name=$SERVICE" --format '{{.Image}}' 2>/dev/null | head -n1)"
CUR_TAG="${RUNNING_IMAGE##*:}"
if echo "$CUR_TAG" | grep -Eq '^[0-9]+(\.[0-9]+)+$' && version_lte "$TARGET" "$CUR_TAG"; then
  echo "docker-updater: refusing non-upgrade $CUR_TAG -> $TARGET; clearing marker"
  rm -f "$MARKER"
  exit 1
fi

# Record the tag we are upgrading FROM so we can roll back. Only accept an
# existing record when it is a strict dotted version; otherwise fall back to
# the deployment default.
PREV_TAG="$(sed -n 's/^\([0-9][0-9.]*\)$/\1/p' "$PREV_TAG_FILE" 2>/dev/null | head -n1)"
[ -n "$PREV_TAG" ] || PREV_TAG="${PKV_SYNC_TAG:-latest}"
echo "$PREV_TAG" >"$PREV_TAG_FILE"

# Pull the requested pinned image and recreate just the pkv-sync service.
export PKV_SYNC_TAG="$TARGET"
compose pull "$SERVICE"
compose up -d --no-deps "$SERVICE"

# Health window: poll up to ~60s for the recreated service to report ready.
ok=0
i=0
while [ "$i" -lt 30 ]; do
  if curl -fsS "$HEALTH_URL" >/dev/null 2>&1; then
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

rm -f "$PREV_TAG_FILE" "$MARKER"
echo "docker-updater: upgraded $SERVICE to $TARGET"
