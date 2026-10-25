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
smoke_tmp_dir="$(mktemp -d /tmp/comic-container-smoke.XXXXXX)"
owns_container=false
cleanup() {
    local result=$?
    trap - EXIT
    if "$owns_container"; then
        docker logs comic-ci || true
        docker rm -f comic-ci >/dev/null 2>&1 || true
    fi
    rm -rf -- "$smoke_tmp_dir"
    exit "$result"
}
trap cleanup EXIT
export COMIC_PROJECT_SNAPSHOT="$smoke_tmp_dir/projects-before.json"
docker run -d --name comic-ci "${runtime_config[@]}" comic-runtime:ci
owns_container=true
wait_ready
node scripts/browser-smoke.cjs
node scripts/chapter-assets-smoke.cjs
node scripts/panels-references-smoke.cjs
node scripts/project-smoke.cjs
node scripts/backup-smoke.cjs
curl --silent --show-error --fail --user comic:disposable-test-password-only \
    http://127.0.0.1:10000/api/workspace -o "$smoke_tmp_dir/comic-before.json"
image_url=$(node -e 'const s=JSON.parse(require("fs").readFileSync(process.argv[1])); const image=s.workspace.panels[0].images[0]; if (!image || !/^\/images\/[0-9a-f-]+\.(png|jpg|webp)$/.test(image.url)) process.exit(1); console.log(image.url)' "$smoke_tmp_dir/comic-before.json")
curl --silent --show-error --fail --user comic:disposable-test-password-only \
    "http://127.0.0.1:10000$image_url" -o "$smoke_tmp_dir/comic-image-before"
docker rm -f comic-ci >/dev/null
owns_container=false
# Start a new container, with no application data volume.
docker run -d --name comic-ci "${runtime_config[@]}" comic-runtime:ci
owns_container=true
wait_ready
curl --silent --show-error --fail --user comic:disposable-test-password-only \
    http://127.0.0.1:10000/api/workspace -o "$smoke_tmp_dir/comic-after.json"
curl --silent --show-error --fail --user comic:disposable-test-password-only \
    "http://127.0.0.1:10000$image_url" -o "$smoke_tmp_dir/comic-image-after"
cmp "$smoke_tmp_dir/comic-before.json" "$smoke_tmp_dir/comic-after.json"
cmp "$smoke_tmp_dir/comic-image-before" "$smoke_tmp_dir/comic-image-after"
node scripts/project-smoke.cjs --verify-retention
node scripts/panels-references-smoke.cjs --verify-retention
node scripts/backup-smoke.cjs --verify-retention
echo 'PASS: fresh container retains workspace, revisions, and original image bytes.'
