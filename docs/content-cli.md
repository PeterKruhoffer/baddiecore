# Track content definitions in Git

[Back to README](../README.md)

The same `baddiecore` binary runs the server and a local CLI. Start the CMS once to initialize its database. The CLI connects directly through `DATABASE_URL`, without editor authentication or a remote HTTP endpoint. Point it only at the local instance you intend to modify.

## Pull and push

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

## Use the CLI with Compose

For a container-based local instance, first [configure Compose](deployment.md#run-in-a-linux-container) and run `docker compose up --build -d`, then use a one-off CLI container with the same database configuration and a bind-mounted export directory:

```sh
mkdir -p baddiecore-content
docker compose run --rm --no-deps --user "$(id -u):$(id -g)" \
  --volume "$PWD/baddiecore-content:/content" cms baddiecore pull /content
docker compose run --rm --no-deps --user "$(id -u):$(id -g)" \
  --volume "$PWD/baddiecore-content:/content" cms baddiecore push /content --dry-run
```

Remove `--dry-run` to apply. Add `--pages` when page content belongs in Git. The user override keeps files writable by your host account on Linux; this workflow also works with OrbStack. The running CMS sees imports on the next reload.
