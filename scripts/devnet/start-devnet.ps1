# Starts the persistent three-node Mbongo v0.3 devnet (producer + two
# followers) from a binary built at exactly the pinned release tag.
#
# Refuses to start if: the manifest/tag/commit/binary hash do not match,
# the deployment is already running, configured ports are occupied, or a
# data directory has unknown provenance. Never deletes or migrates data.
#
# Usage: .\start-devnet.ps1

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

function Write-Step([string]$Message) { Write-Host "==> $Message" }

# ── Build (one-time): clean tag worktree, pinned and verified ──────────
function Initialize-DevnetBuild {
    if ((Test-Path $ManifestPath) -and (Test-Path $BinaryPath)) {
        return
    }
    Write-Step "No verified binary yet; building $DevnetTag from a clean worktree"

    if (-not (Test-Path (Join-Path $RepoRoot '.git'))) {
        throw "Repository not found at $RepoRoot (needed to build the tag)."
    }

    if (-not (Test-Path $BuildDir)) {
        New-Item -ItemType Directory -Force (Split-Path $BuildDir) | Out-Null
        git -C $RepoRoot worktree add $BuildDir $DevnetTag
        if ($LASTEXITCODE -ne 0) {
            throw "git worktree add failed (exit $LASTEXITCODE). Ensure tag $DevnetTag exists (git fetch --tags)."
        }
    }

    # The build tree must be at exactly the pinned tag and commit.
    $head = (git -C $BuildDir rev-parse HEAD).Trim()
    if ($head -ne $DevnetCommit) {
        throw "Build worktree at $BuildDir is at commit $head, expected $DevnetCommit. Remove the worktree (git -C $RepoRoot worktree remove $BuildDir) and re-run."
    }
    $desc = (git -C $BuildDir describe --tags --exact-match).Trim()
    if ($desc -ne $DevnetTag) {
        throw "Build worktree describes as '$desc', expected '$DevnetTag'."
    }
    $dirty = git -C $BuildDir status --porcelain
    if ($dirty) {
        throw "Build worktree is not clean:`n$dirty`nRemove the worktree and re-run."
    }

    Write-Step 'Building mbongo-node (release, --locked) — first build takes several minutes'
    Push-Location $BuildDir
    try {
        cargo build --release --locked -p mbongo-node
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)." }
    } finally {
        Pop-Location
    }

    $built = Join-Path $BuildDir 'target\release\mbongo-node.exe'
    if (-not (Test-Path $built)) { throw "Build succeeded but binary not found at $built." }

    New-Item -ItemType Directory -Force $BinDir | Out-Null
    Copy-Item $built $BinaryPath -Force

    $manifest = [ordered]@{
        tag        = $DevnetTag
        commit     = $DevnetCommit
        binaryPath = $BinaryPath
        sha256     = Get-FileSha256 $BinaryPath
        builtAtUtc = (Get-Date).ToUniversalTime().ToString('o')
    }
    New-Item -ItemType Directory -Force $DevnetRoot | Out-Null
    $manifest | ConvertTo-Json | Out-File -FilePath $ManifestPath -Encoding utf8
    Write-Step "Manifest written: $ManifestPath (sha256 $($manifest.sha256))"
}

# ── Pre-flight checks ──────────────────────────────────────────────────
function Assert-NotRunning {
    foreach ($node in $DevnetNodes) {
        $proc = Get-RunningNodeProcess $node
        if ($null -ne $proc) {
            throw "Node '$($node.Name)' is already running (PID $($proc.Id)). Refusing to overwrite a running deployment; use stop-devnet.ps1 first."
        }
        # Stale PID file (process gone or different executable): remove it.
        $pidFile = Get-NodePidFile $node
        if (Test-Path $pidFile) {
            Write-Step "Removing stale PID file for $($node.Name) ($pidFile)"
            Remove-Item $pidFile -Force
        }
    }
}

function Assert-PortsFree {
    foreach ($node in $DevnetNodes) {
        foreach ($port in (Get-NodePorts $node)) {
            $listener = Get-PortListener $port
            if ($null -ne $listener) {
                $owner = ($listener | Select-Object -First 1).OwningProcess
                throw "Port $port (needed by $($node.Name)) is already in use by PID $owner. Free the port or change devnet-config.ps1."
            }
        }
    }
}

# Data safety: reuse a non-empty data directory ONLY when its provenance
# marker matches this deployment. Never delete, reset, or migrate.
function Assert-DataProvenance([hashtable]$Node) {
    $dataDir = Get-NodeDataDir $Node
    $marker = Get-NodeMarkerFile $Node
    $hasData = (Test-Path $dataDir) -and ($null -ne (Get-ChildItem $dataDir -Force -ErrorAction SilentlyContinue | Select-Object -First 1))

    if ($hasData) {
        if (-not (Test-Path $marker)) {
            throw "Data directory $dataDir is non-empty but has no provenance marker ($marker). Refusing to touch data of unknown origin (it may be a v0.2 or foreign chain). The future reset procedure handles backup and confirmed wiping; for now move the directory aside manually if you intend a fresh start."
        }
        $m = Get-Content $marker -Raw | ConvertFrom-Json
        if (($m.tag -ne $DevnetTag) -or ($m.commit -ne $DevnetCommit)) {
            throw "Data directory $dataDir belongs to deployment '$($m.tag)' ($($m.commit)), not '$DevnetTag' ($DevnetCommit). Refusing to reuse it."
        }
        Write-Step "$($Node.Name): resuming existing v0.3 data directory"
    } else {
        New-Item -ItemType Directory -Force $dataDir | Out-Null
        [ordered]@{
            tag          = $DevnetTag
            commit       = $DevnetCommit
            createdAtUtc = (Get-Date).ToUniversalTime().ToString('o')
        } | ConvertTo-Json | Out-File -FilePath $marker -Encoding utf8
        Write-Step "$($Node.Name): fresh data directory (fresh genesis on first start)"
    }
}

