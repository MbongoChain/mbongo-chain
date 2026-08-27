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
$SoakRoot = Join-Path $DevnetRoot 'soak'

# --- Soak thresholds ----------------------------------------------------
# Conservative devnet values (not SLA claims). Snapshotted immutably
# into each soak session's session.json at start; the report evaluates
# against the snapshot, so changing these mid-soak has no effect.
$SoakThresholds = @{
    ConvergenceSkewBlocks     = 1     # allowed moving-tip height spread
    StalledFailMinutes        = 10    # producer no-progress streak -> FAIL
    RpcOutageWarnMinutes      = 5     # node RPC outage streak -> warning
    RpcOutageFailMinutes      = 15    # node RPC outage streak -> FAIL
    RssWarnMb                 = 500   # per-process working set -> warning
    RssFailMb                 = 1500  # per-process working set -> FAIL
    DataGrowthWarnMbPerHour   = 50    # sustained growth rate -> warning
    WarningsPerIntervalWarn   = 10    # log warnings in one interval -> warning
    ErrorsPerIntervalFail     = 25    # log errors in one interval -> FAIL
    MissingSampleWarnPercent  = 5     # missing samples -> warning
    MissingSampleFailPercent  = 20    # missing samples -> FAIL
}

# --- UTC-safe timestamp parsing -----------------------------------------
# Parses a persisted ISO-8601 timestamp into a DateTimeOffset, preserving
# the true UTC instant regardless of the host time zone.
#
# An implicit [datetime] cast of "....Z" converts the value to LOCAL
# wall-clock time and tags it Kind=Local. PowerShell's subtraction and
# comparison operators on DateTime do NOT normalize Kind, so mixing that
# with a Kind=Utc value (e.g. (Get-Date).ToUniversalTime()) silently adds
# the local UTC offset -- on an America/Toronto host that is +14400
# seconds. Always parse persisted timestamps with this helper and do the
# arithmetic on DateTimeOffset instants.
function ConvertFrom-IsoUtc([string]$Value) {
    return [System.DateTimeOffset]::Parse(
        $Value,
        [System.Globalization.CultureInfo]::InvariantCulture,
        [System.Globalization.DateTimeStyles]::RoundtripKind)
}

# --- Soak CSV schema (single source of truth) ---------------------------
# The 29 columns in fixed order. Rows are built as ordered PSCustomObjects
# with exactly these properties and serialized through ConvertTo-Csv, so
# commas, quotes, newlines, and locale-specific decimal formatting cannot
# alter the field count. Shared by soak-check (writer) and soak-report
# (validator/reader).
$SoakSchema = @(
    'timestampUtc', 'scope', 'node', 'role', 'pid', 'processAlive', 'exeValid',
    'rpcReachable', 'restReachable', 'height', 'tipHash', 'heightDelta', 'rssMb',
    'cpuTotalSec', 'cpuDeltaSec', 'dataSizeMb', 'dataGrowthMb', 'outLogSizeKb',
    'errLogSizeKb', 'newWarnings', 'newErrors', 'allReachable', 'heightSpread',
    'convergence', 'producerDelta', 'totalDataMb', 'totalNewWarnings',
    'totalNewErrors', 'sessionUptimeSec'
)
$SoakNumericColumns = @(
    'pid', 'height', 'heightDelta', 'rssMb', 'cpuTotalSec', 'cpuDeltaSec',
    'dataSizeMb', 'dataGrowthMb', 'outLogSizeKb', 'errLogSizeKb', 'newWarnings',
    'newErrors', 'heightSpread', 'producerDelta', 'totalDataMb',
    'totalNewWarnings', 'totalNewErrors', 'sessionUptimeSec'
)
# The full set of convergence classifications a session row may contain.
# soak-report uses this to detect shifted data (e.g. a numeric value
# landing in the convergence column).
$SoakConvergenceStates = @(
    'converged', 'temporarily-skewed', 'stalled', 'divergent', 'unreachable'
)

