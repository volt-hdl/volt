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
# Under `iex` the script never calls `exit`: that would close the user's
# window. A failure prints a short message (no PowerShell error record) and
# the script returns; run as a file (powershell -File install.ps1) it then
# exits with code 1. Before anything is downloaded the release is resolved:
# GitHub's API first, the releases/latest redirect when the API is rate
# limited. With no published release the script says so and stops.
# This file must stay ASCII without a BOM (check-consistency, check 14):
# GitHub Pages serves it as application/octet-stream with no charset, and
# Windows PowerShell 5.1 decodes such a script in the ANSI code page.
# Everything runs inside one script block, so no function or preference
# leaks into the session that ran `iex`.

& {

$VoltRepo = 'https://github.com/volt-hdl/volt'
$VoltBook = 'https://volt-hdl.github.io/volt/tour/install.html'
# Where releases are looked up and downloaded from. VOLT_INSTALL_TEST_SERVER
# is for install.yml's fake server (scripts/install/test/) and nothing else.
$VoltDownloadRepo = $VoltRepo
$VoltApiRepo = 'https://api.github.com/repos/volt-hdl/volt'
if ($env:VOLT_INSTALL_TEST_SERVER) {
    $VoltDownloadRepo = $env:VOLT_INSTALL_TEST_SERVER.TrimEnd('/') + '/volt-hdl/volt'
    $VoltApiRepo = $env:VOLT_INSTALL_TEST_SERVER.TrimEnd('/') + '/api/repos/volt-hdl/volt'
}
# Transient network failures: this many attempts, waiting 1 s, then 2 s.
$VoltAttempts = 3

function Install-VoltHdl {
    [CmdletBinding()]
    param()

    Set-StrictMode -Version 2.0
    $ErrorActionPreference = 'Stop'

    # Constrained Language Mode (a device policy) forbids the .NET calls
    # this script needs: downloading, the registry, the PATH broadcast.
    if ($ExecutionContext.SessionState.LanguageMode -ne 'FullLanguage') {
        throw ("this PowerShell runs in $($ExecutionContext.SessionState.LanguageMode) mode (set by a policy " +
            "on this computer), in which the installer cannot download or change PATH.`n" +
            "Install by hand instead: $VoltBook#manual-install")
    }

    # Empty here. The copy attached to a release names that release
    # (release.yml writes it), so releases/download/vX.Y.Z/install.ps1
    # installs X.Y.Z.
    $pinnedVersion = ''

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
            Enable-VoltTls12
            $version = $env:VOLT_VERSION
            if (-not $version) { $version = $pinnedVersion }
            if ($version) {
                $tag = 'v' + $version.TrimStart('v')
            } else {
                $tag = Resolve-VoltLatestTag
                Write-Host "Newest release: $tag"
            }
            $base = "$VoltDownloadRepo/releases/download/$tag"
            $name = $asset
            $archive = Join-Path $tmp $name
            $sums = Join-Path $tmp 'SHA256SUMS'
            Write-Host "Downloading $base/$name"
            $status = Save-VoltUrl -Url "$base/SHA256SUMS" -OutFile $sums -What "SHA256SUMS of Volt $tag"
            if ($status -eq 404) {
                throw (Get-VoltNotice ("Volt release $tag was not found. The published releases are listed at`n" +
                    "$VoltRepo/releases`n" + (Get-VoltSourceAdvice)))
            }
            $status = Save-VoltUrl -Url "$base/$name" -OutFile $archive -What "the Volt $tag archive"
            if ($status -eq 404) {
                throw ("Volt release $tag has no archive $name.`n" +
                    "Install by hand ($VoltBook#manual-install) or build from source ($VoltBook#build-from-source).")
            }
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

    $announced = Register-VoltUserPath -Bin $bin
    $fits = Test-VoltPathLength
    Test-VoltShadowing -Bin $bin
    # This session: put the directory in front, once.
    $parts = @($env:Path -split ';' | Where-Object { $_ -and -not (Test-VoltSamePath $_ $bin) })
    $env:Path = (@($bin) + $parts) -join ';'

    Write-Host ''
    Write-Host "$new is ready. This PowerShell window can run volt now."
    if (-not $announced) {
        Write-Warning ('Windows could not be told that PATH changed: other windows find volt after you ' +
            'sign out and sign in again.')
    } elseif ($fits) {
        Write-Host 'New windows started from the Start menu, Win+R or the taskbar find it as well. A terminal'
        Write-Host 'program that is already running (Windows Terminal, VS Code) keeps its old PATH, even in'
        Write-Host 'new tabs: close it completely and start it again.'
    }
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
                "Build from source instead: $VoltBook#build-from-source")
        }
    }
}

