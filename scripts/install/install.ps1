# Volt installer for Windows (ADR-0096).
#
#   irm https://volt-hdl.github.io/volt/install.ps1 | iex
#
# Downloads the newest release archive, checks it against the release's
# SHA256SUMS, installs volt.exe into %LOCALAPPDATA%\Programs\Volt\bin and
# adds that directory to your user PATH (HKCU, no administrator rights).
# Running it again updates Volt. Works in Windows PowerShell 5.1 and
# PowerShell 7.
#
# Settings (environment variables: a script piped into iex takes no
# arguments):
#   VOLT_VERSION=0.1.0     install this release instead of the newest one
#   VOLT_INSTALL_DIR=DIR   install into DIR\bin
#   VOLT_ARCHIVE=FILE      install from a local .zip instead of downloading;
#                          checked against a SHA256SUMS file next to it
#                          when there is one
#   VOLT_UNINSTALL=1       remove the files and the PATH entry again
#
# Example: $env:VOLT_VERSION = '0.1.0'; irm https://volt-hdl.github.io/volt/install.ps1 | iex
#
# The script never calls `exit`: under `iex` that would close the user's
# window. Errors are thrown; `powershell -File` turns them into exit code 1.
# This file is ASCII on purpose: Windows PowerShell 5.1 reads a BOM-less
# script in the ANSI code page. Everything runs inside one script block, so
# no function or preference leaks into the session that ran `iex`.

