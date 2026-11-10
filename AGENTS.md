# AGENTS.md: project context for AI assistants

Read this file fully before making changes. It describes what Autokarta is,
decisions already made, and conventions to follow. If a task conflicts with
something here, ask the human first instead of silently changing direction.

## 1. Product

**Autokarta** is a self-hosted web app (installable PWA) for a small auto
service that also rents out cars (taxi and regular rental) and sells cars.

The first real user (beta tester) is one auto service owner. Today he keeps
everything in a paper notebook or in his head, under a heavy flow of
customers. Paid products he tried were overloaded with features. **Speed and
simplicity of data entry from a phone are the top priority**: if recording a
job takes longer than writing it in the notebook, the app has failed.

Problems to solve:
- No history per car: what was installed or replaced, when, at what mileage.
- No idea what spare parts are in stock and what is missing.
- Rental/taxi fleet maintenance is tracked from memory.
- Cars for sale have no documented history.
- Customers should be able to see what was done on their car.

### Users and roles
- **owner**: sees everything (prices, purchase costs, rentals, revenue), manages staff.
- **mechanic**: vehicles, work orders, parts stock. No financial overview, no staff management.
- **customer**: never logs in. Gets a private share link per vehicle from the owner.
- No public sign-up. The first owner is created with a CLI command; owners create other accounts.

## 2. Domain model (planned)

The **vehicle** is the center of the model.

- `vehicles`: internal UUID primary key. **VIN is NOT the primary key**: many
  right-hand-drive Japanese cars have only a frame (chassis) number, typos
  happen and must be correctable, plates change. Store `vin` and
  `frame_number` as nullable, unique when present. Search works by VIN, frame
  number or plate. A vehicle has a role: `customer`, `fleet` (own rental/taxi),
  `for_sale`. Roles can change over time (a fleet car gets sold).
- `odometer_readings`: mileage history with date. Used for reminders and an honest history.
- `clients`: customers and rental drivers.
- `work_orders` + `work_order_items`: vehicle, mileage, date, status; items are labour or parts.
- `parts`: catalogue with part number (OEM/article) as the main identifier,
  brand, **multiple barcodes** (same part may have different barcodes from
  different suppliers; many parts have none), minimum stock level.
- `stock_movements`: ledger of receipts, consumption into work orders and
  inventory adjustments. **Stock level is the sum of movements, never a
  stored counter.** Add a materialized view only if it ever gets slow.
- Own QR labels for shelves/bins and for parts without barcodes.
- `rentals` + `rental_payments`: contract (taxi or regular), driver, period, rate, payments.
- `sale_listings`: price, description, photos, status.
- `maintenance_rules`: "every N km or M months, whichever comes first".
  System templates (oil, timing belt, brakes, insurance, inspection) have a
  `code` translated in the UI; user-created rules are plain text. For taxi
  cars predict the due date from average daily mileage.
- `vehicle_share_links`: random 32-byte token (base64url), can be revoked and
  regenerated (e.g. when a car is sold).

### Public share page rules
Shows work history and mileage of the vehicle. Does **not** show names or
phone numbers of current or previous owners, and by default no prices. The
history belongs to the car; personal data of past owners must never leak.

## 3. Tech stack

### Backend (`backend/`): done, tested
- Rust 2021, **axum 0.8**, tokio, **sqlx 0.8** (PostgreSQL), argon2 0.5,
  axum-extra 0.10 (cookies), tower-http 0.6, clap 4, thiserror 2, time, uuid.
- Single binary. Migrations embedded via `sqlx::migrate!` and run on startup.
- Queries currently use runtime `sqlx::query_as` (no `DATABASE_URL` needed to
  compile). Switching to compile-time `query!` macros with `cargo sqlx prepare`
  and a committed `.sqlx/` directory is welcome later.
- Newer major versions of some crates exist (sqlx 0.9, tower-http 0.7,
  argon2 0.6). Upgrade deliberately in a separate change, not as a side effect.

