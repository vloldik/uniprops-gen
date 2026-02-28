#!/bin/bash
set -euo pipefail

echo "Publishing packages/uniprops_gen version ${VERSION} to crates.io..."

cd packages/uniprops_gen
cargo publish --token "$CARGO_REGISTRY_TOKEN"