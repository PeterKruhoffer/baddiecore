# Authentication

[Back to README](../README.md)

`BADDIE_AUTH=password` is the default and requires a nonempty `BADDIE_ADMIN_PASSWORD`. It makes no WorkOS requests and needs no WorkOS account. Unknown methods or missing required configuration stop startup. There is no unauthenticated editor mode and no fallback between methods.

## Use WorkOS AuthKit

To opt into [WorkOS AuthKit](https://workos.com/docs/authkit), set these variables on the server or in your Compose `.env`:

```dotenv
BADDIE_AUTH=workos
BADDIE_SECURE_COOKIE=true
WORKOS_API_KEY=<your-server-side-api-key>
WORKOS_CLIENT_ID=<your-client-id>
WORKOS_ORGANIZATION_ID=<your-editors-organization-id>
WORKOS_REDIRECT_URI=https://cms.example.com/api/auth/callback
```

Use your own WorkOS environment for each independently operated installation. Keep the API key in secret storage, never in frontend variables or source control. Password configuration is unused in WorkOS mode and can be removed.

In the WorkOS dashboard, enable hosted AuthKit, register the exact callback URL above and allow `https://cms.example.com/admin` as a logout redirect. Create a dedicated organization for this CMS's editors and invite them into it. Disable public sign-ups and automatic organization enrollment if access must be invitation-only. Authentication into this organization does not grant CMS access by itself. Add the user's stable WorkOS user ID to the CMS's local Organization screen. WorkOS roles do not control local permissions. A WorkOS account outside the configured organization grants no access. See [membership and review](editing.md#membership-and-review) for local roles and grants.

### Bootstrap and recover administrator access

One CMS installation is one organization. On the first startup with local membership support, `BADDIE_BOOTSTRAP_ADMIN_ID=<stable-provider-user-id>` creates that local administrator. Remove the variable afterward. It applies only when the organization record is first created; restarting never restores a removed member or overwrites local membership. Unknown WorkOS and custom-provider IDs are denied by default, including an ID named `shared-admin`.

For an existing installation or recovery, temporarily restart the server with `BADDIE_AUTH=password` and a securely configured `BADDIE_ADMIN_PASSWORD`. The password session is a recovery administrator independent of local membership. Use Organization to add or repair administrator memberships, then restore WorkOS configuration and restart. Password login is deliberately not available alongside WorkOS. Keep password recovery restricted to the operator; all password users have full access. No external WorkOS mutation is required.

### Sessions and local development

Editors use the hosted sign-in page and return to `/admin`. The server checks single-use browser state and PKCE, exchanges the code, and stores tokens in memory. The browser receives only an opaque HttpOnly session cookie. WorkOS sessions refresh on the next admin request after at most five minutes, or earlier if the access token expires. WorkOS revocation and WorkOS organization changes take effect on refresh; local CMS membership changes take effect on the next operation. Refresh failures deny access and discard the local session. Sign-out clears the local session and redirects the browser to WorkOS to end its session too. Run one replica; restarting ends local sessions.

For local development, use a WorkOS staging environment and an HTTP loopback callback, with `BADDIE_SECURE_COOKIE=false`. When using the Vite dev server, register its browser-facing origin with `/api/auth/callback`, not the backend's port. Production callbacks require HTTPS and secure cookies. Configure the reverse proxy to omit query strings on `/api/auth/callback` from access logs because callbacks contain authorization codes.

The [Railway configuration](deployment.md#deploy-to-railway) defaults to password auth. To opt in before applying it, replace its `BADDIE_ADMIN_PASSWORD` environment mapping with `BADDIE_AUTH: "workos"` and mappings from `ctx.shared` for the four `WORKOS_*` variables above. Set those shared variables in the selected Railway environment. Leave `BADDIE_SECURE_COOKIE: "true"`. Review the configuration plan before applying it.

## Supply your own authentication

Implement `auth::AuthProvider` and pass `Arc::new(your_provider)` to `AppState::open_with_auth` in `src/main.rs`. No content handlers need changing. This is a Rust source extension, not a runtime plugin or a `BADDIE_AUTH=custom` option.

- `authorize(&HeaderMap)` returns an `Editor { id }` after validating identity. Use a stable provider ID, not email. Return 401 for absent or invalid credentials. The CMS resolves local membership inside each content transaction and returns 403 for unknown identities. CMS middleware puts the identity in request extensions and preserves its same-origin mutation checks. The optional `recovery_admin` trait method defaults to false; override it only for a trusted operator recovery mechanism, never based on an untrusted role or ID header.
- `routes()` returns a state-bound Axum `Router` for login, logout and callbacks. Match the frontend contract in [CONTRACT.md](../CONTRACT.md). Redirect-based providers can reuse the existing login UI through `GET /api/auth/config`.
- Your provider owns session expiry, revocation, CSRF protection for its routes, and secure credential storage. If using proxy identity headers, block direct backend access and make the trusted proxy strip client-supplied identity headers before setting its own.

CMS routes use `AuthProvider::origin_policy`, which defaults to `BADDIE_ORIGIN` and `BADDIE_SECURE_COOKIE`. Override it with `OriginPolicy::new` for configuration supplied in Rust. Built-in providers use the same policy for their own routes and CMS mutations.

The built-in implementations live in `src/auth.rs` and `src/auth/workos.rs`. Auth tests use a local mock WorkOS endpoint and do not need credentials. A real staging-environment sign-in and sign-out should still be checked before enabling WorkOS in production.