### Frontend (`frontend/`): skeleton done
Stack (as scaffolded, exact versions pinned in `package.json`):
- **SvelteKit 2 + Svelte 5 (runes)**, TypeScript, `adapter-static` in SPA mode
  (`fallback: 'index.html'`, root `+layout.ts` with `export const ssr = false`).
  The built files are served by the Rust binary (`STATIC_DIR`), so production
  is one app container plus PostgreSQL.
- Pinned known-good majors: `@sveltejs/kit` 2.x, `vite` 7.x, `typescript` 5.x,
  `@sveltejs/adapter-static` 3.x, `@vite-pwa/sveltekit` 1.x (options
  `kit: { adapterFallback: 'index.html', spa: true }`). SvelteKit 3, Vite 8 and
  TypeScript 7 exist; do not jump majors without being asked.
- Vite dev server proxies `/api` to `http://localhost:8080`.
- **PWA** via `@vite-pwa/sveltekit`; register the service worker with
  `virtual:pwa-register`, exclude `/api/` from navigation fallback.
- **Barcode scanning**: native `BarcodeDetector` where available (Chrome on
  Android); fallback to `zxing-wasm` (`zxing-wasm/reader`). zxing-wasm loads
  its `.wasm` from the jsDelivr CDN by default: **self-host it** by importing
  `zxing-wasm/reader/zxing_reader.wasm?url` and passing it via
  `prepareZXingModule({ overrides: { locateFile } })`. Camera access requires
  HTTPS (localhost is fine in development).
- Fonts self-hosted via `@fontsource/barlow` and `@fontsource/barlow-condensed`
  (no Google Fonts requests, works offline). **Barlow has no Cyrillic**: the
  Russian UI falls back to the system font. Open question for the author.

### Database
PostgreSQL 18 (dev: `docker-compose.dev.yml`). Daily `pg_dump` backups with
rotation plus an off-site copy are a hard requirement before real data goes in.

## 4. What exists now

`backend/` implements:
- First start (no accounts at all): `serve` creates the owner `autokarta_admin`
  with a random password, prints it once in the log and sets
  `must_change_password`; until that user changes the password, everything
  except `GET /auth/me`, `POST /auth/password` and logout answers 403
  `password_change_required` (extractor `SignedIn` vs `CurrentUser`), and the
  frontend sends them to `/welcome`.
- `autokarta create-admin --username <name>`: prompts for password, creates an owner (recovery).
- `autokarta` / `autokarta serve`: HTTP server.
- Migrations: `users` + `sessions`; vehicles, clients, ownership history,
  odometer readings, work orders + items, business settings; parts, barcodes,
  stock movements, part lines in work orders (`backend/migrations/`).
- Session auth: 256-bit random token in an HttpOnly, SameSite=Lax cookie
  `autokarta_session`; DB stores only its SHA-256 hash. `COOKIE_SECURE=false`
  only for local HTTP development.
- Login rate limiter: 5 failures per username, then 15 minutes lockout (in memory).
- Constant-time-ish login: unknown usernames still run an Argon2 verification.
- Extractors: `CurrentUser` (any staff) and `Owner` (owners only).

API (all JSON, prefix `/api`):

