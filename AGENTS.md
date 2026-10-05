# AGENTS.md

Rules for everyone who changes this repository: maintainers, contributors
and AI coding agents. They hold on every machine and in every session,
local or cloud. Where a rule names an ADR, the ADR is the source; this
file is the summary.

Volt is a hardware description language with a Rust-like syntax. It
compiles to SystemVerilog and checks clock domain safety in its type
system: an unsynchronized clock domain crossing is a compile error.

## Build and test

```
cargo build && cargo test                         # whole workspace
cargo run -p volt-driver -- build <file.volt>     # compile one file
```

The `justfile` holds the gates (`just --list`):

| Recipe | When | What |
|---|---|---|
| `just check` | before every commit | `cargo fmt --check`, clippy with `-D warnings`, all tests; zero warnings |
| `just clippy-strict` | before every push | nightly clippy and clippy for the Linux target: `#[cfg(unix)]` code is invisible to clippy on Windows, and CI runs on Linux |
| `just consistency` | before every commit | spec-code consistency (`scripts/check-consistency.{sh,ps1}`) |
| `just weekly` | weekly | coverage, consistency, benchmarks, fuzz |
| `just coverage` | as needed | report in `target/llvm-cov/html/` |
| `just bench` | local machines, never CI | baseline in `crates/volt-syntax/benches/baseline.json` |

## Documents and their precedence

When documents disagree, the higher one wins:

1. `docs/design/Volt-UX-Anayasasi.md` (the UX constitution)
2. `docs/adr/` (architecture decision records)
3. `docs/spec/` (the language specification)
4. `docs/design/`
5. `docs/research/` (background, not binding)

`docs/spec/` file names say their subject: `grammar-full.ebnf`,
`operator-precedence.md`, `ast-nodes.md`, `error-recovery.md`,
`name-resolution.md`, `type-inference.md`, `domain-inference.md` (clock
domains, CDC), `const-eval.md`, `sv-mapping.md`, `cli-contract.md`;
`GLOSSARY.md` (terminology) and `README.md` (index, translation status).
The canonical language of `docs/spec/` is English; `docs/spec/tr/` is a
translation. Before translating a term, look it up in `GLOSSARY.md`.

## Areas you do not edit

- **`docs/spec/` is not edited.** A new or changed rule gets a new ADR.
  New diagnostic codes are defined in an ADR (the consistency check reads
  codes from `docs/spec/` and `docs/adr/`).
- **`docs/research/` is never edited.** It is background material.
- **ADR decision text never changes.** A decision that changes or extends
  an earlier one is a new ADR.

## ADR rules

