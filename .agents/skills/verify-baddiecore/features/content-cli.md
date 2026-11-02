# Content CLI

Operators sync component and template definitions between the database and a Git-friendly YAML directory with `baddiecore status`, `pull` and `push`. Pages never sync this way.

## Sub-features

- `cli-status` classifies each path as changed in the database, in the files, or both, without writing.
- `cli-pull` writes database changes to the files, plus the `baddiecore.yaml` manifest.
- `cli-push-dry` reports what push would apply and writes nothing.
- `cli-push` applies file changes to the database in one transaction.
- `cli-conflict` refuses when a path changed on both sides, unless `--force`.

## How to get to it (user POV)

- `baddiecore status|pull|push [directory] [--force] [--dry-run]` with `DATABASE_URL` set. In a run that's `$BIN …` after `set -a; source …/env; set +a`.

## Driving it with the baddiecore binary

Preconditions:

- Baseline run; `D=$STATE/content` does not exist yet.

- **Status on a new directory.** `$BIN status $D` lists `added` for `components/callout.yaml`, `components/cards.yaml`, `components/hero.yaml`, `components/text.yaml`, `templates/homepage.yaml` under "Changed in the database; pull to copy them to the files". Exit 0.
- **Pull.** `$BIN pull $D` prints `pull: 5 changes applied to the files`. `$D/baddiecore.yaml` plus the five YAML files exist.
- **Edit a file.** `sed -i 's/^  name: Text$/  name: Text block/' $D/components/text.yaml`; `$BIN status $D` shows `modified  components/text.yaml` under "Changed in the files".
- **Dry run.** `$BIN push $D --dry-run` prints `push: 1 change would be applied to the database`. Then `$S/sql.sh $RUN_ID "SELECT JSON_EXTRACT(data,'$.name') FROM components WHERE id='text'"` still shows `"Text"`.
- **Push.** `$BIN push $D` prints `push: 1 change applied to the database`; the same query shows `"Text block"`, and `Components` in `/admin` lists `Text block`.
- **Conflict.** Change the `text` component's description in `/admin` (Components → Text) and in `$D/components/text.yaml`. `$BIN push $D` exits 2 with `push failed: changed in both the database and the files since the last sync:` followed by `components/text.yaml`. The database keeps its value until `push --force`.

## Gotchas

- `source env` without `set -a` leaves `DATABASE_URL` unexported, and the CLI fails with `DATABASE_URL is required`.
- Keep the directory under `$STATE` so cleanup removes it; never sync into the repo's tracked files.
- Unrelated files inside `components/` or `templates/` make push fail.
- `push --force` applies every difference, so a component added on the database side after the last pull (for example one registered through the headless API) is **deleted**. Run headless recipes after CLI ones, or pull first.
- The database keeps one sync base for all directories, so a second directory counts as file-side edits.
