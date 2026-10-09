# Headless integration

[Back to README](../README.md)

The CMS supports headless delivery alongside its built-in site. Your app needs the CMS's HTTPS URL and a server-side content API key. It gets published pages as JSON and renders them with its own components. Editors still author pages, place components in template regions, and publish or submit for review in `/admin`.

## Configure API keys

Set `BADDIE_CONTENT_API_KEY` on the CMS and the consuming app's server. To register custom component definitions remotely, also set a separate `BADDIE_COMPONENT_API_KEY` on the CMS and in your app's trusted setup or deployment process. Generate each key independently with `openssl rand -hex 32`. Never commit keys, put them in browser bundles, pass them in URLs, or print them in logs. Nonempty keys must contain at least 32 non-whitespace ASCII bytes; identical keys fail startup. Missing or empty values disable the corresponding capability. Rotate or revoke a key by changing or removing it and restarting the CMS. There is one installation-wide key per capability, not per-app access control.

Send keys in `Authorization: Bearer <key>`. Content keys cannot register definitions. Registration keys cannot read content, edit pages, publish, or manage membership. These routes do not use editor sessions, and return `Cache-Control: no-store`. Use server-side requests rather than credentialed cross-origin browser calls. HTTPS and proxy rate limits are still required.

## Fetch published content

```sh
# CMS_URL and keys come from server-side configuration.
curl --fail --silent --show-error "$CMS_URL/api/headless/pages" \
  -H "Authorization: Bearer $BADDIE_CONTENT_API_KEY"
curl --fail --silent --show-error "$CMS_URL/api/headless/content?slug=%2Fabout" \
  -H "Authorization: Bearer $BADDIE_CONTENT_API_KEY"
```

`GET /api/headless/pages` lists published `{id, title, slug, template_id, revision}` records ordered by published path. Use it to discover routes or build navigation. `GET /api/headless/content?slug=/about` returns `{page, template, components}` from the published snapshot. The page contains ordered blocks with `id`, `component_id`, `region`, and `fields`. Render regions in `template.regions` order and preserve block order within each region. Resolve each block's definition through `component_id`. Unpublished paths return 404. Draft changes and definition updates stay invisible until publication. See [CONTRACT.md](../CONTRACT.md#headless-integration) for the API contract.

## Register your app's components

Choose a stable component ID, such as `shop-product-promo`, and keep it in your app's renderer map. Registration sends only the field schema, never JavaScript or HTML. The CMS supports `text`, `textarea`, and `url` string fields with required flags.

```sh
curl --fail --silent --show-error -X PUT \
  "$CMS_URL/api/headless/components/shop-product-promo" \
  -H "Authorization: Bearer $BADDIE_COMPONENT_API_KEY" \
  -H 'Content-Type: application/json' \
  --data '{
    "id": "shop-product-promo",
    "name": "Product promotion",
    "description": "Rendered by the shop frontend",
    "renderer": "external",
    "fields": [
      {"name":"headline","label":"Headline","kind":"text","required":true},
      {"name":"product_url","label":"Product URL","kind":"url","required":false}
    ]
  }'
```

The PUT creates the definition with 201 or updates it with 200. IDs must match the path and contain 1–200 ASCII letters, numbers, underscores or hyphens. Repeated registration preserves IDs and existing page blocks. Updates that invalidate existing drafts fail without changing the definition. Registration cannot replace components using built-in renderers. Use app-prefixed IDs to avoid collisions between apps. The registration key can update any external definition in the installation, so give it only to trusted schema-maintenance code.

An administrator then opens System → Templates and allows the component in the desired regions. Editors can add instances and edit their fields through the usual page editor. The CMS shows a labelled content preview, including empty fields, rather than running remote code or reproducing your app's visual design. In your app, map `shop-product-promo` to your own component and pass the block's `fields` as its data. Escape text and validate links there too. Handle unknown IDs explicitly so a missing renderer does not silently erase content.

## Limits

This first headless version delivers published content only. It does not provide remote draft preview, embedded external renderers, an SDK, or publication webhooks. Build-time consumers must fetch again and rebuild after publishing; server-rendered consumers can fetch on requests. Published content is not private. The existing `/api/content` endpoint and built-in public site remain unauthenticated. Headless API keys control the integration endpoints, not confidentiality of published pages.

Headless consumers receive [route aliases](editing.md#route-aliases) in published Page JSON and must implement redirects on their own frontend host.
