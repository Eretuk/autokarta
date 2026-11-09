# Autokarta

Workshop, parts and fleet management for small auto services.

Autokarta keeps the full history of every vehicle by VIN: what was repaired,
which parts were used, at what mileage. It tracks spare parts in stock
(with barcode scanning from a phone camera), rental and taxi cars, and cars
for sale. Customers can see the service history of their car through a
private link, without creating an account.

> [!WARNING]
> **Work in progress.** Autokarta is in early development and is being tried
> by its first beta tester, a real auto service. Expect missing features,
> rough edges and breaking changes between versions. Keep backups and do not
> rely on it as your only record yet. Feedback is welcome in
> [Issues](https://github.com/Eretuk/autokarta/issues).

## What works now

- Staff accounts (owner and mechanic), English, Polish and Russian
- Vehicles by plate, VIN or frame number, owners and mileage history
- Work orders with labour and parts, printable on A4 (prices for owners only)
- Parts stock: barcode and QR scanning from the phone camera, deliveries,
  stock counts, low-stock list, new and used parts
- Own QR labels for parts, printed on A4 sticker sheets
- Installable web app (PWA) for phones

## Planned

- Share link per vehicle for customers
- Maintenance reminders by mileage and time, Telegram notifications
- Rentals (taxi and regular) and cars for sale

## Development

Requirements: Git, Rust (stable), Node.js 22+, Docker with Docker Compose v2.

```bash
git clone https://github.com/Eretuk/autokarta.git && cd autokarta
cp .env.example .env                              # local dev values, work as they are
docker compose -f docker-compose.dev.yml up -d   # PostgreSQL 18 on localhost:5432
cd backend
cargo run -- create-admin --username owner        # asks for a password (min. 8 characters)
cargo run                                         # API on http://localhost:8080
```

Frontend (second terminal):

```bash
cd frontend
npm install
npm run dev            # http://localhost:5173, proxies /api to :8080
```

Open http://localhost:5173 and sign in as `owner`. The backend finds `.env`
in the repository root by itself.

Checks before a commit (CI runs the same):

```bash
cd backend && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd frontend && npm run check && npm run build
```

`cargo test` needs the dev database running: each API test creates its own
throwaway database there.

Releases: tag a commit on `main` with `v<major>.<minor>.<patch>` (the
version in `backend/Cargo.toml`) and push the tag; CI publishes the image
with that version.

### Testing on a phone

The camera scanner needs HTTPS (only `localhost` is exempt). To open the dev
app from a phone in the same Wi-Fi:

1. Put this computer's LAN address into `LAN_HOST` in `.env` (e.g. `192.168.1.10`).
2. `docker compose -f docker-compose.dev.yml --profile https up -d` (Caddy with a local certificate).
3. Run the frontend with `npm run dev -- --host`.
4. Open `https://<LAN_HOST>:8443` on the phone and accept the certificate warning once.

## Production

### What you need

- A Linux server (x86-64 / amd64) with Docker Engine and the Docker Compose v2
  plugin (`docker compose version` must work).
- **An HTTPS address.** Sign-in cookies are `Secure` and the camera works only
  over HTTPS, so opening `http://<server-ip>:8080` will **not** let anyone sign
  in (the login page says so). Use one of:
  - your domain plus a reverse proxy (Caddy, nginx, Traefik, ...) with a
    certificate, see step 6;
  - for a quick test without a domain or open ports:
    [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/)
    or [Tailscale Funnel](https://tailscale.com/kb/1223/funnel), pointed at
    `http://127.0.0.1:8080`.

The image `ghcr.io/eretuk/autokarta` is built by CI on every commit to `main`
(tags `latest`, `sha-<commit>`) and on version tags `v1.2.3` (tags `1.2.3`,
`1.2`). It is built for amd64 only; on an ARM machine build it yourself (see
"Update").

### Install

1. Download three files into a new directory (paths matter):

   ```bash
   mkdir -p /opt/autokarta/deploy && cd /opt/autokarta
   base=https://raw.githubusercontent.com/Eretuk/autokarta/main
   curl -fsSLO $base/docker-compose.yml
   curl -fsSL -o deploy/backup.sh $base/deploy/backup.sh
   curl -fsSL -o .env $base/.env.example
   ```

2. Edit `.env`:

   ```bash
   openssl rand -hex 32     # paste the output into POSTGRES_PASSWORD
   nano .env
   ```

   In the production section check `AUTOKARTA_IMAGE` (pin a version such as
   `ghcr.io/eretuk/autokarta:0.1.0` for predictable updates), `APP_BIND`,
   `BACKUP_TIME` and `BACKUP_KEEP_DAYS`. Variables of the development section
   are not used by the production stack. `POSTGRES_PASSWORD` is applied only
   when the database is created the first time; changing it later does not
   change the password inside the database.

3. Only if the image is private (a fork, for example): log in to the registry
   once with a GitHub personal access token (classic) that has the
   `read:packages` scope:
   `echo <token> | docker login ghcr.io -u <github-user> --password-stdin`.

4. Start everything and check it:

   ```bash
   docker compose pull
   docker compose up -d
   docker compose ps                          # db, app and backup are "Up"
   curl http://127.0.0.1:8080/api/health      # {"status":"ok",...}
   docker compose logs -f app                 # if something is wrong
   ```

5. Create the first owner (run it in an interactive terminal; it asks for a
   password, at least 8 characters):

   ```bash
   docker compose exec app autokarta create-admin --username owner
   ```

6. Point the reverse proxy at `APP_BIND` (default `127.0.0.1:8080`). The proxy
   only terminates TLS; the app sets its own security headers. Caddy:

   ```
   autokarta.example.com {
   	reverse_proxy 127.0.0.1:8080
   }
   ```

   nginx (inside a `server` block with your TLS certificate):

   ```
   location / {
       proxy_pass http://127.0.0.1:8080;
       proxy_set_header Host $host;
       proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
       proxy_set_header X-Forwarded-Proto https;
   }
   ```

7. Open `https://<your domain>`, sign in as the owner, set the currency and
   distance unit on the account page, and add staff accounts on the staff
   page. On a phone the app can be installed to the home screen.

### Update

```bash
docker compose pull app && docker compose up -d app
```

Database migrations run automatically when the app starts. To build the image
on the server instead of pulling it (e.g. on ARM), keep the whole repository
there and run `docker compose up -d --build`.

### Lost owner password

Create another owner from the server, sign in with it and set a new password
for the old account on the staff page:

```bash
docker compose exec app autokarta create-admin --username rescue
```

### Database

PostgreSQL 18. Its data lives in the Docker volume `autokarta_postgres`,
mounted at `/var/lib/postgresql` (the image keeps each major version in its own
subdirectory). A future major upgrade needs a dump and restore (or
`pg_upgrade`). `docker compose down -v` deletes this volume with all data.

### Backups

The `backup` service writes a `pg_dump` to `./backups` every day
(`BACKUP_TIME`, container time is UTC) and keeps `BACKUP_KEEP_DAYS` days.
Run one immediately: `docker compose run --rm backup now`.

Local dumps do not survive losing the server: **copy `./backups` off-site**
(e.g. a nightly `rclone sync` or `restic` from the host) before entering
real data. Restore (replaces all current data):

```bash
docker compose stop app
docker compose exec -T db pg_restore -U autokarta -d autokarta --clean --if-exists < backups/autokarta-YYYYMMDD-HHMMSS.dump
docker compose start app
```

## License

Free for small businesses under the
[PolyForm Small Business License 1.0.0](LICENSE.md).
See [LICENSING.md](LICENSING.md) for details and commercial licensing.
