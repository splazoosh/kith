#!/usr/bin/env bash
# Local CI for Kith — runs the same checks as .github/workflows/ci.yml, in the
# same order, stopping at the first failure. GitHub Actions runner minutes are
# not used yet, so this script is the source of truth for "is the tree green?".
#
# Usage (from anywhere in the repo):
#   ./scripts/ci-local.sh

set -euo pipefail

cd "$(dirname "$0")/.."

run() {
    local name="$1"
    shift
    echo "==> $name"
    "$@"
}

run fmt    cargo fmt --all -- --check
run clippy cargo clippy --all-targets --all-features -- -D warnings
run build  cargo build --workspace
run test   cargo test --workspace --all-features
run deny   cargo deny check
run fe-install pnpm --dir app install --frozen-lockfile
run fe-check   pnpm --dir app check
run fe-test    pnpm --dir app test
run fe-build   pnpm --dir app build

echo "All local CI checks passed."
