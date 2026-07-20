# Resets the v0.3 devnet deployment to a fresh-genesis state by wiping
# ONLY deployment-owned runtime data, after an automatic verified
# backup and two explicit confirmations. Never wipes automatically.
#
# Removed:   node data directories, node logs, PID metadata, node
#            provenance markers, receipt submission records
# Preserved: deployed binaries, node manifest, receipt-tool manifest,
#            all backups, build sources/worktrees
#
# Confirmations required:
#   1. the -ConfirmReset switch, AND
#   2. the exact resolved deployment root path, typed at the prompt or
#      supplied via -ConfirmRoot (for documented scripted use).
#
# A verified backup is taken automatically before the wipe unless the
# emergency override -EmergencySkipBackupIUnderstandDataLoss is
# supplied. There is no shorter alias for that flag on purpose.
#
# Usage: .\reset-devnet.ps1 -ConfirmReset [-ConfirmRoot <exact root>]
#                           [-EmergencySkipBackupIUnderstandDataLoss]

param(
    [switch]$ConfirmReset,
    [string]$ConfirmRoot,
    [switch]$EmergencySkipBackupIUnderstandDataLoss
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

# ── Validation (nothing is modified before all checks pass) ────────────
$resolvedRoot = Assert-SafeDevnetRoot

if (-not $ConfirmReset) {
    Write-Error "Reset requires the explicit -ConfirmReset switch AND a typed confirmation of the deployment root. Nothing was changed. Deployment root: $resolvedRoot"
    exit 1
}

$running = Get-RunningDevnetNodes
if ($running.Count -gt 0) {
    Write-Error "Cannot reset while nodes are running ($($running -join ', ')). Stop the devnet first: stop-devnet.ps1. Nothing was changed."
    exit 1
}

# Deployment identity must validate before anything is deleted.
$null = Assert-DevnetManifest

# Every non-empty node data directory must carry a provenance marker
# matching this deployment; unknown provenance refuses the reset.
foreach ($node in $DevnetNodes) {
    $dataDir = Get-NodeDataDir $node
    $hasData = (Test-Path $dataDir) -and
        ($null -ne (Get-ChildItem $dataDir -Force -ErrorAction SilentlyContinue | Select-Object -First 1))
    if ($hasData) {
        $marker = Get-NodeMarkerFile $node
        if (-not (Test-Path $marker)) {
            Write-Error "Data directory $dataDir has no provenance marker; refusing to wipe data of unknown origin. Nothing was changed."
            exit 1
        }
        $m = Get-Content $marker -Raw | ConvertFrom-Json
        if (($m.tag -ne $DevnetTag) -or ($m.commit -ne $DevnetCommit)) {
            Write-Error "Data directory $dataDir belongs to '$($m.tag)' ($($m.commit)), not this deployment; refusing. Nothing was changed."
            exit 1
        }
    }
}

# ── Second, typed confirmation: the exact deployment root ──────────────
Write-Host "Deployment root to be reset: $resolvedRoot"
if (-not $ConfirmRoot) {
    $ConfirmRoot = Read-Host 'Type the EXACT deployment root path shown above to confirm the irreversible wipe'
}
if ($ConfirmRoot.TrimEnd('\') -ne $resolvedRoot.TrimEnd('\')) {
    Write-Error 'Typed confirmation does not match the deployment root. Nothing was changed.'
    exit 1
}

# ── Automatic verified backup before wipe ──────────────────────────────
if ($EmergencySkipBackupIUnderstandDataLoss) {
    Write-Warning 'EMERGENCY OVERRIDE: skipping the pre-wipe backup. Wiped data will be UNRECOVERABLE.'
} else {
    $stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMdd-HHmmss') + 'Z'
    $backupLabel = "pre-reset-$stamp"
    Write-Host "==> Taking verified pre-wipe backup ($backupLabel)..."
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'backup-devnet.ps1') `
        -Label $backupLabel 2>&1 | ForEach-Object { "    $_" }
    $backupExit = $LASTEXITCODE
    $ErrorActionPreference = $prevEap
    $backupMeta = Join-Path $BackupRoot "devnet-backup-$backupLabel.metadata.json"
    if (($backupExit -ne 0) -or (-not (Test-Path $backupMeta))) {
        Write-Error "Pre-wipe backup FAILED (exit $backupExit); reset aborted. Nothing was deleted."
        exit 1
    }
    Write-Host "==> Pre-wipe backup verified: $backupMeta"
}

# ── Wipe (deployment-owned runtime data only, all paths re-validated) ──
$removed = @()
$failures = @()

function Remove-RuntimePath([string]$Path) {
    $full = Assert-PathInsideRoot -Path $Path -ResolvedRoot $resolvedRoot
    if (-not (Test-Path $full)) { return }
    try {
        Remove-Item $full -Recurse -Force -Confirm:$false
        $script:removed += $full
    } catch {
        $script:failures += "${full}: $_"
    }
}

foreach ($node in $DevnetNodes) {
    Remove-RuntimePath (Get-NodeDataDir $node)
    Remove-RuntimePath (Get-NodeLogDir $node)
    Remove-RuntimePath (Get-NodePidFile $node)
    Remove-RuntimePath (Get-NodeMarkerFile $node)
}
# Receipt records reference the wiped chain and are removed with it
# (they are inside every pre-wipe backup).
Remove-RuntimePath $ReceiptRecordDir

# Recreate the empty directory structure.
foreach ($node in $DevnetNodes) {
    New-Item -ItemType Directory -Force (Get-NodeDir $node) | Out-Null
}
New-Item -ItemType Directory -Force $ReceiptRecordDir | Out-Null

# ── Report ─────────────────────────────────────────────────────────────
Write-Host ''
Write-Host '── Reset complete ─────────────────────────────────────────────'
Write-Host '  Removed:'
if ($removed.Count -eq 0) { Write-Host '    (nothing existed to remove)' }
foreach ($r in $removed) { Write-Host "    $r" }
Write-Host '  Preserved:'
Write-Host "    $BinDir"
Write-Host "    $ManifestPath"
Write-Host "    $ReceiptToolManifestPath"
Write-Host "    $BackupRoot"
Write-Host "    $(Join-Path $DevnetRoot 'build')"
if ($failures.Count -gt 0) {
    Write-Host '  FAILED to remove:'
    foreach ($f in $failures) { Write-Host "    $f" }
    Write-Error "$($failures.Count) path(s) could not be removed."
    exit 1
}
Write-Host '  Next start-devnet.ps1 will begin from FRESH GENESIS.'
Write-Host '───────────────────────────────────────────────────────────────'
exit 0
