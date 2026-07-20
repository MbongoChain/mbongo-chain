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
