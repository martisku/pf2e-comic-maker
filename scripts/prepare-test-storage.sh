#!/usr/bin/env bash
set -euo pipefail
# Deliberately fixed disposable credentials. Never point this at production.
for attempt in {1..30}; do
    if curl --silent --fail --aws-sigv4 aws:amz:us-east-1:s3 \
        --user comic-test-access:comic-test-secret-123 \
        http://127.0.0.1:19000/ >/dev/null; then break; fi
    sleep 1
done
curl --silent --show-error --fail --aws-sigv4 aws:amz:us-east-1:s3 \
    --user comic-test-access:comic-test-secret-123 \
    --request PUT http://127.0.0.1:19000/comic-test
