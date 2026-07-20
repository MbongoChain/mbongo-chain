# Shared configuration and helper functions for the Mbongo v0.3
# operational devnet (dot-source from the start/stop/status scripts).
#
# Defines variables and functions only — no side effects on load.

Set-StrictMode -Version Latest

# ── Pinned release identity ─────────────────────────────────────────────
# The devnet runs EXACTLY this tag. start-devnet refuses anything else.
$DevnetTag = 'v0.3-devnet-stable'
$DevnetCommit = '751034a121cb26701403cee2796cc3212e7a5365'

# ── Paths ───────────────────────────────────────────────────────────────
# Deployment root lives OUTSIDE the repository. Override with the
# MBONGO_DEVNET_ROOT environment variable.
if ($env:MBONGO_DEVNET_ROOT) {
    $DevnetRoot = $env:MBONGO_DEVNET_ROOT
} else {
    $DevnetRoot = 'C:\mbongo-devnet\v0.3'
}

# Repository that provides the tag (the repo containing this script).
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

$BuildDir = Join-Path $DevnetRoot 'build\src'
$BinDir = Join-Path $DevnetRoot 'bin'
$BinaryPath = Join-Path $BinDir 'mbongo-node.exe'
$ManifestPath = Join-Path $DevnetRoot 'manifest.json'

# Receipt tool: a deployed, hash-verified artifact built from a clean
# pinned source commit (build-receipt-tool.ps1). Never run from the
# live working tree.
$ReceiptToolBuildDir = Join-Path $DevnetRoot 'build\tool-src'
$ReceiptToolPath = Join-Path $BinDir 'submit_receipt.exe'
$ReceiptToolManifestPath = Join-Path $DevnetRoot 'receipt-tool-manifest.json'

# ── Devnet parameters ───────────────────────────────────────────────────
$BlockTimeSecs = 5
$RpcReadyTimeoutSecs = 60
$PeerIdTimeoutSecs = 20
$HeightAdvanceTimeoutSecs = 45

# ── Topology ────────────────────────────────────────────────────────────
$DevnetNodes = @(
    @{ Name = 'producer';   Role = 'producer'; Rpc = 9944; Rest = 8080; P2p = 30333; Producer = $true },
    @{ Name = 'follower-a'; Role = 'follower'; Rpc = 9945; Rest = 8081; P2p = 30334; Producer = $false },
    @{ Name = 'follower-b'; Role = 'follower'; Rpc = 9946; Rest = 8082; P2p = 30335; Producer = $false }
)

$BackupRoot = Join-Path $DevnetRoot 'backups'
$ReceiptRecordDir = Join-Path $DevnetRoot 'receipts'

# ── Path safety ─────────────────────────────────────────────────────────

