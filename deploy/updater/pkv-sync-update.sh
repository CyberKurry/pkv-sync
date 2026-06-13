#!/bin/sh
# PKV Sync systemd updater (opt-in, runs as root via pkv-sync-updater.service).
# Applies an upgrade requested by the admin UI: the unprivileged server writes
# <data_dir>/upgrade-request.json; this script stages + verifies the new binary
# (reusing `pkvsyncd upgrade`), swaps it atomically, restarts the service, and
# rolls back to the previous binary if the new one fails its health check.
set -eu

DATA_DIR="${PKV_DATA_DIR:-/var/lib/pkv-sync}"
MARKER="$DATA_DIR/upgrade-request.json"
BIN="${PKV_BIN:-/usr/local/bin/pkvsyncd}"
HEALTH_URL="${PKV_HEALTH_URL:-http://127.0.0.1:6710/api/health}"
SERVICE="${PKV_SERVICE:-pkv-sync}"

[ -f "$MARKER" ] || exit 0

TARGET="$(sed -n 's/.*"target_version"[[:space:]]*:[[:space:]]*"\([0-9.]*\)".*/\1/p' "$MARKER" | head -n1)"
if [ -z "$TARGET" ]; then
  echo "pkv-sync-update: no target_version in $MARKER; clearing"
  rm -f "$MARKER"
  exit 1
fi

# Stage + verify via the existing CLI (writes <bin>.new, SHA256-checked).
"$BIN" upgrade --yes --version "$TARGET"
NEW="$BIN.new"
if [ ! -f "$NEW" ]; then
  echo "pkv-sync-update: staged binary $NEW missing; clearing"
  rm -f "$MARKER"
  exit 1
fi

cp -f "$BIN" "$BIN.old"
install -m 0755 "$NEW" "$BIN"
rm -f "$NEW"
systemctl restart "$SERVICE"

# Health window: poll up to ~60s for the restarted server to report ready.
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
  echo "pkv-sync-update: health check failed; rolling back to $BIN.old"
  install -m 0755 "$BIN.old" "$BIN"
  systemctl restart "$SERVICE"
  rm -f "$MARKER"
  exit 1
fi

rm -f "$BIN.old" "$MARKER"
echo "pkv-sync-update: upgraded to $TARGET"