# A message that ends the install without being an error (no "error:"
# prefix): there is nothing to install yet.
function Get-VoltNotice {
    param([string]$Message)
    $e = New-Object System.Exception $Message
    $e.Data['VoltNotice'] = $true
    return $e
}

function Get-VoltSourceAdvice {
    return ("Build from source instead (needs Rust, https://rustup.rs):`n" +
        "  git clone $VoltRepo`n  cd volt`n  cargo install --locked --path crates/volt-driver`n" +
        "More: $VoltBook#build-from-source")
}

# Windows PowerShell 5.1 may still offer only TLS 1.0 and 1.1, which GitHub
# refuses. TLS 1.2 is added to what is there; nothing is taken away.
# SystemDefault (0) is left alone: then Windows picks the protocols, TLS 1.2
# among them, and OR-ing Tls12 into 0 would leave TLS 1.2 alone and drop
# TLS 1.3.
function Enable-VoltTls12 {
    $current = [Net.ServicePointManager]::SecurityProtocol
    $tls12 = [Net.SecurityProtocolType]::Tls12
    if ([int]$current -ne 0 -and ($current -band $tls12) -ne $tls12) {
        [Net.ServicePointManager]::SecurityProtocol = $current -bor $tls12
    }
}

# One HTTP GET. Returns Status (0: no HTTP answer), Reason (why it failed),
# Location (on a redirect, with -NoRedirect) and Body (text, without
# -OutFile). Never throws.
#
# HttpWebRequest instead of Invoke-WebRequest: Windows PowerShell 5.1 reads
# at most 64 KB of an error response (DefaultMaximumErrorResponseLength).
# GitHub answers a missing release asset with a 404 page; asked without an
# Accept header that page is about 270 KB of chunked HTML, and .NET drops
# the connection in it and reports "The connection was closed unexpectedly"
# with no status code at all. That was the error a reader saw while no
# release existed (ADR-0096, appendix).
function Invoke-VoltGet {
    param([string]$Url, [string]$Accept = '*/*', [string]$OutFile = '', [switch]$NoRedirect)
    $result = [pscustomobject]@{ Status = 0; Reason = ''; Location = ''; Body = '' }
    $response = $null
    $limit = [Net.HttpWebRequest]::DefaultMaximumErrorResponseLength
    try {
        [Net.HttpWebRequest]::DefaultMaximumErrorResponseLength = -1
        $request = [Net.HttpWebRequest]::Create($Url)
        $request.Accept = $Accept
        $request.UserAgent = 'volt-installer'
        $request.AllowAutoRedirect = -not $NoRedirect
        $request.Timeout = 30000
        $request.ReadWriteTimeout = 60000
        try {
            $response = $request.GetResponse()
        } catch [Net.WebException] {
            $response = $_.Exception.Response
            if ($null -eq $response) {
                $result.Reason = Get-VoltWebReason $_.Exception
                return $result
            }
        }
        $result.Status = [int]$response.StatusCode
        $result.Location = [string]$response.Headers['Location']
        if ($result.Status -ge 200 -and $result.Status -lt 300) {
            Read-VoltBody -Response $response -Result $result -OutFile $OutFile
        }
    } catch {
        $result.Status = 0
        $result.Reason = Get-VoltWebReason $_.Exception
    } finally {
        if ($response) { $response.Close() }
        [Net.HttpWebRequest]::DefaultMaximumErrorResponseLength = $limit
    }
    return $result
}

# The body of a 2xx response into Result.Body or OutFile. A body shorter than
# its Content-Length (the connection dropped) throws.
function Read-VoltBody {
    param($Response, $Result, [string]$OutFile)
    $stream = $Response.GetResponseStream()
    try {
        if (-not $OutFile) {
            $Result.Body = (New-Object IO.StreamReader $stream).ReadToEnd()
            return
        }
        $file = [IO.File]::Create($OutFile)
        try {
            $buffer = New-Object byte[] 65536
            $total = [long]0
            while (($n = $stream.Read($buffer, 0, $buffer.Length)) -gt 0) {
                $file.Write($buffer, 0, $n)
                $total += $n
            }
        } finally {
            $file.Close()
        }
        $expected = $Response.ContentLength
        if ($expected -ge 0 -and $total -ne $expected) {
            throw "the connection closed after $total of $expected bytes"
        }
    } finally {
        $stream.Close()
    }
}

# A short reason for a failed request: the innermost message, plus the
# WebException status (stays English when the message is localized).
function Get-VoltWebReason {
    param([Exception]$Exception)
    $e = $Exception
    $status = ''
    while ($e) {
        if ($e -is [Net.WebException]) { $status = [string]$e.Status }
        if (-not $e.InnerException) { break }
        $e = $e.InnerException
    }
    $message = $e.Message.Trim()
    if ($status -and $status -ne 'UnknownError') { return "$message ($status)" }
    return $message
}