# Validates the deployment root before any destructive or archival
# operation. Refuses empty values, missing paths, drive roots, overly
# shallow paths, and any root overlapping the repository. Returns the
# resolved absolute path.
function Assert-SafeDevnetRoot {
    if ([string]::IsNullOrWhiteSpace($DevnetRoot)) {
        throw 'DevnetRoot is empty; refusing.'
    }
    $resolved = $null
    try { $resolved = (Resolve-Path $DevnetRoot -ErrorAction Stop).ProviderPath } catch {
        throw "DevnetRoot '$DevnetRoot' does not exist."
    }
    $resolved = $resolved.TrimEnd('\')
    $driveRoot = [System.IO.Path]::GetPathRoot($resolved).TrimEnd('\')
    if ($resolved -eq $driveRoot) {
        throw "DevnetRoot '$resolved' is a drive root; refusing."
    }
    $rel = $resolved.Substring($driveRoot.Length).Trim('\')
    if ((@($rel -split '\\')).Count -lt 2) {
        throw "DevnetRoot '$resolved' is too shallow (fewer than two path segments below the drive); refusing."
    }
    $repo = (Resolve-Path $RepoRoot).ProviderPath.TrimEnd('\')
    if (($repo -ieq $resolved) -or
        $repo.StartsWith("$resolved\", [System.StringComparison]::OrdinalIgnoreCase) -or
        $resolved.StartsWith("$repo\", [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "DevnetRoot '$resolved' overlaps the repository '$repo'; refusing."
    }
    return $resolved
}

# Resolves a candidate path and requires it to live strictly inside the
# validated deployment root (defeats traversal values). Returns the
# full path.
function Assert-PathInsideRoot([string]$Path, [string]$ResolvedRoot) {
    $full = [System.IO.Path]::GetFullPath($Path)
    if (-not $full.StartsWith("$($ResolvedRoot.TrimEnd('\'))\", [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to touch '$full': outside the deployment root '$ResolvedRoot'."
    }
    return $full
}

# Returns the nodes of this deployment whose recorded PID is alive and
# still running the deployed binary.
function Get-RunningDevnetNodes {
    $running = @()
    foreach ($node in $DevnetNodes) {
        $proc = Get-RunningNodeProcess $node
        if ($null -ne $proc) { $running += $node.Name }
    }
    return , $running
}

# ── Path helpers ────────────────────────────────────────────────────────
function Get-NodeDir([hashtable]$Node) { Join-Path $DevnetRoot $Node.Name }
function Get-NodeDataDir([hashtable]$Node) { Join-Path (Get-NodeDir $Node) 'data' }
function Get-NodeLogDir([hashtable]$Node) { Join-Path (Get-NodeDir $Node) 'logs' }
function Get-NodePidFile([hashtable]$Node) { Join-Path (Get-NodeDir $Node) 'node.pid.json' }
function Get-NodeMarkerFile([hashtable]$Node) { Join-Path (Get-NodeDir $Node) 'deployment.json' }

# ── Hashing ─────────────────────────────────────────────────────────────
function Get-FileSha256([string]$Path) {
    (Get-FileHash -Algorithm SHA256 -Path $Path).Hash.ToLowerInvariant()
}

# ── Manifest ────────────────────────────────────────────────────────────
function Read-DevnetManifest {
    if (-not (Test-Path $ManifestPath)) { return $null }
    Get-Content $ManifestPath -Raw | ConvertFrom-Json
}

# Verifies the deployment manifest against the pinned tag/commit and the
# on-disk binary hash. Throws an actionable error on any mismatch.
function Assert-DevnetManifest {
    $manifest = Read-DevnetManifest
    if ($null -eq $manifest) {
        throw "No deployment manifest at $ManifestPath. Run start-devnet.ps1 to build the pinned binary first."
    }
    if ($manifest.tag -ne $DevnetTag) {
        throw "Manifest tag '$($manifest.tag)' does not match required '$DevnetTag'. Rebuild the deployment (delete $DevnetRoot\build and $DevnetRoot\bin, then re-run start-devnet.ps1)."
    }
    if ($manifest.commit -ne $DevnetCommit) {
        throw "Manifest commit '$($manifest.commit)' does not match required '$DevnetCommit'. Rebuild the deployment."
    }
    if ($manifest.binaryPath -ne $BinaryPath) {
        throw "Manifest binary path '$($manifest.binaryPath)' does not match expected '$BinaryPath'."
    }
    if (-not (Test-Path $BinaryPath)) {
        throw "Deployed binary missing at $BinaryPath. Re-run start-devnet.ps1 to rebuild."
    }
    $actual = Get-FileSha256 $BinaryPath
    if ($actual -ne $manifest.sha256) {
        throw "Binary hash mismatch: manifest=$($manifest.sha256) actual=$actual. The deployed binary is not the recorded v0.3 build; refusing. Rebuild the deployment."
    }
    return $manifest
}

function Read-ReceiptToolManifest {
    if (-not (Test-Path $ReceiptToolManifestPath)) { return $null }
    Get-Content $ReceiptToolManifestPath -Raw | ConvertFrom-Json
}

# Verifies the deployed receipt tool against its external manifest:
# manifest present, tool path matches, protocol compatibility matches
# the pinned devnet release, and the recomputed SHA-256 matches. Throws
# an actionable error on any mismatch. NEVER rebuilds anything.
function Assert-ReceiptToolManifest {
    $manifest = Read-ReceiptToolManifest
    if ($null -eq $manifest) {
        throw "No receipt-tool manifest at $ReceiptToolManifestPath. Build the tool from a pinned commit first: build-receipt-tool.ps1 -SourceCommit <sha>."
    }
    if ($manifest.toolPath -ne $ReceiptToolPath) {
        throw "Receipt-tool manifest path '$($manifest.toolPath)' does not match expected '$ReceiptToolPath'."
    }
    if ($manifest.protocolTag -ne $DevnetTag) {
        throw "Receipt tool was built for protocol '$($manifest.protocolTag)', but this deployment runs '$DevnetTag'. Rebuild with build-receipt-tool.ps1."
    }
    if ($manifest.protocolCommit -ne $DevnetCommit) {
        throw "Receipt tool targets protocol commit '$($manifest.protocolCommit)', expected '$DevnetCommit'. Rebuild with build-receipt-tool.ps1."
    }
    if (-not (Test-Path $ReceiptToolPath)) {
        throw "Receipt tool missing at $ReceiptToolPath. Rebuild with build-receipt-tool.ps1 -SourceCommit <sha>."
    }
    $actual = Get-FileSha256 $ReceiptToolPath
    if ($actual -ne $manifest.sha256) {
        throw "Receipt tool hash mismatch: manifest=$($manifest.sha256) actual=$actual. The deployed tool is not the recorded build; refusing to submit. Rebuild with build-receipt-tool.ps1 -SourceCommit <sha>."
    }
    return $manifest
}

# ── JSON-RPC helpers ────────────────────────────────────────────────────
function Invoke-DevnetRpc {
    param([int]$Port, [string]$Method)
    $body = @{ jsonrpc = '2.0'; method = $Method; id = 1 } | ConvertTo-Json -Compress
    $resp = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/rpc" -Method Post `
        -ContentType 'application/json' -Body $body -TimeoutSec 5
    if (($resp.PSObject.Properties.Name -contains 'error') -and ($null -ne $resp.error)) {
        throw "RPC error from port ${Port}: $($resp.error | ConvertTo-Json -Compress)"
    }
    return $resp.result
}

function Test-DevnetRpc([int]$Port) {
    try { $null = Invoke-DevnetRpc -Port $Port -Method 'ping'; return $true } catch { return $false }
}

# Waits (bounded) for a node's RPC to answer ping.
function Wait-DevnetRpc {
    param([int]$Port, [int]$TimeoutSecs, [string]$Label)
    $deadline = (Get-Date).AddSeconds($TimeoutSecs)
    while ((Get-Date) -lt $deadline) {
        if (Test-DevnetRpc $Port) { return }
        Start-Sleep -Milliseconds 500
    }
    throw "$Label RPC on port $Port did not become ready within ${TimeoutSecs}s. Check the node's log files."
}

function Get-DevnetHeight([int]$Port) {
    [uint64](Invoke-DevnetRpc -Port $Port -Method 'get_block_height')
}

function Get-DevnetTipHash([int]$Port) {
    [string](Invoke-DevnetRpc -Port $Port -Method 'get_latest_block_hash')
}

# ── Process/PID helpers ─────────────────────────────────────────────────
function Read-NodePidFile([hashtable]$Node) {
    $pidFile = Get-NodePidFile $Node
    if (-not (Test-Path $pidFile)) { return $null }
    Get-Content $pidFile -Raw | ConvertFrom-Json
}

# Returns the live process for a node ONLY if the recorded PID exists and
# still runs the deployed binary (never matches by executable name alone).
function Get-RunningNodeProcess([hashtable]$Node) {
    $info = Read-NodePidFile $Node
    if ($null -eq $info) { return $null }
    $proc = $null
    try { $proc = Get-Process -Id $info.pid -ErrorAction Stop } catch { return $null }
    $procPath = $null
    try { $procPath = $proc.Path } catch { return $null }
    if ($procPath -ne $info.exePath) { return $null }
    return $proc
}

# ── Port helpers ────────────────────────────────────────────────────────
function Get-PortListener([int]$Port) {
    Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue
}

function Get-NodePorts([hashtable]$Node) {
    @($Node.Rpc, $Node.Rest, $Node.P2p)
}
