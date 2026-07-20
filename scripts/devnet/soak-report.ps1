# Generates the final soak report (JSON + human-readable TXT) for a
# session from its samples.csv, session.json, and events.log. Can be
# run at any time; stop-soak.ps1 runs it automatically.
#
# Result criteria (deterministic, evaluated against the thresholds
# snapshotted into session.json at start):
#   FAIL              - any divergent sample; producer stalled streak or
#                       node RPC outage streak beyond the fail minutes;
#                       max RSS at/above the fail limit; missing samples
#                       above the fail percentage; log errors in one
#                       interval at/above the fail count
#   PASS WITH WARNINGS- no FAIL condition, but any stalled/unreachable
#                       sample, warn-level outage streak, warn-level RSS,
#                       data-growth anomaly, any log errors, warn-level
#                       interval warnings, warn-level missing samples,
#                       detected node restarts, or sampler interruptions
#   PASS              - none of the above
#
# Exit code: 0 for PASS or PASS WITH WARNINGS, 1 for FAIL.
#
# Usage: .\soak-report.ps1 -SessionPath <session dir>

param(
    [Parameter(Mandatory = $true)][string]$SessionPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

$resolvedRoot = Assert-SafeDevnetRoot
$SessionPath = Assert-PathInsideRoot -Path $SessionPath -ResolvedRoot $resolvedRoot
$session = Get-Content (Join-Path $SessionPath 'session.json') -Raw | ConvertFrom-Json
$csvPath = Join-Path $SessionPath 'samples.csv'
if (-not (Test-Path $csvPath)) { throw "No samples.csv in $SessionPath; nothing to report." }

# --- Schema/data validation (fail closed on malformed CSV) -------------
# A malformed CSV (e.g. the locale decimal-comma corruption) must NEVER
# produce PASS or PASS WITH WARNINGS. Import-Csv silently normalizes
# shifted rows to 29 columns with garbage values, so validation checks
# the exact header, per-record column count, and semantic sanity of the
# convergence column. On failure it writes a FAIL report and exits 1.
function Write-InvalidCsvReport([string]$Reason) {
    $jsonPath = Join-Path $SessionPath 'final-report.json'
    $txtPath = Join-Path $SessionPath 'final-report.txt'
    [ordered]@{
        sessionId   = $session.sessionId
        result      = 'FAIL'
        failReasons = @("invalid CSV schema/data: $Reason")
        csvPath     = $csvPath
    } | ConvertTo-Json -Depth 4 | Out-File -FilePath $jsonPath -Encoding utf8
    @(
        "=== Soak Report: $($session.sessionId) ===",
        'RESULT: FAIL',
        "  FAIL: invalid CSV schema/data: $Reason",
        '',
        'This session produced malformed sample data and cannot be summarized.',
        'It must be treated as invalid; start a new soak session.'
    ) -join "`r`n" | Out-File -FilePath $txtPath -Encoding utf8
    Write-Host "RESULT: FAIL - invalid CSV schema/data: $Reason"
    Write-Host "JSON: $jsonPath"
}

$csvLines = @(Get-Content $csvPath)
if ($csvLines.Count -eq 0) { Write-InvalidCsvReport 'samples.csv is empty'; exit 1 }
if ($csvLines[0] -ne $SoakCanonicalHeader) {
    Write-InvalidCsvReport 'header does not match the expected 29-column schema'
    exit 1
}
$rows = @(Import-Csv $csvPath)
# Every record must expose exactly the 29 schema columns, in order.
foreach ($r in $rows) {
    $names = @($r.PSObject.Properties.Name)
    if ($names.Count -ne $SoakSchema.Count) {
        Write-InvalidCsvReport "a record has $($names.Count) columns (expected $($SoakSchema.Count))"
        exit 1
    }
}
$sessionRows = @($rows | Where-Object { $_.scope -eq 'session' })
$nodeRows = @($rows | Where-Object { $_.scope -eq 'node' })
if ($sessionRows.Count -eq 0) { Write-InvalidCsvReport 'no session rows'; exit 1 }
# Semantic check: every session row's convergence must be a known state.
# This catches shifted data even if the column count happens to line up.
foreach ($sr in $sessionRows) {
    if ($SoakConvergenceStates -notcontains $sr.convergence) {
        Write-InvalidCsvReport "session row has invalid convergence value '$($sr.convergence)' (shifted data)"
        exit 1
    }
}
# Every scope must be exactly 'node' or 'session' (catches column shift).
foreach ($r in $rows) {
    if (($r.scope -ne 'node') -and ($r.scope -ne 'session')) {
        Write-InvalidCsvReport "row has invalid scope '$($r.scope)' (shifted data)"
        exit 1
    }
}

# Wrap the whole if-expression in @() so a single-line events.log does
# not unwrap to a scalar string (which lacks .Count under StrictMode).
$eventsPath = Join-Path $SessionPath 'events.log'
$events = @(if (Test-Path $eventsPath) { Get-Content $eventsPath } else { @() })

$th = $session.thresholds
$intervalMin = [double]$session.intervalMinutes

# --- Time and sample accounting ---------------------------------------
$startUtc = [datetime]$session.startedAtUtc
$firstSample = [datetime]$sessionRows[0].timestampUtc
$lastSample = [datetime]$sessionRows[-1].timestampUtc
$durationMin = [math]::Max(0, ($lastSample - $startUtc).TotalMinutes)
$expectedSamples = [math]::Floor($durationMin / $intervalMin) + 1
$actualSamples = $sessionRows.Count
$missing = [math]::Max(0, $expectedSamples - $actualSamples)
$missingPct = if ($expectedSamples -gt 0) { [math]::Round(100.0 * $missing / $expectedSamples, 1) } else { 0 }

# Gaps: consecutive session samples farther apart than 1.5x interval.
$gaps = @()
for ($i = 1; $i -lt $sessionRows.Count; $i++) {
    $dt = ([datetime]$sessionRows[$i].timestampUtc - [datetime]$sessionRows[$i - 1].timestampUtc).TotalMinutes
    if ($dt -gt (1.5 * $intervalMin)) {
        $gaps += "gap of $([math]::Round($dt,1)) min after $($sessionRows[$i-1].timestampUtc)"
    }
}

# --- Convergence accounting -------------------------------------------
$classCounts = @{}
foreach ($r in $sessionRows) {
    $c = $r.convergence
    if (-not $classCounts.ContainsKey($c)) { $classCounts[$c] = 0 }
    $classCounts[$c]++
}
$classPct = @{}
foreach ($k in $classCounts.Keys) {
    $classPct[$k] = [math]::Round(100.0 * $classCounts[$k] / $actualSamples, 1)
}
function Get-MaxStreak([string[]]$Values, [string]$Target) {
    $max = 0; $cur = 0
    foreach ($v in $Values) {
        if ($v -eq $Target) { $cur++; if ($cur -gt $max) { $max = $cur } } else { $cur = 0 }
    }
    return $max
}
$classSeq = @($sessionRows | ForEach-Object { $_.convergence })
$stalledStreakMin = (Get-MaxStreak $classSeq 'stalled') * $intervalMin
$unreachableStreakMin = (Get-MaxStreak $classSeq 'unreachable') * $intervalMin
$divergentCount = if ($classCounts.ContainsKey('divergent')) { $classCounts['divergent'] } else { 0 }

# --- Per-node accounting ----------------------------------------------
$perNode = @{}
$maxRssOverall = 0.0
$maxOutageMinOverall = 0.0
$restartsDetected = 0
foreach ($node in $DevnetNodes) {
    $nr = @($nodeRows | Where-Object { $_.node -eq $node.Name })
    if ($nr.Count -eq 0) { continue }
    $aliveN = @($nr | Where-Object { $_.processAlive -eq 'True' }).Count
    $rpcN = @($nr | Where-Object { $_.rpcReachable -eq 'True' }).Count
    $restN = @($nr | Where-Object { $_.restReachable -eq 'True' }).Count
    $rssVals = @($nr | Where-Object { $_.rssMb -ne '' } | ForEach-Object { [double]$_.rssMb })
    $cpuDeltas = @($nr | Where-Object { $_.cpuDeltaSec -ne '' } | ForEach-Object { [double]$_.cpuDeltaSec })
    $rpcSeq = @($nr | ForEach-Object { "$($_.rpcReachable)" })
    $outageMin = (Get-MaxStreak $rpcSeq 'False') * $intervalMin
    if ($outageMin -gt $maxOutageMinOverall) { $maxOutageMinOverall = $outageMin }

    # Restarts: PID changes between consecutive samples with known PIDs.
    $nodeRestarts = 0
    for ($i = 1; $i -lt $nr.Count; $i++) {
        if (($nr[$i].pid -ne '') -and ($nr[$i - 1].pid -ne '') -and ($nr[$i].pid -ne $nr[$i - 1].pid)) {
            $nodeRestarts++
        }
    }
    $restartsDetected += $nodeRestarts

    $maxRss = if ($rssVals.Count -gt 0) { ($rssVals | Measure-Object -Maximum).Maximum } else { 0 }
    if ($maxRss -gt $maxRssOverall) { $maxRssOverall = $maxRss }
    $perNode[$node.Name] = [ordered]@{
        samples          = $nr.Count
        uptimePercent    = [math]::Round(100.0 * $aliveN / $nr.Count, 1)
        rpcPercent       = [math]::Round(100.0 * $rpcN / $nr.Count, 1)
        restPercent      = [math]::Round(100.0 * $restN / $nr.Count, 1)
        maxRssMb         = $maxRss
        avgRssMb         = if ($rssVals.Count -gt 0) { [math]::Round(($rssVals | Measure-Object -Average).Average, 1) } else { 0 }
        totalCpuDeltaSec = [math]::Round(($cpuDeltas | Measure-Object -Sum).Sum, 1)
        maxRpcOutageMin  = $outageMin
        restartsDetected = $nodeRestarts
    }
}

# --- Producer progress -------------------------------------------------
$producerName = ($DevnetNodes | Where-Object { $_.Producer } | Select-Object -First 1).Name
$prodRows = @($nodeRows | Where-Object { ($_.node -eq $producerName) -and ($_.height -ne '') })
$firstHeight = if ($prodRows.Count -gt 0) { [long]$prodRows[0].height } else { 0 }
$finalHeight = if ($prodRows.Count -gt 0) { [long]$prodRows[-1].height } else { 0 }
$blocksProduced = $finalHeight - $firstHeight

# --- Data size and growth ----------------------------------------------
$dataVals = @($sessionRows | Where-Object { $_.totalDataMb -ne '' } | ForEach-Object { [double]$_.totalDataMb })
$initialData = if ($dataVals.Count -gt 0) { $dataVals[0] } else { 0 }
$finalData = if ($dataVals.Count -gt 0) { $dataVals[-1] } else { 0 }
$maxData = if ($dataVals.Count -gt 0) { ($dataVals | Measure-Object -Maximum).Maximum } else { 0 }
$totalGrowth = [math]::Round($finalData - $initialData, 2)
$peakGrowth = 0.0
for ($i = 1; $i -lt $dataVals.Count; $i++) {
    $d = $dataVals[$i] - $dataVals[$i - 1]
    if ($d -gt $peakGrowth) { $peakGrowth = [math]::Round($d, 2) }
}
$durationHours = [math]::Max(0.0001, $durationMin / 60.0)
$growthper_hour = [math]::Round($totalGrowth / $durationHours, 2)

# --- Log warnings/errors -----------------------------------------------
$warnTotal = ($sessionRows | ForEach-Object { [int]$_.totalNewWarnings } | Measure-Object -Sum).Sum
$errTotal = ($sessionRows | ForEach-Object { [int]$_.totalNewErrors } | Measure-Object -Sum).Sum
$peakWarn = ($sessionRows | ForEach-Object { [int]$_.totalNewWarnings } | Measure-Object -Maximum).Maximum
$peakErr = ($sessionRows | ForEach-Object { [int]$_.totalNewErrors } | Measure-Object -Maximum).Maximum

# --- Result ------------------------------------------------------------
$failReasons = @()
$warnReasons = @()
if ($divergentCount -gt 0) { $failReasons += "divergent samples: $divergentCount" }
if ($stalledStreakMin -ge [double]$th.StalledFailMinutes) { $failReasons += "producer stalled for $stalledStreakMin min (>= $($th.StalledFailMinutes))" }
if ($maxOutageMinOverall -ge [double]$th.RpcOutageFailMinutes) { $failReasons += "RPC outage streak $maxOutageMinOverall min (>= $($th.RpcOutageFailMinutes))" }
if ($maxRssOverall -ge [double]$th.RssFailMb) { $failReasons += "max RSS $maxRssOverall MB (>= $($th.RssFailMb))" }
if ($missingPct -gt [double]$th.MissingSampleFailPercent) { $failReasons += "missing samples $missingPct% (> $($th.MissingSampleFailPercent)%)" }
if ($peakErr -ge [int]$th.ErrorsPerIntervalFail) { $failReasons += "peak errors per interval $peakErr (>= $($th.ErrorsPerIntervalFail))" }

if (($classCounts.ContainsKey('stalled')) -and ($classCounts['stalled'] -gt 0)) { $warnReasons += "stalled samples: $($classCounts['stalled'])" }
if (($classCounts.ContainsKey('unreachable')) -and ($classCounts['unreachable'] -gt 0)) { $warnReasons += "unreachable samples: $($classCounts['unreachable'])" }
if (($maxOutageMinOverall -ge [double]$th.RpcOutageWarnMinutes) -and ($maxOutageMinOverall -lt [double]$th.RpcOutageFailMinutes)) { $warnReasons += "RPC outage streak $maxOutageMinOverall min" }
if (($maxRssOverall -ge [double]$th.RssWarnMb) -and ($maxRssOverall -lt [double]$th.RssFailMb)) { $warnReasons += "max RSS $maxRssOverall MB" }
if ($growthper_hour -ge [double]$th.DataGrowthWarnMbPerHour) { $warnReasons += "data growth $growthper_hour MB/h" }
if ($errTotal -gt 0) { $warnReasons += "log errors: $errTotal" }
if ($peakWarn -ge [int]$th.WarningsPerIntervalWarn) { $warnReasons += "peak warnings per interval $peakWarn" }
if (($missingPct -gt [double]$th.MissingSampleWarnPercent) -and ($missingPct -le [double]$th.MissingSampleFailPercent)) { $warnReasons += "missing samples $missingPct%" }
if ($restartsDetected -gt 0) { $warnReasons += "node restarts detected: $restartsDetected" }
if ($gaps.Count -gt 0) { $warnReasons += "sampler interruptions: $($gaps.Count)" }

$result = if ($failReasons.Count -gt 0) { 'FAIL' }
elseif ($warnReasons.Count -gt 0) { 'PASS WITH WARNINGS' }
else { 'PASS' }

# --- Emit --------------------------------------------------------------
$report = [ordered]@{
    sessionId          = $session.sessionId
    result             = $result
    failReasons        = $failReasons
    warnReasons        = $warnReasons
    startedAtUtc       = $session.startedAtUtc
    firstSampleUtc     = $sessionRows[0].timestampUtc
    lastSampleUtc      = $sessionRows[-1].timestampUtc
    durationMinutes    = [math]::Round($durationMin, 1)
    intervalMinutes    = $intervalMin
    samplesActual      = $actualSamples
    samplesExpected    = $expectedSamples
    missingSamples     = $missing
    missingPercent     = $missingPct
    sampleGaps         = $gaps
    protocolTag        = $session.protocolTag
    protocolCommit     = $session.protocolCommit
    toolingCommit      = $session.toolingCommit
    nodeBinarySha256   = $session.nodeBinarySha256
    receiptToolSha256  = $session.receiptToolSha256
    convergencePercent = $classPct
    divergentSamples   = $divergentCount
    stalledStreakMin   = $stalledStreakMin
    unreachableStreakMin = $unreachableStreakMin
    firstProducerHeight = $firstHeight
    finalProducerHeight = $finalHeight
    blocksProduced     = $blocksProduced
    dataInitialMb      = $initialData
    dataFinalMb        = $finalData
    dataMaxMb          = $maxData
    dataTotalGrowthMb  = $totalGrowth
    dataPeakIntervalGrowthMb = $peakGrowth
    dataGrowthMbPerHour = $growthper_hour
    warningsTotal      = $warnTotal
    errorsTotal        = $errTotal
    peakWarningsPerInterval = $peakWarn
    peakErrorsPerInterval = $peakErr
    restartsDetected   = $restartsDetected
    perNode            = $perNode
    thresholds         = $th
    unavailableMetrics = @(
        'peer count (no peer-count RPC exists)',
        'direct receipt-index bytes (no get_receipt RPC yet)',
        'Prometheus/telemetry counters (none exported by the node)'
    )
    eventCount         = $events.Count
}
$jsonPath = Join-Path $SessionPath 'final-report.json'
$txtPath = Join-Path $SessionPath 'final-report.txt'
$report | ConvertTo-Json -Depth 6 | Out-File -FilePath $jsonPath -Encoding utf8

$txt = @()
$txt += "=== Soak Report: $($session.sessionId) ==="
$txt += "RESULT: $result"
foreach ($r in $failReasons) { $txt += "  FAIL: $r" }
foreach ($r in $warnReasons) { $txt += "  WARN: $r" }
$txt += ''
$txt += "Window:    $($session.startedAtUtc) -> $($sessionRows[-1].timestampUtc) ($([math]::Round($durationMin,1)) min)"
$txt += "Samples:   $actualSamples of ~$expectedSamples expected (missing $missingPct%), interval $intervalMin min"
foreach ($g in $gaps) { $txt += "  $g" }
$txt += "Protocol:  $($session.protocolTag) @ $($session.protocolCommit)"
$txt += "Tooling:   $($session.toolingCommit)"
$txt += "Producer:  height $firstHeight -> $finalHeight ($blocksProduced blocks)"
$convParts = @()
foreach ($k in ($classPct.Keys | Sort-Object)) { $convParts += "$k $($classPct[$k])%" }
$txt += "Converge:  $($convParts -join ', ')"
$txt += "Data:      $initialData -> $finalData MB (max $maxData, growth $totalGrowth MB total, $growthper_hour MB/h, peak interval $peakGrowth MB)"
$txt += "Logs:      $warnTotal warnings / $errTotal errors (peak per interval: $peakWarn / $peakErr)"
$txt += "Restarts:  $restartsDetected detected"
$txt += ''
foreach ($name in $perNode.Keys) {
    $n = $perNode[$name]
    $txt += "$name : uptime $($n.uptimePercent)%, rpc $($n.rpcPercent)%, rest $($n.restPercent)%, rss max/avg $($n.maxRssMb)/$($n.avgRssMb) MB, cpu +$($n.totalCpuDeltaSec)s, max rpc outage $($n.maxRpcOutageMin) min, restarts $($n.restartsDetected)"
}
$txt += ''
$txt += 'Not observable in this phase: peer count (no RPC), receipt-index bytes (no get_receipt RPC), Prometheus counters (none exported).'
$txt -join "`r`n" | Out-File -FilePath $txtPath -Encoding utf8

Get-Content $txtPath | Write-Host
Write-Host ''
Write-Host "JSON: $jsonPath"
if ($result -eq 'FAIL') { exit 1 }
exit 0
