# First-version contract

Rust Axum server, MySQL 8.4 persistence, Solid 2 SPA. API JSON uses snake_case.

## Models

- Field: `{name, label, kind: "text" | "textarea" | "url", required: boolean}`
- Component: `{id, name, description, renderer: "hero" | "text" | "callout" | "cards", fields: Field[]}`
- Region: `{name, allowed_components: string[], max_components: number}`. Maximum is positive. An empty allowed list permits no components.
- Template: `{id, name, description, regions: Region[]}`
- Block: `{id, component_id, region, fields: Record<string,string>}`
- Page: `{id, title, slug, template_id, blocks: Block[], revision: number, published_revision: number | null}`. Slugs are case-sensitive absolute paths such as `/` and `/about`, at most 2048 bytes.
- Bootstrap: `{pages: Page[], templates: Template[], components: Component[]}`

## API

All `/api/admin/*` require a signed-in session. Errors are `{error: string}` with suitable HTTP status. Unknown IDs return 404. Mutations validate references and rules before persistence.

- `GET /api/auth/config` returns `{method: "password"}` or `{method: "redirect", label: string}`. It never returns secrets. Login UI waits for this response and shows an error with retry if it fails.
- `POST /api/login` with `{password}` is available only in password mode. Returns 204 and sets an HttpOnly SameSite=Strict session cookie. `BADDIE_AUTH` defaults to `password`, requiring `BADDIE_ADMIN_PASSWORD`. No default password.
- `GET /api/login` starts hosted sign-in in WorkOS mode. `GET /api/auth/callback` checks browser-bound single-use state and PKCE, exchanges the code and requires a verified user in `WORKOS_ORGANIZATION_ID`. Success issues an opaque session cookie and redirects to `/admin`; failure redirects to `/admin?auth_error=1`. The temporary login cookie is HttpOnly SameSite=Lax and expires after ten minutes. WorkOS mode never accepts password login.
- `POST /api/logout` invalidates and clears the local cookie. Returns 204 for password mode or absent sessions. For a WorkOS session it returns `{redirect_url: string}`; the frontend must navigate there to end the hosted session. Custom providers may use either response.
- `GET /api/admin/bootstrap` returns Bootstrap.
- `POST /api/admin/pages` with `{title, slug, template_id}` returns Page, 201, empty blocks and revision 1.
- `PUT /api/admin/pages/{id}` with full Page updates draft and returns Page. Require matching revision, increment on save; stale writes return 409. Changing template must validate all blocks. A changed slug atomically moves all descendants by path-segment prefix and increments their revisions too. Reject root moves, moves into the same subtree, invalid descendant paths, and destination collisions. Published snapshots remain unchanged. Reload bootstrap after a move.
- `POST /api/admin/pages/{id}/publish` with `{revision}` validates and snapshots saved draft, returns Page. Stale revision returns 409.
- `DELETE /api/admin/pages/{id}` returns 204. Returns 409 if any descendant draft remains; no recursive deletion.
- `POST /api/admin/templates` with Template excluding id returns Template, 201.
- `PUT /api/admin/templates/{id}` with Template validates existing drafts and returns Template. Reject changes that invalidate existing pages. Published snapshots remain unchanged.
- `POST /api/admin/components` with Component excluding id returns Component, 201.
- `PUT /api/admin/components/{id}` with Component validates existing drafts and returns Component. Reject changes that invalidate existing pages. Published snapshots remain unchanged.
- `GET /api/content?slug=/about` public, returns `{page, template, components}` from immutable published snapshot, 404 for unpublished.
- `GET /health` returns 200 when the database is accessible, 500 on database failure.

Server serves frontend dist with SPA fallback. Editor path `/admin`; other paths render public pages. Frontend Vite proxy `/api` and `/health` to port 3000. `BADDIE_BIND` overrides the listener; otherwise `PORT` binds `0.0.0.0:$PORT`, or `127.0.0.1:3000` if absent. `DATABASE_URL` is a required MySQL connection URL; `BADDIE_STATIC` defaults `web/dist`. Tables use InnoDB, and each database operation runs in a transaction with a shared lock row to serialize validation and writes across connections. Run one CMS replica because sessions remain process-local.

Seed idempotently on a new database: hero with eyebrow/title/body/button_label/button_url, text with title/body, callout with title/body/button_label/button_url, cards with title/body. Required title; other fields optional. Homepage template with main region allowing all components, maximum 20. Home page draft with hero and text, suitable editorial starter copy. IDs are arbitrary strings, frontend must discover them.

## Local serialization

`baddiecore pull [directory] [--pages] [--force]` and `baddiecore push [directory] [--pages] [--dry-run]` connect directly to the initialized database using `DATABASE_URL`. No HTTP routes or new authentication method. Default directory: `baddiecore-content`.

YAML files have `{schema_version: 1, kind: component | template | page, data: ...}`. Components and templates use the models above. Page data contains only `id`, `title`, `slug`, `template_id`, and `blocks`. Filenames are `<id>.yaml` for 1–200 ASCII alphanumeric, underscore or hyphen bytes; other IDs use `id.<sha256-of-utf8-id>.yaml`. Files are grouped into `components/`, `templates/`, and opt-in `pages/`. Preserve array order and sort block field keys. Reject unknown keys, unsupported versions, incorrect filenames, unsafe paths, and files over 4 MiB.

Push uses the same database lock as API mutations, merges the selected kinds by ID, and validates all resulting drafts before commit. Failure rolls back the entire import; dry run always rolls back. Missing files never delete database records. Import paths are explicit final paths, with no subtree cascade. New pages start at revision 1, changed drafts increment the target revision, and identical drafts retain it. Preserve existing published revisions and all snapshots. Pull refuses differences or obsolete files unless forced; force replaces exports and removes obsolete files within selected kinds. Omitting `--pages` ignores the pages directory entirely.

Security: no raw HTML field rendering, validate URL fields as relative paths or http/https URLs, refuse protocol-relative URLs. Cookie auth mutations require same-origin Origin when present. Production TLS provided by reverse proxy, configurable Secure cookie. Request size limit. Do not log password or cookie.

Authentication is replaceable through `auth::AuthProvider` and `AppState::open_with_auth`. The provider supplies its own state-bound login routes and validates headers into an `Editor { id }`, granting full editor access. CMS middleware rejects unauthorized admin requests, retains same-origin mutation checks, and adds Editor to request extensions. Public content does not call the provider. Custom auth routes must enforce their own CSRF checks and keep credentials server-side.

Built-in sessions expire after twelve hours or server restart. WorkOS requires `WORKOS_API_KEY`, `WORKOS_CLIENT_ID`, `WORKOS_REDIRECT_URI` and `WORKOS_ORGANIZATION_ID`; an HTTPS callback also requires secure cookies. The backend refreshes WorkOS sessions on access after at most five minutes or token expiry, whichever comes first. Refresh rechecks the organization and user, rotates the refresh token, and rejects revoked sessions. Failed refresh discards the local session. Auth responses use `Cache-Control: no-store` and `Referrer-Policy: no-referrer`. Do not log callback query strings or WorkOS tokens.
