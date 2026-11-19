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
