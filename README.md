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

For an Internet-facing installation, put a TLS reverse proxy in front of the loopback-bound port, preserve the original Host header, set `BADDIE_SECURE_COOKIE=true`, and rate-limit `/api/login` at the proxy. Do not expose the admin login over plain HTTP. The default authentication is a single shared administrator password. WorkOS is optional, as described below. Sessions expire after 12 hours and all sessions end on server restart.

## Choose authentication

`BADDIE_AUTH=password` is the default and requires a nonempty `BADDIE_ADMIN_PASSWORD`. It makes no WorkOS requests and needs no WorkOS account. Unknown methods or missing required configuration stop startup. There is no unauthenticated editor mode and no fallback between methods.

To opt into [WorkOS AuthKit](https://workos.com/docs/authkit), set these variables on the server or in your Compose `.env`:

```dotenv
BADDIE_AUTH=workos
BADDIE_SECURE_COOKIE=true
WORKOS_API_KEY=<your-server-side-api-key>
WORKOS_CLIENT_ID=<your-client-id>
WORKOS_ORGANIZATION_ID=<your-editors-organization-id>
WORKOS_REDIRECT_URI=https://cms.example.com/api/auth/callback
```

Use your own WorkOS environment for each independently operated installation. Keep the API key in secret storage, never in frontend variables or source control. Password configuration is unused in WorkOS mode and can be removed.

In the WorkOS dashboard, enable hosted AuthKit, register the exact callback URL above and allow `https://cms.example.com/admin` as a logout redirect. Create a dedicated organization for this CMS's editors and invite them into it. Disable public sign-ups and automatic organization enrollment if access must be invitation-only. Every verified user authenticated into this configured organization has full CMS access, regardless of their WorkOS role. A WorkOS account outside that organization grants no access.

Editors use the hosted sign-in page and return to `/admin`. The server checks single-use browser state and PKCE, exchanges the code, and stores tokens in memory. The browser receives only an opaque HttpOnly session cookie. WorkOS sessions refresh on the next admin request after at most five minutes, or earlier if the access token expires. Revocation and membership changes take effect on refresh, not immediately. Refresh failures deny access and discard the local session. Sign-out clears the local session and redirects the browser to WorkOS to end its session too. Run one replica; restarting ends local sessions.

For local development, use a WorkOS staging environment and an HTTP loopback callback, with `BADDIE_SECURE_COOKIE=false`. When using the Vite dev server, register its browser-facing origin with `/api/auth/callback`, not the backend's port. Production callbacks require HTTPS and secure cookies. Configure the reverse proxy to omit query strings on `/api/auth/callback` from access logs because callbacks contain authorization codes.

The Railway configuration defaults to password auth. To opt in before applying it, replace its `BADDIE_ADMIN_PASSWORD` environment mapping with `BADDIE_AUTH: "workos"` and mappings from `ctx.shared` for the four `WORKOS_*` variables above. Set those shared variables in the selected Railway environment. Leave `BADDIE_SECURE_COOKIE: "true"`. Review the configuration plan before applying it.

### Supply your own authentication

Implement `auth::AuthProvider` and pass `Arc::new(your_provider)` to `AppState::open_with_auth` in `src/main.rs`. No content handlers need changing. This is a Rust source extension, not a runtime plugin or a `BADDIE_AUTH=custom` option.

- `authorize(&HeaderMap)` returns an `Editor { id }` only after validating both identity and CMS access. All editors currently have full access. Return 401 for absent or invalid credentials. CMS middleware puts the editor in request extensions and preserves its same-origin mutation checks.
- `routes()` returns a state-bound Axum `Router` for login, logout and callbacks. Match the frontend contract in `CONTRACT.md`. Redirect-based providers can reuse the existing login UI through `GET /api/auth/config`.
- Your provider owns session expiry, revocation, CSRF protection for its routes, and secure credential storage. If using proxy identity headers, block direct backend access and make the trusted proxy strip client-supplied identity headers before setting its own.

The built-in implementations live in `src/auth.rs` and `src/auth/workos.rs`. Auth tests use a local mock WorkOS endpoint and do not need credentials. A real staging-environment sign-in and sign-out should still be checked before enabling WorkOS in production.

## Editing model

- Pages have a path, template, ordered component instances, and a draft revision.
- Templates define named regions, component allowlists, and maximum counts. An empty allowlist permits nothing.
- Component definitions have typed text, textarea, or URL fields and a renderer. Hero, text, callout, and cards renderers ship with this version. Cards render one body line per card.
- The experience editor renders the page with the same renderer as the public site. Select a block to edit its fields, add allowed components, drag to reorder, or use the move buttons.
- Save draft keeps edits private. Publish saves and snapshots the page, template, and component definitions together. Later draft or schema edits do not alter that snapshot.
- Concurrent page saves return a conflict instead of overwriting another revision. Template and component changes that would invalidate existing drafts are rejected.

The Pages view is an expandable tree. Choose a parent and a URL segment when creating or moving a page. `/about/team` belongs under `/about`; paths are the hierarchy, with no separate parent IDs. Missing intermediate pages appear as folders, so existing nested URLs still work. Renaming or moving a page moves every descendant draft URL in one transaction and increments their revisions. A stale editor must reload before saving. The root page `/` cannot move, and a page with descendants cannot be deleted until those descendants are moved or deleted.

Moving a draft does not change published URLs. Publish each affected page when ready; there are no automatic redirects or link rewrites.

Paths use ASCII letters, numbers, hyphens, underscores, and slash-separated segments, up to 2048 bytes. Paths are case-sensitive. `/admin`, `/api`, `/health`, and `/assets` are reserved. Rules in this version govern placement, counts, required fields, and field types, not audience targeting or personalization.

## Track content definitions in Git

The same `baddiecore` binary runs the server and a local CLI. Start the CMS once to initialize its database. The CLI connects directly through `DATABASE_URL`, without editor authentication or a remote HTTP endpoint. Point it only at the local instance you intend to modify.

```sh
cargo run -- pull                         # components and templates → baddiecore-content/
cargo run -- push --dry-run               # validate the entire import, then roll back
cargo run -- push                         # files → local database drafts
cargo run -- pull --pages --force         # also export pages, replacing local exports
cargo run -- push --pages                 # explicitly import pages too
```

An optional directory follows `pull` or `push`. `pull` refuses differing existing files unless `--force` is supplied, whether the difference came from Git or the editor. Commit or stash local changes before forcing a pull. A forced pull also removes exported files for items no longer in the database, within the selected kinds. Without `--pages`, both commands leave page files and page content alone. Definition imports still validate existing pages.

Each item has a versioned YAML file under `components/`, `templates/`, or `pages/`. Filenames use stable IDs, usually UUIDs; unusual IDs use a SHA-256 filename. Keep IDs and filenames unchanged when editing existing items. Output is deterministic, including sorted block field keys, and excludes revisions, publication metadata, snapshots, sessions, and credentials. Add the export directory to Git and review it normally. The CLI never commits or pushes to Git itself.

`push` merges by ID in one transaction, validates references and all resulting page drafts, and leaves items missing from the files untouched. It does not delete or publish. Changed pages get the target database's next revision; unchanged pages keep theirs. YAML page paths describe the final tree, so when moving a branch in files, update its descendants too. There is no implicit cascade during import and no automatic merge with concurrent editorial changes. A successful dry run is not a reservation; push revalidates the database when it runs.

For a container-based local instance, first run `docker compose up --build -d`, then use a one-off CLI container with the same database configuration and a bind-mounted export directory:

```sh
mkdir -p baddiecore-content
docker compose run --rm --no-deps --user "$(id -u):$(id -g)" \
  --volume "$PWD/baddiecore-content:/content" cms baddiecore pull /content
docker compose run --rm --no-deps --user "$(id -u):$(id -g)" \
  --volume "$PWD/baddiecore-content:/content" cms baddiecore push /content --dry-run
```

Remove `--dry-run` to apply. Add `--pages` when page content belongs in Git. The user override keeps files writable by your host account on Linux; this workflow also works with OrbStack. The running CMS sees imports on the next reload.

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
node --experimental-strip-types --test tests/page-tree.test.mjs
pnpm --dir web run check
pnpm --dir web run typecheck
pnpm --dir web run build
```

Each API test creates a uniquely named `baddie_test_*` database and drops it afterward. Tests fail rather than silently skip when `TEST_DATABASE_URL` is missing. Do not point tests at production.

Configuration: `DATABASE_URL` and the selected authentication method's configuration are required. `BADDIE_BIND` overrides the listener; otherwise `PORT` selects `0.0.0.0:$PORT`, falling back to `127.0.0.1:3000` when absent. `BADDIE_STATIC` defaults to `web/dist`, and `BADDIE_SECURE_COOKIE` to false. The Docker image sets `PORT=3000` and `BADDIE_STATIC=/app/web`. `BADDIE_DB` is no longer used. `/health` checks database access. See [CONTRACT.md](CONTRACT.md) for the JSON API.

Component styles live beside their components in `stylex.create` declarations and use `stylex.attrs` for Solid's DOM attributes. Shared controls live in `web/src/common.stylex.ts`; `styles.css` contains only the document reset. Use explicit color and border properties, and compose conditional styles through `stylex.attrs`. The StyleX plugin runs before Solid and extracts production CSS. Run `pnpm --dir web run fmt` to format the frontend with Vite+.

## Extending and operating

Add renderer names to the Rust `Renderer` enum, the frontend `RendererName` type, and the `renderers` map in `web/src/components/Renderer.tsx`. Define their fields through the component editor. Built-in renderers use the keys `title`, `body`, `eyebrow`, `button_label`, and `button_url` where applicable. Arbitrary extra fields are stored but need renderer code to display them. Text is escaped; this version does not accept executable templates or raw HTML.

Run one CMS replica because sessions are process-local. Back up MySQL with `mysqldump --single-transaction` or Railway's database backups. Test restores before relying on backups. Tables use InnoDB. Database transactions serialize validation and writes through a lock row, including during overlapping deployments. Schema migration tooling beyond initial table creation is not yet included.

Current limits include no media library, rich-text editor, localization, roles, workflow approvals, revision history/rollback UI, unpublish action, or plugin loading. Public pages render client-side, so server-rendered SEO is not yet covered. Template/component updates do not have optimistic concurrency checks. Performance has not been benchmarked; database operations are serialized.
