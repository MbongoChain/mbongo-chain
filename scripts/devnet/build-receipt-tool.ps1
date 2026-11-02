# Builds and deploys the receipt submission tool (submit_receipt.exe)
# from a CLEAN worktree at an explicitly named source commit, and stamps
# the external receipt-tool manifest with the tool's provenance.
#
# The tool is never built from the live working tree: the -SourceCommit
# is mandatory, the worktree is verified to be at exactly that commit
# and clean, and the deployed artifact is hash-recorded so that
# submit-receipt.ps1 can verify it before every submission.
#
# Usage: .\build-receipt-tool.ps1 -SourceCommit <full 40-hex commit sha>

param(
    [Parameter(Mandatory = $true)][string]$SourceCommit
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

if ($SourceCommit -notmatch '^[0-9a-fA-F]{40}$') {
    throw 'SourceCommit must be a full 40-character commit SHA (no branches, no tags, no abbreviations).'
}
$SourceCommit = $SourceCommit.ToLowerInvariant()

if (-not (Test-Path (Join-Path $RepoRoot '.git'))) {
    throw "Repository not found at $RepoRoot."
}

Write-Host "==> Building receipt tool from commit $SourceCommit"

# ── Clean worktree pinned to the explicit commit ───────────────────────
if (Test-Path $ReceiptToolBuildDir) {
    $head = (git -C $ReceiptToolBuildDir rev-parse HEAD).Trim()
    if ($head -ne $SourceCommit) {
        throw "Tool worktree at $ReceiptToolBuildDir is at $head, not $SourceCommit. Remove it (git -C $RepoRoot worktree remove $ReceiptToolBuildDir) and re-run."
    }
} else {
    New-Item -ItemType Directory -Force (Split-Path $ReceiptToolBuildDir) | Out-Null
    git -C $RepoRoot worktree add $ReceiptToolBuildDir $SourceCommit
    if ($LASTEXITCODE -ne 0) {
        throw "git worktree add failed (exit $LASTEXITCODE). Does commit $SourceCommit exist?"
    }
}

$head = (git -C $ReceiptToolBuildDir rev-parse HEAD).Trim()
if ($head -ne $SourceCommit) {
    throw "Tool worktree HEAD $head does not match requested commit $SourceCommit."
}
$dirty = git -C $ReceiptToolBuildDir status --porcelain
if ($dirty) {
    throw "Tool worktree is not clean:`n$dirty`nRemove it and re-run."
}

# Sanity: the pinned tree must actually contain the tool source.
$exampleSrc = Join-Path $ReceiptToolBuildDir 'crates\mbongo-wallet\examples\submit_receipt.rs'
if (-not (Test-Path $exampleSrc)) {
    throw "Commit $SourceCommit does not contain crates/mbongo-wallet/examples/submit_receipt.rs. Use a commit that includes the receipt tool."
}

# ── Build ──────────────────────────────────────────────────────────────
Write-Host '==> cargo build --release --locked -p mbongo-wallet --example submit_receipt'
Push-Location $ReceiptToolBuildDir
try {
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    cargo build --release --locked -p mbongo-wallet --example submit_receipt 2>$null
    $buildExit = $LASTEXITCODE
    $ErrorActionPreference = $prevEap
    if ($buildExit -ne 0) { throw "cargo build failed (exit $buildExit)." }
} finally {
    Pop-Location
}

$built = Join-Path $ReceiptToolBuildDir 'target\release\examples\submit_receipt.exe'
if (-not (Test-Path $built)) { throw "Build succeeded but tool not found at $built." }

# ── Deploy + manifest ──────────────────────────────────────────────────
New-Item -ItemType Directory -Force $BinDir | Out-Null
Copy-Item $built $ReceiptToolPath -Force

$manifest = [ordered]@{
    toolSourceCommit = $SourceCommit
    protocolTag      = $DevnetTag
    protocolCommit   = $DevnetCommit
    toolPath         = $ReceiptToolPath
    sha256           = Get-FileSha256 $ReceiptToolPath
    builtAtUtc       = (Get-Date).ToUniversalTime().ToString('o')
}
New-Item -ItemType Directory -Force $DevnetRoot | Out-Null
$manifest | ConvertTo-Json | Out-File -FilePath $ReceiptToolManifestPath -Encoding utf8

Write-Host ''
Write-Host '── Receipt tool deployed ──────────────────────────────────────'
Write-Host "  Source commit: $SourceCommit"
Write-Host "  Protocol:      $DevnetTag @ $DevnetCommit"
Write-Host "  Tool:          $ReceiptToolPath"
Write-Host "  SHA256:        $($manifest.sha256)"
Write-Host "  Manifest:      $ReceiptToolManifestPath"
Write-Host '───────────────────────────────────────────────────────────────'
exit 0
