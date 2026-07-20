# Soak sampler for the v0.3 devnet. Collects one observation of every
# node (process, RPC/REST, chain, resources, logs) plus session-wide
# metrics, appends them to samples.csv, and records noteworthy
# transitions in events.log.
#
# One-shot by default; -Loop keeps sampling at the session's interval
# until the planned duration ends or the sampler is stopped by
# stop-soak.ps1. Individual node or RPC failures are RECORDED, never
# fatal; the sampler terminates only on invalid session metadata, an
# unsafe path, planned-duration end, or an explicit stop.
#
# This tool is read-only towards the devnet: it never mutates chain
# data and never starts, stops, or restarts node processes.
#
# Usage: .\soak-check.ps1 -SessionPath <session dir> [-Loop]

param(
    [Parameter(Mandatory = $true)][string]$SessionPath,
    [switch]$Loop
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

# --- Session validation (fatal on failure, by design) ------------------
$resolvedRoot = Assert-SafeDevnetRoot
$SessionPath = Assert-PathInsideRoot -Path $SessionPath -ResolvedRoot $resolvedRoot
$sessionFile = Join-Path $SessionPath 'session.json'
if (-not (Test-Path $sessionFile)) {
    throw "No session.json at $SessionPath. Create sessions with start-soak.ps1."
}
$session = Get-Content $sessionFile -Raw | ConvertFrom-Json
$csvPath = Join-Path $SessionPath 'samples.csv'
$statePath = Join-Path $SessionPath 'state.json'
$eventsPath = Join-Path $SessionPath 'events.log'

$intervalMinutes = [double]$session.intervalMinutes
if ($intervalMinutes -lt 1) { throw "Session interval $intervalMinutes < 1 minute minimum." }

# The CSV schema, invariant numeric formatting, row builder, and canonical
# header are defined once in devnet-config.ps1 ($SoakSchema, New-SoakRow,
# $SoakCanonicalHeader) and shared with soak-report.

function Write-SoakEvent([string]$Message) {
    $line = "$((Get-Date).ToUniversalTime().ToString('o')) $Message"
    Add-Content -Path $eventsPath -Value $line -Encoding utf8
}

# Startup guard: if samples.csv already exists, its header must exactly
# match the schema and every record must parse into the 29-column schema.
# Refuses to resume a malformed session (e.g. one written before this CSV
# fix); never appends to a corrupted CSV.
function Assert-ExistingCsvValid {
    if (-not (Test-Path $csvPath)) { return }
    $lines = @(Get-Content $csvPath)
    if ($lines.Count -eq 0) { return }
    if ($lines[0] -ne $SoakCanonicalHeader) {
        throw "samples.csv header does not match the expected schema; refusing to resume a malformed session ($csvPath). Start a NEW soak session."
    }
    $parsed = @(Import-Csv $csvPath)
    foreach ($r in $parsed) {
        $names = @($r.PSObject.Properties.Name)
        if ($names.Count -ne $SoakSchema.Count) {
            throw "samples.csv has a record with $($names.Count) columns (expected $($SoakSchema.Count)); refusing to resume a malformed session."
        }
    }
}

# Counts warning/error pattern matches in a log file (0 when absent).
function Get-LogMatchCount([string]$Path, [string]$Pattern) {
    if (-not ($Path) -or -not (Test-Path $Path)) { return 0 }
    $m = Select-String -Path $Path -Pattern $Pattern -ErrorAction SilentlyContinue
    return @($m).Count
}

function Get-FileSizeKb([string]$Path) {
    if (-not ($Path) -or -not (Test-Path $Path)) { return 0 }
    [math]::Round((Get-Item $Path).Length / 1KB, 1)
}

# Fetches the full block JSON at a height (compact string) or $null.
function Get-BlockJson([int]$Port, [long]$Height) {
    try {
        $body = @{ jsonrpc = '2.0'; method = 'get_block_by_height'
            params = @{ height = $Height }; id = 1 } | ConvertTo-Json -Compress
        $resp = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/rpc" -Method Post `
            -ContentType 'application/json' -Body $body -TimeoutSec 5
        if (($resp.PSObject.Properties.Name -contains 'error') -and ($null -ne $resp.error)) { return $null }
        return ($resp.result | ConvertTo-Json -Depth 20 -Compress)
    } catch { return $null }
}

# --- One sample --------------------------------------------------------
function Invoke-SoakSample {
    $now = (Get-Date).ToUniversalTime()
    $ts = $now.ToString('o')

    # Previous-sample state (external, survives sampler restarts).
    $state = $null
    if (Test-Path $statePath) {
        try { $state = Get-Content $statePath -Raw | ConvertFrom-Json } catch { $state = $null }
    }

    $nodeRows = @()
    $newState = @{}
    $reachableCount = 0
    $heights = @()
    $tips = @{}
    $producerDelta = $null
    $totalData = 0.0
    $totalWarn = 0
    $totalErr = 0

    foreach ($node in $DevnetNodes) {
        $name = $node.Name
        $prev = $null
        if (($null -ne $state) -and ($state.PSObject.Properties.Name -contains $name)) {
            $prev = $state.$name
        }

        # Process identity: recorded PID must still run the deployed binary.
        $pidInfo = Read-NodePidFile $node
        $proc = Get-RunningNodeProcess $node
        $alive = ($null -ne $proc)
        $exeValid = $alive
        $procId = if ($null -ne $pidInfo) { $pidInfo.pid } else { '' }

        # Restart detection: PID metadata changed since the last sample.
        if (($null -ne $prev) -and ($prev.pid -ne '') -and ("$procId" -ne "$($prev.pid)")) {
            Write-SoakEvent "RESTART detected on ${name}: pid $($prev.pid) -> $procId"
        }

        $rssMb = ''
        $cpuTotal = ''
        $cpuDelta = ''
        if ($alive) {
            $rssMb = [math]::Round($proc.WorkingSet64 / 1MB, 1)
            $cpuTotal = [math]::Round($proc.TotalProcessorTime.TotalSeconds, 1)
            if (($null -ne $prev) -and ("$($prev.cpuTotalSec)" -ne '') -and ("$procId" -eq "$($prev.pid)")) {
                $cpuDelta = [math]::Round($cpuTotal - [double]$prev.cpuTotalSec, 1)
            }
        }

        $rpcOk = Test-DevnetRpc $node.Rpc
        $restOk = $false
        try {
            $null = Invoke-RestMethod -Uri "http://127.0.0.1:$($node.Rest)/validators" -TimeoutSec 5
            $restOk = $true
        } catch { $restOk = $false }

        $height = ''
        $tip = ''
        $heightDelta = ''
        if ($rpcOk) {
            $reachableCount++
            try {
                $height = Get-DevnetHeight $node.Rpc
                $tip = Get-DevnetTipHash $node.Rpc
                $heights += [long]$height
                $tips[$name] = $tip
                if (($null -ne $prev) -and ("$($prev.height)" -ne '')) {
                    $heightDelta = [long]$height - [long]$prev.height
                }
                if ($node.Producer -and ($null -ne $prev) -and ("$($prev.height)" -ne '')) {
                    $producerDelta = [long]$height - [long]$prev.height
                }
            } catch {
                $rpcOk = $false
                $reachableCount--
            }
        }
        if (($null -ne $prev) -and (("$($prev.rpcOk)" -eq 'True') -ne $rpcOk)) {
            Write-SoakEvent "RPC reachability changed on ${name}: $($prev.rpcOk) -> $rpcOk"
        }

        $dataMb = Get-DirectorySizeMb (Get-NodeDataDir $node)
        $totalData += $dataMb
        $dataGrowth = ''
        if (($null -ne $prev) -and ("$($prev.dataMb)" -ne '')) {
            $dataGrowth = [math]::Round($dataMb - [double]$prev.dataMb, 2)
        }

        # Logs of the CURRENT run (paths from PID metadata).
        $outLog = if ($null -ne $pidInfo) { $pidInfo.outLog } else { $null }
        $errLog = if ($null -ne $pidInfo) { $pidInfo.errLog } else { $null }
        $outKb = Get-FileSizeKb $outLog
        $errKb = Get-FileSizeKb $errLog
        $warnNow = (Get-LogMatchCount $outLog '\bWARN\b|warning') + (Get-LogMatchCount $errLog '\bWARN\b|warning')
        $errNow = (Get-LogMatchCount $outLog '\bERROR\b') + (Get-LogMatchCount $errLog '\bERROR\b')
        $newWarn = $warnNow
        $newErr = $errNow
        if (($null -ne $prev) -and ($prev.outLog -eq $outLog)) {
            $newWarn = [math]::Max(0, $warnNow - [int]$prev.warnCount)
            $newErr = [math]::Max(0, $errNow - [int]$prev.errCount)
        }
        $totalWarn += $newWarn
        $totalErr += $newErr

        $newState[$name] = @{
            pid = "$procId"; height = "$height"; cpuTotalSec = "$cpuTotal"
            dataMb = $dataMb; rpcOk = $rpcOk
            outLog = $outLog; warnCount = $warnNow; errCount = $errNow
        }

        $nodeRows += New-SoakRow @{
            timestampUtc = $ts; scope = 'node'; node = $name; role = $node.Role
            pid = $procId; processAlive = $alive; exeValid = $exeValid
            rpcReachable = $rpcOk; restReachable = $restOk
            height = $height; tipHash = $tip; heightDelta = $heightDelta
            rssMb = $rssMb; cpuTotalSec = $cpuTotal; cpuDeltaSec = $cpuDelta
            dataSizeMb = $dataMb; dataGrowthMb = $dataGrowth
            outLogSizeKb = $outKb; errLogSizeKb = $errKb
            newWarnings = $newWarn; newErrors = $newErr
        }
    }

    # --- Session-wide row ---
    $allReachable = ($reachableCount -eq $DevnetNodes.Count)
    $spread = ''
    $tipsConsistent = $true
    if ($heights.Count -gt 0) {
        $maxH = ($heights | Measure-Object -Maximum).Maximum
        $minH = ($heights | Measure-Object -Minimum).Minimum
        $spread = [long]$maxH - [long]$minH
        if ($allReachable) {
            if ($spread -eq 0) {
                $tipsConsistent = (@($tips.Values | Sort-Object -Unique).Count -eq 1)
            } else {
                # Ancestry check at the common minimum height: the block
                # JSON must be identical on every node.
                $blocks = @()
                foreach ($node in $DevnetNodes) {
                    $blocks += Get-BlockJson $node.Rpc ([long]$minH)
                }
                $tipsConsistent = (@($blocks | Sort-Object -Unique).Count -eq 1) -and ($null -ne $blocks[0])
            }
        }
    }

    $classification = Get-ConvergenceClassification -AllReachable $allReachable `
        -HeightSpread $(if ("$spread" -eq '') { 0 } else { [long]$spread }) `
        -TipsConsistent $tipsConsistent -ProducerDelta $producerDelta `
        -SkewAllowance ([int]$session.thresholds.ConvergenceSkewBlocks)

    $prevClass = $null
    if (($null -ne $state) -and ($state.PSObject.Properties.Name -contains '_session')) {
        $prevClass = $state._session.classification
    }
    if (($null -ne $prevClass) -and ($prevClass -ne $classification)) {
        Write-SoakEvent "CONVERGENCE state changed: $prevClass -> $classification"
    }

    $uptimeSec = [math]::Round(($now - [datetime]$session.startedAtUtc).TotalSeconds)
    $sessionRow = New-SoakRow @{
        timestampUtc = $ts; scope = 'session'
        allReachable = $allReachable; heightSpread = $spread
        convergence = $classification
        producerDelta = $(if ($null -ne $producerDelta) { $producerDelta } else { '' })
        totalDataMb = [math]::Round($totalData, 2)
        totalNewWarnings = $totalWarn; totalNewErrors = $totalErr
        sessionUptimeSec = $uptimeSec
    }

    # Serialize the whole sample (3 node rows + 1 session row) through a
    # real CSV serializer: correct quoting/escaping and invariant numerics.
    $sampleRows = @($nodeRows) + @($sessionRow)
    $csvLines = @($sampleRows | ConvertTo-Csv -NoTypeInformation)
    $dataLines = @($csvLines | Select-Object -Skip 1)

    # Strict post-serialization validation: parse the generated lines back
    # and require exactly 29 named properties in order, and 3 node + 1
    # session rows. Refuse the append (record a fatal event) on mismatch.
    $parsed = @($csvLines | ConvertFrom-Csv)
    $schemaOk = ($parsed.Count -eq 4)
    if ($schemaOk) {
        foreach ($p in $parsed) {
            $names = @($p.PSObject.Properties.Name)
            if ($names.Count -ne $SoakSchema.Count) { $schemaOk = $false; break }
            for ($c = 0; $c -lt $SoakSchema.Count; $c++) {
                if ($names[$c] -ne $SoakSchema[$c]) { $schemaOk = $false; break }
            }
            if (-not $schemaOk) { break }
        }
    }
    $nodeCount = @($parsed | Where-Object { $_.scope -eq 'node' }).Count
    $sessCount = @($parsed | Where-Object { $_.scope -eq 'session' }).Count
    if ((-not $schemaOk) -or ($nodeCount -ne 3) -or ($sessCount -ne 1)) {
        Write-SoakEvent "FATAL: serialized sample failed schema validation (rows=$($parsed.Count), node=$nodeCount, session=$sessCount); refusing to append"
        throw 'Serialized sample failed 29-column / 3-node + 1-session validation; refusing to corrupt samples.csv.'
    }

    # Atomic-enough append: header once, then the validated data lines.
    if (-not (Test-Path $csvPath)) {
        Set-Content -Path $csvPath -Value $SoakCanonicalHeader -Encoding utf8
    }
    Add-Content -Path $csvPath -Value ($dataLines -join "`r`n") -Encoding utf8

    $newState['_session'] = @{ classification = $classification; sampledAtUtc = $ts }
    $newState | ConvertTo-Json -Depth 4 | Out-File -FilePath $statePath -Encoding utf8

    Write-Host "[$ts] $classification (spread=$spread, reachable=$reachableCount/$($DevnetNodes.Count))"
}

# --- Main --------------------------------------------------------------
# Refuse to resume a malformed session before writing anything.
Assert-ExistingCsvValid

Write-SoakEvent "sampler started (pid $PID, loop=$([bool]$Loop))"
try {
    Invoke-SoakSample
    if ($Loop) {
        while ($true) {
            $plannedEnd = $null
            if (("$($session.plannedHours)" -ne '') -and ([double]$session.plannedHours -gt 0)) {
                $plannedEnd = ([datetime]$session.startedAtUtc).AddHours([double]$session.plannedHours)
            }
            if (($null -ne $plannedEnd) -and ((Get-Date).ToUniversalTime() -ge $plannedEnd)) {
                Write-SoakEvent 'planned duration reached; sampler exiting'
                break
            }
            Start-Sleep -Seconds ([int]($intervalMinutes * 60))
            try {
                Invoke-SoakSample
            } catch {
                # Individual sample failures are recorded, not fatal.
                Write-SoakEvent "SAMPLE ERROR (continuing): $_"
            }
        }
    }
} finally {
    Write-SoakEvent "sampler stopped (pid $PID)"
}
exit 0
