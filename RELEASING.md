# Releasing Volt

The commands the maintainer runs on release day, in order. They are
written for v0.1.0; for a later release, change the version number
everywhere. The release policy is ADR-0100, the workflow ADR-0093 and
ADR-0105; `.github/workflows/README.md` describes what `release.yml`
does.

Agents and contributors do not run steps 3 to 6: tags and releases are the
maintainer's work (AGENTS.md).

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

## Before release day

- `main` is green, and the latest dry run on `main` is green.
- `docs/release-notes/v0.1.0.md` says what the release brings. Its
  "Behavior changes" section lists every change that gives a different
  result for the same input (ADR-0100); `python scripts/release/notes.py
  --check` passes.
- The version is the same in `Cargo.toml` (`[workspace.package]`),
  `crates/volt-sv-emit/src/lib.rs` (`VOLT_VERSION`) and
  `editors/vscode/package.json`. For v0.1.0 it already is `0.1.0`.

## 1. The release commit (a pull request)

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

Four `Smoke test <target>: volt 0.1.0 (<commit>)` lines, one per
platform, and the two commit hashes are the same. The run's summary page
says "Skipped: the publish job".

## 3. Tag and push

The tag goes on the commit the dry run tested:

```powershell
git fetch origin
git tag -a v0.1.0 -m "Volt 0.1.0" origin/main
git push origin v0.1.0
```

The tag is exactly `v0.1.0`, no suffix (ADR-0100). The push starts
`release.yml` (release run) and `book.yml` (the book's root moves to
v0.1.0).

## 4. Watch the release run

```powershell
$run = gh run list --workflow release.yml --event push --limit 1 --json databaseId --jq '.[0].databaseId'
gh run watch $run --exit-status
gh release view v0.1.0 --json isDraft,name,assets --jq '.name, .isDraft, .assets[].name'
```

Expected: `Volt 0.1.0`, `true` (a draft), and eight files:
`volt-x86_64-pc-windows-msvc.zip`, `volt-x86_64-unknown-linux-musl.tar.gz`,
`volt-x86_64-apple-darwin.tar.gz`, `volt-aarch64-apple-darwin.tar.gz`,
`volt-hdl-0.1.0.vsix`, `install.sh`, `install.ps1`, `SHA256SUMS`.

If the run fails before `publish`, nothing was published: fix it on
`main`, delete the tag (step "Undo" below) and start again from step 2.

## 5. Check the draft's files

Download them into an empty folder:

```powershell
$dir = Join-Path $HOME 'volt-0.1.0-check'
New-Item -ItemType Directory -Force $dir | Out-Null
Set-Location $dir
gh release download v0.1.0 -R volt-hdl/volt
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
gh release edit v0.1.0 --draft=false --latest
```

Do not mark it as a pre-release: the install scripts and the book's links
use `releases/latest`, which ignores drafts and pre-releases (ADR-0096).

## 7. After publishing

```powershell
gh workflow run install.yml
gh run list --workflow book.yml --limit 2
```

The `One-line install` jobs of `install.yml` now install 0.1.0 instead of
printing "No Volt release has been published yet". The book run started by
the tag puts the v0.1.0 book at https://volt-hdl.github.io/volt/.

## Undo (a wrong tag, before publishing)

```powershell
gh release delete v0.1.0 --yes
git push origin :refs/tags/v0.1.0
git tag -d v0.1.0
gh workflow run book.yml --ref main
```

The last command puts the book of `main` back at the site root
(ADR-0100). A provenance attestation, once written, stays in the public
log; it names the commit, and a later release of the same version from a
different commit gets its own.