| Method | Path | Who | Notes |
|---|---|---|---|
| GET | `/health` | anyone | checks DB |
| POST | `/auth/login` | anyone | `{username, password}`; username is case-insensitive |
| POST | `/auth/logout` | staff | |
| GET | `/auth/me` | staff | current user |
| PATCH | `/auth/me` | staff | `{display_name?, locale?}` |
| POST | `/auth/password` | staff | `{current_password, new_password}`; signs out other devices; clears `must_change_password`; `password_unchanged` if equal |
| GET | `/users` | owner | list staff |
| POST | `/users` | owner | `{username, display_name, role, password, locale?}` |
| PATCH | `/users/{id}` | owner | `{display_name?, role?, is_active?}`; cannot demote/deactivate self |
| POST | `/users/{id}/password` | owner | `{password}`; signs the user out everywhere and lifts a login lockout; not for yourself (`cannot_reset_own_password`) |
| GET | `/settings` | staff | `{currency, distance_unit}` |
| PATCH | `/settings` | owner | `{currency?, distance_unit?}` |
| GET | `/clients?q=` | staff | search by name or phone |
| POST | `/clients` | staff | `{name, phone?, note?}` |
| PUT | `/clients/{id}` | staff | same body |
| GET | `/vehicles?q=` | staff | search by plate, VIN or frame number, ignoring spaces/dashes |
| POST | `/vehicles` | staff | vehicle fields + `owner_id?`, `odometer?` |
| GET/PUT | `/vehicles/{id}` | staff | detail with owner history, readings, work orders |
| PUT | `/vehicles/{id}/owner` | staff | `{client_id?, since?}`; closes the current ownership |
| POST | `/vehicles/{id}/odometer` | staff | `{reading, read_on?, confirm_lower?}` |
| DELETE | `/vehicles/{id}/odometer/{reading_id}` | owner | manual readings only |
| GET | `/work-orders?status=` | staff | |
| POST | `/work-orders` | staff | `{vehicle_id, odometer?, complaint?, confirm_lower?}` |
| GET/PATCH | `/work-orders/{id}` | staff | `{complaint?, notes?, status?, odometer?, confirm_lower?}` |
| POST | `/work-orders/{id}/items` | staff | `{description?, part_id?, quantity_milli?, unit_price_minor?}`; a part line defaults to the part's name and price |
| PATCH/DELETE | `/work-orders/{id}/items/{item_id}` | staff | |
| GET | `/parts?q=&filter=low` | staff | name, brand, part number (separators ignored) or exact barcode (`barcode_match`) |
| POST | `/parts` | staff | `{name, brand?, part_number?, condition?, unit?, location?, min_stock_milli?, note?, sell_price_minor?, barcodes?, stock_milli?}` |
| GET/PUT | `/parts/{id}` | staff | detail with barcodes and the last 100 movements |
| DELETE | `/parts/{id}` | owner | only if no work order uses it |
| POST/DELETE | `/parts/{id}/barcodes` | staff | `{barcode}`; a barcode belongs to one part |
| POST | `/parts/{id}/movements` | staff | `{kind: receipt\|count, quantity_milli, unit_cost_minor?, note?}`; a count records the difference |

Prices and totals are owner-only: mechanics never receive them and cannot set
them (this includes part sale prices and purchase costs). Stock is the sum of
`stock_movements` and may go negative (fitting a part whose delivery was not
recorded must not be blocked). Used parts are common: `condition` (`new` /
`used`) is part of a part's identity (unique on brand + part number +
condition), so a used one is its own item with its own stock, price and
label, and is marked on screen, on printed orders and on labels. Stock is
not split into purchase batches: costs are kept per receipt. A part line in a work order owns one
consumption movement; cancelling the order removes them, reopening restores
them. A reading lower than the latest one returns 409
`odometer_lower_than_previous` unless `confirm_lower` is sent. API tests in
`backend/src/routes/api_tests.rs` run against PostgreSQL via `sqlx::test`
(`DATABASE_URL` must point to a server where the user may create databases).

`frontend/` implements (roadmap step 1): login, app shell (bottom tab bar on
phones, header nav on wide screens), staff management (owner only), account
page (name, language saved to profile, per-device theme, password change,
sign out), camera scanner test page (`/scanner`), PWA manifest + service
worker, en/pl/ru. Step 2: home page is vehicle search (typing or scanning
VIN/plate), new vehicle form with owner picker, vehicle page (edit, owner
change, odometer history, work orders), work order list and work order page
(items, prices for owners, status, mileage); owners set currency and distance
unit on the account page. Step 3: parts tab (search, scan, low-stock
filter), new part form (barcode from a scan), part page (stock, delivery,
stock count, barcodes, history), parts in work orders (search or scan), client
details editable from the vehicle page. Printable pages live in the
`(print)` route group (no app chrome, always white paper): a work order on A4
(`/print/work-orders/{id}`, prices only for owners because the API only gives
them to owners) and QR labels on A4 sticker sheets, 3 x 8 of 70 x 37 mm
(`/print/labels?part={id}`). A label carries the part's own code `AK-XXXXXXXX`
(Crockford base32), stored as one of its barcodes, so scanning it uses the
normal barcode search. QR codes are drawn with `uqr` (MIT). Shelf labels are
not done yet. The backend sets `Cache-Control`: `immutable` for
`/_app/immutable/*`, `no-cache` for other static files.