# Formats a numeric value as an InvariantCulture string ('' passes
# through). Guarantees a '.' decimal separator regardless of host locale
# (e.g. fr-CA would otherwise emit "18,5", adding a spurious CSV comma).
function Format-SoakNum($v) {
    if ($null -eq $v -or "$v" -eq '') { return '' }
    return ([double]$v).ToString([System.Globalization.CultureInfo]::InvariantCulture)
}

# Builds an ordered PSCustomObject with exactly the 29 schema properties,
# filling any not supplied with ''. Numeric columns are invariant-formatted.
function New-SoakRow([hashtable]$Fields) {
    $o = [ordered]@{}
    foreach ($col in $SoakSchema) {
        $val = if ($Fields.ContainsKey($col)) { $Fields[$col] } else { '' }
        if ($SoakNumericColumns -contains $col) { $val = Format-SoakNum $val }
        $o[$col] = $val
    }
    return [PSCustomObject]$o
}

# The canonical header line exactly as ConvertTo-Csv emits it for the
# schema. Used to validate existing CSVs.
$SoakCanonicalHeader = ((New-SoakRow @{}) | ConvertTo-Csv -NoTypeInformation)[0]

# --- Convergence classification (pure function, unit-testable) ----------
# Classifies one sample using only data available from existing RPCs.
# Priority: unreachable > divergent > stalled > temporarily-skewed >
# converged.
#
#   AllReachable   - every node answered RPC this sample
#   HeightSpread   - max height minus min height across nodes
#   TipsConsistent - equal heights: all tip hashes identical;
#                    skewed heights: the full block JSON at the common
#                    minimum height is identical on every node (an
#                    ancestry check that needs no local hashing)
#   ProducerDelta  - producer height minus previous sample's, or $null
#                    on the first sample
function Get-ConvergenceClassification {
    param(
        [bool]$AllReachable,
        [long]$HeightSpread,
        [bool]$TipsConsistent,
        $ProducerDelta,
        [int]$SkewAllowance
    )
    if (-not $AllReachable) { return 'unreachable' }
    if (-not $TipsConsistent) { return 'divergent' }
    if ($HeightSpread -gt $SkewAllowance) { return 'divergent' }
    if (($null -ne $ProducerDelta) -and ([long]$ProducerDelta -le 0)) { return 'stalled' }
    if ($HeightSpread -gt 0) { return 'temporarily-skewed' }
    return 'converged'
}

# Returns the directory size in MB (2 decimals) or 0 when absent.
function Get-DirectorySizeMb([string]$Path) {
    if (-not (Test-Path $Path)) { return 0 }
    $sum = (Get-ChildItem $Path -Recurse -File -ErrorAction SilentlyContinue |
        Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) { return 0 }
    [math]::Round($sum / 1MB, 2)
}

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

# ── Soak sampler identity & lifecycle (pure / injectable) ────────────────
# stop-soak must recognize THIS session's live sampler before stopping it,
# and must never (a) kill an unrelated PowerShell process, nor (b) declare a
# genuinely-live sampler "stale" and delete its PID file + emit a premature
# report. The decision is factored into these pure functions (no process I/O
# here) so it is fully unit-testable: the caller gathers the live facts and
# passes them in.

# Case-insensitive test that a command line references a script by its leaf
# name (e.g. 'soak-check.ps1'). Ordinal IndexOf, so any characters in the
# name are treated literally (never as wildcards).
function Test-CommandLineReferencesLeaf([string]$CommandLine, [string]$Leaf) {
    if ([string]::IsNullOrWhiteSpace($CommandLine)) { return $false }
    if ([string]::IsNullOrWhiteSpace($Leaf)) { return $false }
    return ($CommandLine.IndexOf($Leaf, [System.StringComparison]::OrdinalIgnoreCase) -ge 0)
}

