#!/usr/bin/env bash
# Start an isolated Baddiecore instance for verification: its own MySQL container,
# database, port, admin password, headless keys and evidence directory.
# Usage: launch.sh [RUN_ID]   (prints the env file to source when ready)
set -euo pipefail

REPO=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
RUN_ID=${1:-$(date +%Y%m%d-%H%M%S)-$RANDOM}
STATE=/tmp/verify-baddiecore/$RUN_ID
EVIDENCE=$REPO/.amp/in/artifacts/verify-baddiecore/$RUN_ID
MYSQL_CONTAINER=vbc-$RUN_ID-mysql

if [ -e "$STATE" ]; then
  echo "run $RUN_ID already exists at $STATE; pick another RUN_ID or run cleanup.sh $RUN_ID" >&2
  exit 1
fi
mkdir -p "$STATE" "$EVIDENCE"

# 1. Docker daemon. In an orb it is not running by default; start it as a
#    supervised orb service whose socket the current user can use without sudo.
if ! docker info >/dev/null 2>&1; then
  if command -v amp >/dev/null && command -v dockerd >/dev/null && sudo -n true 2>/dev/null; then
    echo "starting dockerd as orb service 'dockerd'"
    amp orb service start dockerd --command "sudo dockerd --group $(id -gn)" >/dev/null
    for _ in $(seq 60); do docker info >/dev/null 2>&1 && break; sleep 1; done
  fi
  docker info >/dev/null 2>&1 || { echo "Docker is not reachable. Start Docker or OrbStack and retry." >&2; exit 1; }
fi

# 2. Build the checkout under test: the server binary and the editor bundle it serves.
( cd "$REPO" && cargo build --locked --quiet )
pnpm --dir "$REPO/web" run build >"$STATE/web-build.log" 2>&1 || { cat "$STATE/web-build.log" >&2; exit 1; }

# 3. MySQL 8.4 with no named volume; cleanup removes the container and its anonymous volume.
secret() { openssl rand -hex 24; }
MYSQL_PASSWORD=$(secret)
docker run -d --name "$MYSQL_CONTAINER" \
  -e MYSQL_DATABASE=baddiecore -e MYSQL_USER=baddiecore \
  -e MYSQL_PASSWORD="$MYSQL_PASSWORD" -e MYSQL_ROOT_PASSWORD="$(secret)" \
  -p 127.0.0.1::3306 mysql:8.4 >/dev/null
MYSQL_PORT=$(docker port "$MYSQL_CONTAINER" 3306/tcp | head -1 | sed 's/.*://')

# The image's init phase runs a temporary server without TCP, so a TCP query
# succeeds only once the real server is up with the user and database created.
echo "waiting for MySQL on 127.0.0.1:$MYSQL_PORT"
for i in $(seq 120); do
  if docker exec -e MYSQL_PWD="$MYSQL_PASSWORD" "$MYSQL_CONTAINER" \
      mysql --protocol=TCP -h 127.0.0.1 -u baddiecore -D baddiecore -e 'SELECT 1' >/dev/null 2>&1; then
    break
  fi
  [ "$i" = 120 ] && { docker logs "$MYSQL_CONTAINER" | tail -20 >&2; echo "MySQL never became ready" >&2; exit 1; }
  sleep 1
done

# 4. Server on a free loopback port.
PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
ADMIN_PASSWORD=$(secret)
CONTENT_API_KEY=$(secret)
COMPONENT_API_KEY=$(secret)
DATABASE_URL="mysql://baddiecore:$MYSQL_PASSWORD@127.0.0.1:$MYSQL_PORT/baddiecore"

cat >"$STATE/env" <<EOF
RUN_ID=$RUN_ID
REPO=$REPO
STATE=$STATE
EVIDENCE=$EVIDENCE
BASE_URL=http://127.0.0.1:$PORT
PORT=$PORT
MYSQL_CONTAINER=$MYSQL_CONTAINER
MYSQL_PASSWORD=$MYSQL_PASSWORD
DATABASE_URL=$DATABASE_URL
ADMIN_USER=admin
ADMIN_PASSWORD=$ADMIN_PASSWORD
CONTENT_API_KEY=$CONTENT_API_KEY
COMPONENT_API_KEY=$COMPONENT_API_KEY
BIN=$REPO/target/debug/baddiecore
BROWSER_SESSION=vbc-$RUN_ID
EOF

# Drop inherited BADDIE_* settings (an orb's project env may set deployment values
# such as BADDIE_SECURE_COOKIE=true, which rejects the browser's plain-HTTP origin).
(
  cd "$REPO"
  for v in $(compgen -e | grep '^BADDIE_'); do unset "$v"; done
  BADDIE_BIND=127.0.0.1:$PORT DATABASE_URL=$DATABASE_URL BADDIE_STATIC=$REPO/web/dist \
  BADDIE_ADMIN_PASSWORD=$ADMIN_PASSWORD \
  BADDIE_CONTENT_API_KEY=$CONTENT_API_KEY BADDIE_COMPONENT_API_KEY=$COMPONENT_API_KEY \
    exec "$REPO/target/debug/baddiecore" >"$STATE/server.log" 2>&1
) &
echo "SERVER_PID=$!" >>"$STATE/env"
chmod 600 "$STATE/env"

# 5. Ready means usable: health passes, the admin can sign in, and the seeded Home draft exists.
echo "waiting for server on 127.0.0.1:$PORT"
for i in $(seq 60); do
  if curl -fsS "http://127.0.0.1:$PORT/health" >/dev/null 2>&1; then break; fi
  [ "$i" = 60 ] && { tail -20 "$STATE/server.log" >&2; echo "server never became healthy" >&2; exit 1; }
  sleep 1
done
"$(dirname "$0")/doctor.sh" "$RUN_ID"
echo
echo "ready: source $STATE/env"
