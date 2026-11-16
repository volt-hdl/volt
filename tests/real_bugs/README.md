# Real-bug corpus

One smallest reproduction per silent-wrong issue: every silent-wrong issue
closed since v0.1.0, and every fix of a silent-wrong issue adds its own
(`AGENTS.md`). The corpus is an input of the real-bug test, of the
metamorphic test and of the differential test's design corpus (#254,
ADR-0117).

## Form

- One file per issue, the number first in its name:
  `<issue>_<short_name>.volt`. An issue that needs several files is a
  directory `<issue>_<short_name>/` whose entry file is `main.volt` (the
  other files are found through `use`, as in `tests/ui/multifile/`).
- The first lines are the expected outcome of `volt check`, from the
  specification, never copied from the compiler:
  - `//~ E2001`, one line per code: the set of error and warning codes
    `volt check` must report, exactly (no other code);
  - `//~ PASS`: no error and no warning.
- Then a comment: `// #<issue>: <what went wrong>`, the spec rule or ADR
  that gives the right answer, and the pull request that fixed it.
- A design whose bug was a wrong value carries `test` blocks (ADR-0033)
  with values computed by hand from the specification, the reasoning in a
  comment next to each value. The real-bug test runs them with
  `volt test` in Verilator (`VOLT_TOOL_BACKEND=local`; the tool job
  requires Verilator, so they never skip there).

## Open issues

The reproduction of a silent-wrong issue that is still open may be added
before its fix, when a check needs a design that shows it. Its failure is
then an entry of the known-failures list
(`crates/volt-driver/tests/metamorphic/known_failures.json`) with the
issue number; the pull request that closes the issue removes the entry
and keeps the file.

## Runner

`crates/volt-driver/tests/real_bug_tests.rs`, through the `volt` binary
(`volt check --format json`, `volt test`).
