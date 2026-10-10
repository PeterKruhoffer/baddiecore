# Editing and publishing

[Back to README](../README.md)

## Editing model

- Pages have a path, template, ordered component instances, and a draft revision.
- Templates define named regions, component allowlists, and maximum counts. An empty allowlist permits nothing.
- Component definitions have typed text, textarea, URL, or rich-text fields and a renderer. Hero, text, callout, and cards renderers ship with this version. Cards render one body line per card, or one top-level rich-text block per card. External components use a content preview in the CMS and renderer code in your connected app.
- The experience editor renders the page with the same renderer as the public site. Select a block to edit its fields, add allowed components, drag to reorder, or use the move buttons.
- Save draft keeps edits private. Administrators can publish directly. Editors submit new or existing pages for review; reviewers approve and publish or request changes with feedback. Publication snapshots the page, template, and component definitions together. Later draft or schema edits do not alter that snapshot.
- Concurrent page saves return a conflict instead of overwriting another revision. Template and component changes that would invalidate existing drafts are rejected.

The sidebar keeps the page tree under Site available while editing. Administrators also see templates and components under System. Search finds accessible saved pages by title or path, individual blocks by component name or field text, and administrator-only definitions by name or description. Multiple search words must all match the same result. A block result opens that page with its properties selected; navigation asks before discarding unsaved edits. Search does not include unsaved changes or older published snapshots.

## Configure rich text

Rich-text fields use the bundled Tiptap editor with formatting, links, lists, undo and redo. New installations use rich text for the Hero, Text, and Callout body fields. Existing definitions stay unchanged. In **System → Components**, select a component, set a field's Type to **Rich text**, then save. Existing plain copy remains literal text and becomes a JSON document on the next edit. Strings starting with `{` are reserved for JSON documents in rich-text fields; wrap such literal copy in a paragraph document before switching its field type.

Each rich-text field has checkboxes for bold, italic, underline, strikethrough, headings, bullet lists, numbered lists, quotes, and links. All are enabled by default. Clear every checkbox for a paragraph-only editor. Disabled options also disable shortcuts and pasted formatting; server validation rejects them in API, CLI, and package writes. Remove formatting from existing drafts before disabling the option. The CMS rejects settings changes that would invalidate a draft rather than silently stripping its content.

