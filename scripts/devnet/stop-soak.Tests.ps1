# Focused tests for stop-soak.ps1's sampler recognition and stop mechanics.
# The decision logic (Get-SoakSamplerDisposition), the path/host predicates,
# and the stop-and-confirm helper (Invoke-SamplerStop) are pure / injectable
# functions in devnet-config.ps1, so these tests exercise them directly
# without touching the live devnet, spawning the sampler, or writing any
# report.
#
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File .\stop-soak.Tests.ps1

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'devnet-config.ps1')

$fail = 0
function Assert-Eq($label, $expected, $actual) {
    if ("$expected" -eq "$actual") {
        Write-Host "  PASS  $label (= $actual)"
    } else {
        Write-Host "  FAIL  $label (expected $expected, got $actual)"
        $script:fail++
    }
}
function Assert-True($label, $actual) { Assert-Eq $label $true $actual }
function Assert-False($label, $actual) { Assert-Eq $label $false $actual }

# --- Fixtures ----------------------------------------------------------
$Session = 'C:\mbongo-devnet\v0.3\soak\soak-20260725-173528-v03-2h-skew-confirmation-smoke'
$Ps51 = 'C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe'
$Ps7 = 'C:\Program Files\PowerShell\7\pwsh.exe'
$StartUtc = '2026-07-25T17:35:28.0000000Z'
$LiveUtc = '2026-07-25T17:35:28.5000000Z'   # ~0.5s after the recorded launch

