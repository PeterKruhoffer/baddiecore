#!/bin/sh
set -eu

cd "$(dirname "$0")/../.."

if [ "$(docker context show)" != "orbstack" ]; then
  echo "The database seed test must run through the OrbStack Docker context." >&2
  exit 1
fi

database="baddiecore_seed_test"
mysql="docker compose exec -T -e MYSQL_PWD=baddiecore_root_password mysql mysql -uroot"
app_pid=""
api_log="$(mktemp -t baddiecore-api-roundtrip.XXXXXX)"

cleanup() {
  if [ -n "$app_pid" ]; then
    kill "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  fi

  $mysql -e "REVOKE ALL PRIVILEGES ON ${database}.* FROM 'baddiecore'@'%';" >/dev/null 2>&1 || true
  $mysql -e "DROP DATABASE IF EXISTS ${database}" >/dev/null
  rm -f "$api_log"
}

trap cleanup EXIT

$mysql -e "DROP DATABASE IF EXISTS ${database}; CREATE DATABASE ${database} CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci;"
$mysql "$database" < db/init/001_cms_schema.sql

# Reproduce the important conflict case: a natural key already exists under an
# ID that is not controlled by the demo seed.
$mysql "$database" -e "
  INSERT INTO cms_templates (id, template_key, name, description)
  VALUES ('ffffffff-0000-0000-0000-000000000001', 'folder', 'Existing Folder', 'Created before the demo seed');
"

$mysql "$database" < db/init/002_demo_content.sql

assert_count() {
  table_name="$1"
  expected="$2"
  actual="$($mysql -N -s "$database" -e "SELECT COUNT(*) FROM ${table_name}")"

  if [ "$actual" != "$expected" ]; then
    echo "Expected ${expected} rows in ${table_name}, found ${actual}." >&2
    exit 1
  fi
}

assert_count cms_templates 6
assert_count cms_content_items 7
assert_count cms_page_renderings 4

folder_id="$($mysql -N -s "$database" -e "SELECT id FROM cms_templates WHERE template_key = 'folder'")"
if [ "$folder_id" != "ffffffff-0000-0000-0000-000000000001" ]; then
  echo "The seed replaced the existing folder template ID." >&2
  exit 1
fi

# A second run must neither overwrite the existing row nor duplicate seed data.
$mysql "$database" < db/init/002_demo_content.sql
assert_count cms_templates 6
assert_count cms_content_items 7
assert_count cms_page_renderings 4

# Exercise the actual EF Core provider and Vogen converters against populated
# CHAR(36) columns, then verify that API IDs remain canonical GUID strings.
$mysql -e "GRANT SELECT ON ${database}.* TO 'baddiecore'@'%'; FLUSH PRIVILEGES;"
dotnet build --no-restore >/dev/null

ConnectionStrings__Baddiecore="server=127.0.0.1;port=3307;database=${database};user=baddiecore;password=baddiecore_dev_password" \
ASPNETCORE_URLS="http://127.0.0.1:5299" \
ASPNETCORE_ENVIRONMENT="Production" \
dotnet run --no-build --no-launch-profile >"$api_log" 2>&1 &
app_pid="$!"

if ! python3 - <<'PY'
import json
import re
import time
from urllib.request import urlopen

base_url = "http://127.0.0.1:5299"
guid = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")

for attempt in range(40):
    try:
        with urlopen(f"{base_url}/api/content/tree", timeout=1) as response:
            tree = json.load(response)
        break
    except Exception:
        if attempt == 39:
            raise
        time.sleep(0.25)

def validate_nodes(nodes):
    for node in nodes:
        assert guid.fullmatch(node["id"]), f"Non-canonical content ID: {node['id']}"
        validate_nodes(node["children"])

validate_nodes(tree["items"])

with urlopen(f"{base_url}/api/layout?path=/&mode=preview", timeout=1) as response:
    layout = json.load(response)

assert guid.fullmatch(layout["metadata"]["id"])
for placeholder in layout["placeholders"]:
    for rendering in placeholder["renderings"]:
        assert guid.fullmatch(rendering["id"]), f"Non-canonical rendering ID: {rendering['id']}"
        assert guid.fullmatch(rendering["datasourceId"]), f"Non-canonical datasource ID: {rendering['datasourceId']}"
PY
then
  cat "$api_log" >&2
  exit 1
fi

echo "Demo seed conflict, idempotency, and GUID round-trip checks passed."
