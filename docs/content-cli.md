# Track content definitions in Git

[Back to README](../README.md)

Two kinds of work leave an installation in different ways. Templates and components are the site's structure, shared by everyone working on the project, so they go into Git. Content pages, including their paths and aliases, are editorial work, so they move as zip packages; see [moving content pages](editing.md#move-content-pages-between-installations).

The same `baddiecore` binary runs the server and a local CLI. Start the CMS once to initialize its database. The CLI connects directly through `DATABASE_URL`, without editor authentication or a remote HTTP endpoint. Point it only at the installation you intend to modify.

## Pull and push

```sh
cargo run -- status                       # what changed in the CMS and in the files
cargo run -- pull                         # CMS changes → baddiecore-content/
cargo run -- push --dry-run               # validate applying file changes, then roll back
cargo run -- push                         # file changes → this CMS
```

The everyday loop is simple. After changing templates or components in the CMS, run `pull` and commit. After getting commits from Git, run `push`. Deletions travel both ways: delete a template in the CMS and `pull` deletes its file; delete a file in Git and `push` deletes the template.

Each installation remembers what it and the files agreed on at its last pull or push. Comparing against that tells each command which side changed an item, so neither undoes the other's work:

- `pull` copies only changes made in the CMS. Changes that arrived through Git and aren't pushed yet are left alone and listed, so a pull never exports a deleted template back into Git or reverts a teammate's edit.
- `push` copies only changes made in the files. Templates or components you created in the CMS but haven't pulled yet are kept and listed.
- If an item changed on both sides, both commands stop and list it. Choose with `pull --force` to keep the CMS version, or `push --force` to keep the files. With `--force` the target becomes an exact copy of the source: `push --force` also deletes CMS-only items, and `pull --force` discards unpushed file changes. Run `push --force --dry-run` first to see what it would do.

`push` never deletes a template or component that pages or templates on that installation still use. It stops with a message such as `template article is still used by page /news`; move those pages first. The admin UI applies the same rule when you delete a definition. Published pages keep their own copies, so deleting definitions never changes the live site.

`pull` writes `baddiecore.yaml`, which marks the directory as an export. Commit it. `push` requires it, so run `pull` once to start. Neither command reads or changes content pages. A fresh installation counts its starter templates and components as already synced. Its first `push` therefore applies the repository's version, including deletions, instead of exporting the starter items back. Use one export directory per installation.

Each item has a versioned YAML file under `components/` or `templates/`. Filenames use stable IDs, usually UUIDs; unusual IDs use a SHA-256 filename. Keep IDs and filenames unchanged when editing existing items. Output is deterministic, including sorted block field keys, and excludes revisions, publication metadata, snapshots, sessions, and credentials. Formatting-only edits don't count as changes. Unrelated files are preserved by `pull`, but `push` rejects them inside the item directories. The CLI never commits or pushes to Git itself.

`push` applies everything in one transaction and validates references and all resulting page drafts. Changed pages get the target database's next revision; unchanged pages keep theirs. YAML page paths describe the final tree, so when moving a branch in files, update its descendants too. A successful dry run is not a reservation; push revalidates the database when it runs. `pull` stages output before replacing files, and each file is replaced atomically. An interruption can still leave a mixture of old and new files; run `pull` again.

The same rules let a deployment pipeline run `push` against dev, test and production from the repository. If someone changed a definition directly on that installation, the push stops instead of silently overwriting it. `push --force` resets it to the repository.

## Use the CLI with Compose

For a container-based instance, first [configure Compose](deployment.md#run-in-a-linux-container) and run `docker compose up --build -d`. You don't need Rust installed: `scripts/content` runs the CLI inside the CMS image against `baddiecore-content/` in the repository:

```sh
scripts/content status
scripts/content pull                      # CMS changes → baddiecore-content/
scripts/content push --dry-run
scripts/content push
```

It passes its arguments through, adds the directory, and reads the same `.env`. Set `BADDIE_CONTENT_DIR` to use another directory. Rebuild the image after changing CMS code, since the CLI runs from the image. The equivalent manual command uses a one-off CLI container with the same database configuration and a bind-mounted export directory:

```sh
mkdir -p baddiecore-content
docker compose run --rm --no-deps --user "$(id -u):$(id -g)" \
  --volume "$PWD/baddiecore-content:/content" cms baddiecore pull /content
docker compose run --rm --no-deps --user "$(id -u):$(id -g)" \
  --volume "$PWD/baddiecore-content:/content" cms baddiecore push /content --dry-run
```

Remove `--dry-run` to apply. The user override keeps files writable by your host account on Linux; this workflow also works with OrbStack. The running CMS sees imports on the next reload.