# Invoke-VoltGet, tried again on a transient failure: no HTTP answer, a
# timeout (408) or a server error (5xx). Waits 1 s, then 2 s.
function Invoke-VoltGetWithRetry {
    param([string]$Url, [string]$Accept = '*/*', [string]$OutFile = '', [switch]$NoRedirect)
    for ($attempt = 1; ; $attempt++) {
        $r = Invoke-VoltGet -Url $Url -Accept $Accept -OutFile $OutFile -NoRedirect:$NoRedirect
        $transient = $r.Status -eq 0 -or $r.Status -eq 408 -or $r.Status -ge 500
        if (-not $transient -or $attempt -ge $VoltAttempts) { return $r }
        if ($r.Status -ne 0) { $r.Reason = "HTTP $($r.Status)" }
        Write-Host "           $($r.Reason); trying again in $attempt s"
        Start-Sleep -Seconds $attempt
    }
}

# Downloads Url to OutFile. Returns 200, or 404 when the server has no such
# file; any other failure throws a message that says what could not be
# downloaded (What) and what to do.
function Save-VoltUrl {
    param([string]$Url, [string]$OutFile, [string]$What)
    $r = Invoke-VoltGetWithRetry -Url $Url -OutFile $OutFile
    if ($r.Status -ge 200 -and $r.Status -lt 300) { return 200 }
    if ($r.Status -eq 404) { return 404 }
    $why = $r.Reason
    if ($r.Status -ne 0) { $why = "HTTP $($r.Status)" }
    throw ("could not download $What from $Url`n  $why`n" + (Get-VoltNetworkAdvice))
}

function Get-VoltNetworkAdvice {
    return ('This is usually a network problem (no connection, a proxy or a firewall) or a short ' +
        "GitHub outage.`nRun the same command again in a few minutes, or install by hand: $VoltBook#manual-install")
}

# The tag of the newest published release (drafts and pre-releases do not
# count). Asks the GitHub API; when that is rate limited (403, 429) or
# fails, reads the redirect of releases/latest instead. No release at all
# ends the install with a notice.
function Resolve-VoltLatestTag {
    $none = "No Volt release has been published yet.`n" + (Get-VoltSourceAdvice)
    $api = Invoke-VoltGetWithRetry -Url "$VoltApiRepo/releases/latest" -Accept 'application/vnd.github+json'
    if ($api.Status -eq 200) {
        $tag = $null
        try { $tag = (ConvertFrom-Json $api.Body).tag_name } catch { $tag = $null }
        if ($tag) { return [string]$tag }
        $apiWhy = 'the answer named no release'
    } elseif ($api.Status -eq 404) {
        throw (Get-VoltNotice $none)
    } elseif ($api.Status -eq 403 -or $api.Status -eq 429) {
        $apiWhy = "rate limited (HTTP $($api.Status))"
    } elseif ($api.Status -ne 0) {
        $apiWhy = "HTTP $($api.Status)"
    } else {
        $apiWhy = $api.Reason
    }
    Write-Host "GitHub API: $apiWhy; asking $VoltDownloadRepo/releases/latest instead"
    $web = Invoke-VoltGetWithRetry -Url "$VoltDownloadRepo/releases/latest" -NoRedirect
    if ($web.Status -ge 300 -and $web.Status -lt 400) {
        if ($web.Location -match '/releases/tag/([^/?#]+)$') { return [Uri]::UnescapeDataString($Matches[1]) }
        if ($web.Location -match '/releases/?$') { throw (Get-VoltNotice $none) }
        $webWhy = "redirect to $($web.Location)"
    } elseif ($web.Status -ne 0) {
        $webWhy = "HTTP $($web.Status)"
    } else {
        $webWhy = $web.Reason
    }
    throw ("could not find out which Volt release is the newest.`n" +
        "  $VoltApiRepo/releases/latest`: $apiWhy`n" +
        "  $VoltDownloadRepo/releases/latest`: $webWhy`n" + (Get-VoltNetworkAdvice) + "`n" +
        "Or name the release: `$env:VOLT_VERSION = '0.1.0' (the releases: $VoltRepo/releases)")
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

# Returns whether Explorer was told (Send-VoltSettingChange). Told again
# when the entry is already there: an earlier run may have failed to.
function Register-VoltUserPath {
    param([string]$Bin)
    $key = Get-VoltUserPathKey
    $added = $false
    try {
        $value = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
        if ($null -ne $key.GetValue('Path')) { $kind = $key.GetValueKind('Path') }
        $present = $false
        foreach ($entry in $value -split ';') {
            if ($entry -and (Test-VoltSamePath $entry $Bin)) { $present = $true }
        }
        if (-not $present) {
            # In front: the Volt just installed wins over an older copy
            # elsewhere on the user PATH. Removing "<Bin>;" again gives back
            # the same string.
            if ($value -eq '') {
                $value = $Bin
            } else {
                $value = "$Bin;$value"
            }
            $key.SetValue('Path', $value, $kind)
            $added = $true
        }
    } finally {
        $key.Close()
    }
    if ($added) {
        Write-Host "PATH       added $Bin to your user PATH (HKCU\Environment)"
    } else {
        Write-Host "PATH       already holds $Bin (user PATH, HKCU\Environment)"
    }
    return (Send-VoltSettingChange)
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
        Write-Host "Removed    $Bin from your user PATH (HKCU\Environment)"
        if (-not (Send-VoltSettingChange)) {
            Write-Warning ('Windows could not be told that PATH changed: programs started from the Start menu, ' +
                'Win+R or the taskbar keep finding volt until you sign out and sign in again.')
        }
    }
}

