# End-to-end test of install.ps1 (install.yml; ADR-0096). Changes the user
# PATH in HKCU\Environment and restores it: meant for a CI runner. On your
# own machine, back up the value first (ADR-0096, "Yerel denetim").
#
#   scripts/install/test/test-install.ps1 -Archive dist\volt-x86_64-pc-windows-msvc.zip -Shell powershell
#
# -Shell is the PowerShell that runs the installer: powershell (Windows
# PowerShell 5.1) or pwsh (PowerShell 7). The installer runs as
# `Get-Content -Raw install.ps1 | iex`, the same way as the one-line command.
# Steps: install; a NEW process with the PATH Explorer would give it finds
# volt; install again (update) keeps one PATH entry; uninstall removes the
# files and leaves the user PATH exactly as it was (value and registry type).
# Then: a wrong SHA256SUMS stops the install and leaves nothing behind, and
# a release that does not exist ends with the build-from-source advice.
param(
    [Parameter(Mandatory = $true)][string]$Archive,
    [ValidateSet('powershell', 'pwsh')][string]$Shell = 'powershell'
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$script:fails = 0

function Test-Fail([string]$Message) {
    Write-Host "FAIL: $Message" -ForegroundColor Red
    $script:fails++
}

function Get-UserPath {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment')
    try {
        if ($null -eq $key.GetValue('Path')) { return $null }
        return [pscustomobject]@{
            Value = $key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            Kind  = $key.GetValueKind('Path').ToString()
        }
    } finally {
        $key.Close()
    }
}

function Format-UserPath($p) {
    if ($null -eq $p) { return '<no Path value>' }
    return "[$($p.Kind)] $($p.Value)"
}

$installer = (Resolve-Path (Join-Path $PSScriptRoot '..\install.ps1')).ProviderPath
$Archive = (Resolve-Path $Archive).ProviderPath
$name = Split-Path -Leaf $Archive
$work = Join-Path ([IO.Path]::GetTempPath()) ('volt-install-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path "$work\good", "$work\bad" | Out-Null
Copy-Item $Archive "$work\good\"
Copy-Item $Archive "$work\bad\"
$hash = (Get-FileHash "$work\good\$name" -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$work\good\SHA256SUMS", "$hash  $name`n")
[IO.File]::WriteAllText("$work\bad\SHA256SUMS", ('0' * 64) + "  $name`n")
$bin = Join-Path $env:LOCALAPPDATA 'Programs\Volt\bin'
$pathBefore = Get-UserPath
Write-Host "User PATH before: $(Format-UserPath $pathBefore)"

# Runs the installer the way `irm ... | iex` does, in a fresh $Shell
# process; Vars sets the VOLT_* variables for that run only (through the
# environment: Windows PowerShell 5.1 garbles non-ASCII characters, as in
# C:\Users\<name>, on a native command line).
function Invoke-Installer([hashtable]$Vars, [string]$After = '') {
    foreach ($k in $Vars.Keys) { Set-Item "env:$k" $Vars[$k] }
    $install = "Get-Content -Raw -LiteralPath '$installer' | Invoke-Expression"
    $cmd = "`$ErrorActionPreference = 'Stop'; $install; $After"
    # Windows PowerShell 5.1 turns a native command's stderr into errors
    # under 'Stop'; the exit code is what counts here.
    $ErrorActionPreference = 'Continue'
    try {
        & $Shell -NoProfile -ExecutionPolicy Bypass -Command $cmd | Out-Host
        return $LASTEXITCODE
    } finally {
        $ErrorActionPreference = 'Stop'
        foreach ($k in $Vars.Keys) { Remove-Item "env:$k" -ErrorAction SilentlyContinue }
    }
}

# A new terminal: a process whose PATH is built from the registry, as
# Explorer builds it, not inherited from this one. Command must not hold
# double quotes: Windows PowerShell 5.1 drops them from native arguments.
function Invoke-NewShell([string]$Command) {
    $cmd = "`$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User'); $Command"
    $ErrorActionPreference = 'Continue'
    return (& $Shell -NoProfile -ExecutionPolicy Bypass -Command $cmd 2>&1 | Out-String)
}

try {
    Write-Host "=== install ($Shell)"
    # In the same session as iex: volt is on PATH at once, and none of the
    # installer's functions leaked into the session.
    $after = "if (Get-Command Install-VoltHdl -ErrorAction SilentlyContinue) { exit 7 }; volt --version; if (`$LASTEXITCODE) { exit 8 }"
    $code = Invoke-Installer @{ VOLT_ARCHIVE = "$work\good\$name" } $after
    if ($code -ne 0) { Test-Fail "install exited $code (7: functions leaked, 8: volt not on the session PATH)" }
    if (-not (Test-Path "$bin\volt.exe")) { Test-Fail "$bin\volt.exe missing" }

    Write-Host '=== new shell: volt --version'
    # Another volt.exe earlier on the system PATH would win the plain `volt`
    # lookup (the installer warns about that). The check: the new shell
    # finds ours on its PATH, and it runs.
    $out = Invoke-NewShell '$c = @(Get-Command volt -All); $c.Source; & ($c | Where-Object { $_.Source -like ''*\Programs\Volt\bin\volt.exe'' } | Select-Object -First 1).Source --version'
    Write-Host $out
    if ($out -notmatch [regex]::Escape("$bin\volt.exe") -or $out -notmatch 'volt \d') {
        Test-Fail 'a new shell did not find and run volt from the install directory'
    }

    Write-Host '=== install again (update path)'
    $code = Invoke-Installer @{ VOLT_ARCHIVE = "$work\good\$name" }
    if ($code -ne 0) { Test-Fail "second install exited $code" }
    $entries = @((Get-UserPath).Value -split ';' | Where-Object { $_.TrimEnd('\') -ieq $bin })
    if ($entries.Count -ne 1) { Test-Fail "user PATH holds $bin $($entries.Count) times, expected 1" }

    Write-Host '=== uninstall'
    $code = Invoke-Installer @{ VOLT_UNINSTALL = '1' }
    if ($code -ne 0) { Test-Fail "uninstall exited $code" }
    if (Test-Path (Split-Path -Parent $bin)) { Test-Fail "$(Split-Path -Parent $bin) still exists" }
    $pathAfter = Get-UserPath
    Write-Host "User PATH after:  $(Format-UserPath $pathAfter)"
    if ((Format-UserPath $pathAfter) -cne (Format-UserPath $pathBefore)) {
        Test-Fail 'the user PATH differs from before the install'
    }
    $out = Invoke-NewShell '(Get-Command volt -All -ErrorAction SilentlyContinue).Source'
    if ($out -match [regex]::Escape("$bin\volt.exe")) { Test-Fail 'a new shell still finds volt after the uninstall' }

    Write-Host '=== wrong checksum'
    $tempBefore = @(Get-ChildItem ([IO.Path]::GetTempPath()) -Filter 'volt-install-*' -Directory |
            Where-Object { $_.FullName -ne $work }).Count
    $code = Invoke-Installer @{ VOLT_ARCHIVE = "$work\bad\$name"; VOLT_INSTALL_DIR = "$work\bad-install" }
    if ($code -eq 0) { Test-Fail 'install with a wrong SHA256SUMS succeeded' }
    if (Test-Path "$work\bad-install") { Test-Fail 'wrong checksum left files in the install directory' }
    $tempAfter = @(Get-ChildItem ([IO.Path]::GetTempPath()) -Filter 'volt-install-*' -Directory |
            Where-Object { $_.FullName -ne $work }).Count
    if ($tempAfter -ne $tempBefore) { Test-Fail 'wrong checksum left a temporary directory' }
    if ((Format-UserPath (Get-UserPath)) -cne (Format-UserPath $pathBefore)) { Test-Fail 'wrong checksum changed the user PATH' }

    Write-Host '=== release that does not exist'
    # The child's stdout and stderr go to files: the thrown message is on
    # stderr, Write-Host output on stdout.
    $env:VOLT_VERSION = '0.0.0'
    $env:VOLT_INSTALL_DIR = "$work\none"
    try {
        $p = Start-Process $Shell -Wait -PassThru -NoNewWindow `
            -RedirectStandardOutput "$work\none.out" -RedirectStandardError "$work\none.err" `
            -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', "Get-Content -Raw -LiteralPath '$installer' | Invoke-Expression")
    } finally {
        Remove-Item env:VOLT_VERSION, env:VOLT_INSTALL_DIR -ErrorAction SilentlyContinue
    }
    $text = "$(Get-Content -Raw "$work\none.out")$(Get-Content -Raw "$work\none.err")"
    Write-Host "exit $($p.ExitCode), $($text.Length) characters of output:"
    Write-Host $text
    if ($p.ExitCode -eq 0) { Test-Fail 'install of v0.0.0 succeeded' }
    if ($text -notmatch 'no published Volt release' -or $text -notmatch 'cargo install') {
        Test-Fail 'no build-from-source advice for a missing release'
    }
    if (Test-Path "$work\none") { Test-Fail 'missing release left files' }
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}

if ($script:fails) {
    Write-Host "$($script:fails) check(s) failed" -ForegroundColor Red
    exit 1
}
Write-Host 'all install checks passed' -ForegroundColor Green
# Explicit: $LASTEXITCODE still holds the exit code of the last installer
# run (the expected failure above), and the CI shell wrappers exit with it.
exit 0
