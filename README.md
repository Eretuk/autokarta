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

One server with Docker. DNS of your domain must point to it, ports 80 and
443 open. Caddy obtains the HTTPS certificate automatically (the camera
scanner only works over HTTPS).

```bash
cp .env.example .env     # set POSTGRES_PASSWORD (long random) and DOMAIN
docker compose up -d --build
docker compose exec app autokarta create-admin --username owner
```

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
