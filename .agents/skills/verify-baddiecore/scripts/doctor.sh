#!/usr/bin/env bash
# Is this verification instance worth driving? Changes no content; it only signs
# in once, which adds a session row. Usage: doctor.sh RUN_ID
set -uo pipefail
RUN_ID=${1:?usage: doctor.sh RUN_ID}
ENV=/tmp/verify-baddiecore/$RUN_ID/env
[ -f "$ENV" ] || { echo "FAIL no run $RUN_ID (missing $ENV)"; exit 1; }
# shellcheck disable=SC1090
source "$ENV"
fail=0
ok() { echo "ok   $*"; }
bad() { echo "FAIL $*"; fail=1; }

if [ -r "/proc/$SERVER_PID/cmdline" ] && tr '\0' ' ' <"/proc/$SERVER_PID/cmdline" | grep -q "^$BIN"; then
  ok "server pid $SERVER_PID runs $BIN"
elif ps -p "$SERVER_PID" -o command= 2>/dev/null | grep -q "^$BIN"; then
  ok "server pid $SERVER_PID runs $BIN"
else
  bad "server pid $SERVER_PID is not running $BIN (see $STATE/server.log)"
fi

owner=$(lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t 2>/dev/null | head -1)
[ "$owner" = "$SERVER_PID" ] && ok "port $PORT owned by our server" || bad "port $PORT listener is '${owner:-none}', expected $SERVER_PID"

[ "$(docker inspect -f '{{.State.Running}}' "$MYSQL_CONTAINER" 2>/dev/null)" = true ] \
  && ok "mysql container $MYSQL_CONTAINER running" || bad "mysql container $MYSQL_CONTAINER not running"

code=$(curl -s -o /dev/null -w '%{http_code}' "$BASE_URL/health")
[ "$code" = 200 ] && ok "/health 200" || bad "/health returned $code"

jar=$STATE/doctor.cookies
# Send the Origin a browser sends, so origin/secure-cookie misconfiguration fails here.
code=$(curl -s -o /dev/null -w '%{http_code}' -c "$jar" -H 'content-type: application/json' \
  -H "Origin: $BASE_URL" \
  -d "{\"username\":\"$ADMIN_USER\",\"password\":\"$ADMIN_PASSWORD\"}" "$BASE_URL/api/login")
[ "$code" = 204 ] && ok "admin sign-in 204 (browser origin)" || bad "admin sign-in returned $code"

if curl -fsS -b "$jar" "$BASE_URL/api/admin/bootstrap" 2>/dev/null \
    | python3 -c 'import json,sys; b=json.load(sys.stdin); sys.exit(0 if b["access"]["role"]=="admin" and any(p["slug"]=="/" for p in b["pages"]) else 1)'; then
  ok "bootstrap: admin role, page at /"
else
  bad "bootstrap missing admin role or a page at /"
fi

curl -fsS "$BASE_URL/admin" | grep -q '<div id="root"' && ok "/admin serves the editor shell" || bad "/admin did not serve web/dist/index.html"

echo "build: $(git -C "$REPO" rev-parse --short HEAD)$(git -C "$REPO" diff --quiet HEAD -- src web/src || echo ' +uncommitted')"
echo "url:   $BASE_URL/admin"
exit $fail
