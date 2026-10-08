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

Origin checks compare the scheme, host, and effective port. Set `BADDIE_ORIGIN=https://cms.example.com` to pin the browser-facing origin, especially if the proxy rewrites Host. Without it, the server uses the preserved Host header and requires HTTPS when secure cookies are enabled, HTTP otherwise. Forwarded headers do not override this policy. Both pending logins and active sessions are bounded at 128; new logins receive 429 when full instead of evicting other users. Keep the proxy's per-client login rate limit in place.

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

In the WorkOS dashboard, enable hosted AuthKit, register the exact callback URL above and allow `https://cms.example.com/admin` as a logout redirect. Create a dedicated organization for this CMS's editors and invite them into it. Disable public sign-ups and automatic organization enrollment if access must be invitation-only. Authentication into this organization does not grant CMS access by itself. Add the user's stable WorkOS user ID to the CMS's local Organization screen. WorkOS roles do not control local permissions. A WorkOS account outside the configured organization grants no access.

### Bootstrap and recover administrator access

One CMS installation is one organization. On the **first startup with local membership support**, `BADDIE_BOOTSTRAP_ADMIN_ID=<stable-provider-user-id>` creates that local administrator. Remove the variable afterward. It applies only when the organization record is first created; restarting never restores a removed member or overwrites local membership. Unknown WorkOS and custom-provider IDs are denied by default, including an ID named `shared-admin`.

For an existing installation or recovery, temporarily restart the server with `BADDIE_AUTH=password` and a securely configured `BADDIE_ADMIN_PASSWORD`. The password session is a recovery administrator independent of local membership. Use Organization to add or repair administrator memberships, then restore WorkOS configuration and restart. Password login is deliberately not available alongside WorkOS. Keep password recovery restricted to the operator; all password users have full access. No external WorkOS mutation is required.

Editors use the hosted sign-in page and return to `/admin`. The server checks single-use browser state and PKCE, exchanges the code, and stores tokens in memory. The browser receives only an opaque HttpOnly session cookie. WorkOS sessions refresh on the next admin request after at most five minutes, or earlier if the access token expires. WorkOS revocation and WorkOS organization changes take effect on refresh; local CMS membership changes take effect on the next operation. Refresh failures deny access and discard the local session. Sign-out clears the local session and redirects the browser to WorkOS to end its session too. Run one replica; restarting ends local sessions.

For local development, use a WorkOS staging environment and an HTTP loopback callback, with `BADDIE_SECURE_COOKIE=false`. When using the Vite dev server, register its browser-facing origin with `/api/auth/callback`, not the backend's port. Production callbacks require HTTPS and secure cookies. Configure the reverse proxy to omit query strings on `/api/auth/callback` from access logs because callbacks contain authorization codes.

The Railway configuration defaults to password auth. To opt in before applying it, replace its `BADDIE_ADMIN_PASSWORD` environment mapping with `BADDIE_AUTH: "workos"` and mappings from `ctx.shared` for the four `WORKOS_*` variables above. Set those shared variables in the selected Railway environment. Leave `BADDIE_SECURE_COOKIE: "true"`. Review the configuration plan before applying it.

### Supply your own authentication

Implement `auth::AuthProvider` and pass `Arc::new(your_provider)` to `AppState::open_with_auth` in `src/main.rs`. No content handlers need changing. This is a Rust source extension, not a runtime plugin or a `BADDIE_AUTH=custom` option.

- `authorize(&HeaderMap)` returns an `Editor { id }` after validating identity. Use a stable provider ID, not email. Return 401 for absent or invalid credentials. The CMS resolves local membership inside each content transaction and returns 403 for unknown identities. CMS middleware puts the identity in request extensions and preserves its same-origin mutation checks. The optional `recovery_admin` trait method defaults to false; override it only for a trusted operator recovery mechanism, never based on an untrusted role or ID header.
- `routes()` returns a state-bound Axum `Router` for login, logout and callbacks. Match the frontend contract in `CONTRACT.md`. Redirect-based providers can reuse the existing login UI through `GET /api/auth/config`.
- Your provider owns session expiry, revocation, CSRF protection for its routes, and secure credential storage. If using proxy identity headers, block direct backend access and make the trusted proxy strip client-supplied identity headers before setting its own.

