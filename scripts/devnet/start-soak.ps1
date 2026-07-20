# Starts a soak observation session against the running v0.3 devnet:
# creates immutable session metadata under <DevnetRoot>\soak\ and
# launches soak-check.ps1 in loop mode as a separate PowerShell
# process. Read-only towards the devnet; never uses Task Scheduler and
# needs no administrator rights.
#
# Usage: .\start-soak.ps1 [-IntervalMinutes <n>] [-PlannedHours <n>]
#                         [-Label <name>]

param(
    [double]$IntervalMinutes = 5,
    [double]$PlannedHours = 0,
    [string]$Label
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

$resolvedRoot = Assert-SafeDevnetRoot

if ($IntervalMinutes -lt 1) {
    throw "IntervalMinutes must be at least 1 (got $IntervalMinutes)."
}
if ($Label -and ($Label -notmatch '^[A-Za-z0-9_-]{1,40}$')) {
    throw 'Label may only contain letters, digits, hyphen, underscore (max 40 chars).'
}

# --- The devnet must be running and manifest-valid ---------------------
$manifest = Assert-DevnetManifest
foreach ($node in $DevnetNodes) {
    $proc = Get-RunningNodeProcess $node
    if ($null -eq $proc) {
        throw "Node '$($node.Name)' is not running (or its executable path failed validation). Start the devnet first: start-devnet.ps1"
    }
    if (-not (Test-DevnetRpc $node.Rpc)) {
        throw "Node '$($node.Name)' RPC (port $($node.Rpc)) is unreachable; refusing to start a soak against an unhealthy devnet."
    }
}

# --- Refuse a second concurrent soak session ---------------------------
if (Test-Path $SoakRoot) {
    foreach ($pidFile in (Get-ChildItem $SoakRoot -Recurse -Filter 'soak.pid.json' -ErrorAction SilentlyContinue)) {
        $info = Get-Content $pidFile.FullName -Raw | ConvertFrom-Json
        $proc = $null
        try { $proc = Get-Process -Id $info.pid -ErrorAction Stop } catch {}
        if (($null -ne $proc) -and ($proc.Path -eq $info.exePath)) {
            throw "Another soak sampler is already running (PID $($info.pid), session $($pidFile.DirectoryName)). Stop it first: stop-soak.ps1 -SessionPath '$($pidFile.DirectoryName)'"
        }
    }
}

# --- Create the immutable session --------------------------------------
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMdd-HHmmss')
$sessionId = if ($Label) { "soak-$stamp-$Label" } else { "soak-$stamp" }
$sessionPath = Join-Path $SoakRoot $sessionId
if (Test-Path $sessionPath) { throw "Session directory $sessionPath already exists." }
New-Item -ItemType Directory -Force $sessionPath | Out-Null

$toolManifest = Read-ReceiptToolManifest
$toolingCommit = (git -C $RepoRoot rev-parse HEAD).Trim()
[ordered]@{
    sessionId        = $sessionId
    startedAtUtc     = (Get-Date).ToUniversalTime().ToString('o')
    intervalMinutes  = $IntervalMinutes
    plannedHours     = $PlannedHours
    protocolTag      = $DevnetTag
    protocolCommit   = $DevnetCommit
    toolingCommit    = $toolingCommit
    nodeBinarySha256 = $manifest.sha256
    receiptToolSha256 = if ($null -ne $toolManifest) { $toolManifest.sha256 } else { $null }
    devnetRoot       = $resolvedRoot
    thresholds       = $SoakThresholds
} | ConvertTo-Json -Depth 4 | Out-File -FilePath (Join-Path $sessionPath 'session.json') -Encoding utf8

# --- Launch the sampler as a separate process --------------------------
$samplerScript = Join-Path $PSScriptRoot 'soak-check.ps1'
$samplerOut = Join-Path $sessionPath 'sampler.out.log'
$samplerErr = Join-Path $sessionPath 'sampler.err.log'
$env:MBONGO_DEVNET_ROOT = $DevnetRoot
$proc = Start-Process -FilePath 'powershell' -ArgumentList @(
    '-NoProfile', '-ExecutionPolicy', 'Bypass',
    '-File', $samplerScript, '-SessionPath', $sessionPath, '-Loop'
) -RedirectStandardOutput $samplerOut -RedirectStandardError $samplerErr `
    -WindowStyle Hidden -PassThru

[ordered]@{
    pid           = $proc.Id
    exePath       = $proc.Path
    samplerScript = $samplerScript
    sessionPath   = $sessionPath
    startedAtUtc  = (Get-Date).ToUniversalTime().ToString('o')
} | ConvertTo-Json | Out-File -FilePath (Join-Path $sessionPath 'soak.pid.json') -Encoding utf8

Write-Host ''
Write-Host '--- Soak session started --------------------------------------'
Write-Host "  Session:  $sessionPath"
Write-Host "  Sampler:  PID $($proc.Id), every $IntervalMinutes min$(if ($PlannedHours -gt 0) { ", planned $PlannedHours h" } else { ' (until stopped)' })"
Write-Host "  Stop:     stop-soak.ps1 -SessionPath '$sessionPath'"
Write-Host "  Report:   soak-report.ps1 -SessionPath '$sessionPath'"
Write-Host '----------------------------------------------------------------'
exit 0
