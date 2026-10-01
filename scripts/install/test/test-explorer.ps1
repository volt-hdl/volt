# Do programs started by the Windows shell find volt after an install, and
# stop finding it after the uninstall? (install.yml; ADR-0096, appendix)
#
#   scripts/install/test/test-explorer.ps1 -Archive dist\volt-x86_64-pc-windows-msvc.zip -Shell powershell
#
# The check process is started by explorer.exe itself, the way Win+R or the
# Start menu start one: the desktop's IShellDispatch (ShellWindows,
# FindWindowSW with SWC_DESKTOP) lives in the shell process, so its
# ShellExecute creates the process there, with Explorer's own copy of the
# environment. Not a child of this script; the test asserts that. Neither
# `Start-Process explorer.exe file.cmd` (a new explorer.exe /factory COM
# server builds a fresh environment from the registry) nor a PATH read from
# the registry by this script shows what Explorer hands out.
#
# Windows leaves the whole user PATH out when system PATH + ';' + user PATH
# is longer than 4094 characters. Part 1 runs with a user PATH that fits
# (the real one, or a short stand-in while the test runs); part 2 pads the
# user PATH past the limit and expects the installer's warning. The user
# PATH (value and registry type) is restored in `finally` and compared.
# Changes HKCU\Environment\Path while it runs: back it up first on your own
# machine (ADR-0096, "Yerel denetim").
param(
    [Parameter(Mandatory = $true)][string]$Archive,
    [ValidateSet('powershell', 'pwsh')][string]$Shell = 'powershell'
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$script:fails = 0
$limit = 4094

function Test-Fail([string]$Message) {
    Write-Host "FAIL: $Message" -ForegroundColor Red
    $script:fails++
}

function Get-UserPath {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment')
    try {
        if ($null -eq $key.GetValue('Path')) { return $null }
        return [pscustomobject]@{
            Value = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            Kind  = $key.GetValueKind('Path')
        }
    } finally {
        $key.Close()
    }
}

function Write-UserPathValue($Path) {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
    try {
        if ($null -eq $Path) {
            $key.DeleteValue('Path', $false)
        } else {
            $key.SetValue('Path', $Path.Value, $Path.Kind)
        }
    } finally {
        $key.Close()
    }
    # Explorer reloads its environment: .NET broadcasts WM_SETTINGCHANGE.
    [Environment]::SetEnvironmentVariable('VOLT_INSTALLER_NO_SUCH_VARIABLE', $null, 'User')
}

function Format-UserPath($p) {
    if ($null -eq $p) { return '<no Path value>' }
    $sha = [BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash(
            [Text.Encoding]::Unicode.GetBytes($p.Value))).Replace('-', '')
    return "kind=$($p.Kind) length=$($p.Value.Length) sha256=$sha"
}

# The desktop's IShellDispatch, inside the shell's explorer.exe; $null when
# there is no shell (a service session).
function Get-ShellDispatch {
    try {
        $windows = (New-Object -ComObject Shell.Application).Windows()
        $loc = 0
        $root = $null
        $hwnd = 0
        # SWC_DESKTOP = 8, SWFO_NEEDDISPATCH = 1
        $desktop = $windows.FindWindowSW([ref]$loc, [ref]$root, 8, [ref]$hwnd, 1)
        if ($null -eq $desktop) { return $null }
        return $desktop.Document.Application
    } catch {
        return $null
    }
}

# Runs a hidden powershell.exe through the shell; it writes its parent
# process and every volt.exe on its PATH to a file. Returns that text.
function Invoke-ThroughShell($Dispatch, [string]$Tag) {
    $out = Join-Path $work "$Tag.txt"
    $child = ("`$p = Get-CimInstance Win32_Process -Filter ('ProcessId=' + `$PID); " +
        "`$q = Get-CimInstance Win32_Process -Filter ('ProcessId=' + `$p.ParentProcessId); " +
        "`$v = @(Get-Command volt.exe -All -ErrorAction SilentlyContinue | ForEach-Object Source) -join '|'; " +
        "('parent=' + `$q.Name + ':' + `$q.ProcessId + ' mark=' + `$env:VOLT_EXPLORER_TEST_MARK + ' volt=' + `$v) | " +
        "Set-Content -LiteralPath '$out'")
    $Dispatch.ShellExecute('powershell.exe', "-NoProfile -WindowStyle Hidden -Command $child", '', 'open', 0)
    for ($i = 0; $i -lt 150 -and -not (Test-Path $out); $i++) { Start-Sleep -Milliseconds 200 }
    if (-not (Test-Path $out)) { return 'no answer' }
    return (Get-Content -Raw $out).Trim()
}

# The shell-started process must be a direct child of the shell's
# explorer.exe (the one that owns the desktop) and must not carry this
# test's environment: VOLT_EXPLORER_TEST_MARK is set here only. (The shell
# is an ancestor of almost everything a user starts, this test included;
# what matters is whose environment the new process got.)
function Assert-ShellParent([string]$Text, [string]$Label) {
    if ($Text -notmatch 'parent=explorer\.exe:(\d+)') { Test-Fail "${Label}: not started by explorer.exe ($Text)"; return }
    $parent = Get-CimInstance Win32_Process -Filter "ProcessId=$($Matches[1])"
    if ($parent -and $parent.CommandLine -match '/factory') { Test-Fail "${Label}: started by an explorer.exe COM server, not the shell" }
    if ($Text -notmatch 'mark= volt=') { Test-Fail "${Label}: the process inherited this test's environment ($Text)" }
}

function Invoke-Installer([hashtable]$Vars) {
    foreach ($k in $Vars.Keys) { Set-Item "env:$k" $Vars[$k] }
    $log = Join-Path $work ('installer-' + [Guid]::NewGuid().ToString('N') + '.log')
    try {
        $p = Start-Process $Shell -Wait -PassThru -NoNewWindow -RedirectStandardOutput $log -RedirectStandardError "$log.err" `
            -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command',
            "Get-Content -Raw -LiteralPath '$installer' | Invoke-Expression")
    } finally {
        foreach ($k in $Vars.Keys) { Remove-Item "env:$k" -ErrorAction SilentlyContinue }
    }
    $text = "$(Get-Content -Raw $log)$(Get-Content -Raw "$log.err")"
    Write-Host $text
    return [pscustomobject]@{ Code = $p.ExitCode; Text = $text }
}

$installer = (Resolve-Path (Join-Path $PSScriptRoot '..\install.ps1')).ProviderPath
$Archive = (Resolve-Path $Archive).ProviderPath
$work = Join-Path ([IO.Path]::GetTempPath()) ('volt-explorer-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$dir = Join-Path $work 'install'
$bin = Join-Path $dir 'bin'
$exe = Join-Path $bin 'volt.exe'

$env:VOLT_EXPLORER_TEST_MARK = 'set-in-the-test-process'
$dispatch = Get-ShellDispatch
if ($null -eq $dispatch) {
    Write-Host '::notice::No Windows shell (explorer.exe desktop) in this session: the Explorer check cannot run here.'
    Remove-Item -Recurse -Force $work
    exit 0
}

$before = Get-UserPath
Write-Host "User PATH before: $(Format-UserPath $before)"
$system = [string][Environment]::GetEnvironmentVariable('Path', 'Machine')
Write-Host "System PATH: $($system.Length) characters; the installer runs in $Shell"
try {
    # Part 1: a user PATH that fits with the Volt entry added.
    $userLen = 0
    if ($before) { $userLen = ([Environment]::ExpandEnvironmentVariables($before.Value)).Length }
    if ($system.Length + 1 + $bin.Length + 1 + $userLen -gt $limit) {
        $short = [pscustomobject]@{ Value = '%SystemRoot%\System32\WindowsPowerShell\v1.0'; Kind = [Microsoft.Win32.RegistryValueKind]::ExpandString }
        Write-Host "The user PATH would not fit; standing in for the test: $($short.Value)"
        Write-UserPathValue $short
    }

    Write-Host '=== install, then a process started by the shell'
    $r = Invoke-Installer @{ VOLT_ARCHIVE = $Archive; VOLT_INSTALL_DIR = $dir }
    if (-not (Test-Path -LiteralPath $exe)) { Test-Fail 'volt.exe was not installed' }
    if ($r.Text -match 'could not be told|longer than 4094') { Test-Fail 'the installer warned although PATH fits and the broadcast should work' }
    $seen = Invoke-ThroughShell $dispatch 'after-install'
    Write-Host "shell-started process: $seen"
    Assert-ShellParent $seen 'after install'
    if (-not $seen.Contains($exe)) { Test-Fail "a process started by the shell does not find $exe after the install" }

    Write-Host '=== uninstall, then a process started by the shell'
    $null = Invoke-Installer @{ VOLT_UNINSTALL = '1'; VOLT_INSTALL_DIR = $dir }
    $seen = Invoke-ThroughShell $dispatch 'after-uninstall'
    Write-Host "shell-started process: $seen"
    Assert-ShellParent $seen 'after uninstall'
    if ($seen.Contains($exe)) { Test-Fail "a process started by the shell still finds $exe after the uninstall" }

    Write-Host '=== a user PATH past the limit: the installer must say so'
    $pad = [Math]::Max(1, $limit - $system.Length - 1 + 1)
    $long = [pscustomobject]@{ Value = 'C:\volt-explorer-test-padding' + ('x' * $pad); Kind = [Microsoft.Win32.RegistryValueKind]::String }
    Write-UserPathValue $long
    $r = Invoke-Installer @{ VOLT_ARCHIVE = $Archive; VOLT_INSTALL_DIR = $dir }
    if ($r.Text -notmatch 'longer than 4094') { Test-Fail 'no warning about the 4094-character limit' }
    $seen = Invoke-ThroughShell $dispatch 'over-limit'
    Write-Host "shell-started process: $seen"
    if ($seen.Contains($exe)) { Write-Host 'note: Windows kept the user PATH past 4094 characters here' }
    $null = Invoke-Installer @{ VOLT_UNINSTALL = '1'; VOLT_INSTALL_DIR = $dir }
} finally {
    Write-UserPathValue $before
    $after = Get-UserPath
    Write-Host "User PATH after:  $(Format-UserPath $after)"
    if ((Format-UserPath $after) -cne (Format-UserPath $before)) { Test-Fail 'the user PATH differs from before the test' }
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}

if ($script:fails) {
    Write-Host "$($script:fails) check(s) failed" -ForegroundColor Red
    exit 1
}
Write-Host 'all Explorer checks passed' -ForegroundColor Green
exit 0
