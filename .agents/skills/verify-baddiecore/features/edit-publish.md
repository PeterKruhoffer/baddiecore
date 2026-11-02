# Edit and publish a page

An admin opens a page in the editor at `/admin`, changes a field, sees the preview update, and publishes. The public site and content API then serve the new published snapshot, while later draft edits stay private.

## Sub-features

- `edit-field` changes a block field and updates the preview.
- `publish-now` saves the draft and publishes it from the Publish dialog.
- `public-render` serves the published snapshot at the page's public path.
- `draft-private` keeps unpublished draft edits off the public site.

## How to get to it (user POV)

- Sign in at `/admin`, choose the page in the sidebar (`Home`), edit, choose `Publish`.
- Choose `Publish` with unsaved changes: the dialog saves first (`Save and publish`).
- Read the result at `/` (or `View site ↗`) and `GET /api/content?slug=/`.

## Driving it with agent-browser

Preconditions:

- Baseline run; Home unpublished (`curl -s -o /dev/null -w '%{http_code}' "$BASE_URL/api/content?slug=/"` prints `404`).
- Signed in as admin: `$AB open "$BASE_URL/admin"`, `$AB find label "Username" fill admin`, `$AB find label "Password" fill "$ADMIN_PASSWORD"`, `$AB find role button click --name "Enter workspace"`.

- **Open Home.** `$AB find role button click --name "Home" --exact`. A textbox named `Page title` shows `Home`, and the Hero fields appear.
- **Edit the hero title.** `$AB find role textbox fill "Verified headline $RUN_ID" --name "Title *" --exact`. The preview heading (level 1) shows the new text. Screenshot `1-edited-draft.png`.
- **Open publish.** `$AB find role button click --name "Publish" --exact`. A dialog headed `Publish Home?` with radio `Now` checked and button `Save and publish`.
- **Publish.** `$AB find role button click --name "Save and publish"`. The status reads `Saved draft · Revision 2` and the header shows `Published`. Screenshot `3-after-publish.png`.
- **Public view.** `$AB open "$BASE_URL/"`. Heading level 1 is `Verified headline <run-id>`. Screenshot `4-public-home.png`.
- **API view.** `curl -s "$BASE_URL/api/content?slug=/"` returns `page.revision` 2 and the new hero title.
- **Side effect.** `$S/sql.sh $RUN_ID "SELECT id, JSON_EXTRACT(data,'$.revision') r, JSON_EXTRACT(data,'$.published_revision') p FROM pages; SELECT page_id, slug FROM snapshots"` shows `home | 2 | 2` and a snapshot row `home | /`.
- **Draft stays private.** Edit the title again and choose `Save draft` without publishing. `/` and the content API still show the previous title.

## Gotchas

- `Publish` is admin only. Editors see `Submit for review` instead (see [review.md](./review.md)).
- The status line `Saved draft · Revision N` stays visible after publishing. The `Published` label next to it is the publish signal.
- The Hero `Title *` field and the `Page title` textbox are different fields; the page title is not shown on the public page.
- Stale revisions return 409 and keep the dialog open. Two tabs editing the same page will hit this.
