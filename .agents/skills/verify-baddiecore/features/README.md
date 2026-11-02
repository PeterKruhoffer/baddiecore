# Baddiecore verification map

This directory is the maintained list of user-facing behavior to verify in Baddiecore. Read this index, then use the matching feature file as the recipe.

## Baseline preconditions

- A run launched with `.agents/skills/verify-baddiecore/scripts/launch.sh <run-id>`, its env file sourced with `set -a; source /tmp/verify-baddiecore/<run-id>/env; set +a`, and `doctor.sh <run-id>` all `ok`.
- Fresh seed: unpublished Home draft at `/` (revision 1, hero title "Build your site", text block "Start publishing"), components Hero/Text/Callout/Cards, template Homepage, a single member `admin`.
- `AB="agent-browser --session $BROWSER_SESSION"` for browser steps.
- Never drive an instance this run did not start.

## Driving conventions

- Start each recipe from the baseline unless its preconditions say otherwise. A recipe that mutates Home changes the baseline for later recipes in the same run; launch a new run when that matters.
- Use roles and accessible names. When a name repeats on a screen, use `@eN` refs from a fresh `snapshot -i`.
- After `Sign out`, wait for the `Enter workspace` button; the form shows "Loading sign-in…" first.
- Treat commands as literal.

## Proof and skip reporting

- Capture the action and the resulting state, plus a second view (public page, API, CLI or SQL) that confirms it.
- CLI proof includes the command, stdout and exit code.
- Save artifacts under `$EVIDENCE/<feature-file-name>/`.
- If a path can't be reached, report the command you tried and the precondition that was missing. Don't report it as verified through a different path.

## Feature entry contract

Each feature file has an H1 and one paragraph, then four H2s in order: `Sub-features`, `How to get to it (user POV)`, `Driving it with <harness>` (starting with `Preconditions:`), and `Gotchas`. Keep implementation details out; name user paths, stable handles, required state and observable proof.

## Features

- [Sign in](./sign-in.md) covers browser sign-in, wrong passwords, per-username lockout and sign-out.
- [Edit and publish a page](./edit-publish.md) covers editing a draft in the editor and publishing it to the public site. **Proven by the skill's first run.**
- [Review workflow](./review.md) covers adding an editor, submitting for review, and approving or requesting changes.
- [Headless API](./headless.md) covers content key reads and component key registration.
- [Content CLI](./content-cli.md) covers `status`, `pull`, `push --dry-run` and `push` for components and templates.
