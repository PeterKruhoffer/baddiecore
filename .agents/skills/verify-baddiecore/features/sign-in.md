# Sign in

Members sign in at `/admin` with a username and password, get a session cookie, and sign out from the header. Repeated wrong passwords lock that username for 15 minutes.

## Sub-features

- `signin-ok` signs in and shows the workspace.
- `signin-wrong` shows an error for a wrong or unknown password.
- `signin-lockout` returns 429 on the 6th attempt within 15 minutes for one username, even with the right password.
- `signout` ends the session and returns to the sign-in form.

## How to get to it (user POV)

- Open `/admin` while signed out: the `Welcome back.` form.
- `Sign out` button in the header.
- `POST /api/login` with `{username, password}` for scripts.

## Driving it with agent-browser

Preconditions:

- Baseline run, browser signed out (`$AB open "$BASE_URL/admin"` shows heading `Welcome back.`).

- **Wrong password.** `$AB find label "Username" fill admin`, `$AB find label "Password" fill nope`, `$AB find role button click --name "Enter workspace"`. An `alert` reads `invalid username or password`, and the form stays.
- **Sign in.** Fill `Password` with `$ADMIN_PASSWORD` and choose `Enter workspace`. The navigation `Content navigation` appears with buttons `Home`, `Organization`, `Reviews 0 pending`.
- **Sign out.** `$AB find role button click --name "Sign out"`, then wait for `Enter workspace`. With curl: sign in with `-c jar -H "Origin: $BASE_URL"`, `POST /api/logout` with `-b jar`, and `GET /api/admin/bootstrap` with the old cookie returns 401.
- **Lockout.** Use a non-admin member (create one as in [review.md](./review.md)) so the admin stays usable: `for i in 1 2 3 4 5 6; do curl -s -o /dev/null -w '%{http_code} ' -H 'content-type: application/json' -d '{"username":"erin","password":"wrong"}' "$BASE_URL/api/login"; done` prints `401 401 401 401 401 429`. The correct password then returns 429 `too many failed sign-ins; try again later`.

## Gotchas

- Don't run the lockout against `admin`: it locks out doctor and every browser step for 15 minutes. Failure counters are per server process, so a new launch resets them.
- After sign-out the form shows `Loading sign-in…` until `/api/auth/config` answers; `find label "Username"` fails until then.
- A 403 `origin does not match host` on sign-in means the server inherited deployment `BADDIE_*` settings. Launch clears them; check doctor.
