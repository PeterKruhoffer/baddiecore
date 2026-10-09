# Development and extensions

[Back to README](../README.md)

## Run locally

Install Rust 1.98 or later, Node 24.11 or later, and pnpm. The frontend pins its pnpm version in `web/package.json`. Start MySQL with `docker compose up -d mysql` using the `.env` from [container setup](deployment.md#run-in-a-linux-container). In two terminals:

```sh
# Export BADDIE_ADMIN_PASSWORD and DATABASE_URL securely first.
# DATABASE_URL uses mysql://baddiecore:<MYSQL_PASSWORD>@127.0.0.1:3306/baddiecore
cargo run

pnpm --dir web install --frozen-lockfile
pnpm --dir web run dev
```

Vite+ proxies the API to the Rust server. A production build uses `pnpm --dir web run build`; Rust then serves `web/dist` directly. `.agents/setup` installs dependencies and builds both parts in an Amp orb. Vite+ runs through the project-local `vp` CLI, so no global Vite+ installation is required.

See [server configuration](deployment.md#server-configuration) for environment variables and [CONTRACT.md](../CONTRACT.md) for the JSON API.

## Checks and tests

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

## Frontend styles

Component styles live beside their components in `stylex.create` declarations and use `stylex.attrs` for Solid's DOM attributes. Shared controls live in `web/src/common.stylex.ts`; `styles.css` contains only the document reset. Use explicit color and border properties, and compose conditional styles through `stylex.attrs`. The StyleX plugin runs before Solid and extracts production CSS. Run `pnpm --dir web run fmt` to format the frontend with Vite+.

## Add renderers

Add renderer names to the Rust `Renderer` enum, the frontend `RendererName` type, and the `renderers` map in `web/src/components/Renderer.tsx`. Define their fields through the component editor. Built-in renderers use the keys `title`, `body`, `eyebrow`, `button_label`, and `button_url` where applicable. Arbitrary extra fields are stored but need renderer code to display them. Text is escaped; this version does not accept executable templates or raw HTML.

For renderer code hosted in another app, see [headless integration](headless.md). For custom sign-in, see [authentication providers](authentication.md#supply-your-own-authentication). See [operating limits](deployment.md#backups-and-operating-limits) for features this version does not support.
