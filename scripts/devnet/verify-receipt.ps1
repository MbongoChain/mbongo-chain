# Verifies a previously submitted AnchorReceipt on the running v0.3
# devnet: polls all three nodes (bounded), scans blocks by height for
# the task_id, and reports per-node inclusion, exactly-once presence,
# agreement, and current tips.
#
# With -DuplicateTest, additionally resubmits the same task_id and
# requires the deterministic "already anchored" rejection, then
# confirms the inclusion count is unchanged.
#
# Limitation (documented): no dedicated receipt RPC exists yet, so this
# verifies INCLUSION of the anchoring transaction in the canonical
# chain, not the stored receipt bytes. Canonical byte storage is proven
# by the node test suite and the replay harness at the deployed tag.
#
# Usage: .\verify-receipt.ps1 -TaskId <64 hex chars> [-DuplicateTest]

param(
    [Parameter(Mandatory = $true)][string]$TaskId,
    [switch]$DuplicateTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

$VerifyTimeoutSecs = 30
$VerifyPollMs = 500

$normalizedTaskId = ($TaskId -replace '^0x', '').ToLowerInvariant()
if ($normalizedTaskId -notmatch '^[0-9a-f]{64}$') {
    throw 'TaskId must be exactly 64 hex characters (32 bytes).'
}

# ── Load the submission record ─────────────────────────────────────────
$recordPath = Join-Path (Join-Path $DevnetRoot 'receipts') "$normalizedTaskId.json"
if (-not (Test-Path $recordPath)) {
    throw "No receipt record at $recordPath. Submit first with submit-receipt.ps1."
}
$record = Get-Content $recordPath -Raw | ConvertFrom-Json

# The task_id as the JSON array serde emits for [u8; 32].
$taskIdBytes = @()
for ($i = 0; $i -lt 64; $i += 2) { $taskIdBytes += [Convert]::ToInt32($normalizedTaskId.Substring($i, 2), 16) }
$expected = ($taskIdBytes | ConvertTo-Json -Compress)

# Scans blocks 1..tip on one node; returns every height whose block
# carries an AnchorReceipt with this task_id.
function Get-InclusionHeights([int]$Port) {
    $tip = Get-DevnetHeight $Port
    $found = @()
    for ($h = 1; $h -le $tip; $h++) {
        $body = @{ jsonrpc = '2.0'; method = 'get_block_by_height'; params = @{ height = $h }; id = 1 } |
            ConvertTo-Json -Compress
        $resp = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/rpc" -Method Post `
            -ContentType 'application/json' -Body $body -TimeoutSec 5
        $block = $resp.result
        foreach ($tx in @($block.body.transactions)) {
            $payload = $tx.payload
            if (($payload -is [System.Management.Automation.PSCustomObject]) -and
                ($payload.PSObject.Properties.Name -contains 'AnchorReceipt')) {
                $tid = ($payload.AnchorReceipt.task_id | ConvertTo-Json -Compress)
                if ($tid -eq $expected) { $found += $h }
            }
        }
    }
    return , $found
}

# ── Poll all nodes until the task is visible everywhere (bounded) ──────
Write-Host "==> Verifying task_id 0x$normalizedTaskId (record tx $($record.txHash))"
$deadline = (Get-Date).AddSeconds($VerifyTimeoutSecs)
$results = $null
while ($true) {
    $results = @{}
    $allVisible = $true
    foreach ($node in $DevnetNodes) {
        if (-not (Test-DevnetRpc $node.Rpc)) {
            throw "$($node.Name) RPC (port $($node.Rpc)) unreachable. Is the devnet running?"
        }
        $heights = Get-InclusionHeights $node.Rpc
        $results[$node.Name] = $heights
        if ($heights.Count -eq 0) { $allVisible = $false }
    }
    if ($allVisible) { break }
    if ((Get-Date) -ge $deadline) {
        $state = ($results.GetEnumerator() | ForEach-Object { "    $($_.Key): $($_.Value -join ',')" }) -join "`n"
        Write-Error "Task not visible on all nodes within ${VerifyTimeoutSecs}s:`n$state"
        exit 1
    }
    Start-Sleep -Milliseconds $VerifyPollMs
}

# ── Report ─────────────────────────────────────────────────────────────
$failed = $false
Write-Host ''
Write-Host '── Inclusion ──────────────────────────────────────────────────'
foreach ($node in $DevnetNodes) {
    $heights = $results[$node.Name]
    $tipH = Get-DevnetHeight $node.Rpc
    $tipHash = Get-DevnetTipHash $node.Rpc
    Write-Host "  $($node.Name): included at height $($heights -join ', ') (tx $($record.txHash)); tip $tipH $tipHash"
    if ($heights.Count -ne 1) {
        Write-Host "    ERROR: task_id appears $($heights.Count) times; expected exactly once"
        $failed = $true
    }
}
$distinct = @($results.Values | ForEach-Object { $_[0] } | Sort-Object -Unique)
if ($distinct.Count -eq 1) {
    Write-Host "  All three nodes agree: inclusion height $($distinct[0])"
} else {
    Write-Host "  ERROR: nodes disagree on inclusion height: $($distinct -join ', ')"
    $failed = $true
}
if ($failed) { exit 1 }

# ── Optional duplicate-rejection test ──────────────────────────────────
if ($DuplicateTest) {
    Write-Host ''
    Write-Host '── Duplicate-rejection test ───────────────────────────────────'
    $producer = $DevnetNodes | Where-Object { $_.Producer } | Select-Object -First 1

    Write-Host '  Re-submitting the same task_id (expected to be rejected)...'
    # Run the submitter as a CHILD PROCESS with file-redirected output:
    # its terminating error or exit statement must not end this script,
    # and PowerShell's console error rendering line-wraps messages, so
    # the classification below is whitespace-tolerant.
    $submitScript = Join-Path $PSScriptRoot 'submit-receipt.ps1'
    $outFile = Join-Path $env:TEMP "mbongo-dup-out-$PID.txt"
    $errFile = Join-Path $env:TEMP "mbongo-dup-err-$PID.txt"
    $child = Start-Process -FilePath 'powershell' -ArgumentList @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $submitScript,
        '-TaskId', $normalizedTaskId, '-AllowDuplicate'
    ) -RedirectStandardOutput $outFile -RedirectStandardError $errFile `
        -WindowStyle Hidden -Wait -PassThru
    $dupExit = $child.ExitCode
    $dupText = ''
    foreach ($f in @($outFile, $errFile)) {
        if (Test-Path $f) {
            $dupText += (Get-Content $f -Raw)
            Remove-Item $f -Force
        }
    }
    # Collapse the console line-wrapping before matching.
    $dupFlat = $dupText -replace '\s+', ' '

    if ($dupExit -eq 0) {
        Write-Error 'Duplicate submission was ACCEPTED; expected deterministic rejection.'
        exit 1
    }
    if (($dupFlat -notmatch 'already') -or ($dupFlat -notmatch 'anchored')) {
        Write-Error "Duplicate was rejected, but not with the expected reason. Output:`n$dupText"
        exit 1
    }
    Write-Host '  Rejected with "task_id already anchored" as required'

    # Confirm no new inclusion appeared anywhere.
    foreach ($node in $DevnetNodes) {
        $heights = Get-InclusionHeights $node.Rpc
        if ($heights.Count -ne 1) {
            Write-Error "$($node.Name): task_id now appears $($heights.Count) times after duplicate attempt"
            exit 1
        }
    }
    Write-Host '  Inclusion count unchanged on all nodes (still exactly once)'
}

Write-Host ''
Write-Host 'RECEIPT VERIFICATION: PASS'
Write-Host '  (Note: stored receipt BYTES are not directly checkable without the'
Write-Host '   future get_receipt RPC; byte canonicality is covered by the node'
Write-Host '   test suite and replay harness at the deployed tag.)'
exit 0
