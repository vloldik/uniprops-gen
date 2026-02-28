#!/bin/bash
set -euo pipefail

TAG="v${VERSION}"
echo "Checking if tag ${TAG} already exists in the repository..."

if git ls-remote --tags origin | grep -q "refs/tags/${TAG}$"; then
    echo "Tag ${TAG} already exists. Skipping publish."
    echo "SHOULD_PUBLISH=false" >> "$GITHUB_ENV"
else
    echo "Tag ${TAG} is new. Proceeding to publish."
    echo "SHOULD_PUBLISH=true" >> "$GITHUB_ENV"
fi