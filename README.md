# Baddiecore

> This project is AI-generated. And so is everytihing in this README.

A self-hosted CMS you can run in a Linux container and extend in source. It uses a Rust/Axum server, MySQL 8.4, and a Solid 2 editor styled with StyleX.

Editors build pages from templates and reusable components, save drafts, and publish or submit for review. Use the built-in site or fetch published content from your own app through the headless API.

This is an early version. See [current limits](docs/deployment.md#backups-and-operating-limits) before using it for a production site.

## Quick start

Use Docker Compose on Linux, or OrbStack on macOS.

1. Create an untracked `.env` in the repository root with separate long, unique passwords:

   ```dotenv
   BADDIE_ADMIN_PASSWORD=<admin-password>
   MYSQL_PASSWORD=<database-password>
   MYSQL_ROOT_PASSWORD=<database-root-password>
   ```

   Generate each password separately with `openssl rand -hex 32`. Keep the database password URL-safe because Compose embeds it in `DATABASE_URL`.

2. Start the CMS and database:

   ```sh
   docker compose up --build -d
   ```

3. Open `http://localhost:3000/admin`, sign in, and publish Home to make `/` available.

MySQL stores content in the `mysql_data` volume. **`docker compose down -v` deletes that data.** Before exposing the CMS to the Internet, follow the [TLS, cookie, and proxy setup](docs/deployment.md#run-in-a-linux-container).

## Documentation

- [Deployment and operations](docs/deployment.md) covers containers, Railway, server configuration, backups, and limits.
- [Authentication](docs/authentication.md) covers password login, WorkOS, administrator recovery, and custom providers.
- [Editing and publishing](docs/editing.md) covers pages, templates, route aliases, permissions, and reviews.
- [Headless integration](docs/headless.md) covers content API keys and registering your app's components.
- [Content CLI](docs/content-cli.md) covers YAML exports and imports for tracking content in Git.
- [Development and extensions](docs/development.md) covers local setup, tests, styles, and renderers.
- [API contract](CONTRACT.md) defines the JSON models and endpoints.
