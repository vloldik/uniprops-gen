#!/bin/bash
set -euo pipefail

TOML_PATH="packages/uniprops_gen/Cargo.toml"

if [ ! -f "$TOML_PATH" ]; then
    echo "Error: $TOML_PATH not found!"
    exit 1
fi

VERSION=$(grep -m 1 -E '^version\s*=' "$TOML_PATH" | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$VERSION" ]; then
    echo "Error: Could not parse version from $TOML_PATH"
    exit 1
fi

echo "Extracted version: $VERSION"
echo "VERSION=$VERSION" >> "$GITHUB_ENV"