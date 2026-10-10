# Authentication

[Back to README](../README.md)

Baddiecore signs editors in with local accounts. It needs no external identity service. Each organization member signs in with their username (the member ID) and a password. Passwords are stored as Argon2id hashes in MySQL, and sessions are stored there too. Roles and path grants come from the local organization. See [membership and review](editing.md#membership-and-review).

## First administrator

On startup, if no administrator has a password, `BADDIE_ADMIN_PASSWORD` creates the `admin` account, or repairs its administrator role and password. Sign in as `admin`, then add members under Organization. Once an administrator can sign in, the variable is ignored, so changing it later does not change any password. You can remove it after the first sign-in. If no administrator can sign in and the variable is empty, startup stops with an error.

## Members and passwords

Administrators add members under Organization and give each one a username, display name, role and initial password. They can set a new password for any member later; this ends that member's sessions. Members change their own password under Account, which needs their current password and signs them out on other devices. Removing a member deletes their password and ends their sessions immediately. Passwords need at least 8 characters. There is no email, invitation or self-service reset flow; an administrator sets the new password.

After 5 failed sign-ins for one username within 15 minutes, further attempts for that username return 429 until the window passes. This limits guessing, but it is not a substitute for per-client rate limiting of `/api/login` at the reverse proxy.

## Recover administrator access

Run the CLI against the same database:

```sh
docker compose exec cms baddiecore reset-admin <username>
```

It prompts for a password, or reads one line from stdin when stdin is not a terminal. It makes `<username>` an administrator, adding the member if needed, sets the password and ends that account's sessions. The server does not need restarting. Database access is trusted operator access.

## Sessions

Signing in sets an opaque HttpOnly, SameSite=Strict cookie. The database stores only a SHA-256 digest of the token. Sessions expire 12 hours after sign-in and survive server restarts and redeploys. Sign-out deletes the session. Set `BADDIE_SECURE_COOKIE=true` behind HTTPS. Local development over HTTP uses `BADDIE_SECURE_COOKIE=false`.

## Supply your own authentication

To use an identity provider such as OIDC, SAML, or a reverse proxy that authenticates users, implement `auth::AuthProvider` and pass `Arc::new(your_provider)` to `AppState::open_with_auth` in `src/main.rs`. No content handlers need changing. This is a Rust source extension, not a runtime plugin.

- `authorize(&HeaderMap)` returns an `Editor { id }` after validating identity. Use a stable provider ID, not email. Return 401 for absent or invalid credentials. The CMS resolves local membership inside each content transaction and returns 403 for unknown identities. CMS middleware puts the identity in request extensions and preserves its same-origin mutation checks.
- `routes()` returns a state-bound Axum `Router` for login, logout and callbacks. Match the frontend contract in [CONTRACT.md](../CONTRACT.md). Redirect-based providers can reuse the existing login UI by returning `{"method": "redirect", "label": "..."}` from `GET /api/auth/config` and starting sign-in at `GET /api/login`. Logout may return `{"redirect_url": "..."}` to end a hosted session.
- Your provider owns session expiry, revocation, CSRF protection for its routes, and secure credential storage. If using proxy identity headers, block direct backend access and make the trusted proxy strip client-supplied identity headers before setting its own.
- Add members under Organization using the provider's user IDs. To create the first administrator, run `baddiecore reset-admin <provider-user-id>`. The password it sets is unused by your provider.

CMS routes use `AuthProvider::origin_policy`, which defaults to `BADDIE_ORIGIN` and `BADDIE_SECURE_COOKIE`. Override it with `OriginPolicy::new` for configuration supplied in Rust.

The built-in implementation lives in `src/auth.rs`.