& {

function Install-VoltHdl {
    [CmdletBinding()]
    param()

    Set-StrictMode -Version 2.0
    $ErrorActionPreference = 'Stop'

    $repoUrl = 'https://github.com/volt-hdl/volt'
    # Empty here. The copy attached to a release names that release
    # (release.yml writes it), so releases/download/vX.Y.Z/install.ps1
    # installs X.Y.Z.
    $pinnedVersion = ''
    $bookUrl = 'https://volt-hdl.github.io/volt/tour/install.html'

    if ($env:VOLT_INSTALL_DIR) {
        $dir = [IO.Path]::GetFullPath($env:VOLT_INSTALL_DIR)
    } else {
        $dir = Join-Path $env:LOCALAPPDATA 'Programs\Volt'
    }
    $bin = Join-Path $dir 'bin'

    if ($env:VOLT_UNINSTALL -eq '1') {
        Uninstall-VoltFile -Dir $dir -Bin $bin
        return
    }

    $target = Get-VoltTarget
    $asset = "volt-$target.zip"
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ('volt-install-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        if ($env:VOLT_ARCHIVE) {
            if (-not (Test-Path -LiteralPath $env:VOLT_ARCHIVE -PathType Leaf)) {
                throw "VOLT_ARCHIVE: no such file: $env:VOLT_ARCHIVE"
            }
            $source = (Resolve-Path -LiteralPath $env:VOLT_ARCHIVE).ProviderPath
            $name = Split-Path -Leaf $source
            $archive = Join-Path $tmp $name
            Copy-Item -LiteralPath $source -Destination $archive
            Write-Host "Installing Volt from $source"
            $sums = Join-Path (Split-Path -Parent $source) 'SHA256SUMS'
            if (Test-Path -LiteralPath $sums -PathType Leaf) {
                Assert-VoltChecksum -Archive $archive -Sums $sums -Name $name
            } else {
                Write-Warning "no SHA256SUMS next to $name`: the archive is not verified"
            }
        } else {
            $version = $env:VOLT_VERSION
            if (-not $version) { $version = $pinnedVersion }
            if ($version) {
                $base = "$repoUrl/releases/download/v$($version.TrimStart('v'))"
            } else {
                $base = "$repoUrl/releases/latest/download"
            }
            $name = $asset
            $archive = Join-Path $tmp $name
            $sums = Join-Path $tmp 'SHA256SUMS'
            Write-Host "Downloading $base/$name"
            $status = Save-VoltUrl -Url "$base/SHA256SUMS" -OutFile $sums
            if ($status -eq 404) {
                throw ("no published Volt release was found at $base.`n" +
                    "Build from source instead (needs Rust, https://rustup.rs):`n" +
                    "  git clone $repoUrl`n  cd volt`n  cargo install --path crates/volt-driver`n" +
                    "Details: $bookUrl#build-from-source")
            }
            $status = Save-VoltUrl -Url "$base/$name" -OutFile $archive
            if ($status -eq 404) { throw "the release has no archive $name" }
            Assert-VoltChecksum -Archive $archive -Sums $sums -Name $name
        }

        $unpack = Join-Path $tmp 'unpack'
        Expand-VoltZip -Archive $archive -Destination $unpack
        # The archive holds one folder (volt-<target>\) with the binary in
        # it; a binary at the top is accepted as well.
        $exe = @(
            @(Join-Path $unpack 'volt.exe') +
            @(Get-ChildItem -LiteralPath $unpack -Directory | ForEach-Object { Join-Path $_.FullName 'volt.exe' })
        ) | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
        if (-not $exe) { throw "no volt.exe in $name; nothing was installed" }
        $srcDir = Split-Path -Parent $exe

        $dest = Join-Path $bin 'volt.exe'
        $old = $null
        if (Test-Path -LiteralPath $dest) {
            try { $old = (& $dest --version) -join '' } catch { $old = 'an unknown version' }
        }
        New-Item -ItemType Directory -Force -Path $bin | Out-Null
        # A running volt.exe (for example `volt lsp` in an editor) cannot be
        # overwritten, but it can be renamed; the old copy goes away later.
        $stale = "$dest.old"
        if (Test-Path -LiteralPath $stale) { Remove-Item -LiteralPath $stale -Force -ErrorAction SilentlyContinue }
        if (Test-Path -LiteralPath $dest) { Move-Item -LiteralPath $dest -Destination $stale -Force }
        Copy-Item -LiteralPath $exe -Destination $dest
        if (Test-Path -LiteralPath $stale) { Remove-Item -LiteralPath $stale -Force -ErrorAction SilentlyContinue }
        foreach ($f in 'LICENSE-APACHE', 'LICENSE-MIT', 'README.md') {
            $p = Join-Path $srcDir $f
            if (Test-Path -LiteralPath $p) { Copy-Item -LiteralPath $p -Destination (Join-Path $dir $f) -Force }
        }
    } finally {
        Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }

    $new = (& $dest --version) -join ''
    if ($LASTEXITCODE -ne 0) { throw "$dest does not run" }
    if ($old -and $old -ne $new) {
        Write-Host "Updated    $old -> $new in $bin"
    } elseif ($old) {
        Write-Host "Reinstalled $new in $bin"
    } else {
        Write-Host "Installed  $new in $bin"
    }

    Register-VoltUserPath -Bin $bin
    Test-VoltShadowing -Bin $bin
    # This session: put the directory in front, once.
    $parts = @($env:Path -split ';' | Where-Object { $_ -and -not (Test-VoltSamePath $_ $bin) })
    $env:Path = (@($bin) + $parts) -join ';'

    Write-Host ''
    Write-Host "$new is ready. This PowerShell window can run volt now;"
    Write-Host 'terminals that were already open need to be closed and opened again.'
    Write-Host '  volt doctor      # which commands work here'
    Write-Host '  volt new blinky  # a project to start from'
    Write-Host 'Update: run the same install command again. Uninstall: set $env:VOLT_UNINSTALL = 1 and run it again.'
}

