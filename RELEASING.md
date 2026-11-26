# Releasing Volt

The commands the maintainer runs on release day, in order. They are
written for a patch release, with v0.1.3 as the example: for another
release, replace `0.1.3` with the new number and `0.1.2` with the
previous one. A middle-digit release (`0.Y.0`) takes the same steps,
without the limits of the patch rules below. The release commit of
v0.1.0, which also removed the "no release yet" texts, is kept as a
record in [the appendix](#appendix-the-release-commit-of-v010).

The release policy is ADR-0100, the workflow ADR-0093 and ADR-0105; what
the text of a diagnostic may do in a patch release is ADR-0125 §7.
`.github/workflows/README.md` describes what `release.yml` does.

Agents and contributors may prepare the release pull request (step 1).
They do not merge it and do not run steps 3 to 6: the merge, the tag and
the GitHub Release are the maintainer's work (AGENTS.md).

Commands are for PowerShell on Windows unless a step says "Git Bash".
`gh` commands are the same in every shell.

## What a release run does, and what a dry run leaves out

`release.yml` runs in two modes:

| | Dry run (`gh workflow run release.yml --ref <branch>`) | Release run (push of tag `vX.Y.Z`) |
|---|---|---|
| Version check (Cargo = VS Code extension = tag) | yes (no tag) | yes |
| Four archives, smoke test on each platform (`volt --version` = `volt X.Y.Z (<commit>)`) | yes | yes |
| Install scripts against the four archives | yes | yes |
| `.vsix`, `SHA256SUMS` and `sha256sum -c` | yes | yes |
| Release notes from `docs/release-notes/vX.Y.Z.md`, checked | yes | yes |
| Workflow artifacts | yes | yes |
| `publish` job: provenance attestation (public Sigstore log) | **no** | yes |
| `publish` job: draft GitHub Release with every file | **no** | yes |

The `publish` job runs only when `DRY_RUN` is `false`, and it is the one
job with write permissions.

## Which branch a patch release comes from

A patch release is cut from `main` while `main` holds no change that
belongs to v0.2: every entry under `## [Yayımlanmadı]` in `CHANGELOG.md`
passes the [patch rules](#the-patch-rules). This is ADR-0100 §2: fixes go
into `main` and leave with the next release; there is no backport and no
maintenance branch.

Before the earliest change that needs v0.2.0 is merged into `main` (a
change the patch rules do not allow, such as the width of operations,
#266, or the steps of ADR-0118), the maintainer decides how 0.1.x patch
releases go on, for example from a `release/0.1` branch. This file does
not make that decision. A `release/0.1` branch is a maintenance branch,
which ADR-0100 §2 rules out, so the decision is a new ADR. Until there is
one, every step below starts from `main`.

## The patch rules

A patch release (`0.Y.Z`, `Z > 0`) brings fixes and nothing else
(ADR-0100 §1). Before the release pull request, read every entry under
`## [Yayımlanmadı]` in `CHANGELOG.md` and check:

- **No new feature**, no new CLI option, no new diagnostic code, no new
  JSON field.
- **The exception is a silent wrong result.** A fix of a bug that looked
  successful and gave a wrong result may reject input that the previous
  release accepted. The entry is marked "**Davranış değişikliği.**" and
  goes under "Behavior changes".
- **An ADR may extend these rules** for one kind of change; the entry
  names that ADR. ADR-0119 (İ3) allows a migration warning: a new warning
  that marks the places whose meaning changes in the next middle digit,
  changes no rule and rejects no input (`W2015` in 0.1.1, `W2016` in
  0.1.2).
- **Diagnostic text** (message, reason, note, help, `volt explain`) and
  the content of a machine suggestion may change (ADR-0125 §7), when no
  new code, option or JSON field comes with the change, the input that
  triggers the diagnostic stays the same, and the generated SV, SVA, SDC
  and driver output stay the same. Codes whose machine suggestion is
  added, changed or removed are listed under "Behavior changes"; text
  fixes alone are summed up in one line ("The diagnostic text of N codes
  was corrected"). A diagnostic that now appears, or no longer appears,
  for some input is a different matter: ADR-0100 judges it, not §7.

An entry that breaks these rules needs v0.2.0, and a patch release then
no longer comes from `main` ([Which branch a patch release comes
from](#which-branch-a-patch-release-comes-from)).

A fix of a silent wrong result does not wait for other fixes: it calls
for a patch release of its own (ADR-0100 §4).

## Before release day

- `main` is green, and the latest dry run on `main` is green.
- Every entry under `## [Yayımlanmadı]` passes the
  [patch rules](#the-patch-rules).
- The version is the same in `Cargo.toml` (`[workspace.package]`),
  `crates/volt-sv-emit/src/lib.rs` (`VOLT_VERSION`) and
  `editors/vscode/package.json`: the previous release's, `0.1.2`. Step 1
  raises it.

## 1. The release pull request

Start from an up-to-date `main`:

```powershell
git switch main
git pull --ff-only
git switch -c chore/release-0.1.3
```

The pull request has two commits: the new version number, then the
changelog, the release notes, the README and the book. The book's output
is recorded with a binary built from the commit that carries the new
number.

### 1a. The version number

The number appears in these files:

| File | What changes |
|---|---|
| `Cargo.toml` | `version` in `[workspace.package]` |
| `crates/volt-sv-emit/src/lib.rs` | `VOLT_VERSION` |
| `editors/vscode/package.json` | `"version"` |
| `editors/vscode/package-lock.json` | `"version"` of the package, twice |
| `editors/vscode/README.md` | the `.vsix` file name, twice |
| `tests/fixtures/counter.expected.sv` | the `// Version:` line |
| `crates/volt-driver/tests/golden/Table.sv` | the `// Version:` line |
| `Cargo.lock` | the version of every workspace crate |
| `fuzz/Cargo.lock` | the version of the four crates the fuzzer uses: `volt-span`, `volt-diagnostics`, `volt-ast`, `volt-syntax` |

`release.yml` checks that the Cargo version, the VS Code extension's
version and the tag agree; a `cli_tests` test checks that
`volt --version` prints the Cargo version. Without the two `// Version:`
lines, `emit_tests`, `cli_tests` and `sim_golden_tests` fail.

```powershell
$old = '0.1.2'; $new = '0.1.3'
$edits = [ordered]@{
    'Cargo.toml'                               = 'version = "{0}"'
    'crates/volt-sv-emit/src/lib.rs'           = 'VOLT_VERSION: &str = "{0}"'
    'editors/vscode/package.json'              = '"version": "{0}"'
    'editors/vscode/package-lock.json'         = '"version": "{0}"'
    'editors/vscode/README.md'                 = 'volt-hdl-{0}.vsix'
    'tests/fixtures/counter.expected.sv'       = '// Version: {0}'
    'crates/volt-driver/tests/golden/Table.sv' = '// Version: {0}'
}
foreach ($name in $edits.Keys) {
    $f = Join-Path $PWD $name
    $t = [IO.File]::ReadAllText($f)
    $from = $edits[$name] -f $old
    if (-not $t.Contains($from)) { Write-Host "${name}: '$from' not found"; continue }
    [IO.File]::WriteAllText($f, $t.Replace($from, ($edits[$name] -f $new)))
}
cargo update --workspace --offline
cargo update --manifest-path fuzz/Cargo.toml --offline --workspace
git diff --stat
```

No `not found` line, and `git diff --stat` shows nine files with one line
per occurrence. With 13 workspace crates:

```text
 Cargo.lock                               | 26 +++++++++++++-------------
 Cargo.toml                               |  2 +-
 crates/volt-driver/tests/golden/Table.sv |  2 +-
 crates/volt-sv-emit/src/lib.rs           |  2 +-
 editors/vscode/README.md                 |  4 ++--
 editors/vscode/package-lock.json         |  4 ++--
 editors/vscode/package.json              |  2 +-
 fuzz/Cargo.lock                          |  8 ++++----
 tests/fixtures/counter.expected.sv       |  2 +-
 9 files changed, 26 insertions(+), 26 deletions(-)
```

A larger count in `editors/vscode/package-lock.json` means that a
dependency has the old number as its own version: undo that line.

```powershell
just check
git add Cargo.toml Cargo.lock fuzz/Cargo.lock crates/volt-sv-emit/src/lib.rs `
    editors/vscode/package.json editors/vscode/package-lock.json editors/vscode/README.md `
    tests/fixtures/counter.expected.sv crates/volt-driver/tests/golden/Table.sv
git commit -m "chore(release): bump the version to 0.1.3"
```

### 1b. The changelog

In `CHANGELOG.md`, the `## [Yayımlanmadı]` heading becomes
`## [0.1.3] - <today>`:

```powershell
$f = Join-Path $PWD 'CHANGELOG.md'
$t = [IO.File]::ReadAllText($f)
$t = $t -replace '(?m)^## \[Yay\S+\]$', "## [0.1.3] - $(Get-Date -Format yyyy-MM-dd)"
[IO.File]::WriteAllText($f, $t)
git diff --stat
```

The same in Git Bash:

```sh
sed -i "s/^## \[Yayımlanmadı\]\$/## [0.1.3] - $(date +%F)/" CHANGELOG.md
```

`git diff --stat` shows `CHANGELOG.md | 2 +-`: one heading, nothing else.
Then check by hand that every entry of the new `[0.1.3]` section marked
"**Davranış değişikliği.**" sits under its `### Behavior changes`
(ADR-0100 §3), and that the section has no behavior change in it that the
patch rules do not allow.

An entry may name an ADR with the status it had when the entry was
written ("ADR-0140 önerisi", "ADR-0121, Önerildi"). Compare each such
mention in the new section with the `> Statü:` line of the ADR in
`docs/adr/`: when the ADR is now `Uygulandı` or `Kabul edildi`, drop the
old status from the entry; when it is still `Önerildi`, leave the entry
as it is and say so in the pull request.

### 1c. The release notes

`docs/release-notes/v0.1.3.md`, in English, becomes the body of the
GitHub Release (ADR-0105):

- The top line is `# Volt 0.1.3`, followed by a short summary: what the
  release fixes, and whether upgrading is recommended.
- The earliest `##` section is `## Behavior changes`. It is built from
  the entries of the `[0.1.3]` section marked "**Davranış
  değişikliği.**": one item for each, in English, with its issue link,
  saying what 0.1.2 did and what 0.1.3 does for the same input. Codes
  whose machine suggestion was added, changed or removed are items there
  too; diagnostic text fixes are one line (ADR-0125 §7). When nothing
  changed, the section says "None."; it is never left out (ADR-0100 §3).
- Bugs that the release still has go under a "Known issues" section, as
  in `docs/release-notes/v0.1.2.md`.

```powershell
python scripts/release/notes.py --check
python scripts/release/notes.py --build 0.1.3 (Join-Path $env:TEMP 'volt-0.1.3-notes.md')
```

The check prints `0 failure(s)`. The second command writes the body of
the GitHub Release as `release.yml` builds it, to read before the pull
request.

### 1d. README and the book

**README.** In the status note, `Version 0.1.2 is the latest release`
becomes `Version 0.1.3 is the latest release`.

**Book.** The book at https://volthdl.org/ describes the latest release.
These pages show the version and output of `volt`:

| Page | What changes |
|---|---|
| `book/src/setup.md` | both `volt --version` outputs, "This book describes Volt 0.1.3.", both `volt doctor` outputs |
| `book/src/tour/install.md` | "recorded with Volt 0.1.3", the `volt --version` output, both `volt doctor` outputs |
| `book/src/tour/new-check-build.md` | "recorded with Volt 0.1.3", the `// Version:` line of the generated SV |
| `book/src/limitations.md` | "Known bugs": the entries of the bugs 0.1.3 fixes leave; every open bug a user of 0.1.3 can hit has an entry, with its issue and a workaround |

Output in the book comes from a real run on the system the page names,
never typed by hand (AGENTS.md, "Book writing rules"). Build `volt` from
the commit of step 1a and record each output block again with it:

```powershell
cargo build -p volt-driver
$env:PATH = "$PWD\target\debug;$env:PATH"
volt --version
```

`volt --version` prints `volt 0.1.3 (<commit>)`, where `<commit>` is the
short hash of the commit of step 1a (`git rev-parse --short HEAD`).

`volt doctor` is recorded twice on each of the two pages, from the home
folder (the pages replace the user name with `you`): once with Docker
Desktop running, and once with Docker Desktop installed but not running.
For the second one, quit Docker Desktop, wait until `docker info` fails,
record, and start it again.

An agent does not quit Docker Desktop on the maintainer's machine. It
records the second output with Docker Desktop running, pointing
`DOCKER_HOST` at a named pipe that does not exist for that one command:

```powershell
Set-Location $HOME
$env:DOCKER_HOST = 'npipe:////./pipe/volt_no_such_docker_engine'
docker info *> $null; "docker info exit=$LASTEXITCODE"   # 1: the daemon is not reachable
volt doctor
Remove-Item Env:DOCKER_HOST
```

`volt doctor` asks the daemon through `docker info`, so the code path is
the same as with Docker Desktop stopped: the `test, run`, `verify` and
`docker` lines say that the daemon is not running. The pull request says
that the output was recorded this way. When an image is not downloaded on the
recording machine, `volt doctor` prints `image not downloaded yet` under
its line; record the output as it is.

A page
whose output is recorded again for another reason also gets the line
"recorded with Volt 0.1.3"; a page that is not recorded again keeps its
line. `docs/roadmap.md` is not part of the release pull request.

### 1e. Check, commit, open the pull request

```powershell
just check
just consistency
just clippy-strict
git add CHANGELOG.md README.md docs/release-notes/v0.1.3.md `
    book/src/setup.md book/src/tour/install.md book/src/tour/new-check-build.md `
    book/src/limitations.md
git commit -m "chore(release): Volt 0.1.3"
git status --short
```

Add every other book page you recorded again to the `git add` line.
`git status --short` prints nothing: no change is left out of the
commit. Then write the pull request's description into a file (it says
which ADR allows each behavior change), push and wait for CI:

```powershell
git push -u origin chore/release-0.1.3
gh pr create --title "chore(release): Volt 0.1.3" --body-file (Join-Path $env:TEMP 'release-0.1.3-pr.md')
gh pr checks --watch
```

A dry run of the branch (`gh workflow run release.yml --ref
chore/release-0.1.3`) finds a version that does not agree before the
merge. The maintainer merges the pull request when CI is green.

## 2. A dry run of the merged commit

```powershell
git switch main
git pull --ff-only
gh workflow run release.yml --ref main
Start-Sleep 10   # the new run takes a few seconds to appear
$run = gh run list --workflow release.yml --branch main --limit 1 --json databaseId --jq '.[0].databaseId'
gh run watch $run --exit-status
gh run view $run --log | Select-String 'Smoke test [a-z0-9_-]+: volt|notice\]Dry run of Volt'
gh run view $run --json headSha --jq .headSha
git rev-parse HEAD
```

Four `Smoke test <target>: volt 0.1.3 (<commit>)` lines, one per
platform, and the two commit hashes are the same. The run's summary page
says "Skipped: the publish job".

## 3. Tag and push

The tag goes on the commit the dry run tested:

```powershell
git fetch origin
git tag -a v0.1.3 -m "Volt 0.1.3" origin/main
git push origin v0.1.3
```

The tag is exactly `v0.1.3`, no suffix (ADR-0100). The push starts
`release.yml` (release run) and `book.yml` (the book's root moves to
v0.1.3).

The book's root moves at this push, not when the GitHub Release is
published in step 6: `book.yml` puts the book of the latest release tag
at https://volthdl.org/, whether that tag has a published Release or
not (observed with v0.1.3). From the push until step 6, the root
describes 0.1.3 while the install scripts, which take
`releases/latest`, still install 0.1.2. Go through steps 4 to 6 without
a break; if the release run fails, delete the tag ([Undo](#undo-a-wrong-tag-before-publishing)),
which also moves the root back.

## 4. Watch the release run

```powershell
$run = gh run list --workflow release.yml --event push --limit 1 --json databaseId --jq '.[0].databaseId'
gh run watch $run --exit-status
gh release view v0.1.3 --json isDraft,name,assets --jq '.name, .isDraft, .assets[].name'
```

Expected: `Volt 0.1.3`, `true` (a draft), and eight files:
`volt-x86_64-pc-windows-msvc.zip`, `volt-x86_64-unknown-linux-musl.tar.gz`,
`volt-x86_64-apple-darwin.tar.gz`, `volt-aarch64-apple-darwin.tar.gz`,
`volt-hdl-0.1.3.vsix`, `install.sh`, `install.ps1`, `SHA256SUMS`.

If the run fails before `publish`, nothing was published: fix it on
`main`, delete the tag (step "Undo" below) and start again from step 2.

## 5. Check the draft's files

Download them into an empty folder:

```powershell
$dir = Join-Path $HOME 'volt-0.1.3-check'
New-Item -ItemType Directory -Force $dir | Out-Null
Set-Location $dir
gh release download v0.1.3 -R volt-hdl/volt
```

Check every file against `SHA256SUMS`, and that the list and the folder
hold the same files:

```powershell
$failed = 0
foreach ($line in Get-Content SHA256SUMS) {
    if ($line -notmatch '^([0-9a-f]{64}) [ *](.+)$') { Write-Host "unreadable line: $line"; $failed++; continue }
    $expected = $Matches[1]
    $name = $Matches[2]
    if (-not (Test-Path -LiteralPath $name)) { Write-Host "${name}: MISSING"; $failed++; continue }
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $name).Hash.ToLower()
    if ($actual -eq $expected) { Write-Host "${name}: OK" } else { Write-Host "${name}: FAILED"; $failed++ }
}
$listed = @(Get-Content SHA256SUMS).Count
$present = @(Get-ChildItem -File | Where-Object Name -ne 'SHA256SUMS').Count
if ($listed -ne $present) { Write-Host "SHA256SUMS lists $listed files, the folder has $present"; $failed++ }
"$failed problem(s)"
```

Expected: seven `OK` lines and `0 problem(s)`.

Check the provenance attestation of every file but `SHA256SUMS`:

```powershell
$failed = 0
foreach ($f in Get-ChildItem -File -Name | Where-Object { $_ -ne 'SHA256SUMS' }) {
    gh attestation verify $f -R volt-hdl/volt
    if ($LASTEXITCODE -ne 0) { Write-Host "${f}: attestation NOT verified"; $failed++ }
}
"$failed attestation problem(s)"
```

Expected: `0 attestation problem(s)`. Files from a dry run have no
attestation, so the same command reports a problem for each of them.

## 6. Publish

Read the draft's notes on the Releases page, then:

```powershell
gh release edit v0.1.3 --draft=false --latest
```

Do not mark it as a pre-release: the install scripts and the book's links
use `releases/latest`, which ignores drafts and pre-releases (ADR-0096).

## 7. After publishing

```powershell
gh workflow run install.yml
gh run list --workflow book.yml --limit 2
```

The `One-line install` jobs of `install.yml` now install 0.1.3 instead of
0.1.2. The book run that the tag push started in step 3 has already put
the v0.1.3 book at https://volthdl.org/; publishing does not rebuild it.
The `book.yml` list shows that run and the run on `main` it started.

## Undo (a wrong tag, before publishing)

```powershell
gh release delete v0.1.3 --yes
git push origin :refs/tags/v0.1.3
git tag -d v0.1.3
gh workflow run book.yml --ref main
```

The last command rebuilds the site: the root goes back to the book of
the latest release tag that is left, v0.1.2 (ADR-0100 §6). A provenance attestation, once written, stays in the public
log; it names the commit, and a later release of the same version from a
different commit gets its own.

## Appendix: the release commit of v0.1.0

Step 1 as it was run for v0.1.0, the earliest release, kept as a record.
It does not apply to a later release: the F0 heading and the "no release
yet" texts are gone, and step 1 above replaces it.

Start from an up-to-date `main`:

```powershell
git switch main
git pull --ff-only
git switch -c chore/release-0.1.0
```

In `CHANGELOG.md`, the `## [Yayımlanmadı]` heading becomes
`## [0.1.0] - <today>`. The old phase heading
`## [0.1.0] - 2026-09-03 — F0 tamamlandı` was a development phase, not a
release; rename it so that `[0.1.0]` appears once:

```powershell
$f = Join-Path $PWD 'CHANGELOG.md'
$t = [IO.File]::ReadAllText($f)
$t = $t -replace '(?m)^## \[0\.1\.0\] - 2026-09-03 .+$', '## F0 aşaması - 2026-09-03 — sürüm değil'
$t = $t -replace '(?m)^## \[Yay\S+\]$', "## [0.1.0] - $(Get-Date -Format yyyy-MM-dd)"
[IO.File]::WriteAllText($f, $t)
git diff --stat
```

The same in Git Bash:

```sh
sed -i -e "s/^## \[0\.1\.0\] - 2026-09-03 — F0 tamamlandı\$/## F0 aşaması - 2026-09-03 — sürüm değil/" \
       -e "s/^## \[Yayımlanmadı\]\$/## [0.1.0] - $(date +%F)/" CHANGELOG.md
```

`git diff --stat` shows `CHANGELOG.md | 4 ++--`: two headings, nothing
else. Then, under `### Behavior changes` of the new `[0.1.0]` section,
collect the entries marked "**Davranış değişikliği.**" further down
(ADR-0100 §3), by hand.

### README and roadmap: the "no release yet" texts

Four places in `README.md` and one section of `docs/roadmap.md` say that
no release exists. Change them in the same commit.

**a. README, status note.** Replace:

```markdown
> **Status:** early-stage project (started 2026-09). No release has been
> published yet: build Volt from source ([Installation](#installation)).
> Not used in production. No silicon. See [Limitations](#limitations).
```

with:

```markdown
> **Status:** early-stage project (started 2026-09). Version 0.1.0 is the
> earliest release; it makes no stability promise. Not used in production.
> No silicon. See [Limitations](#limitations).
```

**b. README, Installation.** Delete this paragraph (below the Linux and
macOS command):

```markdown
**No release has been published yet.** Until then, both commands print
`No Volt release has been published yet.` with the command that builds
Volt from source, and install nothing. Use **From source** below.
```

**c. README, Installation, VS Code.** Replace:

````markdown
**VS Code:** until the earliest release, package the extension from
source (Node.js 18 or later):

```sh
cd editors/vscode
npm ci
npx vsce package
code --install-extension volt-hdl-0.1.0.vsix
```

The extension starts
````

with:

```markdown
**VS Code:** download `volt-hdl-<version>.vsix` from the
[latest release](https://github.com/volt-hdl/volt/releases/latest) and
install it with `code --install-extension volt-hdl-<version>.vsix`. The
extension starts
```

**d. README, Limitations.** Replace:

```markdown
- **The VS Code extension is not published.** Until the earliest
  release, package it from `editors/vscode/` (see
  [Installation](#installation)); it is not on the Visual Studio
  Marketplace or Open VSX.
```

with:

```markdown
- **The VS Code extension is not on the Visual Studio Marketplace or
  Open VSX.** Each release carries it as a `.vsix` file.
```

**e. Roadmap.** Finished items leave `docs/roadmap.md`. Delete the whole
`## Toward v0.1` section: from its heading down to, not including,
`## Next`.

Then nothing that says "no release" is left. This command prints nothing:

```powershell
Select-String -Path README.md, docs/roadmap.md -Pattern 'published yet|until the earliest release|Toward v0.1'
```

Check, commit, open the pull request and wait for CI:

```powershell
just check
just consistency
git add CHANGELOG.md README.md docs/roadmap.md
git commit -m "chore(release): Volt 0.1.0"
git push -u origin chore/release-0.1.0
gh pr create --fill
gh pr checks --watch
```

Merge the pull request when CI is green.
