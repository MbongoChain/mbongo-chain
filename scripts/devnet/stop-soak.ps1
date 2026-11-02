# Stops the soak sampler recorded for one session and generates the final
# report. Positively identifies the recorded PID as THIS session's live
# soak-check.ps1 process before stopping it, and only ever stops that exact
# process. A PID that is gone, or has been reused by an unrelated process,
# is treated as stale: its pointer is removed but no process is killed. A
# live process that cannot be positively identified as our sampler is left
# untouched, the PID file is RETAINED (fail closed), and NO (misleading)
# final report is produced. Never touches node processes.
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

function Add-SoakEvent([string]$Session, [string]$Message) {
    Add-Content -Path (Join-Path $Session 'events.log') `
        -Value "$((Get-Date).ToUniversalTime().ToString('o')) $Message" -Encoding utf8
}

if (-not (Test-Path $pidFile)) {
    Write-Host 'No sampler PID file (sampler not running or already cleaned up).'
} else {
    $info = Get-Content $pidFile -Raw | ConvertFrom-Json
    $samplerPid = [int]$info.pid

    # --- Gather live facts (each read fails soft to $null) --------------
    $proc = $null
    try { $proc = Get-Process -Id $samplerPid -ErrorAction Stop } catch { $proc = $null }
    $alive = ($null -ne $proc)

    $liveExe = $null
    if ($alive) { try { $liveExe = $proc.Path } catch { $liveExe = $null } }

    $cmdline = $null
    if ($alive) {
        try {
            $cmdline = (Get-CimInstance Win32_Process -Filter "ProcessId=$samplerPid" -ErrorAction Stop).CommandLine
        } catch { $cmdline = $null }
    }

    $liveStart = $null
    if ($alive) { try { $liveStart = $proc.StartTime.ToUniversalTime().ToString('o') } catch { $liveStart = $null } }

    $recordedExe = if ($info.PSObject.Properties.Name -contains 'exePath') { $info.exePath } else { $null }
    $recordedStart = if ($info.PSObject.Properties.Name -contains 'startedAtUtc') { $info.startedAtUtc } else { $null }

    # --- Decide (pure) --------------------------------------------------
    $disp = Get-SoakSamplerDisposition -PidFilePresent $true -ProcessAlive $alive `
        -RecordedExePath $recordedExe -LiveExePath $liveExe -CommandLine $cmdline `
        -ExpectedScript 'soak-check.ps1' -SessionPath $SessionPath `
        -RecordedStartUtc $recordedStart -LiveStartUtc $liveStart

    switch ($disp.Action) {
        'stop' {
            Write-Host "Sampler PID $samplerPid confirmed for this session; stopping..."
            $stopped = Invoke-SamplerStop -ProcessId $samplerPid -TimeoutSeconds 15
            if (-not $stopped) {
                # Stop failed: keep the PID file and do NOT emit a report.
                Add-SoakEvent $SessionPath "stop-soak FAILED to stop sampler pid $samplerPid; PID file retained, no report"
                throw "Failed to stop sampler PID $samplerPid within 15s. The PID file has been RETAINED and NO final report was generated. Investigate the process and retry."
            }
            Add-SoakEvent $SessionPath "sampler stopped by stop-soak (pid $samplerPid)"
            Remove-Item $pidFile -Force
            Write-Host 'Sampler stopped and confirmed dead; PID file removed.'
        }
        'remove-stale' {
            Write-Host "Recorded PID $samplerPid is not this session's live sampler ($($disp.Identity): $($disp.Reason))."
            Write-Host 'Removing the stale PID file; no process was stopped.'
            Add-SoakEvent $SessionPath "stale/foreign sampler pointer removed by stop-soak (pid $samplerPid, $($disp.Identity))"
            Remove-Item $pidFile -Force
        }
        'abort' {
            # Fail closed: alive but unverified. Retain the PID file, no report.
            Add-SoakEvent $SessionPath "stop-soak ABORTED: unverified live PID $samplerPid ($($disp.Reason)); PID file retained, no report"
            throw @"
Sampler PID $samplerPid is alive but could not be positively identified as this session's sampler.
Reason: $($disp.Reason)
The PID file has been RETAINED and NO final report was generated (fail closed).
Investigate manually before retrying, e.g.:
  Get-CimInstance Win32_Process -Filter "ProcessId=$samplerPid" | Select-Object ProcessId, CommandLine
"@
        }
        default {
            throw "Unexpected sampler disposition '$($disp.Action)'."
        }
    }
}

# --- Final report ------------------------------------------------------
# Reached only when the sampler is confirmed stopped, confirmed stale, or
# absent -- never while a live-but-unverified sampler is still running.
Write-Host 'Generating final report...'
& (Join-Path $PSScriptRoot 'soak-report.ps1') -SessionPath $SessionPath
exit $LASTEXITCODE
