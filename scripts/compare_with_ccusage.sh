#!/usr/bin/env bash
# Compare `ccu` output against `ccusage` for visual verification.
# Not run in CI. Requires `ccusage` and `jq` in PATH.
set -euo pipefail

if ! command -v ccusage >/dev/null 2>&1; then
    echo "ccusage not found in PATH — install from https://github.com/ryoppippi/ccusage" >&2
    exit 1
fi

if ! command -v ccu >/dev/null 2>&1; then
    echo "ccu not found — run 'cargo install --path .' first" >&2
    exit 1
fi

diff_cmd="${DIFF:-diff}"

echo "=== daily ==="
"$diff_cmd" <(ccu daily --json | jq -S .) <(ccusage daily --json | jq -S .) || true

echo
echo "=== monthly ==="
"$diff_cmd" <(ccu monthly --json | jq -S .) <(ccusage monthly --json | jq -S .) || true
