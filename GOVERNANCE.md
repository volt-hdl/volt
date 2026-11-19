# How Volt is run

This page describes the roles, the areas of the compiler, how decisions
are made and how releases happen. It is written for a small team of
people, each of whom may also work with AI coding agents. Today Volt has
one maintainer; the structure below is ready for more people and is
filled in as they join.

## Roles

**Maintainer.** Owns the language and the product: the specification,
the decisions recorded in ADRs, the priorities, the milestones and the
releases. Approves every change to `docs/spec/`, every new ADR, and every
release. Merges pull requests, pushes tags and publishes releases.

**Area owner.** Owns one area of the compiler (below). Reviews every pull
request that touches the area, keeps its issues triaged, and knows where
its rules are computed. An area may have more than one owner; one person
may own more than one area.

**Contributor.** Anyone who opens an issue or a pull request. A
contributor who has made several reviewed changes in an area can become
an owner of that area; the maintainer asks.

**AI coding agents** work for the person who runs them. They follow
AGENTS.md, open pull requests, and never merge, tag, release or edit the
specification.

## Areas

| Area | What it covers | Main paths |
|---|---|---|
| Language and specification | The rules of the language, ADRs, decision sessions | `docs/spec/`, `docs/adr/` |
| Front end | Source positions, syntax tree, lexer, parser, desugaring, generic expansion, name resolution | `crates/volt-span/`, `crates/volt-ast/`, `crates/volt-syntax/`, `crates/volt-hir/src/resolve/` |
| Analysis | Types and widths, clock domains (CDC), trust, timing, handshake, drivers, module summaries | `crates/volt-hir/` |
| Back end | Lowering, SystemVerilog, constraints (SDC/XDC), drivers and register maps, simulation and formal glue | `crates/volt-lower/`, `crates/volt-sv-emit/`, `crates/volt-sdc-emit/`, `crates/volt-sw-emit/`, `crates/volt-tools/` |
| Diagnostics and learning | Diagnostic messages and `volt explain` texts in English and Turkish, the book, examples, the learning goal of the roadmap | `crates/volt-diagnostics/`, `book/`, `examples/` |
| Command line and editors | The `volt` binary, the language server, editor support | `crates/volt-driver/`, `crates/volt-lsp/`, `editors/` |
| Quality infrastructure | Mutation gate, differential test, metamorphic test, real-bug and seeded-bug corpora, fuzzing, CI | `crates/volt-difftest/`, `tests/`, `fuzz/`, `.github/`, `scripts/ci/`, `.cargo/mutants.toml` |
| Release | Release workflow, installers, release notes | `RELEASING.md`, `scripts/release/`, `docs/release-notes/` |

When areas have owners, `.github/CODEOWNERS` maps these paths to them,
so that a pull request asks the right people for review automatically.
Until then every area is owned by the maintainer.

## How the areas work together

- **The one who finds is not the one who fixes.** The quality
  infrastructure and independent reviews find bugs; the area owner fixes
  them; a reviewer who did not write the change checks it. No change
  grades itself.
- **Every bug found makes the tools stronger.** The reproduction of each
  fixed `silent-wrong` bug goes into the real-bug corpus
  (`tests/real_bugs/`), which the metamorphic test reads on every pull
  request and every night.
- **One place per rule.** A semantic fact (the width of an expression,
  whether a port is a clock, whether a match arm is written to the
  design) is computed in one place and read everywhere else (ADR-0075 §5,
  #235). Most `silent-wrong` bugs so far came from the same rule being
  computed twice and the copies drifting apart: in #216, #328 and #331
  the rule "an arm that earlier arms already cover, such as an arm after
  the wildcard, is never chosen" lived in several
  layers that did not agree. A pull request that computes a fact a second
  time explains why in its description.

## Decisions

- **Small decisions** (how to fix a bug inside the existing rules) are
  made in the pull request and its review.
- **Rule decisions** (what a design means, a new diagnostic, a behavior
  change) are written as an ADR with the status `Önerildi` and listed in
  the issue. An ADR that adds or changes a rule of the language carries an
  "İlke denetimi" section (ADR-0119). A new escape hatch, a place where
  the compiler takes the user's word instead of checking, follows the
  rules of ADR-0118: narrow, deliberate, with a reason, visible and
  reported. The maintainer approves or rejects; an approved ADR gets the
  status `Uygulandı` when its change is merged, or `Kabul edildi` when it
  is accepted but implemented later. Approved decision text never
  changes; a later decision is a new ADR.
- **Decision sessions.** Open rule questions (issues labelled `decision`,
  such as #122) are collected and
  decided in a session about once a month. The outcome is a set of ADRs
  and issues.
- **Disagreement.** Arguments are made in the issue, with examples. When
  the area owner and the contributor disagree, the maintainer decides and
  writes down why.

## Quality gates

| When | What runs | A red result |
|---|---|---|
| Every pull request | CI (build, tests with every tool required, book, consistency), the metamorphic test's PR tier, the mutation gate on the changed Rust lines | is fixed or explained in the pull request before it merges; the mutation gate is informational until the maintainer makes it a required check |
| Every night | The large differential test, the whole-corpus metamorphic test, fuzzing; each night with a new seed | opens or updates an issue for that night (differential and metamorphic test); a fuzzing crash input is kept as an artifact of the run |
| Every week | Install tests of the released binaries | is investigated before the next release |

The tools these gates use are pinned: the reference Verilator image by
digest (ADR-0121), the runner image by version.

## Planning and rhythm

- **Weekly triage.** New issues get an area, a milestone and labels. A
  short check-in says who works on what, so that two people do not take
  the same issue.
- **Milestones.** Patch releases (`v0.1.x`) fix bugs; they change no
  rule of the language and add no option and no diagnostic code
  (ADR-0100). Three narrow exceptions are written down: a fix for a
  silent-wrong result (ADR-0100), a warning that points at every place
  whose meaning changes in the next minor release (ADR-0119), and a change
  of the generated SystemVerilog that keeps every value and makes a Volt
  command work again on a valid design (ADR-0122). Rule changes wait for
  the next minor release (`v0.2.0`).
- **Release train.** Patch releases go out on a fixed rhythm. What is
  merged and green by the cut-off date is in; the rest goes with the next
  train. A release is not held for one more fix unless that fix is a
  silent-wrong result that users are likely to hit.

## Working in parallel

To keep parallel pull requests from colliding:

- Each pull request stays inside one area where it can, and says in the
  issue before it starts when it cannot.
- Pull requests are tested against the latest `main` before they merge
  (a merge queue), so that two green pull requests cannot combine into a
  red `main`.
- Files that every pull request touches are not edited by hand in every
  pull request: changelog entries are separate fragment files, generated
  counters are produced by CI or at release, test fixtures are named after
  their issue, and ADR numbers are reserved before the pull request opens.

The items in this section that are not in place yet are tracked in
[the issue for parallel-work infrastructure](https://github.com/volt-hdl/volt/issues/338);
until then the rules in AGENTS.md
hold as written.

## Security

Report a security problem privately, not in a public issue. What counts
as a security problem, how to report it and what happens next are in
[SECURITY.md](SECURITY.md).
