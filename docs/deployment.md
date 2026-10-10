# Deployment and operations

[Back to README](../README.md)

## Run in a Linux container

Use Docker Compose on Linux, or OrbStack on macOS. Create an untracked `.env` with `BADDIE_ADMIN_PASSWORD`, `MYSQL_PASSWORD`, and `MYSQL_ROOT_PASSWORD` set to separate long unique passwords. Use URL-safe passwords, for example generated with `openssl rand -hex 32`, because the app password is embedded in `DATABASE_URL`. Then run:

```sh
docker compose up --build -d
```

Open `http://localhost:3000/admin`. Nothing is published initially. Open Home, edit its components, and publish to make `/` available. The CMS container runs as a non-root user. MySQL persists content in the `mysql_data` volume. Never run `docker compose down -v` unless you intend to delete that data. Both exposed ports bind to loopback only.

For an Internet-facing installation, put a TLS reverse proxy in front of the loopback-bound port, preserve the original Host header, set `BADDIE_SECURE_COOKIE=true`, and rate-limit `/api/login` at the proxy. Do not expose the admin login over plain HTTP. `BADDIE_ADMIN_PASSWORD` creates the first `admin` account; sign in as `admin` and add other members under Organization. See [authentication](authentication.md) for accounts, recovery and custom providers. Sessions are stored in MySQL, expire after 12 hours and survive restarts.

Origin checks compare the scheme, host, and effective port. Set `BADDIE_ORIGIN=https://cms.example.com` to pin the browser-facing origin, especially if the proxy rewrites Host. Without it, the server uses the preserved Host header and requires HTTPS when secure cookies are enabled, HTTP otherwise. Forwarded headers do not override this policy. Repeated failed sign-ins for one username return 429 for 15 minutes. Keep the proxy's per-client login rate limit in place.

## Deploy to Railway

The root Dockerfile builds both the server and frontend. `.railway/railway.ts` declares a `baddiecore` service and a `MySQL` database in the linked project, connects `DATABASE_URL` to MySQL's private `MYSQL_URL`, enables secure cookies, and configures `/health`. The server listens on Railway's `PORT`; do not set `BADDIE_BIND` on Railway.

With a current Railway CLI, authenticate with `railway login` and create or link the intended project. Set a long unique `BADDIE_ADMIN_PASSWORD` as a shared variable in that project's environment. Keep it out of source control and chat logs. Then review and apply the configuration:

```sh
npm ci --prefix .railway
railway config plan
railway config apply
railway up --service baddiecore
railway domain --service baddiecore
```

Review the target project/environment and plan before applying, especially when linking an existing project. This creates billable services. The configuration has no GitHub source, so uploads deploy the current checkout rather than an older remote branch. Open the resulting HTTPS domain at `/admin` and sign in as `admin`. Enable database backups in Railway and test restores. The CMS needs no volume; MySQL owns persistent storage.

Railway's current [Infrastructure as Code](https://docs.railway.com/infrastructure-as-code) replaces the deprecated `railway.toml` format. See [Railway's agent setup guide](https://railway.com/agents) for CLI authentication and agent integration.

This version requires MySQL 8.4. It creates tables and seeds a new database on startup. It does not import an existing SQLite database. Back up any old SQLite `content` volume and migrate its content separately before switching an existing site; do not delete that volume.

## Server configuration

`DATABASE_URL` is required. `BADDIE_ADMIN_PASSWORD` is required until an administrator has a password. See [authentication](authentication.md) for account settings, and [headless integration](headless.md) for API keys.

`BADDIE_BIND` overrides the listener; otherwise `PORT` selects `0.0.0.0:$PORT`, falling back to `127.0.0.1:3000` when absent. `BADDIE_STATIC` defaults to `web/dist`, and `BADDIE_SECURE_COOKIE` to false. The Docker image sets `PORT=3000` and `BADDIE_STATIC=/app/web`. `BADDIE_DB` is no longer used. `/health` checks database access.

## Backups and operating limits

Back up MySQL with `mysqldump --single-transaction` or Railway's database backups. Test restores before relying on backups. Tables use InnoDB. Database transactions serialize validation and writes through a lock row, including during overlapping deployments. Schema migration tooling beyond initial table creation is not yet included.

Current limits include no media library, rich-text editor, localization, review audit history, revision history/rollback UI, unpublish action, or plugin loading. Public pages render client-side, so server-rendered SEO is not yet covered. Template/component updates do not have optimistic concurrency checks. Performance has not been benchmarked. Admin and CLI operations are serialized; public content and health checks do not take the CMS lock. The server admits at most 16 database jobs at once and returns 503 when full.
