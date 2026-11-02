#!/usr/bin/env bash
# Tear down what launch.sh RUN_ID started: its server process, MySQL container
# (with its anonymous volume), browser session and scratch state. Keeps the
# evidence directory. Leaves dockerd running because other runs may share it.
# Usage: cleanup.sh RUN_ID
set -uo pipefail
RUN_ID=${1:?usage: cleanup.sh RUN_ID}
STATE=/tmp/verify-baddiecore/$RUN_ID
MYSQL_CONTAINER=vbc-$RUN_ID-mysql
if [ -f "$STATE/env" ]; then
  # shellcheck disable=SC1090
  source "$STATE/env"
  if [ -n "${SERVER_PID:-}" ] && ps -p "$SERVER_PID" -o command= 2>/dev/null | grep -q "^$BIN"; then
    kill "$SERVER_PID"
    for _ in $(seq 10); do ps -p "$SERVER_PID" >/dev/null || break; sleep 1; done
    ps -p "$SERVER_PID" >/dev/null && kill -9 "$SERVER_PID"
  fi
  agent-browser --session "$BROWSER_SESSION" close >/dev/null 2>&1 || true
fi
docker rm -f -v "$MYSQL_CONTAINER" >/dev/null 2>&1 || true
rm -rf "$STATE"
echo "cleaned run $RUN_ID"
[ -n "${EVIDENCE:-}" ] && [ -d "$EVIDENCE" ] && echo "evidence kept: $EVIDENCE"
exit 0
