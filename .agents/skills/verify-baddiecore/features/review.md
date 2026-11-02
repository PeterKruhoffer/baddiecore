# Review workflow

Editors can't publish. They submit a saved revision for review; an admin or reviewer sees the exact submitted snapshot in Reviews and either approves it, which publishes it, or requests changes with feedback.

## Sub-features

- `member-add` lets an admin add an editor with a password and path grants.
- `review-submit` saves and submits the editor's draft; nothing goes public.
- `review-approve` publishes the submitted snapshot.
- `review-changes` requires feedback and marks the review `changes_requested`.

## How to get to it (user POV)

- Admin: `Organization` → `Add member` form → `Save member`.
- Editor: open a page → `Submit for review` → `Save and submit`.
- Admin/reviewer: `Reviews N pending` → submission button → `Approve and publish` or `Request changes`.

## Driving it with agent-browser

Preconditions:

- Baseline run, signed in as admin in the browser.

- **Add editor.** `$AB find role button click --name "Organization"`, then `$AB snapshot -i`. Under heading `Add member`, fill the `Username` (`erin`), `Display name` (`Erin Editor`), `Password` (`editor-pass-123`) and `Individual path grants, separated by commas` (`/`) textboxes by their `@eN` refs, and click that form's `Save member`. A heading `Erin Editor` appears under `Members`.
- **Editor submits.** `Sign out`, sign in as `erin`, open `Home`, fill `Title *` with `Erin's headline`, choose `Submit for review`. Dialog `Submit Home for review?` with radio `As soon as it is approved`; choose `Save and submit`. Status shows `In review` and `Submitted revision 3` (2 if Home was never published).
- **Still private.** `curl -s "$BASE_URL/api/content?slug=/"` still shows the previous title (or 404 if never published).
- **Admin approves.** Sign out, sign in as admin, choose `Reviews 1 pending`. Button `Home / Revision 3 · Submitted by erin`; region `Submitted preview` shows heading `Erin's headline`. Choose `Approve and publish`. The `Approved (1)` filter count updates.
- **Side effects.** The content API returns `Erin's headline`; `$S/sql.sh $RUN_ID "SELECT id, JSON_EXTRACT(data,'$.status'), JSON_EXTRACT(data,'$.submitted_by'), JSON_EXTRACT(data,'$.reviewed_by') FROM reviews"` shows `home | "approved" | "erin" | "admin"`.
- **Request changes.** Submit again as erin. As admin, fill the `Feedback Required when requesting changes.` textbox and choose `Request changes`. The review status becomes `changes_requested`, and the public page is unchanged. Feedback is required; an empty one must be refused.

## Gotchas

- `Username`, `Password` and `Save member` appear twice on Organization (existing admin and Add member). Use refs, not `find label`.
- An editor with no path grants can't see Home. Grant `/` or a path covering the page.
- Editing the draft after submission makes approval return 409; resubmit first.
- A requested publish time (`At a specific date and time`) makes the button `Approve and schedule`, which schedules instead of publishing.