# ── Node startup ───────────────────────────────────────────────────────
function Start-Node {
    param([hashtable]$Node, [string]$Bootnode)

    $logDir = Get-NodeLogDir $Node
    New-Item -ItemType Directory -Force $logDir | Out-Null
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $outLog = Join-Path $logDir "$($Node.Name)-$stamp.out.log"
    $errLog = Join-Path $logDir "$($Node.Name)-$stamp.err.log"

    $nodeArgs = @(
        '--rpc-port', $Node.Rpc,
        '--rest-port', $Node.Rest,
        '--p2p-port', $Node.P2p,
        '--data-dir', (Get-NodeDataDir $Node)
    )
    if ($Node.Producer) { $nodeArgs += @('--producer', '--block-time', $BlockTimeSecs) }
    if ($Bootnode) { $nodeArgs += @('--bootnodes', $Bootnode) }

    $env:RUST_LOG = 'info'
    $proc = Start-Process -FilePath $BinaryPath -ArgumentList $nodeArgs `
        -RedirectStandardOutput $outLog -RedirectStandardError $errLog `
        -WindowStyle Hidden -PassThru

    [ordered]@{
        pid          = $proc.Id
        role         = $Node.Role
        exePath      = $BinaryPath
        startedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
        outLog       = $outLog
        errLog       = $errLog
    } | ConvertTo-Json | Out-File -FilePath (Get-NodePidFile $Node) -Encoding utf8

    Write-Step "$($Node.Name) started (PID $($proc.Id)); logs: $outLog"
    return $outLog
}

# Reads the freshly started producer's PeerId from its stdout log
# (bounded). The identity is EPHEMERAL per process start, so the bootnode
# address is derived fresh every start and never persisted.
function Get-ProducerPeerId([string]$OutLog) {
    $deadline = (Get-Date).AddSeconds($PeerIdTimeoutSecs)
    while ((Get-Date) -lt $deadline) {
        if (Test-Path $OutLog) {
            $line = Select-String -Path $OutLog -Pattern '^\s*PeerId:\s*(\S+)' -ErrorAction SilentlyContinue |
                Select-Object -First 1
            if ($null -ne $line) { return $line.Matches[0].Groups[1].Value }
        }
        Start-Sleep -Milliseconds 500
    }
    return $null
}

# ── Main ───────────────────────────────────────────────────────────────
Write-Step "Mbongo v0.3 devnet start (root: $DevnetRoot)"

Initialize-DevnetBuild
$manifest = Assert-DevnetManifest
Write-Step "Verified binary: $DevnetTag @ $($manifest.commit.Substring(0,7)) (sha256 $($manifest.sha256.Substring(0,12))...)"

Assert-NotRunning
Assert-PortsFree
foreach ($node in $DevnetNodes) { Assert-DataProvenance $node }

# Producer first.
$producer = $DevnetNodes | Where-Object { $_.Producer } | Select-Object -First 1
$producerLog = Start-Node -Node $producer
Wait-DevnetRpc -Port $producer.Rpc -TimeoutSecs $RpcReadyTimeoutSecs -Label 'producer'
Write-Step 'Producer RPC ready'

$peerId = Get-ProducerPeerId $producerLog
$bootnode = $null
if ($peerId) {
    $bootnode = "/ip4/127.0.0.1/tcp/$($producer.P2p)/p2p/$peerId"
    Write-Step "Producer PeerId: $peerId"
} else {
    Write-Warning 'Could not read producer PeerId from logs; followers will rely on same-host mDNS discovery.'
}

# Followers.
foreach ($node in ($DevnetNodes | Where-Object { -not $_.Producer })) {
    $null = Start-Node -Node $node -Bootnode $bootnode
    Wait-DevnetRpc -Port $node.Rpc -TimeoutSecs $RpcReadyTimeoutSecs -Label $node.Name
    Write-Step "$($node.Name) RPC ready"
}

# Verify block height begins advancing on the producer.
Write-Step 'Waiting for block production to advance...'
$h0 = Get-DevnetHeight $producer.Rpc
$deadline = (Get-Date).AddSeconds($HeightAdvanceTimeoutSecs)
$advanced = $false
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 2
    $h = Get-DevnetHeight $producer.Rpc
    if ($h -gt $h0) { $advanced = $true; break }
}
if (-not $advanced) {
    Write-Error "Producer height did not advance past $h0 within ${HeightAdvanceTimeoutSecs}s. Check $producerLog"
    exit 1
}

# ── Summary ────────────────────────────────────────────────────────────
Write-Host ''
Write-Host '── Devnet started ─────────────────────────────────────────────'
Write-Host "  Tag:    $DevnetTag @ $DevnetCommit"
Write-Host "  Binary: $BinaryPath"
Write-Host "  Root:   $DevnetRoot"
foreach ($node in $DevnetNodes) {
    $info = Read-NodePidFile $node
    $height = Get-DevnetHeight $node.Rpc
    Write-Host ("  {0,-10} PID {1,-7} RPC {2}  REST {3}  P2P {4}  height {5}" -f `
        $node.Name, $info.pid, $node.Rpc, $node.Rest, $node.P2p, $height)
}
Write-Host '  Check convergence anytime with: status-devnet.ps1'
Write-Host '───────────────────────────────────────────────────────────────'
exit 0
