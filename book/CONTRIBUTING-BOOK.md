# Writing for the Volt book

This guide covers the book in `book/`. It is checked by
`book/tools/check_book.py` in CI (workflow `book.yml`): every rule marked
**(checked)** fails the build when broken.

## Readers

The book has two audiences at once:

- people new to hardware who have written software, and
- FPGA and ASIC engineers who know SystemVerilog.

The main text serves both. Extra detail for one group goes into a side
box (below). Neither group should feel the book was written for the
other.

## Tone

- Plain English. Short sentences, one idea each. Present tense, active
  voice, "you" for the reader, "we" for the book.
- Show, then explain: code or a command, then what it does.
- Define a term before using it, or link the glossary.
- State limits plainly. If Volt cannot do something, say so and point to
  the workaround.
- No hype, no jokes about other languages. Compare with SystemVerilog by
  facts: what you write there, what you write here, what changes.
- Numbers come from measurements, with the environment they were measured
  in ("45 s on a home connection", "Docker Desktop on Windows 11").

## Words we do not use (checked)

The same rule as the README: no marketing superlatives and no claims we
cannot back with a source. The checker rejects these words in prose
(outside code and inline code):

`revolutionary`, `only`, `first`, `unique`, `unprecedented`,
`game-changing`, `best-in-class`, `world-class`, `blazing`, `effortless`.

`only` and `first` are banned even in harmless uses, so that no claim of
being the sole or earliest tool slips through. Rewrite instead:

| Instead of | Write |
|---|---|
| `only one clock` | "a single clock", "just one clock" |
| `the first test` | "the earliest test", "test 1", "the test above" |
| `first, install ...` | "start by installing ...", "step 1: install ..." |
| `read-only` (prose) | "cannot be written", or put it in `code` |

## Chapter layout

1. `# Title`: a noun phrase ("A counter", "Two clocks and CDC").
2. A goal paragraph of two or three sentences, "In this chapter you will
   ...", inside `<div class="chapter-goal">` (blank lines around the
   Markdown, as in a side box).
3. Sections that each build one step of the design.
4. A short recap and a link to the next chapter.

Every chapter must be listed in `src/SUMMARY.md` (checked).

A planned chapter keeps its title and goal paragraph and lists existing
examples on the same topic.

## Side boxes

Two kinds, styled in `theme/volt.css` (solid green border and double blue
border, so they differ without colour too):

```html
<div class="box new-to-hw">

**New to hardware?** Clocks and flip-flops

Text in Markdown. The blank lines around it are required.

</div>

<div class="box from-sv">

**Coming from SystemVerilog?** Where is `always_ff`?

Text in Markdown.

</div>
```

Rules:

- **Skippable.** The main text must read correctly with every box
  removed. Never put a required step, a command or a code change in a
  box.
- **"New to hardware?"** explains a basic idea the main text relies on
  (clock, flip-flop, reset, metastability, testbench). No SystemVerilog in
  it.
- **"Coming from SystemVerilog?"** has three parts: the SystemVerilog you
  would write, what Volt does differently, and why. Keep the SV short and
  correct; it is read by people who know it well.
- One topic per box; after the bold label, a few words name the topic.
- At most one box of each kind per section.

## Code blocks (checked)

Every ```` ```volt ```` block is compiled with `volt check`. It must pass
with no error and no warning unless an attribute says otherwise.
Attributes follow the language, separated by commas:

| Info string | Meaning |
|---|---|
| ```` ```volt ```` | a complete file; must pass `volt check` cleanly |
| ```` ```volt,file=counter.volt ```` | also saved as `counter.volt` in the workspace of the part (one per directory: `tour/`, `tutorial/`, ..., filled in `SUMMARY.md` order, like the reader's project folder): a later `counter_test.volt` block finds its design; a later block with the same name replaces it |
| ```` ```volt,should_fail=E3001 ```` | wrong on purpose: `volt check` must fail with exactly `E3001` |
| ```` ```volt,should_warn=W2010 ```` | must pass with exactly the warning `W2010` |
| ```` ```volt,file=x_test.volt,test_fails ```` | a test file whose test is meant to fail (the checker expects `volt test` to exit 5) |
| ```` ```volt,from=templates/minimal/counter.volt ```` | must be identical to that repository file, so a template shown in the book cannot drift from the real one |

With `--run-tests` (on in CI), every block saved as `*_test.volt` also
runs `volt test`.

- Show complete files. If a chapter changes one line, show the whole file
  again with the same `file=` name, or show the change as a `diff` block
  (not checked) followed by the full file.
- Say in the text when a block is wrong on purpose.
- Other languages (`console`, `text`, `systemverilog`, `toml`, `diff`) are
  not checked.

## Command output

- Copy output from a real run; never type it by hand. Keep the command
  prompt `$ ` in `console` blocks.
- Name the environment in the chapter (operating system, how Verilator
  ran). The Tour was recorded with Volt 0.1.0 on Windows 11 with Docker
  Desktop.
- Mark shortened output with `...` and say "(output trimmed)" in the
  text.

## Terms

Use the terms of [`docs/spec/GLOSSARY.md`](../docs/spec/GLOSSARY.md). Add
every hardware term the book introduces to `src/glossary.md`.

## Screenshots

Put images in `src/images/`, give them `class="screenshot"` and alt text
that says what the image shows. Say how the screenshot was made (for
example, GTKWave 3.3.116 on Linux).

## Building and checking locally

```console
$ cargo build -p volt-driver --bin volt
$ python3 book/tools/check_book.py --volt target/debug/volt
$ python3 book/tools/check_book.py --volt target/debug/volt --run-tests   # needs Verilator; with Docker add --backend docker
$ python3 book/tools/check_book.py --volt target/debug/volt --self-test   # the checker's own tests
$ mdbook serve book --open
```
