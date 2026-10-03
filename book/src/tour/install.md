# Install Volt

<div class="chapter-goal">

In this chapter you will install the `volt` binary with one command, check
it with `volt --version` and ask `volt doctor` what works on your machine.
You will also set up Docker, which runs the simulator for you.

</div>

The output in the Tour was recorded with Volt 0.1.0 on Windows 11, with
Docker Desktop running the simulator. On Linux and macOS the paths use `/`
instead of `\`; everything else looks the same.

## Install with one command

**Windows** (PowerShell, no administrator rights needed):

```powershell
irm https://volt-hdl.github.io/volt/install.ps1 | iex
```

**Linux and macOS** (any shell):

```console
$ curl -fsSL https://volt-hdl.github.io/volt/install.sh | sh
```

No `curl`? `wget -qO- https://volt-hdl.github.io/volt/install.sh | sh`
does the same.

The script prints each step: the download, the SHA256 check, where it put
`volt` and what it changed on your `PATH`. On Windows, the PowerShell
window that ran the command can use `volt` at once. Other terminals, and
every terminal on Linux and macOS, see it after you **open a new one**.
A terminal program that is already running, such as Windows Terminal or
VS Code, hands its old `PATH` to every new tab: **close it completely**
and start it again.

While no release is published, the command installs nothing and says
"No Volt release has been published yet.", followed by the commands to
[build from source](#build-from-source).

| Platform | Supported by the script |
|---|---|
| Windows x86_64 (10 and 11) | yes |
| Windows 11 on Arm | yes, with the x86_64 build (runs through the x64 emulation of Windows 11; there is no native Arm build yet) |
| Linux x86_64, any distribution | yes (static binary) |
| macOS, Apple silicon | yes |
| macOS, Intel | yes |
| Linux on Arm, other systems | no: the script says so; [build from source](#build-from-source) |

## Check the install

In a new terminal:

```console
$ volt --version
volt 0.1.0
```

If you see `command not found` (or, on Windows, "The term 'volt' is not
recognized"), the terminal was opened before the install: close it and
open a new one. With Windows Terminal or VS Code, close the whole program,
not just the tab. If the script warned that your `PATH` is longer than
4094 characters, Windows leaves your user `PATH` out of new windows: follow
the warning (it names any entries repeated in the system `PATH`) or run
`volt` by its full path, `%LOCALAPPDATA%\Programs\Volt\bin\volt.exe`.

## What the script did

1. It asked GitHub for the newest release, downloaded the archive for
   your platform and the release's `SHA256SUMS` file from
   `https://github.com/volt-hdl/volt/releases/download/<version>/`, and
   checked the archive against it. If the check fails, the script stops
   and deletes the download: nothing is installed and nothing changes.
2. It put the binary in a folder of your own account:

   | Platform | Binary | Also there |
   |---|---|---|
   | Windows | `%LOCALAPPDATA%\Programs\Volt\bin\volt.exe` | `LICENSE-APACHE`, `LICENSE-MIT`, `README.md` in `...\Programs\Volt\` |
   | Linux, macOS | `~/.volt/bin/volt` | the same three files in `~/.volt/` |

3. It added that `bin` folder to your `PATH`, once:
   - **Windows:** at the front of your user `PATH` (the registry value
     `HKCU\Environment\Path`, the one "Edit environment variables for your
     account" shows). The system `PATH` is not touched. If a `volt.exe` in
     a system `PATH` folder would still win, the script warns you.
   - **Linux and macOS:** one line at the end of your shell's startup
     file. The line ends with `# added by the Volt installer`:

     | Your shell (`$SHELL`) | File |
     |---|---|
     | bash on Linux | `~/.bashrc` |
     | bash on macOS | `~/.bash_profile` |
     | zsh | `~/.zshrc` |
     | fish | `~/.config/fish/config.fish` |
     | any other | `~/.profile` |

**Update:** run the same command again. It replaces the binary (even while
an editor runs `volt lsp`) and leaves the `PATH` as it is.

**Uninstall:** run it again with `VOLT_UNINSTALL=1`. It deletes the files
listed above and removes the `PATH` entry or line, and nothing else:

```powershell
$env:VOLT_UNINSTALL = 1; irm https://volt-hdl.github.io/volt/install.ps1 | iex; Remove-Item Env:VOLT_UNINSTALL
```

```console
$ curl -fsSL https://volt-hdl.github.io/volt/install.sh | VOLT_UNINSTALL=1 sh
```

The script reads these settings from environment variables:

| Variable | Effect |
|---|---|
| `VOLT_VERSION=0.1.0` | install that release instead of the newest one |
| `VOLT_INSTALL_DIR=<folder>` | install into `<folder>` (the binary goes to `<folder>/bin`); give the same value again to uninstall |
| `VOLT_ARCHIVE=<file>` | install from an archive you already have; a `SHA256SUMS` file next to it is used for the check |
| `VOLT_UNINSTALL=1` | remove Volt again |

Each release also carries the two scripts. A copy taken from a release
installs that release, for example
`curl -fsSL https://github.com/volt-hdl/volt/releases/download/v0.1.0/install.sh | sh`.

## Manual install

Download the archive for your platform. These links always point at the
newest release:

| Platform | Archive |
|---|---|
| Windows x86_64 | [volt-x86_64-pc-windows-msvc.zip](https://github.com/volt-hdl/volt/releases/latest/download/volt-x86_64-pc-windows-msvc.zip) |
| Linux x86_64 (static, any distribution) | [volt-x86_64-unknown-linux-musl.tar.gz](https://github.com/volt-hdl/volt/releases/latest/download/volt-x86_64-unknown-linux-musl.tar.gz) |
| macOS, Apple silicon | [volt-aarch64-apple-darwin.tar.gz](https://github.com/volt-hdl/volt/releases/latest/download/volt-aarch64-apple-darwin.tar.gz) |
| macOS, Intel | [volt-x86_64-apple-darwin.tar.gz](https://github.com/volt-hdl/volt/releases/latest/download/volt-x86_64-apple-darwin.tar.gz) |
| Checksums | [SHA256SUMS](https://github.com/volt-hdl/volt/releases/latest/download/SHA256SUMS) |

Older releases are on the
[Releases page](https://github.com/volt-hdl/volt/releases).

### Check the download

Compare the archive with its line in `SHA256SUMS`. On Linux:

```console
$ grep volt-x86_64-unknown-linux-musl.tar.gz SHA256SUMS | sha256sum -c
volt-x86_64-unknown-linux-musl.tar.gz: OK
```

On macOS, use `shasum -a 256 -c` in place of `sha256sum -c`. On Windows,
PowerShell prints the hash; it must equal the one in `SHA256SUMS` (the case
of the letters does not matter):

```powershell
(Get-FileHash volt-x86_64-pc-windows-msvc.zip -Algorithm SHA256).Hash
Select-String volt-x86_64-pc-windows-msvc.zip SHA256SUMS
```

With the [GitHub CLI](https://cli.github.com/) you can also check where the
archive was built: `gh attestation verify <archive> -R volt-hdl/volt`.

### Unpack it and put `volt` on your PATH

**Each archive unpacks into a folder of its own**, named like the archive.
The `volt` binary is inside that folder:

```text
volt-x86_64-pc-windows-msvc/
├── volt.exe
├── LICENSE-APACHE
├── LICENSE-MIT
└── README.md
```

The binary needs nothing else: no Rust, no runtime library, no data files.

**Windows.** In PowerShell, from the folder that holds the download:

```powershell
Expand-Archive volt-x86_64-pc-windows-msvc.zip -DestinationPath $env:LOCALAPPDATA\Programs
$dir = "$env:LOCALAPPDATA\Programs\volt-x86_64-pc-windows-msvc"
[Environment]::SetEnvironmentVariable("Path", "$dir;" + [Environment]::GetEnvironmentVariable("Path", "User"), "User")
```

Or with the mouse: unpack the `.zip`, open **Settings**, search for
"environment variables", choose **Edit environment variables for your
account**, select **Path**, click **New** and paste the path of the
`volt-x86_64-pc-windows-msvc` folder. Then open a new terminal.

The binary is not code-signed. If Windows SmartScreen says "Windows
protected your PC", choose **More info → Run anyway**. You need to do this
once.

**Linux:**

```console
$ tar -xzf volt-x86_64-unknown-linux-musl.tar.gz
$ mkdir -p ~/.local/bin
$ mv volt-x86_64-unknown-linux-musl/volt ~/.local/bin/
```

Most distributions put `~/.local/bin` on the `PATH` when it exists; you may
need to log in again. If `volt` is still not found, add
`export PATH="$HOME/.local/bin:$PATH"` to `~/.bashrc` (or your shell's
startup file).

**macOS:**

```console
$ tar -xzf volt-aarch64-apple-darwin.tar.gz
$ xattr -d com.apple.quarantine volt-aarch64-apple-darwin/volt
$ mkdir -p ~/.local/bin
$ mv volt-aarch64-apple-darwin/volt ~/.local/bin/
```

Use `x86_64-apple-darwin` in these names on an Intel Mac, and add
`export PATH="$HOME/.local/bin:$PATH"` to `~/.zshrc` if `volt` is not
found. **A browser marks downloaded files as quarantined**, and macOS
refuses to start an unsigned binary with that mark ("cannot be opened
because the developer cannot be verified"). `xattr -d` removes the mark.
If the file was never marked (for example, downloaded with `curl`, as the
install script does), `xattr` says so and you can go on.

## Build from source

For contributors, for platforms without a prebuilt binary, and while no
release is published. You need [Rust](https://rustup.rs) (stable):

```console
$ git clone https://github.com/volt-hdl/volt
$ cd volt
$ cargo install --locked --path crates/volt-driver
```

`cargo install` puts `volt` into `~/.cargo/bin` (on Windows
`%USERPROFILE%\.cargo\bin`), which the Rust installer has already put on
your `PATH`. A cold build takes a few minutes. To update, `git pull` and
run `cargo install` again; to remove, `cargo uninstall volt-driver`.

## When you need Docker

`volt build`, `volt check`, `volt explain`, `volt new`, `volt doctor` and
`volt lsp` need nothing but the binary. `volt test` and `volt run` need the
Verilator simulator, and `volt verify` needs SymbiYosys. You can install
those yourself; if they are missing and Docker is running, Volt runs them
in a container instead. So Docker is needed for simulation and formal
verification when the tools are not installed, and never for anything
else.

## Ask `volt doctor`

`volt doctor` lists every command and whether it can run on this machine.
Here it is on Windows before Docker was started:

```console
$ volt doctor
volt 0.1.0 (windows-x86_64)

✓ build, check, explain — no external tools needed
✗ test, run — Verilator, C++ compiler, make not found
    fix (Windows): start Docker Desktop: Volt runs Verilator in a container (DOCKER below), or use WSL
    see: volt explain simulation-setup
✗ verify — sby, Yosys not found; no SMT solver found (boolector, bitwuzla, yices, z3)
    fix (Windows): start Docker Desktop: Volt runs sby in a container (DOCKER below), or use WSL
    see: volt explain verify-setup
- timing (optional) — OpenSTA not found; checks generated .sdc files
- driver checks (optional) — C compiler not found (found: rustc 1.95.0); compiles drivers from --emit=c,rust
! docker — 29.7.2 installed, daemon did not answer within 5 s
- project — no Volt.toml (searched up to the filesystem root); single .volt files still work
```

`volt build`, `volt check` and `volt explain` work already. Simulation
(`volt test`, `volt run`) and formal verification (`volt verify`) need
external tools.

<div class="box new-to-hw">

**New to hardware?** Simulation and formal verification

A hardware design is not a program you run. To see what it does, a
*simulator* computes the value of every signal, clock cycle by clock
cycle, and a *testbench* drives the inputs and checks the outputs. Volt
uses Verilator, an open-source simulator. *Formal verification* goes
further: a solver searches every possible input sequence for one that
breaks a rule you wrote. Volt uses SymbiYosys for that.

</div>

## Docker: the simulator without an install

You can install Verilator and SymbiYosys yourself (`volt explain
simulation-setup` and `volt explain verify-setup` say how). The shorter
way is Docker: when Verilator or SymbiYosys is missing and Docker is
running, Volt runs the tool in a container on its own and says so in one
line.

Install [Docker Desktop](https://www.docker.com/products/docker-desktop/)
on Windows or macOS, or Docker Engine on Linux, and start it. Then run
`volt doctor` again:

```console
$ volt doctor
volt 0.1.0 (windows-x86_64)

✓ build, check, explain — no external tools needed
✓ test, run — via Docker (verilator/verilator:v5.052: Verilator 5.052, g++ 13.3)
✓ verify — via Docker (hdlc/formal:all: Yosys 0.66, SBY 0.69, boolector 3.2.4, yices 2.7.0, z3 4.15.0)
- timing (optional) — OpenSTA not found; checks generated .sdc files
- driver checks (optional) — C compiler not found (found: rustc 1.95.0); compiles drivers from --emit=c,rust
✓ docker — 29.7.2, daemon running
- project — no Volt.toml (searched up to the filesystem root); single .volt files still work
```

**Expect a download on initial use.** The images are downloaded once,
when a command needs them: about 250 MB for simulation
on your initial `volt test`, about 404 MB for formal verification on your
initial `volt verify`, so about 650 MB in total. Volt prints the size
before the download and the time after it. On our connection the
simulation image took 45 seconds; on a slow day (0.5 MB/s) the same
machine took 8½ minutes for the simulation image and 12 minutes for the
formal one. The Tour needs just the simulation image.

<div class="box from-sv">

**Coming from SystemVerilog?** Your own tools take priority

If `verilator` and `sby` are on your `PATH`, Volt uses them and never
starts a container. The images are pinned by digest
(`verilator/verilator:v5.052`, `hdlc/formal:all`), so a result does not
change when an image tag moves. `VOLT_TOOL_BACKEND=local` turns the
Docker bridge off; `VOLT_TOOL_BACKEND=docker` forces it. On Linux the
files the container writes belong to you, not to root.

</div>

Next: [a project in three commands](new-check-build.md).
