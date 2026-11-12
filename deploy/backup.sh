#!/bin/sh
# Example backup job for the host, not part of the containers: one pg_dump
# through the running `db` service into BACKUP_DIR, with rotation. Use it
# from cron if you have no backup tool of your own, e.g.
#
#   sudo install -m 700 backup.sh /opt/docker/autokarta/backup.sh
#   sudo crontab -e
#   30 3 * * * /opt/docker/autokarta/backup.sh >> /var/log/autokarta-backup.log 2>&1
#
# Exits non-zero when the dump failed, so cron mail or monitoring notices.
# Dumps contain personal data: they are created readable by root only.
# Local dumps do not survive losing the server: copy them off-site as well.
set -eu
umask 077

COMPOSE_DIR=${COMPOSE_DIR:-/opt/docker/autokarta}
BACKUP_DIR=${BACKUP_DIR:-$COMPOSE_DIR/backups}
KEEP_DAYS=${KEEP_DAYS:-14}

case $KEEP_DAYS in
'' | *[!0-9]* | 0)
	echo "backup: KEEP_DAYS must be a whole number of days, at least 1" >&2
	exit 2
	;;
esac

mkdir -p "$BACKUP_DIR"
stamp=$(date -u +%Y%m%d-%H%M%S)
tmp="$BACKUP_DIR/.autokarta-$stamp.dump.partial"
out="$BACKUP_DIR/autokarta-$stamp.dump"
# A failed or interrupted dump leaves nothing that looks like a backup.
trap 'rm -f "$tmp"' EXIT
trap 'exit 1' INT TERM

cd "$COMPOSE_DIR"
# pg_dump inside the db container always matches the server version.
docker compose exec -T db sh -c \
	'pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" --format=custom --compress=9' >"$tmp"
# A dump that pg_restore cannot read is not a backup.
docker compose exec -T db pg_restore --list <"$tmp" >/dev/null
mv "$tmp" "$out"
echo "backup: wrote $out ($(du -h "$out" | cut -f1))"

# -mtime +N matches files at least N+1 whole days old.
find "$BACKUP_DIR" -name 'autokarta-*.dump' -type f -mtime "+$((KEEP_DAYS - 1))" -print -delete
# Leftovers of a run killed hard (power loss), where the trap never ran.
find "$BACKUP_DIR" -name '.autokarta-*.dump.partial' -type f -mmin +1440 -delete
