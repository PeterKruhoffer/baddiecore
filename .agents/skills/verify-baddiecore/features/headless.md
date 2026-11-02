# Headless API

Another app reads published content and registers its own components over server-to-server HTTP with bearer keys. The content key reads; the component key registers `external` components. Neither works in place of the other or of an editor session.

## Sub-features

- `headless-auth` rejects missing or wrong keys with 401 and `WWW-Authenticate: Bearer`.
- `headless-pages` lists published pages only.
- `headless-content` returns `{page, template, components}` for a published slug, and 404 otherwise.
- `headless-register` creates (201) or updates (200) an external component that admins can then allow in templates.

## How to get to it (user POV)

- `GET /api/headless/pages`, `GET /api/headless/content?slug=/…` with `Authorization: Bearer $CONTENT_API_KEY`.
- `PUT /api/headless/components/{id}` with `Authorization: Bearer $COMPONENT_API_KEY`.
- Registered components appear under `Components` in `/admin`.

## Driving it with curl

Preconditions:

- Baseline run with Home published (see [edit-publish.md](./edit-publish.md)), or expect an empty list and 404s.

- **No key.** `curl -si "$BASE_URL/api/headless/pages"` returns `401` with `www-authenticate: Bearer`.
- **Wrong key type.** The same with `-H "Authorization: Bearer $COMPONENT_API_KEY"` returns 401.
- **List.** `curl -s -H "Authorization: Bearer $CONTENT_API_KEY" "$BASE_URL/api/headless/pages"` returns `[{"id":"home","title":"Home","slug":"/","template_id":"homepage","revision":<published revision>}]`.
- **Unpublished slug.** `…/api/headless/content?slug=/nope` returns 404.
- **Register.** `curl -s -w ' %{http_code}' -X PUT -H "Authorization: Bearer $COMPONENT_API_KEY" -H 'content-type: application/json' -d '{"id":"product-grid","name":"Product grid","description":"From app","renderer":"external","fields":[{"name":"title","label":"Title","kind":"text","required":true}]}' "$BASE_URL/api/headless/components/product-grid"` echoes the component with `201`. Repeating it returns 200.
- **Built-in refused.** The same PUT to `/api/headless/components/hero` with `"id":"hero"` returns 409.
- **Side effect.** `$S/sql.sh $RUN_ID "SELECT id FROM components"` includes `product-grid`; in `/admin`, `Expand Components` should list `Product grid`.

## Gotchas

- Launch always sets both keys. To test "disabled" you need a launch without them, which the script doesn't offer.
- Registering a built-in renderer (`"renderer":"hero"`) or an existing built-in ID returns 409.
- Registration doesn't add the component to any template; `Add to main` won't list it until an admin allows it.
