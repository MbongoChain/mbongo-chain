# Focused tests for the soak sampler's classification-spread confirmation
# logic (Resolve-ClassificationSpread) and its interaction with the shared
# Get-ConvergenceClassification. No external test framework is used; the
# function under test is extracted from soak-check.ps1 via the PowerShell
# parser so the script's normal entry point is never executed.
#
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File .\soak-check.Tests.ps1

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Get-ConvergenceClassification (shared, unmodified) comes from config.
. (Join-Path $PSScriptRoot 'devnet-config.ps1')

# Define Resolve-ClassificationSpread by extracting it from soak-check.ps1
# without running the script.
$src = Get-Content (Join-Path $PSScriptRoot 'soak-check.ps1') -Raw
$ast = [System.Management.Automation.Language.Parser]::ParseInput($src, [ref]$null, [ref]$null)
$fn = $ast.FindAll({
        param($n)
        ($n -is [System.Management.Automation.Language.FunctionDefinitionAst]) -and
        ($n.Name -eq 'Resolve-ClassificationSpread')
    }, $true) | Select-Object -First 1
if ($null -eq $fn) { throw 'Resolve-ClassificationSpread not found in soak-check.ps1' }
Invoke-Expression $fn.Extent.Text

$fail = 0
function Assert-Eq($label, $expected, $actual) {
    if ("$expected" -eq "$actual") {
        Write-Host "  PASS  $label (= $actual)"
    } else {
        Write-Host "  FAIL  $label (expected $expected, got $actual)"
        $script:fail++
    }
}

# Height readers record invocations in a shared hashtable ($Probe is a
# reference, so increments made inside the closure remain visible).
function New-Reader([object[]]$Heights, [hashtable]$Probe) {
    return {
        $Probe.calls++
        return $Heights
    }.GetNewClosure()
}
function New-ThrowingReader([hashtable]$Probe) {
    return {
        $Probe.calls++
        throw 'RPC read failed'
    }.GetNewClosure()
}
$probe = @{ calls = 0 }

# Full pipeline: resolve the classification spread, then classify, exactly
# as the sampler does. DelaySeconds 0 keeps the tests fast.
function Classify($rawSpread, $allReachable, $tipsConsistent, $reader, $producerDelta) {
    $cs = Resolve-ClassificationSpread -RawSpread $rawSpread -AllReachable $allReachable `
        -TipsConsistent $tipsConsistent -ExpectedNodeCount 3 -HeightReader $reader -DelaySeconds 0
    $class = Get-ConvergenceClassification -AllReachable $allReachable -HeightSpread $cs `
        -TipsConsistent $tipsConsistent -ProducerDelta $producerDelta -SkewAllowance 1
    return @{ spread = $cs; class = $class }
}

Write-Host 'Resolve-ClassificationSpread tests:'

# a. raw spread 1, re-read confirms 0 -> classification spread 0 -> converged
$probe.calls = 0
$r = Classify 1 $true $true (New-Reader @(100, 100, 100) $probe) 5
Assert-Eq 'a. re-read 0 => classificationSpread 0' 0 $r.spread
Assert-Eq 'a. re-read 0 => converged' 'converged' $r.class
Assert-Eq 'a. reader was consulted once' 1 $probe.calls

# b. raw spread 1, re-read still 1 -> keep 1 -> temporarily-skewed
$probe.calls = 0
$r = Classify 1 $true $true (New-Reader @(100, 100, 101) $probe) 5
Assert-Eq 'b. re-read 1 => classificationSpread 1' 1 $r.spread
Assert-Eq 'b. re-read 1 => temporarily-skewed' 'temporarily-skewed' $r.class

# c. raw spread 1, confirmation read fails -> keep raw spread
$probe.calls = 0
$r = Classify 1 $true $true (New-ThrowingReader $probe) 5
Assert-Eq 'c. failed re-read => keeps raw spread 1' 1 $r.spread
Assert-Eq 'c. failed re-read => temporarily-skewed' 'temporarily-skewed' $r.class

# c2. wrong number of heights returned -> keep raw spread
$probe.calls = 0
$r = Classify 1 $true $true (New-Reader @(100, 100) $probe) 5
Assert-Eq 'c2. wrong height count => keeps raw spread 1' 1 $r.spread

# d. ancestry divergence (tips inconsistent) -> no re-read, stays divergent
$probe.calls = 0
$r = Classify 1 $true $false (New-Reader @(100, 100, 100) $probe) 5
Assert-Eq 'd. inconsistent tips => no re-read' 0 $probe.calls
Assert-Eq 'd. inconsistent tips => divergent' 'divergent' $r.class

# e. raw spread > 1 -> no special re-read, stays divergent
$probe.calls = 0
$r = Classify 2 $true $true (New-Reader @(100, 100, 100) $probe) 5
Assert-Eq 'e. spread 2 => no re-read' 0 $probe.calls
Assert-Eq 'e. spread 2 => classificationSpread 2' 2 $r.spread
Assert-Eq 'e. spread 2 => divergent' 'divergent' $r.class

# f. unreachable node -> no re-read, unreachable regardless of spread
$probe.calls = 0
$r = Classify 1 $false $true (New-Reader @(100, 100, 100) $probe) 5
Assert-Eq 'f. unreachable => no re-read' 0 $probe.calls
Assert-Eq 'f. unreachable => unreachable' 'unreachable' $r.class

Write-Host ''
if ($fail -eq 0) {
    Write-Host 'ALL RESOLVE-CLASSIFICATION-SPREAD TESTS PASSED'
    exit 0
}
Write-Host "$fail TEST(S) FAILED"
exit 1
