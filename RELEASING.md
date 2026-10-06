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

Check, commit, open the pull request and wait for CI:

```powershell
just check
just consistency
git add CHANGELOG.md
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

Download them into an empty folder and check them (Git Bash):

```sh
mkdir -p ~/volt-0.1.0-check && cd ~/volt-0.1.0-check
gh release download v0.1.0 -R volt-hdl/volt
sha256sum -c SHA256SUMS
for f in volt-* install.sh install.ps1; do gh attestation verify "$f" -R volt-hdl/volt; done
```

Every line of `sha256sum -c` ends with `OK`, and every attestation
verifies.

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
