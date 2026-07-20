# Submits one AnchorReceipt to the running v0.3 devnet through the
# existing submit_transaction RPC, using the mbongo-wallet
# submit_receipt example to build and sign the transaction with the
# PUBLIC devnet key (unsafe outside this devnet; never use for funds).
#
# Saves a receipt record under <DevnetRoot>\receipts\<task_id>.json and
# refuses accidental task_id reuse unless -AllowDuplicate is passed
# (used by verify-receipt.ps1 for the duplicate-rejection test).
#
# Usage: .\submit-receipt.ps1 -TaskId <64 hex chars>
#        [-InputCommitment <64 hex>] [-OutputCommitment <64 hex>]
#        [-Metadata <hex bytes>] [-AllowDuplicate]

param(
    [Parameter(Mandatory = $true)][string]$TaskId,
    [string]$InputCommitment = ('00' * 32),
    [string]$OutputCommitment = ('00' * 32),
    [string]$Metadata = '',
    [switch]$AllowDuplicate
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

# The PUBLIC devnet account (signing key seed 0xAA x 32, pre-funded by
# ensure_genesis). Public by design; not a secret.
$DevAccountAddress = '0xe734ea6c2b6257de72355e472aa05a4c487e6b463c029ed306df2f01b5636b58'

# ── Validate inputs ────────────────────────────────────────────────────
$normalizedTaskId = $TaskId -replace '^0x', ''
if ($normalizedTaskId -notmatch '^[0-9a-fA-F]{64}$') {
    throw 'TaskId must be exactly 64 hex characters (32 bytes).'
}
$normalizedTaskId = $normalizedTaskId.ToLowerInvariant()

# ── Devnet must be running ─────────────────────────────────────────────
$producer = $DevnetNodes | Where-Object { $_.Producer } | Select-Object -First 1
if (-not (Test-DevnetRpc $producer.Rpc)) {
    throw "Producer RPC on port $($producer.Rpc) is unreachable. Start the devnet first (start-devnet.ps1)."
}

# ── Refuse accidental task_id reuse across runs ────────────────────────
$recordDir = Join-Path $DevnetRoot 'receipts'
$recordPath = Join-Path $recordDir "$normalizedTaskId.json"
if ((Test-Path $recordPath) -and (-not $AllowDuplicate)) {
    throw "A receipt record for task_id $normalizedTaskId already exists ($recordPath). Anchoring the same task_id twice is rejected by consensus. Pass -AllowDuplicate only for the deliberate duplicate-rejection test."
}

# ── Current sender nonce from the chain (never assumed) ────────────────
$account = Invoke-RestMethod -Uri "http://127.0.0.1:$($producer.Rest)/accounts/$DevAccountAddress" `
    -Method Get -TimeoutSec 5
$nonce = [uint64]$account.nonce

# ── Verify the deployed receipt tool (provenance + hash) ───────────────
# The tool is a deployed artifact built from a pinned commit; this
# script NEVER executes code from an arbitrary working tree and never
# rebuilds anything itself.
$toolManifest = Assert-ReceiptToolManifest
Write-Host "==> Receipt tool verified (source $($toolManifest.toolSourceCommit.Substring(0,7)), sha256 $($toolManifest.sha256.Substring(0,12))...)"

# ── Build the signed request via the verified tool ─────────────────────
Write-Host "==> Building AnchorReceipt (task_id 0x$normalizedTaskId, nonce $nonce)"
$toolArgs = @(
    '--nonce', $nonce, '--task-id', $normalizedTaskId,
    '--input-commitment', $InputCommitment,
    '--output-commitment', $OutputCommitment
)
if ($Metadata -ne '') { $toolArgs += @('--metadata', $Metadata) }

# Native stderr (the tool's devnet-key warning) must not become a
# terminating error under Stop preference in Windows PowerShell 5.1.
$prevEap = $ErrorActionPreference
$ErrorActionPreference = 'Continue'
$builderOutput = & $ReceiptToolPath @toolArgs 2>$null
$builderExit = $LASTEXITCODE
$ErrorActionPreference = $prevEap
if ($builderExit -ne 0) { throw "receipt tool failed (exit $builderExit)." }
$built = ($builderOutput -join "`n") | ConvertFrom-Json

# ── Submit ─────────────────────────────────────────────────────────────
$submittedAt = (Get-Date).ToUniversalTime().ToString('o')
$body = $built.rpc_request | ConvertTo-Json -Depth 16 -Compress
$resp = Invoke-RestMethod -Uri "http://127.0.0.1:$($producer.Rpc)/rpc" -Method Post `
    -ContentType 'application/json' -Body $body -TimeoutSec 10
if (($resp.PSObject.Properties.Name -contains 'error') -and ($null -ne $resp.error)) {
    throw "submit_transaction rejected: $($resp.error | ConvertTo-Json -Compress)"
}
$txHash = [string]$resp.result

# ── Record + report ────────────────────────────────────────────────────
New-Item -ItemType Directory -Force $recordDir | Out-Null
[ordered]@{
    taskId         = "0x$normalizedTaskId"
    txHash         = $txHash
    nonce          = $nonce
    sender         = $built.metadata.sender
    receiptHash    = $built.metadata.receipt_hash
    rpcPort        = $producer.Rpc
    submittedAtUtc = $submittedAt
} | ConvertTo-Json | Out-File -FilePath $recordPath -Encoding utf8

Write-Host ''
Write-Host '── Receipt submitted ──────────────────────────────────────────'
Write-Host "  task_id:   0x$normalizedTaskId"
Write-Host "  tx hash:   $txHash"
Write-Host "  nonce:     $nonce"
Write-Host "  endpoint:  http://127.0.0.1:$($producer.Rpc)/rpc"
Write-Host "  submitted: $submittedAt"
Write-Host "  record:    $recordPath"
Write-Host '  Verify inclusion with: verify-receipt.ps1 -TaskId <task_id>'
Write-Host '───────────────────────────────────────────────────────────────'
exit 0
