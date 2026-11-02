# Stops the Mbongo v0.3 devnet nodes recorded in this deployment's PID
# files. Only stops PIDs whose live process still runs the deployed
# binary — never kills by executable name. Stale PID files are cleaned
# up safely. Data directories are never touched.
#
# Usage: .\stop-devnet.ps1

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

$failures = 0

# Stop followers first, then the producer.
$ordered = @($DevnetNodes | Where-Object { -not $_.Producer }) + @($DevnetNodes | Where-Object { $_.Producer })

foreach ($node in $ordered) {
    $pidFile = Get-NodePidFile $node
    if (-not (Test-Path $pidFile)) {
        Write-Host "$($node.Name): not running (no PID file)"
        continue
    }

    $info = Read-NodePidFile $node
    $proc = Get-RunningNodeProcess $node

    if ($null -eq $proc) {
        Write-Host "$($node.Name): stale PID file (PID $($info.pid) not running this deployment's binary); removing it"
        Remove-Item $pidFile -Force
        continue
    }

    Write-Host "$($node.Name): stopping PID $($proc.Id) ($($info.exePath))"
    try {
        Stop-Process -Id $proc.Id -Force -Confirm:$false -ErrorAction Stop
        $deadline = (Get-Date).AddSeconds(15)
        while ((Get-Date) -lt $deadline) {
            try { $null = Get-Process -Id $proc.Id -ErrorAction Stop } catch { break }
            Start-Sleep -Milliseconds 250
        }
        $still = $null
        try { $still = Get-Process -Id $proc.Id -ErrorAction Stop } catch {}
        if ($null -ne $still) {
            Write-Error "$($node.Name): PID $($proc.Id) did not exit within 15s"
            $failures++
            continue
        }
        Remove-Item $pidFile -Force
        Write-Host "$($node.Name): stopped"
    } catch {
        Write-Error "$($node.Name): failed to stop PID $($proc.Id): $_"
        $failures++
    }
}

if ($failures -gt 0) {
    Write-Error "$failures node(s) failed to stop cleanly."
    exit 1
}
Write-Host 'Devnet stopped. Data directories are preserved.'
exit 0
