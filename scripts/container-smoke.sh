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