CMS routes use `AuthProvider::origin_policy`, which defaults to `BADDIE_ORIGIN` and `BADDIE_SECURE_COOKIE`. Override it with `OriginPolicy::new` for configuration supplied in Rust. Built-in providers use the same policy for their own routes and CMS mutations.

The built-in implementations live in `src/auth.rs` and `src/auth/workos.rs`. Auth tests use a local mock WorkOS endpoint and do not need credentials. A real staging-environment sign-in and sign-out should still be checked before enabling WorkOS in production.

## Use another app as the frontend

The CMS supports headless delivery alongside its built-in site. Your app needs the CMS's HTTPS URL and a server-side content API key. It gets published pages as JSON and renders them with its own components. Editors still author pages, place components in template regions, and publish or submit for review in `/admin`.

Set `BADDIE_CONTENT_API_KEY` on the CMS and the consuming app's server. To register custom component definitions remotely, also set a separate `BADDIE_COMPONENT_API_KEY` on the CMS and in your app's trusted setup or deployment process. Generate each key independently with `openssl rand -hex 32`. Never commit keys, put them in browser bundles, pass them in URLs, or print them in logs. Nonempty keys must contain at least 32 non-whitespace ASCII bytes; identical keys fail startup. Missing or empty values disable the corresponding capability. Rotate or revoke a key by changing or removing it and restarting the CMS. There is one installation-wide key per capability, not per-app access control.

Send keys in `Authorization: Bearer <key>`. Content keys cannot register definitions. Registration keys cannot read content, edit pages, publish, or manage membership. These routes do not use editor sessions, and return `Cache-Control: no-store`. Use server-side requests rather than credentialed cross-origin browser calls. HTTPS and proxy rate limits are still required.

```sh
# CMS_URL and keys come from server-side configuration.
curl --fail --silent --show-error "$CMS_URL/api/headless/pages" \
  -H "Authorization: Bearer $BADDIE_CONTENT_API_KEY"
curl --fail --silent --show-error "$CMS_URL/api/headless/content?slug=%2Fabout" \
  -H "Authorization: Bearer $BADDIE_CONTENT_API_KEY"
```

`GET /api/headless/pages` lists published `{id, title, slug, template_id, revision}` records ordered by published path. Use it to discover routes or build navigation. `GET /api/headless/content?slug=/about` returns `{page, template, components}` from the published snapshot. The page contains ordered blocks with `id`, `component_id`, `region`, and `fields`. Render regions in `template.regions` order and preserve block order within each region. Resolve each block's definition through `component_id`. Unpublished paths return 404. Draft changes and definition updates stay invisible until publication.

### Register your app's components

Choose a stable component ID, such as `shop-product-promo`, and keep it in your app's renderer map. Registration sends only the field schema, never JavaScript or HTML. The CMS supports `text`, `textarea`, and `url` string fields with required flags.

```sh
curl --fail --silent --show-error -X PUT \
  "$CMS_URL/api/headless/components/shop-product-promo" \
  -H "Authorization: Bearer $BADDIE_COMPONENT_API_KEY" \
  -H 'Content-Type: application/json' \
  --data '{
    "id": "shop-product-promo",
    "name": "Product promotion",
    "description": "Rendered by the shop frontend",
    "renderer": "external",
    "fields": [
      {"name":"headline","label":"Headline","kind":"text","required":true},
      {"name":"product_url","label":"Product URL","kind":"url","required":false}
    ]
  }'
```

The PUT creates the definition with 201 or updates it with 200. IDs must match the path and contain 1–200 ASCII letters, numbers, underscores or hyphens. Repeated registration preserves IDs and existing page blocks. Updates that invalidate existing drafts fail without changing the definition. Registration cannot replace components using built-in renderers. Use app-prefixed IDs to avoid collisions between apps. The registration key can update any external definition in the installation, so give it only to trusted schema-maintenance code.

