# Install Volt

<div class="chapter-goal">

In this chapter you will download the `volt` binary, put it
on your `PATH` and ask `volt doctor` what works on your machine. You will
also set up Docker, which runs the simulator for you.

</div>

The output in the Tour was recorded with Volt 0.1.0 on Windows 11, with
Docker Desktop running the simulator. On Linux and macOS the paths use `/`
instead of `\`; everything else looks the same.

## Download

Every release on the
[Releases page](https://github.com/volt-hdl/volt/releases) has one archive
per platform:

| Platform | Archive |
|---|---|
| Windows x86_64 | `volt-v0.1.0-x86_64-pc-windows-msvc.zip` |
| Linux x86_64 (static, any distribution) | `volt-v0.1.0-x86_64-unknown-linux-musl.tar.gz` |
| macOS, Apple silicon | `volt-v0.1.0-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `volt-v0.1.0-x86_64-apple-darwin.tar.gz` |

**Each archive unpacks into a folder of its own**, named like the archive.
The `volt` binary is inside that folder:

```text
volt-v0.1.0-x86_64-pc-windows-msvc/
├── volt.exe
├── LICENSE-APACHE
├── LICENSE-MIT
└── README.md
```

So the directory to add to your `PATH` is that inner folder, not the
folder you unpacked the archive into. The binary needs nothing else: no
Rust, no runtime library, no data files.

## Put `volt` on your PATH

### Windows

In PowerShell, from the folder that holds the download:

```powershell
Expand-Archive volt-v0.1.0-x86_64-pc-windows-msvc.zip -DestinationPath $env:LOCALAPPDATA\Programs
$dir = "$env:LOCALAPPDATA\Programs\volt-v0.1.0-x86_64-pc-windows-msvc"
[Environment]::SetEnvironmentVariable("Path", [Environment]::GetEnvironmentVariable("Path", "User") + ";$dir", "User")
```

Or with the mouse: unpack the `.zip`, open **Settings**, search for
"environment variables", choose **Edit environment variables for your
account**, select **Path**, click **New** and paste the path of the
`volt-v0.1.0-x86_64-pc-windows-msvc` folder.

Then **open a new terminal**. A terminal that was already open keeps the
old `PATH` and will not find `volt`.

The binary is not code-signed. If Windows SmartScreen says "Windows
protected your PC", choose **More info → Run anyway**. You need to do this
once.

### Linux

```console
$ tar -xzf volt-v0.1.0-x86_64-unknown-linux-musl.tar.gz
$ mkdir -p ~/.local/bin
$ mv volt-v0.1.0-x86_64-unknown-linux-musl/volt ~/.local/bin/
```

Most distributions put `~/.local/bin` on the `PATH` when it exists; you may
need to log in again. If `volt` is still not found, add
`export PATH="$HOME/.local/bin:$PATH"` to `~/.bashrc` (or your shell's
startup file).

### macOS

```console
$ tar -xzf volt-v0.1.0-aarch64-apple-darwin.tar.gz
$ xattr -d com.apple.quarantine volt-v0.1.0-aarch64-apple-darwin/volt
$ sudo mv volt-v0.1.0-aarch64-apple-darwin/volt /usr/local/bin/
```

Use `x86_64-apple-darwin` in these names on an Intel Mac. A browser marks
downloaded files as quarantined, and macOS refuses to start an unsigned
binary with that mark ("cannot be opened because the developer cannot be
verified"). `xattr -d` removes the mark. If the file was never marked (for
example, downloaded with `curl`), `xattr` says so and you can go on.

## Check the install

```console
$ volt --version
volt 0.1.0
```

## Ask `volt doctor`

`volt doctor` lists every command and whether it can run on this machine.
Here it is on Windows before Docker was started:

```console
$ volt doctor
volt 0.1.0 (windows-x86_64)

✓ build, check, explain — no external tools needed
✗ test, run — Verilator, C++ compiler, make not found
    install (Windows): install Docker Desktop: Volt runs Verilator in a container (DOCKER below), or use WSL
    see: volt explain simulation-setup
✗ verify — sby, Yosys not found; no SMT solver found (boolector, bitwuzla, yices, z3)
    install (Windows): install Docker Desktop: Volt runs sby in a container (DOCKER below), or use WSL
    see: volt explain verify-setup
- timing (optional) — OpenSTA not found; checks generated .sdc files (ADR-0065)
- driver checks (optional) — C compiler not found (found: rustc 1.95.0); compiles drivers from --emit=c,rust (ADR-0053)
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
    (bitwuzla is not in this image; --engine boolector|yices|z3, ADR-0082)
- timing (optional) — OpenSTA not found; checks generated .sdc files (ADR-0065)
- driver checks (optional) — C compiler not found (found: rustc 1.95.0); compiles drivers from --emit=c,rust (ADR-0053)
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
