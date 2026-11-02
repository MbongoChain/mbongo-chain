# Reports the state of the Mbongo v0.3 devnet deployment: per-node
# process/RPC/chain state, port listeners, convergence, and the deployed
# tag/commit/binary-hash from the manifest. Read-only.
#
# Exit code: 0 when all nodes are running, reachable, and converged;
# 1 otherwise.
#
# Usage: .\status-devnet.ps1

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

$healthy = $true

# ── Deployment identity ────────────────────────────────────────────────
Write-Host '── Deployment ─────────────────────────────────────────────────'
$manifest = Read-DevnetManifest
if ($null -eq $manifest) {
    Write-Host "  No manifest at $ManifestPath (deployment not built yet)"
    $healthy = $false
} else {
    Write-Host "  Tag:    $($manifest.tag)"
    Write-Host "  Commit: $($manifest.commit)"
    Write-Host "  Binary: $($manifest.binaryPath)"
    if (Test-Path $manifest.binaryPath) {
        $actual = Get-FileSha256 $manifest.binaryPath
        if ($actual -eq $manifest.sha256) {
            Write-Host "  SHA256: $actual (verified)"
        } else {
            Write-Host "  SHA256: MISMATCH manifest=$($manifest.sha256) actual=$actual"
            $healthy = $false
        }
    } else {
        Write-Host '  Binary: MISSING on disk'
        $healthy = $false
    }
}

# ── Per-node status ────────────────────────────────────────────────────
Write-Host ''
Write-Host '── Nodes ──────────────────────────────────────────────────────'
$heights = @{}
$hashes = @{}
foreach ($node in $DevnetNodes) {
    Write-Host "  $($node.Name) [$($node.Role)]"

    $info = Read-NodePidFile $node
    if ($null -eq $info) {
        Write-Host '    process:  no PID file (not started)'
        $healthy = $false
    } else {
        $proc = Get-RunningNodeProcess $node
        if ($null -ne $proc) {
            Write-Host "    process:  alive (PID $($proc.Id), exe verified, started $($info.startedAtUtc))"
        } else {
            Write-Host "    process:  DEAD or wrong executable (recorded PID $($info.pid))"
            $healthy = $false
        }
        Write-Host "    log:      $($info.outLog)"
    }
    Write-Host "    data dir: $(Get-NodeDataDir $node)"

    if (Test-DevnetRpc $node.Rpc) {
        $h = Get-DevnetHeight $node.Rpc
        $tip = Get-DevnetTipHash $node.Rpc
        $heights[$node.Name] = $h
        $hashes[$node.Name] = $tip
        Write-Host "    rpc:      reachable (port $($node.Rpc))"
        Write-Host "    height:   $h"
        Write-Host "    tip:      $tip"
    } else {
        Write-Host "    rpc:      UNREACHABLE (port $($node.Rpc))"
        $healthy = $false
    }

    $portStates = @()
    foreach ($port in (Get-NodePorts $node)) {
        $listening = $null -ne (Get-PortListener $port)
        if (-not $listening) { $healthy = $false }
        $stateText = if ($listening) { 'listening' } else { 'NOT LISTENING' }
        $portStates += "${port}:$stateText"
    }
    Write-Host "    ports:    $($portStates -join '  ')"
}

# ── Convergence (single sample; production is continuous, so a one-block
# height skew between samples of different nodes is normal — identical
# tip hashes at identical heights is the converged signal) ─────────────
Write-Host ''
Write-Host '── Convergence ────────────────────────────────────────────────'
if ($heights.Count -eq $DevnetNodes.Count) {
    # Force arrays: Sort-Object -Unique returning one value unwraps to a
    # scalar in PowerShell 5.1, which has no .Count under StrictMode.
    $uniqueHeights = @($heights.Values | Sort-Object -Unique)
    $uniqueHashes = @($hashes.Values | Sort-Object -Unique)
    if (($uniqueHeights.Count -eq 1) -and ($uniqueHashes.Count -eq 1)) {
        Write-Host "  CONVERGED at height $($uniqueHeights[0])"
    } elseif ([math]::Abs([int64]($heights.Values | Measure-Object -Maximum).Maximum - [int64]($heights.Values | Measure-Object -Minimum).Minimum) -le 1) {
        Write-Host '  Within one block of each other (normal while producing); re-run to confirm'
    } else {
        Write-Host "  NOT converged: heights $($heights.Values -join ', ')"
        $healthy = $false
    }
} else {
    Write-Host '  Cannot assess: not all nodes reachable'
    $healthy = $false
}

Write-Host ''
if ($healthy) {
    Write-Host 'STATUS: healthy'
    exit 0
}
Write-Host 'STATUS: degraded (see items above)'
exit 1
