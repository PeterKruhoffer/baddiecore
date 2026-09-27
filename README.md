# Baddiecore

A self-hosted CMS with a Rust/Axum server, MySQL storage, and a Solid 2 editor styled with StyleX. The frontend uses Vite+ and pnpm. This is an early first version, not a Sitecore replacement.

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

Review the target project/environment and plan before applying, especially when linking an existing project. This creates billable services. The configuration has no GitHub source, so uploads deploy the current checkout rather than an older remote branch. Open the resulting HTTPS domain at `/admin`. Keep one CMS replica because sessions are in memory. Enable database backups in Railway and test restores. The CMS needs no volume; MySQL owns persistent storage.

Railway's current [Infrastructure as Code](https://docs.railway.com/infrastructure-as-code) replaces the deprecated `railway.toml` format. See [Railway's agent setup guide](https://railway.com/agents) for CLI authentication and agent integration.

This version requires MySQL 8.4. It creates tables and seeds a new database on startup. It does not import an existing SQLite database. Back up any old SQLite `content` volume and migrate its content separately before switching an existing site; do not delete that volume.

## Run in a Linux container

Use Docker Compose on Linux, or OrbStack on macOS. Create an untracked `.env` with `BADDIE_ADMIN_PASSWORD`, `MYSQL_PASSWORD`, and `MYSQL_ROOT_PASSWORD` set to separate long unique passwords. Use URL-safe passwords, for example generated with `openssl rand -hex 32`, because the app password is embedded in `DATABASE_URL`. Then run:

```sh
docker compose up --build -d
```

Open `http://localhost:3000/admin`. Nothing is published initially. Open Home, edit its components, and publish to make `/` available. The CMS container runs as a non-root user. MySQL persists content in the `mysql_data` volume. Never run `docker compose down -v` unless you intend to delete that data. Both exposed ports bind to loopback only.

For an Internet-facing installation, put a TLS reverse proxy in front of the loopback-bound port, preserve the original Host header, set `BADDIE_SECURE_COOKIE=true`, and rate-limit `/api/login` at the proxy. Do not expose the admin login over plain HTTP. Authentication is a single shared administrator password, not multi-user accounts. Sessions expire after 12 hours and all sessions end on server restart.

## Editing model

- Pages have a path, template, ordered component instances, and a draft revision.
- Templates define named regions, component allowlists, and maximum counts. An empty allowlist permits nothing.
- Component definitions have typed text, textarea, or URL fields and a renderer. Hero, text, callout, and cards renderers ship with this version. Cards render one body line per card.
- The experience editor renders the page with the same renderer as the public site. Select a block to edit its fields, add allowed components, drag to reorder, or use the move buttons.
- Save draft keeps edits private. Publish saves and snapshots the page, template, and component definitions together. Later draft or schema edits do not alter that snapshot.
- Concurrent page saves return a conflict instead of overwriting another revision. Template and component changes that would invalidate existing drafts are rejected.

Paths use ASCII letters, numbers, hyphens, underscores, and slash-separated segments, up to 2048 bytes. Paths are case-sensitive. `/admin`, `/api`, `/health`, and `/assets` are reserved. Rules in this version govern placement, counts, required fields, and field types, not audience targeting or personalization.

## Development

Install Rust 1.98 or later, Node 24.11 or later, and pnpm. The frontend pins its pnpm version in `web/package.json`. Start MySQL with `docker compose up -d mysql` using the `.env` above. In two terminals:

```sh
# Export BADDIE_ADMIN_PASSWORD and DATABASE_URL securely first.
# DATABASE_URL uses mysql://baddiecore:<MYSQL_PASSWORD>@127.0.0.1:3306/baddiecore
cargo run

pnpm --dir web install --frozen-lockfile
pnpm --dir web run dev
```

Vite+ proxies the API to the Rust server. A production build uses `pnpm --dir web run build`; Rust then serves `web/dist` directly. `.agents/setup` installs dependencies and builds both parts in an Amp orb. Vite+ runs through the project-local `vp` CLI, so no global Vite+ installation is required.

```sh
# TEST_DATABASE_URL must point to a disposable MySQL server, with credentials
# allowed to create/drop databases, and no database name or query string.
# Example shape: mysql://root:<password>@127.0.0.1:3306
cargo test --locked
cargo clippy --all-targets -- -D warnings
pnpm --dir web run check
pnpm --dir web run typecheck
pnpm --dir web run build
```

Each API test creates a uniquely named `baddie_test_*` database and drops it afterward. Tests fail rather than silently skip when `TEST_DATABASE_URL` is missing. Do not point tests at production.

Configuration: `DATABASE_URL` and `BADDIE_ADMIN_PASSWORD` are required. `BADDIE_BIND` overrides the listener; otherwise `PORT` selects `0.0.0.0:$PORT`, falling back to `127.0.0.1:3000` when absent. `BADDIE_STATIC` defaults to `web/dist`, and `BADDIE_SECURE_COOKIE` to false. The Docker image sets `PORT=3000` and `BADDIE_STATIC=/app/web`. `BADDIE_DB` is no longer used. `/health` checks database access. See [CONTRACT.md](CONTRACT.md) for the JSON API.

Component styles live beside their components in `stylex.create` declarations and use `stylex.attrs` for Solid's DOM attributes. Shared controls live in `web/src/common.stylex.ts`; `styles.css` contains only the document reset. Use explicit color and border properties, and compose conditional styles through `stylex.attrs`. The StyleX plugin runs before Solid and extracts production CSS. Run `pnpm --dir web run fmt` to format the frontend with Vite+.

## Extending and operating

Add renderer names to the Rust `Renderer` enum, the frontend `RendererName` type, and the `renderers` map in `web/src/components/Renderer.tsx`. Define their fields through the component editor. Built-in renderers use the keys `title`, `body`, `eyebrow`, `button_label`, and `button_url` where applicable. Arbitrary extra fields are stored but need renderer code to display them. Text is escaped; this version does not accept executable templates or raw HTML.

Run one CMS replica because sessions are process-local. Back up MySQL with `mysqldump --single-transaction` or Railway's database backups. Test restores before relying on backups. Tables use InnoDB. Database transactions serialize validation and writes through a lock row, including during overlapping deployments. Schema migration tooling beyond initial table creation is not yet included.

Current limits include no media library, rich-text editor, localization, page hierarchy, roles, workflow approvals, revision history/rollback UI, unpublish action, or plugin loading. Public pages render client-side, so server-rendered SEO is not yet covered. Template/component updates do not have optimistic concurrency checks. Performance has not been benchmarked; database operations are serialized.
