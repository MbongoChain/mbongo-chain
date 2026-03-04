# Devnet Convergence Test Script
# Builds the workspace and runs the devnet harness.
# Exit code 0 on success, 1 on failure.

$ErrorActionPreference = "Stop"

Write-Host "=== Mbongo Chain Devnet Test ===" -ForegroundColor Cyan
Write-Host ""

# Step 1: Build the workspace
Write-Host "Building workspace..." -ForegroundColor Yellow
cargo build --workspace 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "BUILD FAILED" -ForegroundColor Red
    exit 1
}
Write-Host "Build complete." -ForegroundColor Green
Write-Host ""

# Step 2: Run the devnet harness
Write-Host "Running devnet convergence harness..." -ForegroundColor Yellow
Write-Host ""

cargo run -p mbongo-node --bin devnet_harness 2>&1
$harness_exit = $LASTEXITCODE

Write-Host ""

if ($harness_exit -eq 0) {
    Write-Host "========================================" -ForegroundColor Green
    Write-Host "           DEVNET TEST: PASS            " -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Green
    exit 0
} else {
    Write-Host "========================================" -ForegroundColor Red
    Write-Host "           DEVNET TEST: FAIL            " -ForegroundColor Red
    Write-Host "========================================" -ForegroundColor Red
    exit 1
}
