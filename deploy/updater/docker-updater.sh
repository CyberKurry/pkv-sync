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
PREV_TAG_FILE="$DATA_DIR/upgrade-previous-tag"
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

# Record the tag we are upgrading FROM so we can roll back.
PREV_TAG="$(cat "$PREV_TAG_FILE" 2>/dev/null || echo "${PKV_SYNC_TAG:-latest}")"
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