ADRs live in `docs/adr/` as `ADR-NNNN-<slug>.md`, numbered in sequence
(take the next free number; check open pull requests for a number in use).
The full rules are in `docs/adr/README.md` ("Durum sözlüğü ve başlık
bloğu"); `just consistency` (checks 9-12) enforces them.

- **Header block.** The `>` lines after the title. It has exactly one
  `> Statü:` line whose value is one of the six words of the status
  vocabulary, optionally followed by ` — ` and a free-text note:
  `Uygulandı` (in force and in the code), `Kabul edildi` (in force, part
  of its plan not in the code yet), `Rezerve` (a name, keyword or code
  reserved, no mechanism), `Kısmen yerini aldı: ADR-X` (part replaced by
  ADR-X), `Yerini aldı: ADR-X` (wholly replaced), `Reddedildi`.
- **The header lines are the one part of an existing ADR you may edit**:
  its status and link lines. The decision text stays as written.
- **Links go both ways.**
  - Replacement: the new ADR has `> Önceki karar: ADR-X — …`, and ADR-X's
    status names the new ADR (`Yerini aldı:` or `Kısmen yerini aldı:`).
  - Other relations (implements, extends, closes a limitation):
    `> İlgili: ADR-X (…), ADR-Y (…)` in the new ADR, and the older ADR's
    `> İlgili:` line lists the newer one.
- **Index.** Every ADR appears exactly once in a table of
  `docs/adr/README.md`, linked to its file, with the same status as its
  header.
- Every link target in a header must exist.

## Code rules

- **No `unsafe` code.**
- **Never delete or skip a test**: no `#[ignore]`, no removal, no
  commenting out. A test that is wrong is fixed, with the reason in the
  commit body.
- **New dependencies** go through `[workspace.dependencies]` in the root
  `Cargo.toml`, in a pull request that gives the reason.
- **Every diagnostic has five parts**: error code, location, description,
  a fix-it suggestion, and the spec reference (the `volt explain <CODE>`
  line). Text shown to users (diagnostics, `volt explain`, `volt doctor`)
  carries no ADR numbers and no spec file paths (ADR-0099).
- **Every compiler suggestion round-trips**: applied, it removes the
  diagnostic and adds no new error or warning (`tests/suggestions/`,
  ADR-0099).
- **A new feature has at least one pass and one fail test** in
  `tests/ui/`.

## Tests: a failing test before the fix

- For a bug fix, write the test that reproduces the bug, run it and see it
  fail, then fix the code and see it pass. Name these tests in the report
  ("failed before, passes after").
- `tests/ui/pass/` must compile; `tests/ui/fail/` must produce the
  expected diagnostic, and each file starts with `//~ EXXXX` (or `WXXXX`).
  Numeric file prefixes do not repeat within a directory.
- `tests/fixtures/` holds input and expected-output pairs.
- Tests write into a temporary directory, never into the checkout; a
  child process (`volt`, a fake tool) runs there too. After `just check`,
  `git status` is clean; CI fails when the tests change or create a file.
- **Set the tool backend explicitly** in every test, golden or script
  that runs `volt test`, `volt run` or `volt verify`:
  `VOLT_TOOL_BACKEND=local` (a missing tool fails the test or skips it
  under the ADR-0079 rule), or `auto` on purpose in a test of the Docker
  fallback itself. Never rely on the default, which silently falls back to
  Docker. `book/tools/check_book.py` defaults to `local`; `--backend
  docker` opts in. CI jobs that have the tools set `VOLT_REQUIRE_TOOLS`,
  so a missing tool fails instead of skipping (ADR-0079, ADR-0099).

## Generated files: `.test-baseline` and `.github/badges.json`

Never edit them by hand. Regenerate both with
`scripts/check-consistency.ps1 -Update` (Windows) or
`scripts/check-consistency.sh --update` (Unix) after adding tests. The
test count can never go below `.test-baseline`.

## Branches, commits and pull requests

1. Branch from an up-to-date `main`: `<type>/<short-name>`, where type is
   `feat`, `fix`, `refactor`, `docs`, `chore` or `test`.
2. Commit message: `<type>(<scope>): <summary>`. The body says what
   changed, why, and which ADR. **Language:** the commit subject and the
   pull request title are in English; commit and pull request bodies may
   be in Turkish.
3. Push the branch and open a pull request with `gh pr create`.
4. Wait for CI (`gh pr checks <number> --watch`) and verify the result in
   the logs (next section).
5. Stop there. **Never merge a pull request, push to `main`, push a tag,
   or create, edit or publish a release.** No force pushes. The
   maintainer merges, tags and releases.
6. Remove any git worktrees you created.

## Verifying CI from the logs

A green check mark is not evidence. A step can pass while the tests it was
meant to run were skipped (a tool was not found), a job can be
informational, and a step can succeed after swallowing an error. Read the
log:

```
gh run view <run-id> --log          # or --log-failed for a red run
```

Confirm in the log that the test summaries report the expected counts and
no failures, that tool-dependent tests ran rather than skipped, and that
every step you expected actually ran. Quote the run ID and the lines you
checked in the report.

## Banned words

Prose in the book (`book/src/`), `README.md`, `CHANGELOG.md` and
`docs/roadmap.md` does not use these words, outside code and inline code:

`revolutionary`, `only`, `first`, `unique`, `unprecedented`,
`game-changing`, `best-in-class`, `world-class`, `blazing`, `effortless`.

`book/tools/check_book.py` (`BANNED_WORDS`) enforces the list for the
book; `book/CONTRIBUTING-BOOK.md` ("Words we do not use") shows how to
rewrite a sentence without them.

## Book writing rules

The book's guided pages (`book/src/setup.md`,
`book/src/cookbook/existing-project.md` and every chapter in
`book/src/tour/`) are written for a beginner on Windows who has never
used a terminal. `book/CONTRIBUTING-BOOK.md` ("Guided pages") has the
details and examples; `book/tools/check_book.py` enforces rules 2 and 3
(the box, the labels, no `$ ` prompt, `output` on output blocks, the
file name on file blocks).

1. **Assume nothing about the computer.** The book does not assume that
   the reader knows the terminal, folders, an editor or how to create a
   file. Setup teaches them; later pages link back to it.
