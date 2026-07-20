# Creates a consistent, verified backup of the v0.3 devnet deployment's
# runtime state as a single timestamped ZIP archive plus a metadata
# sidecar under <DevnetRoot>\backups\.
#
# Included: node data directories, node provenance markers, PID
# metadata (audit only — never treated as live processes on restore),
# the latest log pair per node, receipt submission records, and the
# node/receipt-tool manifests. Excluded: build worktrees, Cargo target
# directories, and deployed binaries (reproducible from the pinned
# commits recorded in the manifests).
#
# All nodes must be STOPPED for a consistent RocksDB copy; the script
# refuses otherwise. Existing archives are never overwritten.
#
# Usage: .\backup-devnet.ps1 [-Label <name>]

param(
    [string]$Label
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

$resolvedRoot = Assert-SafeDevnetRoot

# ── Consistency: every node must be stopped ────────────────────────────
$running = Get-RunningDevnetNodes
if ($running.Count -gt 0) {
    throw "Cannot take a consistent backup while nodes are running ($($running -join ', ')). Stop the devnet first: stop-devnet.ps1"
}

# ── Names (never overwrite) ────────────────────────────────────────────
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMdd-HHmmss') + 'Z'
$name = if ($Label) { "devnet-backup-$Label" } else { "devnet-backup-$stamp" }
$zipPath = Join-Path $BackupRoot "$name.zip"
$metaPath = Join-Path $BackupRoot "$name.metadata.json"
if ((Test-Path $zipPath) -or (Test-Path $metaPath)) {
    throw "Backup '$name' already exists under $BackupRoot; refusing to overwrite. Choose a different -Label."
}

$nodeManifest = Read-DevnetManifest
if ($null -eq $nodeManifest) {
    throw "No deployment manifest at $ManifestPath; nothing recognizable to back up."
}

Write-Host "==> Backing up deployment at $resolvedRoot"
New-Item -ItemType Directory -Force $BackupRoot | Out-Null
$staging = Join-Path $BackupRoot ".staging-$name"
if (Test-Path $staging) { Remove-Item $staging -Recurse -Force }
New-Item -ItemType Directory -Force $staging | Out-Null

$copied = @()
function Copy-IntoStaging([string]$Source, [string]$RelativeTarget) {
    if (-not (Test-Path $Source)) { return }
    $dest = Join-Path $staging $RelativeTarget
    New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
    Copy-Item $Source $dest -Recurse -Force
}

try {
    foreach ($node in $DevnetNodes) {
        $nodeDir = Get-NodeDir $node
        Copy-IntoStaging (Get-NodeDataDir $node) "$($node.Name)\data"
        Copy-IntoStaging (Get-NodeMarkerFile $node) "$($node.Name)\deployment.json"
        # PID metadata: audit trail only. Restored PID files must never
        # be treated as live process state.
        Copy-IntoStaging (Get-NodePidFile $node) "$($node.Name)\node.pid.json"

        # Log policy: the most recent .out.log/.err.log pair per node.
        $logDir = Get-NodeLogDir $node
        if (Test-Path $logDir) {
            $latestOut = Get-ChildItem $logDir -Filter '*.out.log' -ErrorAction SilentlyContinue |
                Sort-Object LastWriteTime -Descending | Select-Object -First 1
            $latestErr = Get-ChildItem $logDir -Filter '*.err.log' -ErrorAction SilentlyContinue |
                Sort-Object LastWriteTime -Descending | Select-Object -First 1
            foreach ($log in @($latestOut, $latestErr)) {
                if ($null -ne $log) {
                    Copy-IntoStaging $log.FullName "$($node.Name)\logs\$($log.Name)"
                }
            }
        }
    }
    Copy-IntoStaging $ReceiptRecordDir 'receipts'
    Copy-IntoStaging $ManifestPath 'manifest.json'
    Copy-IntoStaging $ReceiptToolManifestPath 'receipt-tool-manifest.json'

    $stagedFiles = Get-ChildItem $staging -Recurse -File
    if ($stagedFiles.Count -eq 0) { throw 'Nothing was staged; refusing to write an empty backup.' }
    $fileList = $stagedFiles | ForEach-Object { $_.FullName.Substring($staging.Length + 1) }

    Write-Host "==> Compressing $($stagedFiles.Count) files..."
    Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $zipPath -CompressionLevel Optimal

    # ── Verify the completed archive ───────────────────────────────────
    $archiveSha = Get-FileSha256 $zipPath
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
    try {
        $entryCount = @($zip.Entries | Where-Object { -not $_.FullName.EndsWith('/') }).Count
    } finally {
        $zip.Dispose()
    }
    if ($entryCount -ne $stagedFiles.Count) {
        throw "Archive verification failed: $entryCount entries in zip, $($stagedFiles.Count) staged files."
    }

    # ── Metadata sidecar ───────────────────────────────────────────────
    $toolManifest = Read-ReceiptToolManifest
    $toolingCommit = (git -C $RepoRoot rev-parse HEAD).Trim()
    [ordered]@{
        createdAtUtc     = (Get-Date).ToUniversalTime().ToString('o')
        protocolTag      = $DevnetTag
        protocolCommit   = $DevnetCommit
        toolingCommit    = $toolingCommit
        nodeBinarySha256 = $nodeManifest.sha256
        receiptToolSha256 = if ($null -ne $toolManifest) { $toolManifest.sha256 } else { $null }
        sourceRoot       = $resolvedRoot
        archive          = $zipPath
        archiveSha256    = $archiveSha
        archiveEntryCount = $entryCount
        configSnapshot   = @{
            blockTimeSecs = $BlockTimeSecs
            nodes         = @($DevnetNodes | ForEach-Object {
                @{ name = $_.Name; role = $_.Role; rpc = $_.Rpc; rest = $_.Rest; p2p = $_.P2p }
            })
        }
        files            = $fileList
    } | ConvertTo-Json -Depth 6 | Out-File -FilePath $metaPath -Encoding utf8
} finally {
    if (Test-Path $staging) { Remove-Item $staging -Recurse -Force }
}

Write-Host ''
Write-Host '── Backup complete ────────────────────────────────────────────'
Write-Host "  Archive:  $zipPath"
Write-Host "  SHA256:   $archiveSha"
Write-Host "  Files:    $entryCount"
Write-Host "  Metadata: $metaPath"
Write-Host '───────────────────────────────────────────────────────────────'
exit 0