An administrator then opens System → Templates and allows the component in the desired regions. Editors can add instances and edit their fields through the usual page editor. The CMS shows a labelled content preview, including empty fields, rather than running remote code or reproducing your app's visual design. In your app, map `shop-product-promo` to your own component and pass the block's `fields` as its data. Escape text and validate links there too. Handle unknown IDs explicitly so a missing renderer does not silently erase content.

This first headless version delivers published content only. It does not provide remote draft preview, embedded external renderers, an SDK, or publication webhooks. Build-time consumers must fetch again and rebuild after publishing; server-rendered consumers can fetch on requests. **Published content is not private:** the existing `/api/content` endpoint and built-in public site remain unauthenticated. Headless API keys control the integration endpoints, not confidentiality of published pages.

## Editing model

- Pages have a path, template, ordered component instances, and a draft revision.
- Templates define named regions, component allowlists, and maximum counts. An empty allowlist permits nothing.
- Component definitions have typed text, textarea, or URL fields and a renderer. Hero, text, callout, and cards renderers ship with this version. Cards render one body line per card. External components use a content preview in the CMS and renderer code in your connected app.
- The experience editor renders the page with the same renderer as the public site. Select a block to edit its fields, add allowed components, drag to reorder, or use the move buttons.
- Save draft keeps edits private. Administrators can publish directly. Editors submit new or existing pages for review; reviewers approve and publish or request changes with feedback. Publication snapshots the page, template, and component definitions together. Later draft or schema edits do not alter that snapshot.
- Concurrent page saves return a conflict instead of overwriting another revision. Template and component changes that would invalidate existing drafts are rejected.

The sidebar keeps the page tree under Site available while editing. Administrators also see templates and components under System. Search finds accessible saved pages by title or path, individual blocks by component name or field text, and administrator-only definitions by name or description. Multiple search words must all match the same result. A block result opens that page with its properties selected; navigation asks before discarding unsaved edits. Search does not include unsaved changes or older published snapshots.

Choose a parent and a URL segment when creating or moving a page. `/about/team` belongs under `/about`; paths are the hierarchy, with no separate parent IDs. Missing intermediate pages appear as folders, so existing nested URLs still work. Renaming or moving a page moves every descendant draft URL in one transaction and increments their revisions. A stale editor must reload before saving. The root page `/` cannot move, and a page with descendants cannot be deleted until those descendants are moved or deleted.

Moving a draft does not change published URLs. Publish each affected page when ready; there are no automatic redirects or link rewrites.

To give a page a shorter or friendlier entry URL, open **Page details → Route aliases** and enter one absolute path per line, such as `/summer-sale`. Save and publish, or submit for review. Each alias then sends a server-side `301 Moved Permanently` redirect to the page's published path, preserving query parameters. The original page path remains canonical and appears in the browser after the redirect. Aliases are exact paths and do not redirect descendants. Paths already used by other pages or aliases are rejected; editors need grants for alias paths they add or remove.

Alias edits stay private until publication. Moving and republishing a page updates its alias targets without redirect chains. To keep the old page URL working after a move, explicitly add it as an alias. Clear an alias and publish to remove its live redirect; deleting the page removes all its aliases. Browsers and search engines may cache permanent redirects. Aliases also round-trip through page YAML exports. Headless consumers receive aliases in published Page JSON and must implement redirects on their own frontend host.

### Membership and review

Administrators manage membership and groups, edit schemas, delete pages, and publish directly. Reviewers, shown as super users in the membership form, can edit all pages and decide submissions, but cannot manage membership, change schemas, delete pages, or bypass review through direct publish. Editors can read, create, edit, move, and submit drafts only inside their individual or group path grants. All members can read component and template definitions for authoring.

