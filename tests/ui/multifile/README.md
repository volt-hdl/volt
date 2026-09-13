# Multi-file UI fixtures (ADR-0042)

Each directory is one compilation unit. The entry file is `main.volt`
(or `a.volt` for the cycle case); the others are found through `use`.
The first line of the entry file names the expected outcome:
`//~ PASS` or `//~ EXXXX`. Driven by `crates/volt-driver/src/unit.rs`
tests, `crates/volt-driver/tests/cli_tests.rs` and
`crates/volt-hir/tests/ui_semantic_tests.rs`.
