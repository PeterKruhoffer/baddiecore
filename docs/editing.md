# Editing and publishing

[Back to README](../README.md)

## Editing model

- Pages have a path, template, ordered component instances, and a draft revision.
- Templates define named regions, component allowlists, and maximum counts. An empty allowlist permits nothing.
- Component definitions have typed text, textarea, or URL fields and a renderer. Hero, text, callout, and cards renderers ship with this version. Cards render one body line per card. External components use a content preview in the CMS and renderer code in your connected app.
- The experience editor renders the page with the same renderer as the public site. Select a block to edit its fields, add allowed components, drag to reorder, or use the move buttons.
- Save draft keeps edits private. Administrators can publish directly. Editors submit new or existing pages for review; reviewers approve and publish or request changes with feedback. Publication snapshots the page, template, and component definitions together. Later draft or schema edits do not alter that snapshot.
- Concurrent page saves return a conflict instead of overwriting another revision. Template and component changes that would invalidate existing drafts are rejected.

The sidebar keeps the page tree under Site available while editing. Administrators also see templates and components under System. Search finds accessible saved pages by title or path, individual blocks by component name or field text, and administrator-only definitions by name or description. Multiple search words must all match the same result. A block result opens that page with its properties selected; navigation asks before discarding unsaved edits. Search does not include unsaved changes or older published snapshots.

## Page paths and moves

Choose a parent and a URL segment when creating or moving a page. `/about/team` belongs under `/about`; paths are the hierarchy, with no separate parent IDs. Missing intermediate pages appear as folders, so existing nested URLs still work. Renaming or moving a page moves every descendant draft URL in one transaction and increments their revisions. A stale editor must reload before saving. The root page `/` cannot move, and a page with descendants cannot be deleted until those descendants are moved or deleted.

Moving a draft does not change published URLs. Publish each affected page when ready; there are no automatic redirects or link rewrites.

Paths use ASCII letters, numbers, hyphens, underscores, and slash-separated segments, up to 2048 bytes. Paths are case-sensitive. `/admin`, `/api`, `/health`, and `/assets` are reserved. Rules in this version govern placement, counts, required fields, and field types, not audience targeting or personalization.

## Route aliases

To give a page a shorter or friendlier entry URL, open Page details → Route aliases and enter one absolute path per line, such as `/summer-sale`. Save and publish, or submit for review. Each alias then sends a server-side `301 Moved Permanently` redirect to the page's published path, preserving query parameters. The original page path remains canonical and appears in the browser after the redirect. Aliases are exact paths and do not redirect descendants. Paths already used by other pages or aliases are rejected; editors need grants for alias paths they add or remove.

Alias edits stay private until publication. Moving and republishing a page updates its alias targets without redirect chains. To keep the old page URL working after a move, explicitly add it as an alias. Clear an alias and publish to remove its live redirect; deleting the page removes all its aliases. Browsers and search engines may cache permanent redirects. Aliases also travel in [content packages](#move-content-pages-between-installations). Headless consumers receive aliases in published Page JSON and must implement redirects on their own frontend host.

## Membership and review

Administrators manage membership and groups, edit schemas, delete pages, and publish directly. Reviewers, shown as super users in the membership form, can edit all pages and decide submissions, but cannot manage membership, change schemas, delete pages, or bypass review through direct publish. Editors can read, create, edit, move, and submit drafts only inside their individual or group path grants. All members can read component and template definitions for authoring.

A grant for `/news` covers `/news` and `/news/story`, not `/newspaper`. Grants are case-sensitive and remain tied to paths when pages move. `/` covers everything. Editors with no grants see no drafts. A move requires access to the old and new paths of the page and every descendant; a denied move changes nothing. Membership changes take effect on the next operation without signing in again. Removing a member also deletes their password and ends their sessions. Local authorization and data changes share a database transaction. Once a local administrator exists, the API prevents removing the last one. See [authentication](authentication.md) for initial access and administrator recovery.

Use Submit for review in the page editor, then Reviews to inspect the submitted preview and exact data. Feedback appears in Reviews and the page editor. Each submission replaces the previous review for that page and gets a unique submission ID. Approval checks that ID, pending status, revision, full draft content, template, and used component definitions under the database lock. Any mismatch returns 409 and requires resubmission. This also catches package installs, CLI imports, subtree moves, schema edits, and replacement submissions at the same revision. Request changes requires nonempty feedback. Approval publishes immediately. Administrators and reviewers may approve their own submissions; this skeleton does not enforce separation of duties or keep an audit history of earlier reviews.

## Move content pages between installations

Content pages don't go into Git. To move pages between installations, for example from a local instance to test or production, use content packages in `/admin`:

1. In **Pages**, choose **Export** on a row. You download a zip with that page, all pages below it and the templates and components they use. The `/` row exports the whole site.
2. Share the zip however you like.
3. On the target installation, choose **Import package** and upload the zip.

Installed pages are drafts; review and publish them as usual. Pages keep their IDs, so installing a newer package of the same pages updates them with a new draft revision. Templates and components missing on the target are added, and existing ones are left unchanged. Deliver definition changes [through Git](content-cli.md) first. The zip uses the same YAML format as the Git export, with a `pages/` directory added. If the pages don't fit the target's definitions, or a different page already uses one of their paths, nothing is installed and the error explains why. Packages are limited to the 1 MiB upload size, which holds a lot of text. Only administrators can export or import packages.
