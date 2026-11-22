# Contributing to Volt

Thank you for helping with Volt. This page tells you how a change gets from
an idea to `main`. The rules every change follows, written for people and
for AI coding agents alike, are in [AGENTS.md](AGENTS.md); this page does
not repeat them, it points to them. How the project is run (roles, areas,
decisions, releases) is in [GOVERNANCE.md](GOVERNANCE.md).

## What Volt promises, and what that asks of a change

Volt's promise is that "0 errors" can be trusted: a design the compiler
accepts computes what the specification (`docs/spec/`) says, and an unsafe
design (an unsynchronized clock-domain crossing, a secret reaching a public
output, a combinational valid-from-ready path) is rejected. A change that
makes the compiler accept a wrong design without a word is the worst kind
of bug we have; the issues for it are titled `silent-wrong:`.

So every change, however small, comes with evidence:

- **A failing test before the fix.** The test comes before the fix: you
  run it and see it fail without the fix, then fix the code and see it
  pass (AGENTS.md, "Tests: a failing test before the fix").
- **Behavior, not text.** Tests check what a design does (simulation, an
  equivalence check, the diagnostics a user sees through the command they
  run), not the exact text the compiler prints or emits.
- **Results from the logs.** CI results are read from the job logs and
  copied, not summarized from memory (AGENTS.md, "Verifying CI from the
  logs").

## Ways to help

- **Report a bug.** Include the smallest `.volt` file that shows it, the
  command, what you expected and what happened. If the compiler accepted a
  design that computes the wrong thing, say so in the title
  (`silent-wrong:`).
- **Pick an issue.** Issues labelled `good first issue` are small and
  self-contained. Comment on the issue before you start, so that two
  people do not do the same work.
- **Improve the book.** The book is for people who are new to hardware
  design; see [book/CONTRIBUTING-BOOK.md](book/CONTRIBUTING-BOOK.md).
- **Propose a language change.** Start with an issue. A change to a rule
  of the language becomes an ADR (below); the maintainer decides.

## From an issue to a merged pull request

1. **Agree on the issue.** The issue says what is wrong or missing and
   which area it belongs to. For a change of a rule, the decision comes
   before the code: an ADR with the status `Önerildi`, approved by the
   maintainer. An ADR that adds or changes a rule of the language carries
   a short "İlke denetimi" section (ADR-0119): does the rule ask the user
   to declare something, does it infer something, does it change an
   existing meaning, does it guess intent.
2. **Branch from an up-to-date `main`**: `<type>/<short-name>`
   (AGENTS.md, "Branches, commits and pull requests").
3. **Write the failing tests before the fix**, run them and see them fail.
   When a red run of the normal CI is the evidence, push the tests alone
   from an `evidence/` branch, quote the red run's log lines in your pull
   request, then delete the branch (AGENTS.md, "Branches, commits and pull
   requests"). A fix for a `silent-wrong` issue also
   adds the issue's smallest reproduction to `tests/real_bugs/` and removes
   the issue's entries from the metamorphic test's list of known failures
   (ADR-0117; AGENTS.md, "Real-bug corpus and metamorphic test").
4. **Fix**, keeping the change inside one area where you can. If the fix
   needs a change in another area, say so in the issue before you start.
   A rule is computed in one place: if your change needs a semantic fact
   that is already computed elsewhere, read it from there instead of
   computing it again (GOVERNANCE.md, "One place per rule").
5. **Run the local gates**: `just check`, `just consistency`,
   `just clippy-strict` (AGENTS.md, "Build and test"). Besides format,
   lint and tests, `just check` checks the mutation exclusions of
   `.cargo/mutants.toml` when the cargo-mutants version that `mutants.yml`
   pins is installed, and says that it skipped them otherwise; CI always
   checks them (ADR-0133).
6. **Open the pull request.** The description says what changed and why,
   which issues it closes (`Closes #N`), the evidence (the tests that
   failed before the fix and pass after it, and the CI results quoted from
   the logs), every behavior change, and what was left out.
7. **The mutation gate runs by itself** (ADR-0109, "Mutation gate" in
   AGENTS.md). It tries small deliberate bugs (mutants) on the Rust lines
   your pull request changes and runs the tests against each one. Every
   mutant the tests miss is classified in your pull request: a test for a
   blind spot (B), a proven exclusion for an equivalent mutant (A1, A2),
   or an issue for a suspected bug (C). The gate can take an hour or two on
   a large change; a later push that changes no Rust and no file the tests
   read reuses the result of the earlier run.
8. **Review.** The owner of the area reviews. Changes to `docs/spec/`, to
   ADRs and to release files are reviewed by the maintainer.
9. **Merge.** A maintainer merges, after the required checks pass. Agents
   never merge, tag or release (AGENTS.md).

## Documents you will meet

| Document | What it is | Who changes it |
|---|---|---|
| `docs/spec/` | The language specification: what a design means. English; a Turkish translation is in `docs/spec/tr/` | The maintainer, applying reviewed changes; contributors and agents do not edit it (AGENTS.md, "Areas you do not edit"). A new or changed rule starts as an ADR. |
| `docs/adr/` | Decision records: why a rule is what it is | New ADRs through pull requests; decision text never changes once approved |
| `AGENTS.md` | The rules for every change | The maintainer |
| `CHANGELOG.md` | What changed for users, release by release | Every pull request with a user-visible change |
| `docs/release-notes/` | The notes of each release | The release pull request (`RELEASING.md`) |
| `book/` | The guide for users | Anyone, following the book rules |
| `docs/roadmap.md` | Where Volt is going | The maintainer |

When two documents disagree, the order of precedence in AGENTS.md
("Documents and their precedence") decides.

## Working with AI coding agents

Many changes to Volt are written with an AI coding agent. That is
welcome, and the same rules hold: the agent follows AGENTS.md, the person
who runs the agent is responsible for the pull request, and a person
reviews it. Run one agent session per working copy; use git worktrees for
parallel sessions. Agents do not merge, tag, release, edit `docs/spec/` or
`docs/research/`, or change an approved ADR's decision text.

## Language

Code, commit subjects, pull request titles, issue titles, release notes
and the specification are in English. ADRs and their index are in Turkish
today; an English summary index is planned (see the roadmap). Commit and
pull request bodies may be in Turkish (AGENTS.md).

## Conduct

Be kind, be precise, and assume good intent. Disagree about the design,
not about the person. Report unacceptable behavior to the maintainer.

## License

By contributing you agree that your contribution is licensed under the
terms in [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT),
like the rest of Volt.
