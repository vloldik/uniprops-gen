#!/bin/bash
set -euo pipefail

TAG="v${VERSION}"
echo "Creating GitHub tag and release for ${TAG}..."

git tag "$TAG"
git push origin "$TAG"

gh release create "$TAG" --title "Release $TAG" --generate-notes