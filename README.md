# Baddiecore

A self-hosted CMS with a Rust/Axum server, SQLite storage, and a Solid 2 editor styled with StyleX. The frontend uses Vite+ and pnpm. This is an early first version, not a Sitecore replacement.

## Run in a Linux container

Use Docker Compose on Linux, or OrbStack on macOS. Create an untracked `.env` with `BADDIE_ADMIN_PASSWORD` set to a long unique password, then run:

```sh
docker compose up --build -d
```

Open `http://localhost:3000/admin`. Nothing is published initially. Open Home, edit its components, and publish to make `/` available. The container runs as a non-root user and persists SQLite in the `content` volume. Never run `docker compose down -v` unless you intend to delete that data.

For an Internet-facing installation, put a TLS reverse proxy in front of the loopback-bound port, preserve the original Host header, set `BADDIE_SECURE_COOKIE=true`, and rate-limit `/api/login` at the proxy. Do not expose the admin login over plain HTTP. Authentication is a single shared administrator password, not multi-user accounts. Sessions expire after 12 hours and all sessions end on server restart.

## Editing model

- Pages have a path, template, ordered component instances, and a draft revision.
- Templates define named regions, component allowlists, and maximum counts. An empty allowlist permits nothing.
- Component definitions have typed text, textarea, or URL fields and a renderer. Hero, text, callout, and cards renderers ship with this version. Cards render one body line per card.
- The experience editor renders the page with the same renderer as the public site. Select a block to edit its fields, add allowed components, drag to reorder, or use the move buttons.
- Save draft keeps edits private. Publish saves and snapshots the page, template, and component definitions together. Later draft or schema edits do not alter that snapshot.
- Concurrent page saves return a conflict instead of overwriting another revision. Template and component changes that would invalidate existing drafts are rejected.

Paths use ASCII letters, numbers, hyphens, underscores, and slash-separated segments. `/admin`, `/api`, `/health`, and `/assets` are reserved. Rules in this version govern placement, counts, required fields, and field types, not audience targeting or personalization.

## Development

Install Rust 1.98 or later, Node 24.11 or later, and pnpm. The frontend pins its pnpm version in `web/package.json`. In two terminals:

```sh
# Export BADDIE_ADMIN_PASSWORD securely in this shell first.
cargo run

pnpm --dir web install --frozen-lockfile
pnpm --dir web run dev
```

Vite+ proxies the API to the Rust server. A production build uses `pnpm --dir web run build`; Rust then serves `web/dist` directly. `.agents/setup` installs dependencies and builds both parts in an Amp orb. Vite+ runs through the project-local `vp` CLI, so no global Vite+ installation is required.

```sh
cargo test --locked
cargo clippy --all-targets -- -D warnings
pnpm --dir web run check
pnpm --dir web run typecheck
pnpm --dir web run build
```

Configuration: `BADDIE_BIND` defaults to `127.0.0.1:3000`, `BADDIE_DB` to `data/baddiecore.db`, `BADDIE_STATIC` to `web/dist`, and `BADDIE_SECURE_COOKIE` to false. `BADDIE_ADMIN_PASSWORD` is required. See [CONTRACT.md](CONTRACT.md) for the JSON API.

Component styles live beside their components in `stylex.create` declarations and use `stylex.attrs` for Solid's DOM attributes. Shared controls live in `web/src/common.stylex.ts`; `styles.css` contains only the document reset. Use explicit color and border properties, and compose conditional styles through `stylex.attrs`. The StyleX plugin runs before Solid and extracts production CSS. Run `pnpm --dir web run fmt` to format the frontend with Vite+.

## Extending and operating

Add renderer names to the Rust `Renderer` enum, the frontend `RendererName` type, and the `renderers` map in `web/src/components/Renderer.tsx`. Define their fields through the component editor. Built-in renderers use the keys `title`, `body`, `eyebrow`, `button_label`, and `button_url` where applicable. Arbitrary extra fields are stored but need renderer code to display them. Text is escaped; this version does not accept executable templates or raw HTML.

Run one CMS instance per SQLite volume. Back up with SQLite's online backup API, or stop the container before copying the whole volume. Do not copy only the database file while WAL writes are active. Test restores before relying on backups. Schema migration tooling is not yet included.

Current limits include no media library, rich-text editor, localization, page hierarchy, roles, workflow approvals, revision history/rollback UI, unpublish action, or plugin loading. Public pages render client-side, so server-rendered SEO is not yet covered. Template/component updates do not have optimistic concurrency checks. Performance has not been benchmarked; the database serializes operations through one connection.
