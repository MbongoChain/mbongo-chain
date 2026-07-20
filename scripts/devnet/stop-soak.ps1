# Stops the soak sampler recorded for one session and generates the
# final report. Stops ONLY the recorded sampler PID, and only after
# validating that the live process is the expected PowerShell
# executable running soak-check.ps1 for this exact session. Never
# touches node processes. Stale PID metadata is handled safely.
#
# Usage: .\stop-soak.ps1 -SessionPath <session dir>

param(
    [Parameter(Mandatory = $true)][string]$SessionPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

$resolvedRoot = Assert-SafeDevnetRoot
$SessionPath = Assert-PathInsideRoot -Path $SessionPath -ResolvedRoot $resolvedRoot
if (-not (Test-Path (Join-Path $SessionPath 'session.json'))) {
    throw "No session.json at $SessionPath."
}
$pidFile = Join-Path $SessionPath 'soak.pid.json'

if (-not (Test-Path $pidFile)) {
    Write-Host 'No sampler PID file (sampler not running or already cleaned up).'
} else {
    $info = Get-Content $pidFile -Raw | ConvertFrom-Json
    $proc = $null
    try { $proc = Get-Process -Id $info.pid -ErrorAction Stop } catch {}

    $valid = $false
    if ($null -ne $proc) {
        $procPath = $null
        try { $procPath = $proc.Path } catch {}
        if ($procPath -eq $info.exePath) {
            # Verify the command line references the sampler script AND
            # this session (never stop an unrelated PowerShell process).
            $cmdline = (Get-CimInstance Win32_Process -Filter "ProcessId=$($info.pid)" `
                -ErrorAction SilentlyContinue).CommandLine
            if (($null -ne $cmdline) -and
                ($cmdline -like "*soak-check.ps1*") -and
                ($cmdline -like "*$($SessionPath)*")) {
                $valid = $true
            }
        }
    }

    if ($valid) {
        Write-Host "Stopping sampler PID $($info.pid)..."
        Stop-Process -Id $info.pid -Force -Confirm:$false
        $deadline = (Get-Date).AddSeconds(15)
        while ((Get-Date) -lt $deadline) {
            try { $null = Get-Process -Id $info.pid -ErrorAction Stop } catch { break }
            Start-Sleep -Milliseconds 250
        }
        Add-Content -Path (Join-Path $SessionPath 'events.log') `
            -Value "$((Get-Date).ToUniversalTime().ToString('o')) sampler stopped by stop-soak (pid $($info.pid))" `
            -Encoding utf8
        Write-Host 'Sampler stopped.'
    } else {
        Write-Host "Stale sampler PID file (PID $($info.pid) is not this session's sampler); removing it."
    }
    Remove-Item $pidFile -Force
}

# --- Final report ------------------------------------------------------
Write-Host 'Generating final report...'
& (Join-Path $PSScriptRoot 'soak-report.ps1') -SessionPath $SessionPath
exit $LASTEXITCODE
