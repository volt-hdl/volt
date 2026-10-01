# Network failure tests of install.ps1 against a fake GitHub
# (fake-github.py; install.yml; ADR-0096, appendix).
#
#   scripts/install/test/test-network.ps1 -Archive dist\volt-x86_64-pc-windows-msvc.zip -Shell powershell
#
# -Shell is the PowerShell that runs the installer: powershell (Windows
# PowerShell 5.1) or pwsh (PowerShell 7). Each scenario runs the installer
# as `Get-Content -Raw install.ps1 | iex` in a new process, the way the
# one-line command does, with VOLT_INSTALL_TEST_SERVER pointing at the fake
# server and VOLT_INSTALL_DIR at a temporary folder. Checks: the expected
# message, no PowerShell error record (stack trace), the session is still
# there after a failure (no `exit`), nothing left in the install folder,
# the default install folder or the temporary folder. Two scenarios succeed
# (rate-limited API, transient errors): those install, then uninstall, and
# the user PATH (HKCU\Environment) must end as it was. Run on your own
# machine only after backing up that value (ADR-0096, "Yerel denetim").
param(
    [Parameter(Mandatory = $true)][string]$Archive,
    [ValidateSet('powershell', 'pwsh')][string]$Shell = 'powershell',
    [string]$Python = 'python'
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$script:fails = 0

function Test-Fail([string]$Message) {
    Write-Host "FAIL: $Message" -ForegroundColor Red
    $script:fails++
}

function Get-UserPathText {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment')
    try {
        if ($null -eq $key.GetValue('Path')) { return '<no Path value>' }
        $value = $key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        return "[$($key.GetValueKind('Path'))] $value"
    } finally {
        $key.Close()
    }
}

# Every file below the default install folder with its size and time, or
# "absent". A reader's own installation stays exactly as it was.
function Get-DefaultDirText {
    $d = Join-Path $env:LOCALAPPDATA 'Programs\Volt'
    if (-not (Test-Path -LiteralPath $d)) { return 'absent' }
    return (@(Get-ChildItem -LiteralPath $d -Recurse -Force | Sort-Object FullName |
                ForEach-Object { "$($_.FullName)|$(if ($_.PSIsContainer) { 'dir' } else { $_.Length })|$($_.LastWriteTimeUtc.Ticks)" }) -join "`n")
}

function Get-TempCount {
    return @(Get-ChildItem ([IO.Path]::GetTempPath()) -Filter 'volt-install-*' -Directory |
            Where-Object { $_.FullName -ne $work }).Count
}

$here = $PSScriptRoot
$installer = (Resolve-Path (Join-Path $here '..\install.ps1')).ProviderPath
$Archive = (Resolve-Path $Archive).ProviderPath
$work = Join-Path ([IO.Path]::GetTempPath()) ('volt-network-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$run = [Guid]::NewGuid().ToString('N').Substring(0, 8)
$script:serial = 0

$portFile = Join-Path $work 'port'
$server = $null

# Runs the installer in a new $Shell process. Mode iex: as the one-line
# command does, with a marker printed after it (the session survived).
# Mode file: powershell -File install.ps1, for the exit code. Returns the
# exit code and stdout + stderr.
function Invoke-Installer([string]$Scenario, [hashtable]$Vars, [string]$Mode = 'iex') {
    $script:serial++
    $all = @{ VOLT_INSTALL_TEST_SERVER = "http://127.0.0.1:$port/$Scenario-$run$($script:serial)" }
    foreach ($k in $Vars.Keys) { $all[$k] = $Vars[$k] }
    foreach ($k in $all.Keys) { Set-Item "env:$k" $all[$k] }
    $out = Join-Path $work "run$($script:serial).out"
    $err = Join-Path $work "run$($script:serial).err"
    if ($Mode -eq 'iex') {
        $argList = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command',
            "Get-Content -Raw -LiteralPath '$installer' | Invoke-Expression; 'VOLT-SESSION-ALIVE'")
    } else {
        $argList = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$installer`"")
    }
    try {
        $p = Start-Process $Shell -Wait -PassThru -NoNewWindow -RedirectStandardOutput $out -RedirectStandardError $err -ArgumentList $argList
    } finally {
        foreach ($k in $all.Keys) { Remove-Item "env:$k" -ErrorAction SilentlyContinue }
    }
    $text = "$(Get-Content -Raw $out)$(Get-Content -Raw $err)"
    Write-Host "--- $Scenario ($Mode): exit $($p.ExitCode)"
    Write-Host $text
    return [pscustomobject]@{ Code = $p.ExitCode; Text = $text }
}

# What every run must show: no error record, and under iex a live session.
function Assert-Clean($Result, [string]$Label, [string]$Mode = 'iex') {
    foreach ($pattern in 'At line:\d', 'CategoryInfo', 'FullyQualifiedErrorId', '(?m)^\s*Line \|', '(?m)^\+ ', 'ScriptStackTrace') {
        if ($Result.Text -match $pattern) { Test-Fail "${Label}: PowerShell error record in the output ($pattern)" }
    }
    if ($Mode -eq 'iex' -and $Result.Text -notmatch 'VOLT-SESSION-ALIVE') {
        Test-Fail "${Label}: the session did not go on after the installer (exit?)"
    }
}

function Assert-Text($Result, [string]$Label, [string[]]$Expected) {
    foreach ($e in $Expected) {
        if (-not $Result.Text.Contains($e)) { Test-Fail "${Label}: output lacks '$e'" }
    }
}

# A scenario that must install nothing: message, clean output, no folder.
function Test-Failure([string]$Scenario, [string[]]$Expected, [string[]]$Absent = @()) {
    Write-Host "=== $Scenario"
    $dir = Join-Path $work $Scenario
    $r = Invoke-Installer $Scenario @{ VOLT_INSTALL_DIR = $dir }
    Assert-Clean $r $Scenario
    Assert-Text -Result $r -Label $Scenario -Expected $Expected
    foreach ($a in $Absent) { if ($r.Text.Contains($a)) { Test-Fail "${Scenario}: output holds '$a'" } }
    if (Test-Path -LiteralPath $dir) { Test-Fail "${Scenario}: left $dir behind" }
}

# A scenario that must install: then uninstall again.
function Test-Success([string]$Scenario, [string[]]$Expected) {
    Write-Host "=== $Scenario"
    $dir = Join-Path $work $Scenario
    $r = Invoke-Installer $Scenario @{ VOLT_INSTALL_DIR = $dir }
    Assert-Clean $r $Scenario
    Assert-Text -Result $r -Label $Scenario -Expected $Expected
    if (-not (Test-Path -LiteralPath (Join-Path $dir 'bin\volt.exe'))) { Test-Fail "${Scenario}: volt.exe was not installed" }
    $u = Invoke-Installer $Scenario @{ VOLT_INSTALL_DIR = $dir; VOLT_UNINSTALL = '1' }
    Assert-Clean $u "$Scenario uninstall"
    if (Test-Path -LiteralPath $dir) { Test-Fail "${Scenario}: uninstall left $dir behind" }
}

$pathBefore = Get-UserPathText
$defaultBefore = Get-DefaultDirText
$tempBefore = Get-TempCount
try {
    $server = Start-Process $Python -PassThru -NoNewWindow -RedirectStandardError (Join-Path $work 'server.log') `
        -ArgumentList @("`"$(Join-Path $here 'fake-github.py')`"", "`"$Archive`"", "`"$portFile`"")
    for ($i = 0; $i -lt 150 -and -not (Test-Path $portFile); $i++) { Start-Sleep -Milliseconds 100 }
    if (-not (Test-Path $portFile)) { throw "the fake server did not start: $(Get-Content -Raw (Join-Path $work 'server.log'))" }
    $port = (Get-Content -Raw $portFile).Trim()
    Write-Host "fake GitHub on 127.0.0.1:$port ($Shell)"

    $none = 'No Volt release has been published yet.'
    $source = 'cargo install --locked --path crates/volt-driver'
    $book = 'https://volt-hdl.github.io/volt/tour/install.html#build-from-source'
    $retry = 'trying again in 1 s'
    Test-Failure 'none' -Expected @($none, $source, $book) -Absent @('error:')
    Test-Failure 'ratelimit-none' -Expected @('rate limited (HTTP 429)', $none, $source) -Absent @('error:')
    Test-Failure 'asset404' @('Newest release: v9.9.9', 'error: Volt release v9.9.9 has no archive', '#manual-install')
    Test-Failure 'cut' @('error: could not download the Volt v9.9.9 archive', $retry, 'trying again in 2 s',
        'network problem', '#manual-install')
    Test-Failure 'down' @('error: could not find out which Volt release is the newest', $retry, 'network problem')
    Test-Success 'ratelimit' @('rate limited (HTTP 403)', 'Newest release: v9.9.9', 'Verified', 'Installed')
    Test-Success 'flaky' @($retry, 'Newest release: v9.9.9', 'HTTP 503', 'Verified', 'Installed')

    Write-Host '=== VOLT_VERSION names a release that does not exist'
    $dir = Join-Path $work 'pinned'
    $r = Invoke-Installer -Scenario 'none' -Vars @{ VOLT_INSTALL_DIR = $dir; VOLT_VERSION = '0.0.1' }
    Assert-Clean $r 'pinned'
    Assert-Text -Result $r -Label 'pinned' -Expected @('Volt release v0.0.1 was not found', $source)
    if ($r.Text -match 'error:|trying again') { Test-Fail 'pinned: a 404 was reported as an error or retried' }
    if (Test-Path -LiteralPath $dir) { Test-Fail "pinned: left $dir behind" }

    Write-Host '=== exit code when run as a file'
    foreach ($s in 'none', 'cut') {
        $dir = Join-Path $work "$s-file"
        $r = Invoke-Installer -Scenario $s -Vars @{ VOLT_INSTALL_DIR = $dir } -Mode 'file'
        Assert-Clean -Result $r -Label "$s (file)" -Mode 'file'
        if ($r.Code -ne 1) { Test-Fail "$s (file): exit code $($r.Code), expected 1" }
        if (Test-Path -LiteralPath $dir) { Test-Fail "$s (file): left $dir behind" }
    }

    if ((Get-UserPathText) -cne $pathBefore) { Test-Fail 'the user PATH differs from before the tests' }
    if ((Get-DefaultDirText) -cne $defaultBefore) { Test-Fail 'the default install folder changed' }
    if ((Get-TempCount) -ne $tempBefore) { Test-Fail 'a temporary volt-install-* folder was left behind' }
} finally {
    if ($server) {
        Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
        $server.WaitForExit()
    }
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}

if ($script:fails) {
    Write-Host "$($script:fails) check(s) failed" -ForegroundColor Red
    exit 1
}
Write-Host 'all network checks passed' -ForegroundColor Green
exit 0
