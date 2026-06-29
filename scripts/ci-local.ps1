#!/usr/bin/env pwsh
# Local CI for Kith — runs the same checks as .github/workflows/ci.yml, in the
# same order, stopping at the first failure. GitHub Actions runner minutes are
# not used yet, so this script is the source of truth for "is the tree green?".
#
# Usage (from anywhere in the repo):
#   pwsh scripts/ci-local.ps1

Set-Location (Split-Path -Parent $PSScriptRoot)

# Gate strictly on each command's exit code; don't let cargo's stderr/progress
# output trip PowerShell's native-command error handling.
if (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

$steps = [ordered]@{
    'fmt'        = { cargo fmt --all -- --check }
    'clippy'     = { cargo clippy --all-targets --all-features -- -D warnings }
    'build'      = { cargo build --workspace }
    'test'       = { cargo test --workspace --all-features }
    'deny'       = { cargo deny check }
    'fe-install' = { pnpm --dir app install --frozen-lockfile }
    'fe-check'   = { pnpm --dir app check }
    'fe-test'    = { pnpm --dir app test }
    'fe-build'   = { pnpm --dir app build }
}

foreach ($name in $steps.Keys) {
    Write-Host "==> $name" -ForegroundColor Cyan
    & $steps[$name]
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: $name (exit $LASTEXITCODE)" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

Write-Host "All local CI checks passed." -ForegroundColor Green
