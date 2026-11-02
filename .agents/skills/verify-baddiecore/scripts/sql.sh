#!/usr/bin/env bash
# Run a SQL statement against a run's database, for checking side effects.
# Usage: sql.sh RUN_ID "SELECT id, revision, published_revision FROM pages"
set -euo pipefail
RUN_ID=${1:?usage: sql.sh RUN_ID SQL}
# shellcheck disable=SC1090
source "/tmp/verify-baddiecore/$RUN_ID/env"
docker exec -e MYSQL_PWD="$MYSQL_PASSWORD" "$MYSQL_CONTAINER" \
  mysql --protocol=TCP -h 127.0.0.1 -u baddiecore -D baddiecore --table -e "${2:?SQL required}"