# Tell Explorer that the environment changed (WM_SETTINGCHANGE,
# "Environment"). Explorer keeps its own copy of the environment and hands
# it to everything it starts (Start menu, Win+R, taskbar); without this
# message that copy stays as it was until the user signs out. Returns
# whether the message went out.
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
        $sent = [VoltInstall.NativeMethods]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$result)
        if ($sent -ne [IntPtr]::Zero) { return $true }
    } catch {
        Write-Verbose "Add-Type/SendMessageTimeout failed: $($_.Exception.Message)"
    }
    # Where Add-Type cannot compile (a policy that blocks it): .NET sends
    # the same broadcast after SetEnvironmentVariable for the User target.
    # Deleting a variable that does not exist changes nothing else.
    try {
        [Environment]::SetEnvironmentVariable('VOLT_INSTALLER_NO_SUCH_VARIABLE', $null, 'User')
        return $true
    } catch {
        Write-Verbose "SetEnvironmentVariable failed: $($_.Exception.Message)"
    }
    return $false
}

# Windows builds the PATH of a new process from the system PATH, ';' and
# the user PATH, and leaves the user PATH out completely when the result
# would be longer than 4094 characters (measured on Windows 11, ADR-0096
# appendix). Warns then: new windows would not find volt. Returns whether
# the user PATH fits.
function Test-VoltPathLength {
    $system = [string][Environment]::GetEnvironmentVariable('Path', 'Machine')
    $user = [string][Environment]::GetEnvironmentVariable('Path', 'User')
    $total = $system.Length + 1 + $user.Length
    if ($total -le 4094) { return $true }
    $message = ("Windows will leave your whole user PATH out of programs started from now on (Start menu, " +
        "Win+R, taskbar, new terminals): the system PATH ($($system.Length) characters) and your user PATH " +
        "($($user.Length)) together are longer than 4094 characters. Volt is installed, but new windows will " +
        'not find it, nor anything else on your user PATH, until PATH is shorter.')
    $dups = @($system -split ';' | Where-Object { $_ } | Group-Object | Where-Object { $_.Count -gt 1 } |
            Sort-Object Count -Descending)
    if ($dups.Count -gt 0) {
        $extra = ($dups | Measure-Object -Property Count -Sum).Sum - $dups.Count
        $message += (" The system PATH holds $extra repeated entries (most: '$($dups[0].Name)', " +
            "$($dups[0].Count) times); removing them in 'Edit the system environment variables' " +
            '(administrator) fixes this.')
    }
    Write-Warning $message
    return $false
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
    Write-Host ('Volt is uninstalled. A terminal program that is already running (Windows Terminal, VS Code) ' +
        'keeps the old PATH, even in new tabs, until it is closed completely.')
}

# The whole message, without PowerShell's error record (script line,
# category): what failed and what to do is in the message itself.
function Write-VoltFailure {
    param($Record)
    $e = $Record.Exception
    $notice = $false
    # Guarded: Constrained Language Mode forbids the method call, and this
    # must not fail while reporting that mode.
    try { $notice = $e.Data.Contains('VoltNotice') } catch { $notice = $false }
    if ($notice) {
        Write-Host $e.Message
    } else {
        Write-Host "error: $($e.Message)" -ForegroundColor Red
    }
}

$failed = $false
try {
    Install-VoltHdl
} catch {
    Write-VoltFailure $_
    $failed = $true
}
# Run as a file (powershell -File install.ps1): exit code 1. Under iex there
# is no file ({}.File is empty) and `exit` would close the window: return.
if ($failed -and {}.File) { exit 1 }

}