A grant for `/news` covers `/news` and `/news/story`, not `/newspaper`. Grants are case-sensitive and remain tied to paths when pages move. `/` covers everything. Editors with no grants see no drafts. A move requires access to the old and new paths of the page and every descendant; a denied move changes nothing. Membership changes take effect on the next operation without signing in again. Local authorization and data changes share a database transaction. Once a local administrator exists, the API prevents removing the last one.

Use **Submit for review** in the page editor, then **Reviews** to inspect the submitted preview and exact data. Feedback appears in Reviews and the page editor. Each submission replaces the previous review for that page and gets a unique submission ID. Approval checks that ID, pending status, revision, full draft content, template, and used component definitions under the database lock. Any mismatch returns 409 and requires resubmission. This also catches CLI imports, subtree moves, schema edits, and replacement submissions at the same revision. Request changes requires nonempty feedback. Approval publishes immediately. Administrators and reviewers may approve their own submissions; this skeleton does not enforce separation of duties or keep an audit history of earlier reviews.

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

`pull --force` can replace malformed exports. Canonical export filenames in the selected directories belong to the export; unrelated regular files are preserved. All output is staged before replacement, and each file is replaced atomically. An interruption during replacement can still leave a mixture of old and new files; rerun pull before pushing that directory. Pull without `--pages` does not load page data.

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
node --experimental-strip-types --test tests/*.test.mjs
pnpm --dir web run check
pnpm --dir web run typecheck
pnpm --dir web run build
```

The browser tests use mock API responses and need no database. Install Chromium once, then run:

```sh
pnpm --dir web exec playwright install chromium
pnpm --dir web run test:ui
```

Set `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` to use an existing Chromium executable instead. The tests start the frontend dev server automatically and cover editing, publishing, reviews, template creation, permissions, and narrow-screen navigation. They do not replace the MySQL API tests.

Each API test creates a uniquely named `baddie_test_*` database and drops it afterward. Tests fail rather than silently skip when `TEST_DATABASE_URL` is missing. Do not point tests at production.

Configuration: `DATABASE_URL` and the selected authentication method's configuration are required. `BADDIE_BIND` overrides the listener; otherwise `PORT` selects `0.0.0.0:$PORT`, falling back to `127.0.0.1:3000` when absent. `BADDIE_STATIC` defaults to `web/dist`, and `BADDIE_SECURE_COOKIE` to false. The Docker image sets `PORT=3000` and `BADDIE_STATIC=/app/web`. `BADDIE_DB` is no longer used. `/health` checks database access. See [CONTRACT.md](CONTRACT.md) for the JSON API.

Component styles live beside their components in `stylex.create` declarations and use `stylex.attrs` for Solid's DOM attributes. Shared controls live in `web/src/common.stylex.ts`; `styles.css` contains only the document reset. Use explicit color and border properties, and compose conditional styles through `stylex.attrs`. The StyleX plugin runs before Solid and extracts production CSS. Run `pnpm --dir web run fmt` to format the frontend with Vite+.

## Extending and operating

Add renderer names to the Rust `Renderer` enum, the frontend `RendererName` type, and the `renderers` map in `web/src/components/Renderer.tsx`. Define their fields through the component editor. Built-in renderers use the keys `title`, `body`, `eyebrow`, `button_label`, and `button_url` where applicable. Arbitrary extra fields are stored but need renderer code to display them. Text is escaped; this version does not accept executable templates or raw HTML.

Run one CMS replica because sessions are process-local. Back up MySQL with `mysqldump --single-transaction` or Railway's database backups. Test restores before relying on backups. Tables use InnoDB. Database transactions serialize validation and writes through a lock row, including during overlapping deployments. Schema migration tooling beyond initial table creation is not yet included.

Current limits include no media library, rich-text editor, localization, review audit history, revision history/rollback UI, unpublish action, or plugin loading. Public pages render client-side, so server-rendered SEO is not yet covered. Template/component updates do not have optimistic concurrency checks. Performance has not been benchmarked. Admin and CLI operations are serialized; public content and health checks do not take the CMS lock. The server admits at most 16 database jobs at once and returns 503 when full.