Deployment: `Dockerfile` (node build + cargo-chef + debian-slim, non-root),
`docker-compose.yml` (db, app, backup; **no proxy**: production runs behind
an external reverse proxy that terminates TLS and forwards to `APP_BIND`),
`deploy/backup.sh` (daily `pg_dump` with rotation, built into the app image
as `autokarta-backup` together with `pg_dump` 18; the `backup` service runs
the app image and is enabled by `COMPOSE_PROFILES=backup`). The server needs
only `docker-compose.yml` and `.env`.
CI builds the image for linux/amd64 and linux/arm64 on native runners and
pushes one multi-arch tag to `ghcr.io/eretuk/autokarta` (only ghcr.io, never Docker
Hub) from `main` (`latest`, `sha-*`) and `v*` tags; the server pulls it via
the version pinned in `docker-compose.yml` (`x-image`, not `.env`, which
users keep encrypted). A `v*` tag must equal the version in
`backend/Cargo.toml`, `frontend/package.json` and `x-image` (CI checks it);
the tag build publishes a GitHub Release with `docker-compose.yml` and
`.env.example` attached. No Docker volumes: production data is bind-mounted from
`DATA_DIR` (`db/` at `/var/lib/postgresql`, `backups/`), dev data from
`./.data/`. Every other compose value comes from `.env` (`.env.example` for
production, `.env.dev.example` for development) and is required (`:?`).
Logging (`backend/src/request_log.rs`): one INFO line per `/api` request in a
span with request id, client IP (last `X-Forwarded-For` hop) and username
(recorded by the session extractor), never the query string (search terms are
personal data); static files and `/api/health` only at DEBUG, 5xx at ERROR.
`AppError` puts its code into the response extensions for that line. Auth
and staff changes log their own events; log new security-relevant actions the
same way. Responses carry `X-Request-Id`.
Security headers (Referrer-Policy, nosniff, frame deny, Permissions-Policy,
HSTS when `COOKIE_SECURE`) are set by the backend, never only in a proxy.
Dev HTTPS for phones: `docker-compose.dev.yml --profile https` (Caddy with
`tls internal` on `LAN_HOST:8443`, proxies to Vite). **Off-site
backup copy is not automated yet.** CI: `.github/workflows/ci.yml`.

## 5. Roadmap (in order)

1. Frontend skeleton: login, app shell, staff management, account page
   (change password, language), camera scanner test page. Production
   `Dockerfile` (multi-stage, cargo-chef), `docker-compose.yml` with app,
   PostgreSQL and backup service (HTTPS via an external reverse proxy), CI workflow.
2. Vehicles, odometer readings, work orders.
3. Parts stock with scanning, consumption into work orders, low-stock list.
4. Public share link page.
5. Maintenance reminders: background task in the same binary + Telegram bot.
6. Rentals and payments, then cars for sale.

After step 3 the beta tester uses it for 1–2 weeks; his feedback reshapes the rest.
Offline mode is explicitly **out of scope** for the first versions.

## 6. Conventions

### API and errors
- Errors are JSON `{"error": "<code>"}` with snake_case codes
  (`invalid_credentials`, `username_taken`, `password_too_short`, ...).
  The backend never returns human-readable text; the frontend translates codes.
- Status codes: 401 unauthenticated / bad credentials, 403 wrong role,
  404, 409 conflict, 422 validation, 429 rate limited (with `Retry-After`).
- Validation lives in `backend/src/validate.rs` and returns error codes.
- New error kinds go into `AppError` in `backend/src/error.rs`.

### Database
- Migrations: `backend/migrations/YYYYMMDDHHMMSS_description.sql`, never edit
  a migration that has been committed; add a new one.