# The Rust target triple of the archive for this machine.
function Get-VoltTarget {
    $arch = $null
    try {
        $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    } catch {
        # .NET Framework before 4.7.1 has no RuntimeInformation.
        $arch = $env:PROCESSOR_ARCHITEW6432
        if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    }
    switch -Regex ($arch) {
        '^(X64|AMD64)$' { return 'x86_64-pc-windows-msvc' }
        '^(Arm64|ARM64)$' {
            Write-Host ('Note: there is no native arm64 Windows build of Volt yet. Installing the x86_64 build, ' +
                'which Windows 11 on Arm runs through its x64 emulation.')
            return 'x86_64-pc-windows-msvc'
        }
        default {
            throw ("no prebuilt Volt binary for Windows $arch. Prebuilt binaries exist for Windows x86_64 " +
                '(also used on Windows 11 on Arm), Linux x86_64, macOS arm64 and macOS x86_64. ' +
                'Build from source instead: https://volt-hdl.github.io/volt/tour/install.html#build-from-source')
        }
    }
}

# Downloads Url to OutFile. Returns 200, or 404 when the server has no such
# file; throws on any other failure.
function Save-VoltUrl {
    param([string]$Url, [string]$OutFile)
    # Windows PowerShell 5.1 may default to TLS 1.0, which GitHub refuses.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $progress = $ProgressPreference
    # The progress bar slows Invoke-WebRequest down many times over in 5.1.
    $ProgressPreference = 'SilentlyContinue'
    try {
        Invoke-WebRequest -Uri $Url -OutFile $OutFile -UseBasicParsing
        return 200
    } catch {
        $response = $null
        if ($_.Exception.PSObject.Properties['Response']) { $response = $_.Exception.Response }
        if ($response -and [int]$response.StatusCode -eq 404) { return 404 }
        throw "could not download $Url`: $($_.Exception.Message)"
    } finally {
        $ProgressPreference = $progress
    }
}

# The line for Name in Sums must carry the SHA256 of Archive. Lines look like
# "<hash>  <name>" or "<hash> *<name>". On a mismatch nothing is installed:
# the caller's finally block deletes the download.
function Assert-VoltChecksum {
    param([string]$Archive, [string]$Sums, [string]$Name)
    $expected = $null
    foreach ($line in Get-Content -LiteralPath $Sums) {
        $fields = $line.Trim() -split '\s+', 2
        if ($fields.Count -eq 2 -and $fields[1].TrimStart('*') -eq $Name) {
            $expected = $fields[0].ToLowerInvariant()
            break
        }
    }
    if (-not $expected) { throw "SHA256SUMS has no line for $Name; nothing was installed" }
    $actual = (Get-FileHash -LiteralPath $Archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw ("checksum mismatch for $Name`n  expected $expected`n  got      $actual`n" +
            'The download is damaged or was altered. Nothing was installed.')
    }
    Write-Host "Verified   SHA256 $actual"
}

function Expand-VoltZip {
    param([string]$Archive, [string]$Destination)
    # ZipFile is faster than Expand-Archive and does not care about the
    # file extension.
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    try {
        [IO.Compression.ZipFile]::ExtractToDirectory($Archive, $Destination)
    } catch {
        throw "cannot unpack $(Split-Path -Leaf $Archive): $($_.Exception.Message); nothing was installed"
    }
}