# Case-insensitive test that a command line references an absolute path as a
# WHOLE path token. Normalizes separators (so '/' vs '\' does not matter),
# tolerates a trailing separator, but refuses to match a longer sibling
# directory: path 'C:\a\smoke' must NOT match a command line that contains
# only 'C:\a\smoke-2'. This replaces the previous `-like "*$SessionPath*"`
# check, which was wildcard-unsafe, separator- and trailing-slash-sensitive,
# and permissive to sibling prefixes.
function Test-CommandLineReferencesPath([string]$CommandLine, [string]$Path) {
    if ([string]::IsNullOrWhiteSpace($CommandLine)) { return $false }
    if ([string]::IsNullOrWhiteSpace($Path)) { return $false }
    $needle = $null
    try { $needle = ([System.IO.Path]::GetFullPath($Path)).TrimEnd('\') } catch { return $false }
    if ([string]::IsNullOrWhiteSpace($needle)) { return $false }
    $hay = $CommandLine.Replace('/', '\')
    $start = 0
    while ($true) {
        $idx = $hay.IndexOf($needle, $start, [System.StringComparison]::OrdinalIgnoreCase)
        if ($idx -lt 0) { return $false }
        $after = $idx + $needle.Length
        if ($after -ge $hay.Length) { return $true }
        $c = $hay[$after]
        # A separator, whitespace, or a closing quote ends the path token.
        if (($c -eq '\') -or ($c -eq ' ') -or ($c -eq "`t") -or ($c -eq '"') -or ($c -eq "'")) { return $true }
        $start = $after
    }
}

# True only when the live executable is the SAME PowerShell host recorded at
# launch: the recorded and live paths must resolve equal (case-insensitive,
# separator-normalized) AND the leaf must be a known PowerShell host
# (powershell.exe for Windows PowerShell 5.1, pwsh.exe for PowerShell 7+).
# Never matches on the process name alone.
function Test-SamePowerShellHost([string]$LiveExePath, [string]$RecordedExePath) {
    if ([string]::IsNullOrWhiteSpace($LiveExePath)) { return $false }
    if ([string]::IsNullOrWhiteSpace($RecordedExePath)) { return $false }
    $live = $null; $rec = $null
    try {
        $live = ([System.IO.Path]::GetFullPath($LiveExePath)).TrimEnd('\')
        $rec = ([System.IO.Path]::GetFullPath($RecordedExePath)).TrimEnd('\')
    } catch { return $false }
    if (-not $live.Equals($rec, [System.StringComparison]::OrdinalIgnoreCase)) { return $false }
    $leaf = [System.IO.Path]::GetFileName($live).ToLowerInvariant()
    return (@('powershell.exe', 'pwsh.exe') -contains $leaf)
}

# Decides what stop-soak must do with a recorded sampler PID, from facts the
# caller gathered from the live system. Pure: performs no process I/O.
#
# Returned Action:
#   'report-only'  - no PID file: nothing to stop; just generate the report
#   'stop'         - CONFIRMED this session's live sampler: stop it, confirm
#                    death, then remove the PID file, then report
#   'remove-stale' - the recorded sampler is genuinely not running (PID gone,
#                    or the PID now belongs to an unrelated/foreign process):
#                    never kill anything, remove the stale PID file, report
#   'abort'        - a process is alive but could NOT be positively confirmed
#                    as this session's sampler: never kill, KEEP the PID file,
#                    do NOT report (fail closed, surface an explicit error)
function Get-SoakSamplerDisposition {
    param(
        [bool]$PidFilePresent = $true,
        [bool]$ProcessAlive,
        [string]$RecordedExePath,
        [string]$LiveExePath,
        [string]$CommandLine,
        [string]$ExpectedScript = 'soak-check.ps1',
        [Parameter(Mandatory = $true)][string]$SessionPath,
        [string]$RecordedStartUtc,
        [string]$LiveStartUtc
    )

    if (-not $PidFilePresent) {
        return [pscustomobject]@{ Action = 'report-only'; Identity = 'absent'
            Reason = 'no sampler PID file (sampler not running or already cleaned up)'
        }
    }
    if (-not $ProcessAlive) {
        return [pscustomobject]@{ Action = 'remove-stale'; Identity = 'gone'
            Reason = 'no live process holds the recorded PID'
        }
    }

    # Alive: it must be positively identified before ANY action is taken.
    # A live process whose command line cannot be read can be proven neither
    # ours nor foreign -> fail closed rather than delete a live sampler's PID.
    if ([string]::IsNullOrWhiteSpace($CommandLine)) {
        return [pscustomobject]@{ Action = 'abort'; Identity = 'indeterminate'
            Reason = 'process is alive but its command line is inaccessible; cannot prove ownership'
        }
    }

    $scriptMatch = Test-CommandLineReferencesLeaf -CommandLine $CommandLine -Leaf $ExpectedScript
    $sessionMatch = Test-CommandLineReferencesPath -CommandLine $CommandLine -Path $SessionPath

    if (-not ($scriptMatch -and $sessionMatch)) {
        # Readable command line that does NOT belong to this session's
        # sampler => the recorded PID was reused by an unrelated process (or
        # belongs to a different session's sampler). Our sampler is not
        # running; never kill the other process, just drop our stale pointer.
        return [pscustomobject]@{ Action = 'remove-stale'; Identity = 'foreign'
            Reason = 'recorded PID is held by an unrelated process (not this session''s sampler)'
        }
    }

    # The command line positively identifies our sampler AND this session.
    # PID-reuse guard: a genuine sampler cannot have started before we
    # recorded its launch (allow a few seconds of slack for the pid-file
    # write). A live process that predates the launch cannot be ours.
    if ((-not [string]::IsNullOrWhiteSpace($RecordedStartUtc)) -and
        (-not [string]::IsNullOrWhiteSpace($LiveStartUtc))) {
        try {
            $rec = ConvertFrom-IsoUtc $RecordedStartUtc
            $liv = ConvertFrom-IsoUtc $LiveStartUtc
            if ($liv -lt $rec.AddSeconds(-5)) {
                return [pscustomobject]@{ Action = 'abort'; Identity = 'indeterminate'
                    Reason = 'live process started before the recorded sampler launch; possible PID reuse'
                }
            }
        } catch { }
    }

    # Validate the executable too: never stop on a command line match alone.
    if (Test-SamePowerShellHost -LiveExePath $LiveExePath -RecordedExePath $RecordedExePath) {
        return [pscustomobject]@{ Action = 'stop'; Identity = 'confirmed'
            Reason = 'live PowerShell host runs the sampler script for this session'
        }
    }

    # Command line says it is ours, but the executable could not be verified
    # as the recorded PowerShell host. Refuse to kill (exe unverified) and
    # refuse to purge (it looks alive and ours). Fail closed.
    return [pscustomobject]@{ Action = 'abort'; Identity = 'indeterminate'
        Reason = 'command line matches this session but the executable is not the recorded PowerShell host'
    }
}

# Stops a process by PID and returns $true ONLY after confirming it is gone
# within the timeout; $false if it is still alive afterwards. The stopper,
# alive-probe, and sleeper are injectable so the stop/confirm decision is
# unit-testable without spawning or killing real processes.
function Invoke-SamplerStop {
    param(
        [Parameter(Mandatory = $true)][int]$ProcessId,
        [int]$TimeoutSeconds = 15,
        [scriptblock]$Stopper = { param($p) Stop-Process -Id $p -Force -Confirm:$false -ErrorAction SilentlyContinue },
        [scriptblock]$AliveProbe = {
            param($p)
            $x = $null
            try { $x = Get-Process -Id $p -ErrorAction Stop } catch { $x = $null }
            return ($null -ne $x)
        },
        [scriptblock]$Sleeper = { Start-Sleep -Milliseconds 250 }
    )
    & $Stopper $ProcessId | Out-Null
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (-not (& $AliveProbe $ProcessId)) { return $true }
        & $Sleeper | Out-Null
    }
    return (-not (& $AliveProbe $ProcessId))
}

# ── Port helpers ────────────────────────────────────────────────────────
function Get-PortListener([int]$Port) {
    Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue
}

function Get-NodePorts([hashtable]$Node) {
    @($Node.Rpc, $Node.Rest, $Node.P2p)
}
