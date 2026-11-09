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
cp .env.dev.example .env                          # dev settings, work as they are
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
in the repository root by itself. Database files live in `./.data/` (ignored
by git): to start from an empty database, run
`docker compose -f docker-compose.dev.yml down`, delete `.data/db` and start
again.

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
4. Open `https://<LAN_HOST>:8443` on the phone and accept the certificate
   warning once (or install `.data/caddy/caddy/pki/authorities/local/root.crt`
   on the phone).

## Production

### What you need

- A Linux server (x86-64 / amd64) with Docker Engine and the Docker Compose v2
  plugin (`docker compose version` must work).
- **An HTTPS address.** Sign-in cookies are `Secure` and the camera works only
  over HTTPS, so opening `http://<server-ip>:8080` will **not** let anyone sign
  in (the login page says so). Use one of:
  - your domain plus a reverse proxy (Caddy, nginx, Traefik, ...) with a
    certificate, on this server or on another machine, see step 5;
  - for a quick test without a domain or open ports:
    [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/)
    or [Tailscale Funnel](https://tailscale.com/kb/1223/funnel), pointed at
    the app's `APP_BIND` address.

The image `ghcr.io/eretuk/autokarta` is built by CI on every commit to `main`
(tags `latest`, `sha-<commit>`) and on version tags `v1.2.3` (tags `1.2.3`,
`1.2`). It is built for amd64 only; on an ARM machine build it yourself (see
"Update").

### Layout on the server

Everything lives in one directory, `/opt/docker/autokarta` in this guide
(any path works):

```
/opt/docker/autokarta/
├── docker-compose.yml
├── .env                 all settings (template: .env.example)
├── deploy/backup.sh     daily dump script
├── db/                  PostgreSQL files, created on first start
└── backups/             daily dumps, created on first start
```

`db/` and `backups/` are placed under `DATA_DIR` from `.env`, so data can
also live on another disk.

### Install

1. Download three files:

   ```bash
   sudo mkdir -p /opt/docker/autokarta/deploy && cd /opt/docker/autokarta
   base=https://raw.githubusercontent.com/Eretuk/autokarta/main
   sudo curl -fsSLO $base/docker-compose.yml
   sudo curl -fsSL -o deploy/backup.sh $base/deploy/backup.sh
   sudo curl -fsSL -o .env $base/.env.example
   sudo chmod 600 .env
   ```

2. Fill in `.env`; every line is explained there. At least:

   - `POSTGRES_PASSWORD`: paste the output of `openssl rand -hex 32`;
   - `APP_BIND`: where the reverse proxy reaches the app. A proxy on another
     machine needs this server's network address (e.g. `10.0.0.5:8080`, or
     `0.0.0.0:8080` for every interface); a proxy on this server outside
     Docker uses `127.0.0.1:8080`. Do not expose the port to the internet:
     ports published by Docker bypass ufw/firewalld on the same host;
   - `DATA_DIR`: `/opt/docker/autokarta` for the layout above.

   A missing value stops `docker compose` with a message naming it.
   `POSTGRES_USER`, `POSTGRES_DB` and `POSTGRES_PASSWORD` are applied only
   when the database is created the first time.

3. Start everything and check it:

   ```bash
   docker compose pull
   docker compose up -d
   docker compose ps                         # db, app and backup are "Up"
   curl http://127.0.0.1:8080/api/health     # {"status":"ok",...}; use your APP_BIND address
   docker compose logs -f app                # if something is wrong
   ```

4. Create the first owner (run it in an interactive terminal; it asks for a
   password, at least 8 characters):

   ```bash
   docker compose exec app autokarta create-admin --username owner
   ```

5. Point the reverse proxy at `APP_BIND`. It only terminates TLS; the app sets
   its own security headers. Caddy (here the proxy is another machine and the
   app server is `10.0.0.5`):

   ```
   autokarta.example.com {
   	reverse_proxy 10.0.0.5:8080
   }
   ```

   nginx (inside a `server` block with your TLS certificate):

   ```
   location / {
       proxy_pass http://10.0.0.5:8080;
       proxy_set_header Host $host;
       proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
       proxy_set_header X-Forwarded-Proto https;
   }
   ```

6. Open `https://<your domain>`, sign in as the owner, set the currency and
   distance unit on the account page, and add staff accounts on the staff
   page. On a phone the app can be installed to the home screen.

If the image is private (a fork, for example), log in to the registry once
before `docker compose pull`, with a GitHub personal access token (classic)
that has the `read:packages` scope:
`echo <token> | docker login ghcr.io -u <github-user> --password-stdin`.

### Update

Set the new version in `AUTOKARTA_IMAGE` in `.env` (or keep `latest`), then:

```bash
docker compose pull app && docker compose up -d app
```

Database migrations run automatically when the app starts. When
`docker-compose.yml` or `.env.example` change in a release, download them
again and compare with your `.env`. To build the image on the server instead
of pulling it (e.g. on ARM), keep the whole repository there and run
`docker compose up -d --build`.

### Lost owner password

Create another owner from the server, sign in with it and set a new password
for the old account on the staff page:

```bash
docker compose exec app autokarta create-admin --username rescue
```

### Database

PostgreSQL 18, files in `<DATA_DIR>/db` (the image keeps each major version
in its own subdirectory, `db/18/docker`). A future major upgrade needs a dump
and restore (or `pg_upgrade`). Deleting `<DATA_DIR>/db` deletes all data.

### Backups

The `backup` service writes a `pg_dump` to `<DATA_DIR>/backups` every day
(`BACKUP_TIME`, container time is UTC) and keeps `BACKUP_KEEP_DAYS` days.
Run one immediately: `docker compose run --rm backup now`.

Local dumps do not survive losing the server: **copy `<DATA_DIR>/backups`
off-site** (e.g. a nightly `rclone sync` or `restic` from the host) before
entering real data. Restore (replaces all current data):

```bash
docker compose stop app
docker compose exec -T db sh -c 'pg_restore -U "$POSTGRES_USER" -d "$POSTGRES_DB" --clean --if-exists' \
  < /opt/docker/autokarta/backups/autokarta-YYYYMMDD-HHMMSS.dump
docker compose start app
```

## License

Free for small businesses under the
[PolyForm Small Business License 1.0.0](LICENSE.md).
See [LICENSING.md](LICENSING.md) for details and commercial licensing.