function Test-VoltSamePath {
    param([string]$A, [string]$B)
    $x = [Environment]::ExpandEnvironmentVariables($A).TrimEnd('\')
    $y = [Environment]::ExpandEnvironmentVariables($B).TrimEnd('\')
    return [string]::Equals($x, $y, [StringComparison]::OrdinalIgnoreCase)
}

# The user PATH is read and written in the registry directly, never through
# [Environment]::SetEnvironmentVariable: that expands %VARIABLES% in the
# value and turns a REG_EXPAND_SZ into a REG_SZ. Here the value keeps its
# type and every other entry, byte for byte.
function Get-VoltUserPathKey {
    return [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
}

function Register-VoltUserPath {
    param([string]$Bin)
    $key = Get-VoltUserPathKey
    try {
        $value = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
        if ($null -ne $key.GetValue('Path')) { $kind = $key.GetValueKind('Path') }
        foreach ($entry in $value -split ';') {
            if ($entry -and (Test-VoltSamePath $entry $Bin)) {
                Write-Host "PATH       already holds $Bin (user PATH, HKCU\Environment)"
                return
            }
        }
        # In front: the Volt just installed wins over an older copy
        # elsewhere on the user PATH. Removing "<Bin>;" again gives back
        # the same string.
        if ($value -eq '') {
            $value = $Bin
        } else {
            $value = "$Bin;$value"
        }
        $key.SetValue('Path', $value, $kind)
    } finally {
        $key.Close()
    }
    Send-VoltSettingChange
    Write-Host "PATH       added $Bin to your user PATH (HKCU\Environment)"
}

function Unregister-VoltUserPath {
    param([string]$Bin)
    $key = Get-VoltUserPathKey
    $removed = $false
    try {
        if ($null -eq $key.GetValue('Path')) { return }
        $value = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        $kind = $key.GetValueKind('Path')
        $entries = @($value -split ';')
        $kept = @($entries | Where-Object { -not ($_ -and (Test-VoltSamePath $_ $Bin)) })
        if ($kept.Count -eq $entries.Count) { return }
        $removed = $true
        $newValue = $kept -join ';'
        if ($newValue -eq '') {
            $key.DeleteValue('Path')
        } else {
            $key.SetValue('Path', $newValue, $kind)
        }
    } finally {
        $key.Close()
    }
    if ($removed) {
        Send-VoltSettingChange
        Write-Host "Removed    $Bin from your user PATH (HKCU\Environment)"
    }
}

# Tell Explorer that the environment changed, so terminals started from now
# on see the new PATH without logging out.
function Send-VoltSettingChange {
    try {
        if (-not ('VoltInstall.NativeMethods' -as [type])) {
            Add-Type -Namespace VoltInstall -Name NativeMethods -MemberDefinition @'
[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
'@
        }
        $result = [UIntPtr]::Zero
        # HWND_BROADCAST, WM_SETTINGCHANGE, SMTO_ABORTIFHUNG, 5 s.
        [VoltInstall.NativeMethods]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$result) | Out-Null
    } catch {
        Write-Verbose "WM_SETTINGCHANGE broadcast failed: $($_.Exception.Message); log out and in to refresh PATH"
    }
}

# New terminals search the system PATH before the user PATH: a volt.exe in
# a system PATH directory would run instead of this one.
function Test-VoltShadowing {
    param([string]$Bin)
    $order = @([Environment]::GetEnvironmentVariable('Path', 'Machine') -split ';') +
        @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';')
    foreach ($d in $order) {
        if (-not $d) { continue }
        if (Test-VoltSamePath $d $Bin) { return }
        $other = Join-Path ([Environment]::ExpandEnvironmentVariables($d)) 'volt.exe'
        if (Test-Path -LiteralPath $other -PathType Leaf) {
            Write-Warning ("$other comes earlier on PATH than $Bin, so new terminals run that one. " +
                'Remove or rename it, or run this installation by its full path.')
            return
        }
    }
}

function Uninstall-VoltFile {
    param([string]$Dir, [string]$Bin)
    foreach ($f in 'bin\volt.exe', 'bin\volt.exe.old', 'LICENSE-APACHE', 'LICENSE-MIT', 'README.md') {
        $p = Join-Path $Dir $f
        if (Test-Path -LiteralPath $p) {
            Remove-Item -LiteralPath $p -Force
            Write-Host "Removed    $p"
        }
    }
    # Only empty directories go: whatever else is there is not ours.
    foreach ($d in $Bin, $Dir) {
        if ((Test-Path -LiteralPath $d) -and -not (Get-ChildItem -LiteralPath $d -Force)) {
            Remove-Item -LiteralPath $d -Force
        }
    }
    Unregister-VoltUserPath -Bin $Bin
    $env:Path = @($env:Path -split ';' | Where-Object { $_ -and -not (Test-VoltSamePath $_ $Bin) }) -join ';'
    Write-Host 'Volt is uninstalled. Terminals that are already open keep the old PATH until they are closed.'
}

Install-VoltHdl

}