2. **Every guided page opens with a "Before you start" box**
   (`<div class="before-you-start">`): which window, which folder (with
   the `cd` command), and what must be running (for example Docker
   Desktop with "Engine running" when Verilator is not installed).
3. **Three kinds of blocks, each with a visible label:**
   - **Command**, labelled "Type this": the command alone, with no `$ `
     prompt and no output, so that it runs as copied.
   - **Output**, labelled "You should see": ```` ```text,output ````.
     It is not meant to be copied; `output` hides mdBook's copy button
     (`book/theme/volt.css`).
   - **File**, labelled "Create this file: `<name>`" (or "Replace this
     file:"): the file name, and on its page's earliest file a link to
     Setup's "Create a file".
4. **Windows and macOS/Linux.** When a command differs, show both, as two
   labelled blocks ("Type this (Windows, PowerShell):", "Type this (macOS
   and Linux):"). No mdBook plugin is used for this.
5. **Output comes from a real run** on the system the page names; never
   typed or edited by hand. The one allowed change is replacing the
   recorder's user name in a path, which the page says.

## Machines with limited resources

- Before running cargo, set:

  ```
  CARGO_BUILD_JOBS=2
  CARGO_PROFILE_DEV_DEBUG=line-tables-only
  CARGO_PROFILE_TEST_DEBUG=line-tables-only
  ```

- **Run one Docker container at a time.** `volt test`, `volt run` and
  `volt verify` start a container when the tools are not installed
  locally (the Docker bridge, ADR-0094); formal and simulation containers
  count too. Check that `docker ps` is empty before starting another.
- Heavy formal runs (cover or BMC depth above 20, full-core equivalence)
  run one at a time, with `-j 1`.
- A formal run expected to take longer than 30 minutes does not run
  locally: propose moving it to a nightly GitHub Actions job.

## Secrets and personal data

- Never write a token, password, key or other credential into a file, a
  commit, a pull request, an issue, a comment or a log. Workflows use
  `${{ secrets.* }}` or `github.token` and never echo them.
- Keep personal data out of the repository: no e-mail addresses beyond
  git metadata, no user names, no absolute local paths.

## Reports

Reports are written in Turkish. When a task ends, the pull request
description and the final report contain:

- **What changed and why**, with the ADR numbers.
- **Tests:** which ones failed before the change and pass after it.
- **Verification:** the commands run locally and their exit codes (`just
  check`, `just consistency`, `just clippy-strict`), and what could not
  run locally and why (a tool or platform missing).
- **CI:** the run IDs and the result as read in the logs.
- **Behavior changes:** input that was accepted and is now rejected, or
  output that differs. They go into `CHANGELOG.md` marked
  "**Davranış değişikliği.**".
- **Open findings:** each one classified (a silent wrong result, an
  explicit error, an enhancement, or a decision to make), with a short
  reproduction, and either a GitHub issue or the roadmap item it belongs
  to.
- **Skipped work:** anything left out, out of scope or not verified, said
  plainly.

## Releases

The release policy before 1.0 is ADR-0100: release tags are exactly
`vX.Y.Z`; in 0.x a new middle digit may break existing code and a new
last digit brings bug fixes; the latest release is the supported one;
every release note has a "Behavior changes" section. The release
procedure is in `.github/workflows/README.md` (ADR-0093). Releases and
tags are the maintainer's work.

## Repository layout

- `crates/`: `volt-span` → `volt-diagnostics` → `volt-ast` →
  `volt-syntax` → `volt-hir` → `volt-lower`; the emitters `volt-sv-emit`,
  `volt-sdc-emit`, `volt-sw-emit`; `volt-lsp`; `volt-tools`; and
  `volt-driver` (the `volt` binary) on top.
- `tests/ui/{pass,fail}/`, `tests/fixtures/`, `tests/suggestions/`,
  `tests/quickfix/`, `tests/fuzz_regressions/` (see `tests/README.md`).
- `demo/`: the README demo, recorded from a VHS script
  (`book/CONTRIBUTING-BOOK.md`, "The README demo").
- `book/`: the Volt book (mdBook), published to
  https://volt-hdl.github.io/volt/ (the latest release) and
  https://volt-hdl.github.io/volt/dev/ (main).
- `docs/`: specification, ADRs, design documents, roadmap.

## A note for contributors

The ADRs in `docs/adr/` are written in Turkish, and so is their index. An
English summary index is planned: see
[English summary of the design decisions](docs/roadmap.md#english-summary-of-the-design-decisions)
in the roadmap.
