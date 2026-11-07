#!/bin/sh
# PKV Sync systemd updater (opt-in, runs as root via pkv-sync-updater.service).
# Applies an upgrade requested by the admin UI: the unprivileged server writes
# <data_dir>/upgrade-request.json; this script stages + verifies the new binary
# (reusing `pkvsyncd upgrade`), swaps it atomically, restarts the service, and
# rolls back to the previous binary if the new one fails its health check.
# Every outcome is recorded in <data_dir>/upgrade-status.json (2026-11-07
# report: the admin panel must be able to tell a dropped click from an
# applied/refused/failed one), and requests older than PKV_UPDATER_MAX_AGE
# seconds are refused as stale instead of applied against a moved-on world.
set -eu

DATA_DIR="${PKV_DATA_DIR:-/var/lib/pkv-sync}"
MARKER="$DATA_DIR/upgrade-request.json"
STATUS="$DATA_DIR/upgrade-status.json"
MAX_AGE="${PKV_UPDATER_MAX_AGE:-1800}"
BIN="${PKV_BIN:-/usr/local/bin/pkvsyncd}"
HEALTH_URL="${PKV_HEALTH_URL:-http://127.0.0.1:6710/api/health}"
SERVICE="${PKV_SERVICE:-pkv-sync}"

write_status() {
  # $1 phase, $2 from, $3 target, $4 reason
  now=$(date +%s)
  esc=$(printf '%s' "$4" | tr -d '\r\n' | sed 's/\\/\\\\/g; s/"/\\"/g')
  printf '{\n  "phase": "%s",\n  "from": "%s",\n  "target": "%s",\n  "at_unix": %s,\n  "reason": "%s"\n}\n' \
    "$1" "$2" "$3" "$now" "$esc" > "$STATUS.tmp" \
    && mv -f "$STATUS.tmp" "$STATUS" \
    || echo "pkv-sync-update: warning: could not write $STATUS" >&2
}

[ -f "$MARKER" ] || exit 0

TARGET="$(sed -n 's/.*"target_version"[[:space:]]*:[[:space:]]*"\([0-9.]*\)".*/\1/p' "$MARKER" | head -n1)"
REQ_AT="$(sed -n 's/.*"requested_at_unix"[[:space:]]*:[[:space:]]*\([0-9]*\).*/\1/p' "$MARKER" | head -n1)"
if [ -z "$TARGET" ]; then
  echo "pkv-sync-update: no target_version in $MARKER; clearing"
  write_status refused "" "" "no target_version in marker"
  rm -f "$MARKER"
  exit 1
fi

# Stale request guard: see the Docker updater's note; same semantics here.
now=$(date +%s)
age=$(( now - ${REQ_AT:-0} ))
[ "$age" -lt 0 ] && age=0
if [ "$MAX_AGE" -gt 0 ] && [ "$age" -gt "$MAX_AGE" ]; then
  echo "pkv-sync-update: refusing stale request (age ${age}s > ${MAX_AGE}s); clearing"
  write_status refused "" "$TARGET" "request older than ${MAX_AGE}s"
  rm -f "$MARKER"
  exit 1
fi

# Refuse downgrades/same-version reinstalls. The marker lives in a directory
# the unprivileged pkv-sync user can write, so the privileged side must not
# trust it: an attacker with app-level code execution could otherwise have
# root install a known-vulnerable old release.
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
CUR="$("$BIN" --version 2>/dev/null | awk '{print $2}')"
if [ -n "$CUR" ] && version_lte "$TARGET" "$CUR"; then
  echo "pkv-sync-update: refusing non-upgrade $CUR -> $TARGET; clearing marker"
  write_status refused "$CUR" "$TARGET" "non-upgrade target"
  rm -f "$MARKER"
  exit 1
fi

write_status in_progress "$CUR" "$TARGET" ""

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
  write_status failed "$CUR" "$TARGET" "health check failed; rolled back"
  install -m 0755 "$BIN.old" "$BIN"
  systemctl restart "$SERVICE"
  rm -f "$MARKER"
  exit 1
fi

rm -f "$BIN.old" "$MARKER"
write_status success "$CUR" "$TARGET" ""
echo "pkv-sync-update: upgraded to $TARGET"
