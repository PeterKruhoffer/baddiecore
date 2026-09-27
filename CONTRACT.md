# First-version contract

Rust Axum server, SQLite persistence, Solid 2 SPA. API JSON uses snake_case.

## Models

- Field: `{name, label, kind: "text" | "textarea" | "url", required: boolean}`
- Component: `{id, name, description, renderer: "hero" | "text" | "callout" | "cards", fields: Field[]}`
- Region: `{name, allowed_components: string[], max_components: number}`. Maximum is positive. An empty allowed list permits no components.
- Template: `{id, name, description, regions: Region[]}`
- Block: `{id, component_id, region, fields: Record<string,string>}`
- Page: `{id, title, slug, template_id, blocks: Block[], revision: number, published_revision: number | null}`. Slugs are absolute paths such as `/` and `/about`.
- Bootstrap: `{pages: Page[], templates: Template[], components: Component[]}`

## API

All `/api/admin/*` require a signed-in session. Errors are `{error: string}` with suitable HTTP status. Unknown IDs return 404. Mutations validate references and rules before persistence.

- `POST /api/login` with `{password}` sets HttpOnly SameSite=Strict session cookie. `POST /api/logout` clears it. Admin password comes from `BADDIE_ADMIN_PASSWORD`, required at startup. No default password.
- `GET /api/admin/bootstrap` returns Bootstrap.
- `POST /api/admin/pages` with `{title, slug, template_id}` returns Page, 201, empty blocks and revision 1.
- `PUT /api/admin/pages/{id}` with full Page updates draft and returns Page. Require matching revision, increment on save; stale writes return 409. Changing template must validate all blocks.
- `POST /api/admin/pages/{id}/publish` with `{revision}` validates and snapshots saved draft, returns Page. Stale revision returns 409.
- `DELETE /api/admin/pages/{id}` returns 204.
- `POST /api/admin/templates` with Template excluding id returns Template, 201.
- `PUT /api/admin/templates/{id}` with Template validates existing drafts and returns Template. Reject changes that invalidate existing pages. Published snapshots remain unchanged.
- `POST /api/admin/components` with Component excluding id returns Component, 201.
- `PUT /api/admin/components/{id}` with Component validates existing drafts and returns Component. Reject changes that invalidate existing pages. Published snapshots remain unchanged.
- `GET /api/content?slug=/about` public, returns `{page, template, components}` from immutable published snapshot, 404 for unpublished.
- `GET /health` returns 200.

Server serves frontend dist with SPA fallback. Editor path `/admin`; other paths render public pages. Frontend Vite proxy `/api` and `/health` to port 3000. Server defaults port 3000, `BADDIE_BIND` override; `BADDIE_DB` defaults `data/baddiecore.db`; `BADDIE_STATIC` defaults `web/dist`.

Seed idempotently on a new database: hero with eyebrow/title/body/button_label/button_url, text with title/body, callout with title/body/button_label/button_url, cards with title/body. Required title; other fields optional. Homepage template with main region allowing all components, maximum 20. Home page draft with hero and text, suitable editorial starter copy. IDs are arbitrary strings, frontend must discover them.

Security: no raw HTML field rendering, validate URL fields as relative paths or http/https URLs, refuse protocol-relative URLs. Cookie auth mutations require same-origin Origin when present. Production TLS provided by reverse proxy, configurable Secure cookie. Request size limit. Do not log password or cookie.
