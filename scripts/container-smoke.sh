#!/usr/bin/env bash
set -euo pipefail
# Disposable services from compose.storage.yaml; never production credentials.
runtime_config=(
    --network host
    -e DATABASE_URL=postgres://comic:local-disposable-test@127.0.0.1:54329/comic_test
    -e COMIC_INSECURE_LOCAL=1
    -e COMIC_S3_ENDPOINT=http://127.0.0.1:19000
    -e COMIC_S3_REGION=us-east-1
    -e COMIC_S3_BUCKET=comic-test
    -e COMIC_S3_ACCESS_KEY_ID=comic-test-access
    -e COMIC_S3_SECRET_ACCESS_KEY=comic-test-secret-123
    -e COMIC_ORIGIN=http://127.0.0.1:10000
    -e COMIC_PASSWORD=disposable-test-password-only
)
wait_ready() {
    for attempt in {1..30}; do
        if curl --silent --fail http://127.0.0.1:10000/healthz >/dev/null; then return; fi
        sleep 1
    done
    docker logs comic-ci
    return 1
}
docker run --rm "${runtime_config[@]}" comic-runtime:ci migrate
docker run -d --name comic-ci "${runtime_config[@]}" comic-runtime:ci
trap 'docker logs comic-ci; docker rm -f comic-ci >/dev/null' EXIT
wait_ready
node scripts/browser-smoke.cjs
curl --silent --show-error --fail --user comic:disposable-test-password-only \
    http://127.0.0.1:10000/api/workspace -o /tmp/comic-before.json
image_url=$(node -e 'const s=JSON.parse(require("fs").readFileSync("/tmp/comic-before.json")); console.log(s.workspace.panels[0].images[0].url)')
curl --silent --show-error --fail --user comic:disposable-test-password-only \
    "http://127.0.0.1:10000$image_url" -o /tmp/comic-image-before
docker rm -f comic-ci >/dev/null
# Start a new container, with no application data volume.
docker run -d --name comic-ci "${runtime_config[@]}" comic-runtime:ci
wait_ready
curl --silent --show-error --fail --user comic:disposable-test-password-only \
    http://127.0.0.1:10000/api/workspace -o /tmp/comic-after.json
curl --silent --show-error --fail --user comic:disposable-test-password-only \
    "http://127.0.0.1:10000$image_url" -o /tmp/comic-image-after
cmp /tmp/comic-before.json /tmp/comic-after.json
cmp /tmp/comic-image-before /tmp/comic-image-after
echo 'PASS: fresh container retains workspace, revisions, and original image bytes.'

# Exercise the real CLI against an owner-readable, read-only mounted source.
# The runtime UID differs from the host source owner; no chmod of private data.
source_dir=$(mktemp -d)
python3 - "$source_dir" <<'PY'
import hashlib, json, sqlite3, sys, uuid
from pathlib import Path
root = Path(sys.argv[1])
workspace = json.loads(Path('/tmp/comic-before.json').read_text())['workspace']
workspace['title'] = 'CLI import fixture'
for panel in workspace['panels']:
    panel['images'] = []
    panel['selected_image'] = None
identity = str(uuid.uuid4())
filename = identity + '.png'
(root / 'images').mkdir()
(root / 'images' / filename).write_bytes(Path('/tmp/comic-image-before').read_bytes())
workspace['panels'][0]['images'] = [{'id': identity, 'url': '/images/' + filename}]
workspace['panels'][0]['selected_image'] = identity
with sqlite3.connect(root / 'workspace.sqlite') as db:
    db.execute('CREATE TABLE workspace (id INTEGER PRIMARY KEY, revision INTEGER, data TEXT)')
    db.execute('INSERT INTO workspace VALUES (1,17,?)', (json.dumps(workspace),))
(root / 'source.sha256').write_text(hashlib.sha256((root / 'workspace.sqlite').read_bytes()).hexdigest())
PY
docker compose -f compose.storage.yaml exec -T comic-postgres createdb -U comic comic_import_cli
import_url=postgres://comic:local-disposable-test@127.0.0.1:54329/comic_import_cli
docker run --rm "${runtime_config[@]}" -e DATABASE_URL="$import_url" comic-runtime:ci migrate
docker run --rm "${runtime_config[@]}" -e DATABASE_URL="$import_url" \
    --user "$(id -u):$(id -g)" \
    --mount "type=bind,src=$source_dir,dst=/source,readonly" \
    -e COMIC_SOURCE_OFFLINE=1 comic-runtime:ci import-sqlite /source/workspace.sqlite
test "$(docker compose -f compose.storage.yaml exec -T comic-postgres psql -U comic -d comic_import_cli -Atc \
    "SELECT revision = 17 AND data->>'title' = 'CLI import fixture' AND import_complete FROM workspace")" = t
python3 - "$source_dir" <<'PY'
import hashlib, sys
from pathlib import Path
root = Path(sys.argv[1])
assert hashlib.sha256((root / 'workspace.sqlite').read_bytes()).hexdigest() == (root / 'source.sha256').read_text()
print('PASS: real CLI import on read-only source mount preserves the source database.')
PY
rm -rf -- "$source_dir"
