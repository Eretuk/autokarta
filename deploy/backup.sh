#!/bin/sh
# Daily pg_dump with rotation. Runs inside the postgres image (see docker-compose.yml).
#
# Dumps land in ./backups on the host. This protects against mistakes and a
# broken database, NOT against losing the server: copy ./backups off-site too
# (see README, "Backups").
set -eu

: "${BACKUP_KEEP_DAYS:=14}"
: "${BACKUP_TIME:=03:30}"

backup() {
	stamp=$(date +%Y%m%d-%H%M%S)
	tmp="/backups/.autokarta-$stamp.dump.partial"
	out="/backups/autokarta-$stamp.dump"
	if pg_dump --format=custom --compress=9 --file="$tmp"; then
		mv "$tmp" "$out"
		echo "backup: wrote $out ($(du -h "$out" | cut -f1))"
		find /backups -name 'autokarta-*.dump' -type f -mtime "+$BACKUP_KEEP_DAYS" -print -delete
	else
		rm -f "$tmp"
		echo "backup: pg_dump FAILED" >&2
	fi
}

seconds_until() {
	now=$(date +%s)
	target=$(date -d "$(date +%Y-%m-%d) $1" +%s 2>/dev/null || date -D '%Y-%m-%d %H:%M' -d "$(date +%Y-%m-%d) $1" +%s)
	[ "$target" -le "$now" ] && target=$((target + 86400))
	echo $((target - now))
}

# "now" runs a single backup and exits: docker compose run --rm backup now
if [ "${1:-}" = "now" ]; then
	backup
	exit 0
fi

echo "backup: daily at $BACKUP_TIME, keeping $BACKUP_KEEP_DAYS days"
while true; do
	sleep "$(seconds_until "$BACKUP_TIME")"
	backup
done