# A correct sampler command line for a given host / script / session, in the
# exact shape start-soak.ps1 launches (redirected, hidden, -File ... -Loop).
function Cmd([string]$exe, [string]$script, [string]$session) {
    return "`"$exe`" -NoProfile -ExecutionPolicy Bypass -File C:\Dev\mbongo-chain\scripts\devnet\$script -SessionPath $session -Loop "
}
$GoodCmd = Cmd $Ps51 'soak-check.ps1' $Session

Write-Host 'Get-SoakSamplerDisposition tests:'

# a. active + correct exe + correct script + correct session => stop
$d = Get-SoakSamplerDisposition -ProcessAlive $true -RecordedExePath $Ps51 -LiveExePath $Ps51 `
    -CommandLine $GoodCmd -SessionPath $Session -RecordedStartUtc $StartUtc -LiveStartUtc $LiveUtc
Assert-Eq 'a. active+correct => stop' 'stop' $d.Action
Assert-Eq 'a. identity confirmed' 'confirmed' $d.Identity

# b. PID no longer exists => stale (gone)
$d = Get-SoakSamplerDisposition -ProcessAlive $false -RecordedExePath $Ps51 `
    -CommandLine $null -SessionPath $Session
Assert-Eq 'b. dead PID => remove-stale' 'remove-stale' $d.Action
Assert-Eq 'b. identity gone' 'gone' $d.Identity

# c. PID reused by an unrelated PowerShell command => never stop (foreign)
$reuseCmd = "`"$Ps51`" -NoProfile -Command Get-ChildItem C:\somewhere"
$d = Get-SoakSamplerDisposition -ProcessAlive $true -RecordedExePath $Ps51 -LiveExePath $Ps51 `
    -CommandLine $reuseCmd -SessionPath $Session -RecordedStartUtc $StartUtc -LiveStartUtc $LiveUtc
Assert-Eq 'c. reused PID => remove-stale' 'remove-stale' $d.Action
Assert-Eq 'c. reused PID => foreign' 'foreign' $d.Identity
Assert-False 'c. reused PID never stops' ($d.Action -eq 'stop')

# d. sampler for ANOTHER session => never stop (foreign to this session)
$otherCmd = Cmd $Ps51 'soak-check.ps1' 'C:\mbongo-devnet\v0.3\soak\soak-19990101-000000-other'
$d = Get-SoakSamplerDisposition -ProcessAlive $true -RecordedExePath $Ps51 -LiveExePath $Ps51 `
    -CommandLine $otherCmd -SessionPath $Session -RecordedStartUtc $StartUtc -LiveStartUtc $LiveUtc
Assert-Eq 'd. other session => remove-stale' 'remove-stale' $d.Action
Assert-False 'd. other session never stops' ($d.Action -eq 'stop')

# e. correct command line but WRONG executable => never stop (fail closed)
$d = Get-SoakSamplerDisposition -ProcessAlive $true -RecordedExePath $Ps51 -LiveExePath 'C:\evil\powershell.exe' `
    -CommandLine $GoodCmd -SessionPath $Session -RecordedStartUtc $StartUtc -LiveStartUtc $LiveUtc
Assert-Eq 'e. wrong exe => abort' 'abort' $d.Action
Assert-False 'e. wrong exe never stops' ($d.Action -eq 'stop')

# f. command line inaccessible (null) on a live process => fail closed
$d = Get-SoakSamplerDisposition -ProcessAlive $true -RecordedExePath $Ps51 -LiveExePath $Ps51 `
    -CommandLine $null -SessionPath $Session -RecordedStartUtc $StartUtc -LiveStartUtc $LiveUtc
Assert-Eq 'f. null cmdline => abort' 'abort' $d.Action
Assert-Eq 'f. null cmdline => indeterminate' 'indeterminate' $d.Identity

# g. Windows PowerShell 5.1 host path => recognized
$d = Get-SoakSamplerDisposition -ProcessAlive $true -RecordedExePath $Ps51 -LiveExePath $Ps51 `
    -CommandLine (Cmd $Ps51 'soak-check.ps1' $Session) -SessionPath $Session
Assert-Eq 'g. WinPS 5.1 => stop' 'stop' $d.Action

# h. PowerShell 7 pwsh.exe host path => recognized
$d = Get-SoakSamplerDisposition -ProcessAlive $true -RecordedExePath $Ps7 -LiveExePath $Ps7 `
    -CommandLine (Cmd $Ps7 'soak-check.ps1' $Session) -SessionPath $Session
Assert-Eq 'h. pwsh 7 => stop' 'stop' $d.Action

# i. different casing in exe and session path => still recognized
$d = Get-SoakSamplerDisposition -ProcessAlive $true `
    -RecordedExePath $Ps51 -LiveExePath $Ps51.ToUpperInvariant() `
    -CommandLine (Cmd $Ps51 'soak-check.ps1' $Session.ToUpperInvariant()) -SessionPath $Session
Assert-Eq 'i. case-insensitive => stop' 'stop' $d.Action

# j. forward-slash session path in the command line => normalized, recognized
$fwd = $Session.Replace('\', '/')
$d = Get-SoakSamplerDisposition -ProcessAlive $true -RecordedExePath $Ps51 -LiveExePath $Ps51 `
    -CommandLine (Cmd $Ps51 'soak-check.ps1' $fwd) -SessionPath $Session
Assert-Eq 'j. forward-slash path => stop' 'stop' $d.Action

# k. no PID file => report-only (nothing to stop)
$d = Get-SoakSamplerDisposition -PidFilePresent $false -ProcessAlive $false -SessionPath $Session
Assert-Eq 'k. no PID file => report-only' 'report-only' $d.Action

# l. sampler already exited (process gone) => stale
$d = Get-SoakSamplerDisposition -ProcessAlive $false -RecordedExePath $Ps51 -SessionPath $Session
Assert-Eq 'l. completed sampler => remove-stale' 'remove-stale' $d.Action

# x. PID-reuse guard: a live process that started before our recorded launch,
#    even with a matching command line, is not trusted (fail closed).
$d = Get-SoakSamplerDisposition -ProcessAlive $true -RecordedExePath $Ps51 -LiveExePath $Ps51 `
    -CommandLine $GoodCmd -SessionPath $Session `
    -RecordedStartUtc $StartUtc -LiveStartUtc '2026-07-25T10:00:00.0000000Z'
Assert-Eq 'x. live predates launch => abort' 'abort' $d.Action

# y. sibling-prefix session must NOT be mistaken for this session.
$siblingCmd = Cmd $Ps51 'soak-check.ps1' ($Session + '-2')
$d = Get-SoakSamplerDisposition -ProcessAlive $true -RecordedExePath $Ps51 -LiveExePath $Ps51 `
    -CommandLine $siblingCmd -SessionPath $Session -RecordedStartUtc $StartUtc -LiveStartUtc $LiveUtc
Assert-False 'y. sibling-prefix never stops' ($d.Action -eq 'stop')
Assert-Eq 'y. sibling-prefix => foreign remove-stale' 'remove-stale' $d.Action

Write-Host ''
Write-Host 'Helper predicate tests:'
Assert-True  'path exact match'         (Test-CommandLineReferencesPath -CommandLine "-SessionPath $Session -Loop" -Path $Session)
Assert-True  'path trailing-slash arg'  (Test-CommandLineReferencesPath -CommandLine "-SessionPath $Session\ -Loop" -Path $Session)
Assert-True  'path quoted arg'          (Test-CommandLineReferencesPath -CommandLine "-SessionPath `"$Session`" -Loop" -Path $Session)
Assert-False 'path sibling no match'    (Test-CommandLineReferencesPath -CommandLine "-SessionPath ${Session}-2 -Loop" -Path $Session)
Assert-False 'path null cmdline'        (Test-CommandLineReferencesPath -CommandLine $null -Path $Session)
Assert-True  'leaf script match'        (Test-CommandLineReferencesLeaf -CommandLine $GoodCmd -Leaf 'soak-check.ps1')
Assert-False 'leaf script no match'     (Test-CommandLineReferencesLeaf -CommandLine $reuseCmd -Leaf 'soak-check.ps1')
Assert-True  'host 5.1 match'           (Test-SamePowerShellHost -LiveExePath $Ps51 -RecordedExePath $Ps51)
Assert-True  'host 7 match'             (Test-SamePowerShellHost -LiveExePath $Ps7 -RecordedExePath $Ps7)
Assert-True  'host case-insensitive'    (Test-SamePowerShellHost -LiveExePath $Ps51.ToUpperInvariant() -RecordedExePath $Ps51)
Assert-False 'host mismatch'            (Test-SamePowerShellHost -LiveExePath $Ps51 -RecordedExePath $Ps7)
Assert-False 'host null live'           (Test-SamePowerShellHost -LiveExePath $null -RecordedExePath $Ps51)
Assert-False 'host non-powershell exe'  (Test-SamePowerShellHost -LiveExePath 'C:\x\node.exe' -RecordedExePath 'C:\x\node.exe')

Write-Host ''
Write-Host 'Invoke-SamplerStop tests:'

# m. stop succeeds: the process is gone afterwards => confirmed => report allowed
$stopped = Invoke-SamplerStop -ProcessId 424242 -TimeoutSeconds 2 `
    -Stopper { param($p) } `
    -AliveProbe { param($p) $false } `
    -Sleeper { }
Assert-True 'm. stop confirmed dead' $stopped

# n. stop fails: the process stays alive => not confirmed => report forbidden
$stopped = Invoke-SamplerStop -ProcessId 424242 -TimeoutSeconds 0 `
    -Stopper { param($p) } `
    -AliveProbe { param($p) $true } `
    -Sleeper { }
Assert-False 'n. stop failed still alive' $stopped

# m2. stop succeeds only after a couple of probes (exercises the wait loop).
$probe = @{ calls = 0 }
$stopped = Invoke-SamplerStop -ProcessId 424242 -TimeoutSeconds 5 `
    -Stopper { param($p) } `
    -AliveProbe { param($p) $probe.calls++; return ($probe.calls -lt 3) }.GetNewClosure() `
    -Sleeper { }
Assert-True 'm2. stop confirmed after retries' $stopped

Write-Host ''
if ($fail -eq 0) {
    Write-Host 'ALL STOP-SOAK TESTS PASSED'
    exit 0
}
Write-Host "$fail TEST(S) FAILED"
exit 1