The toolbar groups formatting icons, lists, links, and history. Hover a button for its label. **Text style** switches between Paragraph, Heading 2, and Heading 3. To add or edit a link, select text and click **Link**. Choose **Site link** to pick a page you can access, or **Custom site path** for a route such as `/about#team`. Unpublished pages are marked in the picker. Choose **External link** to enter a full HTTP(S) URL. Apply saves the destination; Remove link keeps the text. Escape or Cancel discards pending link changes. Links store paths, not page IDs, so update links when moving their destination pages, or add a [route alias](#route-aliases). Both link types open in the same tab.

Use **Add site icon** to register an icon's stable ID, readable label, and image URL. Editors then choose it from **Site icon**. Images can be SVG or raster files already hosted on your site, using a root-relative path or an HTTP(S) URL. SVG loads as an image, never inline executable markup. The CMS does not upload files, accept raw SVG, or load icon scripts. Removing an icon used by a draft is rejected. Icon URLs and labels are part of published definitions; keep the referenced image files available and version their URLs when changing the images themselves.

Options live on the field definition, so they also work through component registration and Git/YAML imports. For example:

```json
{
  "name": "body",
  "label": "Body",
  "kind": "richtext",
  "required": true,
  "richtext": {
    "features": ["bold", "italic", "bullet_list", "link"],
    "icons": [{"id": "brand-star", "label": "Brand star", "src": "/assets/icons/star.svg"}]
  }
}
```

Block fields remain strings. A rich-text value is a serialized Tiptap document, for example `{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Hello","marks":[{"type":"bold"}]}]}]}`. Supported nodes are `doc`, `paragraph`, `text`, `hardBreak`, `heading` at levels 2 and 3, `bulletList`, `orderedList`, `listItem`, `blockquote`, and `icon` with `attrs: {id}`. Supported marks are `bold`, `italic`, `underline`, `strike`, and `link` with a safe `href`. An empty document does not satisfy a required field. Nested content is limited to 20 levels.

The shared renderer displays rich text in draft previews, reviews, and public pages without injecting HTML. Titles and button labels render formatting inline; body and external fields support block formatting. Headless apps must parse the JSON and render supported nodes and marks themselves. Resolve icons through the field's captured configuration, escape text, and validate URLs. Published snapshots retain the document and its field options until republished. To add a different node or mark in a source fork, update `src/richtext.rs`, `web/src/admin/RichTextEditor.tsx`, and `web/src/components/RichText.tsx` together.

## Page paths and moves

Choose a parent and a URL segment when creating or moving a page. `/about/team` belongs under `/about`; paths are the hierarchy, with no separate parent IDs. Missing intermediate pages appear as folders, so existing nested URLs still work. Renaming or moving a page moves every descendant draft URL in one transaction and increments their revisions. A stale editor must reload before saving. The root page `/` cannot move, and a page with descendants cannot be deleted until those descendants are moved or deleted.

Moving a draft does not change published URLs. Publish each affected page when ready; there are no automatic redirects or link rewrites.

Paths use ASCII letters, numbers, hyphens, underscores, and slash-separated segments, up to 2048 bytes. Paths are case-sensitive. `/admin`, `/api`, `/health`, and `/assets` are reserved. Rules in this version govern placement, counts, required fields, and field types, not audience targeting or personalization.

## Route aliases

To give a page a shorter or friendlier entry URL, open Page details → Route aliases and enter one absolute path per line, such as `/summer-sale`. Save and publish, or submit for review. Each alias then sends a server-side `301 Moved Permanently` redirect to the page's published path, preserving query parameters. The original page path remains canonical and appears in the browser after the redirect. Aliases are exact paths and do not redirect descendants. Paths already used by other pages or aliases are rejected; editors need grants for alias paths they add or remove.

Alias edits stay private until publication. Moving and republishing a page updates its alias targets without redirect chains. To keep the old page URL working after a move, explicitly add it as an alias. Clear an alias and publish to remove its live redirect; deleting the page removes all its aliases. Browsers and search engines may cache permanent redirects. Aliases also travel in [content packages](#move-content-pages-between-installations). Headless consumers receive aliases in published Page JSON and must implement redirects on their own frontend host.

## Membership and review

Administrators manage membership and groups, edit schemas, delete pages, and publish directly. Reviewers, shown as super users in the membership form, can edit all pages and decide submissions, but cannot manage membership, change schemas, delete pages, or bypass review through direct publish. Editors can read, create, edit, move, and submit drafts only inside their individual or group path grants. All members can read component and template definitions for authoring.

A grant for `/news` covers `/news` and `/news/story`, not `/newspaper`. Grants are case-sensitive and remain tied to paths when pages move. `/` covers everything. Editors with no grants see no drafts. A move requires access to the old and new paths of the page and every descendant; a denied move changes nothing. Membership changes take effect on the next operation without signing in again. Local authorization and data changes share a database transaction. Once a local administrator exists, the API prevents removing the last one. See [authentication](authentication.md) for initial access and administrator recovery.

Use Submit for review in the page editor, then Reviews to inspect the submitted preview and exact data. Feedback appears in Reviews and the page editor. Each submission replaces the previous review for that page and gets a unique submission ID. Approval checks that ID, pending status, revision, full draft content, template, and used component definitions under the database lock. Any mismatch returns 409 and requires resubmission. This also catches package installs, CLI imports, subtree moves, schema edits, and replacement submissions at the same revision. Request changes requires nonempty feedback. Approval publishes immediately. Administrators and reviewers may approve their own submissions; this skeleton does not enforce separation of duties or keep an audit history of earlier reviews.

## Move content pages between installations

Content pages don't go into Git. To move pages between installations, for example from a local instance to test or production, use content packages in `/admin`:

1. In **Pages**, choose **Export** on a row. You download a zip with that page, all pages below it and the templates and components they use. The `/` row exports the whole site.
2. Share the zip however you like.
3. On the target installation, choose **Import package** and upload the zip.

Installed pages are drafts; review and publish them as usual. Pages keep their IDs, so installing a newer package of the same pages updates them with a new draft revision. Templates and components missing on the target are added, and existing ones are left unchanged. Deliver definition changes [through Git](content-cli.md) first. The zip uses the same YAML format as the Git export, with a `pages/` directory added. If the pages don't fit the target's definitions, or a different page already uses one of their paths, nothing is installed and the error explains why. Packages are limited to the 1 MiB upload size, which holds a lot of text. Only administrators can export or import packages.