- UUID primary keys (`gen_random_uuid()`), `TIMESTAMPTZ`, `created_at`/`updated_at`.
- Money: integer minor units (`BIGINT`, e.g. grosze/kopecks) plus currency
  from business settings. Never floats.
- Destructive or multi-step writes go in a transaction.

### Internationalization
- Languages: English (base), Polish, Russian. **No hardcoded UI strings**, even
  while only English exists. Every string goes through the i18n layer.
- Frontend: **decided: own small typed module** `frontend/src/lib/i18n/`
  (no Paraglide). `en.json` is the source of truth; `MessageKey` is derived
  from it, so `t('typo')` fails `npm run check`. `pl.json`/`ru.json` may miss
  keys (English fallback). Plurals are objects `{one, few, many, other}`
  picked by `Intl.PluralRules`; `{count}` is number-formatted. Server error
  codes map to `errors.<code>`. Keep the three files' key sets identical.
- **Plurals**: Polish and Russian have three forms with different rules
  (RU "21 деталь" vs PL "21 części"). Always use `Intl.PluralRules` /
  library plural support, never `count + " items"`.
- Dates and numbers via `Intl.DateTimeFormat` / `Intl.NumberFormat`.
- **Currency and distance units (km/mi) are business settings, not tied to
  the UI language.**
- Polish and Russian text is 20–30% longer than English: design with room.
- Backend text (Telegram, share page) later via Fluent (`fluent-templates`).
- User-entered data (part names, job descriptions) is never translated.
- Each user has `locale` in the profile; clients will get a locale field for
  notifications; the public page uses `Accept-Language` plus a switcher.

### UI and design
- Mobile first. Mechanics use phones with dirty hands, in a noisy workshop,
  sometimes in sunlight: large touch targets (min 48px), high contrast,
  short forms, the most common action reachable in one tap.
- Light and dark themes via CSS custom properties.
- Visual direction: vehicle registration document (pale green-grey paper,
  dark ink, stamp blue accent); typefaces Barlow (body) and Barlow Condensed
  (headings, wordmark). One memorable element: the wordmark styled with a
  blue strip like a European number plate. Keep everything else quiet.
- Copy: plain verbs, sentence case, buttons say what happens ("Save changes",
  "Add part"), errors explain what to do next.

### Code style
- Rust: `cargo fmt`, `cargo clippy -- -D warnings`, unit tests for pure logic.
- Keep handlers thin; shared logic in modules. Comments explain why, not what.
- Commit messages in English, imperative mood.

## 7. Licensing rules (important)

- The project is **source-available** under PolyForm Small Business 1.0.0
  (`LICENSE.md`, with a `Required Notice:` line that must be preserved).
  See `LICENSING.md`. The author plans possible commercial licensing later.
- **Only add dependencies under permissive licenses** (MIT, Apache-2.0,
  BSD, ISC, Zlib, MPL-2.0 is acceptable as a library). **No GPL/AGPL/LGPL
  code or dependencies**: they are incompatible with the license model.
  Check the license before adding any crate or npm package.
- Do not copy code from other projects unless its license is permissive and
  attribution is kept.
- External contributors sign `CLA.md` (enforced by `.github/workflows/cla.yml`).

## 8. Security and data rules

- Never commit `.env`, secrets, database dumps or **any real customer data**
  (names, phones, VINs of real clients). Use invented demo data.
- Passwords: Argon2 only. Tokens: store hashes, never plaintext.
- Share-link pages must not expose personal data (see section 2).
- No third-party CDNs or trackers at runtime: everything self-hosted.

## 9. Local development

```bash
cp .env.dev.example .env
docker compose -f docker-compose.dev.yml up -d
cd backend
cargo run                      # first start prints the autokarta_admin password
                               # http://localhost:8080/api/health
cargo test
```
Frontend: `cd frontend && npm install && npm run dev` (http://localhost:5173,
proxies `/api` to :8080). `npm run check` must pass with zero warnings.
To serve the built app from the backend: `npm run build`, then `STATIC_DIR=../frontend/build`.
