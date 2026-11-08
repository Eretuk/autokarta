# Autokarta

Workshop, parts and fleet management for small auto services.

Autokarta keeps the full history of every vehicle by VIN: what was repaired,
which parts were used, at what mileage. It tracks spare parts in stock
(with barcode scanning from a phone camera), rental and taxi cars, and cars
for sale. Customers can see the service history of their car through a
private link, without creating an account.

> Status: early development, first beta tester is a real auto service.

## Planned features

- Vehicles by VIN or frame number, mileage history
- Work orders with parts and labour
- Parts stock with barcode / QR scanning, low-stock alerts
- Maintenance reminders by mileage and time, Telegram notifications
- Rentals (taxi and regular) and cars for sale
- Share link per vehicle for customers
- Installable web app (PWA), English, Polish and Russian

## Development

Requirements: Rust (stable), Node.js 22+, Docker.

```bash
cp .env.example .env                              # local dev values
docker compose -f docker-compose.dev.yml up -d   # PostgreSQL
cd backend
cargo run -- create-admin --username owner        # first owner account
cargo run                                         # API on :8080
```

Frontend (second terminal):

```bash
cd frontend
npm install
npm run dev            # http://localhost:5173, proxies /api to :8080
```

## Production

One server with Docker. The app must be reached over HTTPS (session cookies
are `Secure`, and the camera scanner only works over HTTPS), so put a reverse
proxy in front of it.

CI builds the image and pushes it to GitHub Container Registry on every
commit to `main` (tags `latest` and `sha-<commit>`) and on version tags
`v1.2.3` (tags `1.2.3` and `1.2`): `ghcr.io/eretuk/autokarta`. If the package
is private, log in on the server once with a personal access token that has
`read:packages`: `echo <token> | docker login ghcr.io -u <github-user> --password-stdin`.

The server needs only `docker-compose.yml`, `deploy/backup.sh` and `.env`:

```bash
cp .env.example .env     # set POSTGRES_PASSWORD (long random) and AUTOKARTA_IMAGE
docker compose pull
docker compose up -d
docker compose exec app autokarta create-admin --username owner
```

Update to a new image: `docker compose pull app && docker compose up -d app`
(migrations run on startup). To build on the server instead of pulling:
`docker compose up -d --build`.

The reverse proxy (nginx, Traefik, Caddy, ...) only terminates TLS and
forwards the domain to `APP_BIND` (default `127.0.0.1:8080`). The app sets its
own security headers. For example, with Caddy:

```
autokarta.example.com {
	reverse_proxy 127.0.0.1:8080
}
```

PostgreSQL is version 18. Its data lives in the `postgres` volume, mounted at
`/var/lib/postgresql` (the image keeps each major version in its own
subdirectory). A major upgrade needs a dump and restore (or `pg_upgrade`).

### Backups

The `backup` service writes a `pg_dump` to `./backups` every day
(`BACKUP_TIME`, container time is UTC) and keeps `BACKUP_KEEP_DAYS` days.
Run one immediately: `docker compose run --rm backup now`.

Local dumps do not survive losing the server: **copy `./backups` off-site**
(e.g. a nightly `rclone sync` or `restic` from the host) before entering
real data. Restore:

```bash
docker compose exec -T db pg_restore -U autokarta -d autokarta --clean --if-exists < backups/autokarta-YYYYMMDD-HHMMSS.dump
```

## License

Free for small businesses under the
[PolyForm Small Business License 1.0.0](LICENSE.md).
See [LICENSING.md](LICENSING.md) for details and commercial licensing.
