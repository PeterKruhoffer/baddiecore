---
name: verify-baddiecore
description: Launch an isolated Baddiecore CMS (Rust server + MySQL 8.4 in Docker) from this checkout and drive it like an editor, admin, headless consumer or CLI operator, capturing proof. Use to verify a change to the admin editor at /admin, the public site, the /api endpoints, headless API, or the baddiecore status/pull/push CLI. Runs in an Amp orb; no Mac or OrbStack needed.
---

# Verify Baddiecore

Every run gets its own MySQL container, empty database, server port, admin password, headless keys, browser session and evidence folder. Runs never touch the `compose.yaml` stack on ports 3000/3306, so several can run side by side. Scripts live in `.agents/skills/verify-baddiecore/scripts/`; paths below are relative to the repository root.

The Playwright tests in `web/tests` mock the API. They are not proof that the real server works; drive a launched instance instead.

## Launch

```sh
S=.agents/skills/verify-baddiecore/scripts
$S/launch.sh <run-id>          # run-id optional; defaults to a timestamp
set -a; source /tmp/verify-baddiecore/<run-id>/env; set +a
```

`launch.sh`:

1. Starts Docker if it is not reachable. In an orb it runs `sudo dockerd --group $(id -gn)` as the supervised orb service `dockerd`, so `docker` works without sudo and survives Amp CLI restarts. Elsewhere it stops and asks you to start Docker.
2. Builds the checkout under test: `cargo build --locked` and `pnpm --dir web run build`.
3. Starts `mysql:8.4` as container `vbc-<run-id>-mysql` on a random loopback port, and waits until a TCP query as the app user succeeds. The image's init phase has no TCP, so this means init is finished.
4. Starts `target/debug/baddiecore` on a free loopback port with a generated `BADDIE_ADMIN_PASSWORD` and both headless keys. It **unsets every inherited `BADDIE_*` variable** first: the orb's project env sets `BADDIE_SECURE_COOKIE=true`, which makes browser sign-in fail with 403 "origin does not match host".
5. Waits for `/health`, then runs doctor. It prints `ready:` only when doctor passes.

A new database migrates and seeds itself: components hero/text/callout/cards, template `homepage`, and an **unpublished** Home draft at `/` (id `home`, revision 1, hero title "Build your site"). Public `/` returns 404 until Home is published.

The env file defines `BASE_URL`, `PORT`, `ADMIN_USER` (`admin`), `ADMIN_PASSWORD`, `CONTENT_API_KEY`, `COMPONENT_API_KEY`, `DATABASE_URL`, `BIN`, `BROWSER_SESSION`, `EVIDENCE`, `STATE`, `MYSQL_CONTAINER` and `SERVER_PID`. Read secrets from it; don't print them.

The server runs as a plain background process. If the Amp CLI restarts mid-run it dies; doctor reports that, and the fix is cleanup and a new launch.

## Doctor

```sh
$S/doctor.sh <run-id>
```

Run it first whenever something looks off. Every line must be `ok`: our PID runs our binary, it owns the port, the MySQL container is running, `/health` is 200, admin sign-in returns 204 **with the browser's Origin header**, bootstrap shows the admin role and a page at `/`, and `/admin` serves the built editor. It also prints the git revision and whether `src` or `web/src` have uncommitted changes. Doctor signs in once, which adds a session row; it changes no content.

## Drive

- **Browser** (admin editor, public site): `agent-browser --session "$BROWSER_SESSION" …`. After `open`, run `set viewport 1280 800 2`. Find elements by role and accessible name (`find role button click --name "Publish" --exact`, `find label "Username" fill admin`). When labels repeat on a screen, such as the member forms on Organization, take `@eN` refs from `snapshot -i` and use them right away; refs change on every snapshot.
- **HTTP**: `curl` against `$BASE_URL`. Cookie mutations need `-H "Origin: $BASE_URL"` or no Origin at all. Headless routes need `Authorization: Bearer $CONTENT_API_KEY` or `$COMPONENT_API_KEY`.
- **CLI**: `$BIN status|pull|push <dir>` with `DATABASE_URL` exported (that's why the env file is sourced with `set -a`). Use a directory under `$STATE`.
- **Database side effects**: `$S/sql.sh <run-id> "<SQL>"`. Page, review and component rows keep their model as JSON in `data`, e.g. `SELECT id, JSON_EXTRACT(data,'$.revision'), JSON_EXTRACT(data,'$.published_revision') FROM pages`. Published content lives in `snapshots`.

Feature recipes are in [features/README.md](features/README.md). That map is the list of what to cover; a proof that drives one convenient path is incomplete when the map lists others.

## Evidence

Save everything to `$EVIDENCE/<feature>/` (that is, `.amp/in/artifacts/verify-baddiecore/<run-id>/<feature>/`, which git ignores):

- a screenshot of the state before the action, after the action, and of a second view that shows the result (for example the public page after publishing in the editor);
- the API or CLI command with its output and exit code;
- a database query showing the side effect (revision, `published_revision`, snapshot row, review status).

Rules: go through the real user path (UI, public HTTP, CLI), not internal setters or test-only routes. Show the action and the resulting state, not just the final screen. Check side effects as well as what's visible. `push --dry-run` is the one dry run here; prove it skipped the write by querying the database afterwards. Look at every screenshot with view_media before citing it.

## Cleanup

```sh
$S/cleanup.sh <run-id>
```

Kills the server only if `SERVER_PID` still runs `$BIN`, removes `vbc-<run-id>-mysql` and its anonymous volume, closes the browser session, and deletes `/tmp/verify-baddiecore/<run-id>`. It keeps `$EVIDENCE` and prints its path. It leaves the shared `dockerd` service running; stop it with `amp orb service stop dockerd` only when nothing else uses Docker. Run cleanup after failed attempts too. Never kill by process name.
