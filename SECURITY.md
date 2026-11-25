# Security policy

## Supported versions

Security fixes go into the next patch release of the latest minor version
(today `0.1.x`). Older releases do not get separate fixes; please update
to the latest release.

## What to report privately

Report these privately, not in a public issue:

- **A bypass of a safety check that a design relies on for security.**
  For example, the compiler accepts a design in which a secret value
  reaches a public output without `declassify` (E3009), or a trust
  annotation is lost on some path, and gives no error.
- **A problem in the Volt toolchain itself.** For example, a crafted
  `.volt` file or `Volt.toml` that makes `volt` run unexpected commands,
  read or write files outside the project, or reach the network; a
  problem in the installers or the published release files (checksums,
  attestations).

Other wrong results of the compiler are bugs, not security reports: open
a public issue and title it `silent-wrong:` when the compiler accepts a
design that computes the wrong thing. Public issues are tested and fixed
in the open.

## Where Volt writes files

Every file Volt writes stays under the output root of the command: the
current directory for `build/` (or the directory given with
`--target-dir`), the new project directory for `volt new`, the current
directory for `volt init`, and the directory of the file given with
`volt run --vcd`. The root itself and the path above it are your choice
and are not checked. Below the root, Volt refuses to write, create,
copy or delete through a symbolic link, or to a path that leaves the
root with `..`; the command stops with exit code 3 and names the link.
This keeps a link that comes with a cloned repository (for example
`build/rtl/Top.sv` pointing at a file in your home directory) from
making `volt build` overwrite that file (#221).

On Windows:

- Git for Windows clones symbolic links as plain text files unless
  symbolic links were enabled when Git was installed
  (`core.symlinks=true`, which needs Developer Mode or administrator
  rights). With the default setting a cloned repository cannot plant a
  link.
- Volt treats a junction and every other reparse point like a symbolic
  link. Git never creates junctions, but a script or an archive tool can.
- If a directory under the output root is a reparse point for another
  reason (for example a cloud-sync placeholder), Volt refuses to write
  there too; choose an output directory outside it with `--target-dir`.

A tool that Volt starts (Verilator, SymbiYosys) writes inside directories
that Volt checks before the tool starts. Between that check and the write
there is a short window in which another program running as you could
swap a directory for a link; Volt does not close it, because such a
program can already write to your files.

## How to report

Use the **Report a vulnerability** button on the repository's
[Security tab](https://github.com/volt-hdl/volt/security). The report is
visible to the maintainers and to you.

Please include:

- the output of `volt --version`;
- the smallest `.volt` file (and `Volt.toml`, if it matters) that shows
  the problem;
- the command you ran, what you expected and what happened;
- what an attacker or a design could gain from it.

## What happens next

The maintainer confirms the report, works on a fix in a private fork,
and publishes a security advisory with the release that contains the
fix. Volt is maintained by a small team; we aim to answer a report within
a week. You are credited in the advisory unless you ask not to be.
