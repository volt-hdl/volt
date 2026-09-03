# Volt HDL — Specification Index

> **Canonical language: English.**
> Turkish translations live in `tr/`.
> On conflict, the English version wins.

## Documents

| File | Phase | Description |
|---|---|---|
| `GLOSSARY.md` | — | Bilingual terminology (BINDING) |
| `grammar-full.ebnf` | F1 | Complete grammar, LL(2) |
| `operator-precedence.md` | F1 | Precedence and associativity |
| `ast-nodes.md` | F1 | AST node definitions |
| `error-recovery.md` | F1 | Parser error recovery |
| `name-resolution.md` | F1-F2 | Scopes, DefId, forward refs |
| `type-inference.md` | F2 | Bidirectional type checking |
| `domain-inference.md` | F2 | CDC/RDC/PDC inference |
| `const-eval.md` | F2 | Compile-time evaluation |
| `sv-mapping.md` | F0-F3 | Volt → SystemVerilog |
| `cli-contract.md` | F0+ | CLI, exit codes, JSON schema |

## Translation Status

| File | EN | TR |
|---|---|---|
| GLOSSARY.md | ✓ | ✓ (same file) |
| operator-precedence.md | ✓ | ✓ |
| grammar-full.ebnf | ○ | ✓ |
| ast-nodes.md | ○ | ✓ |
| error-recovery.md | ○ | ✓ |
| name-resolution.md | ○ | ✓ |
| type-inference.md | ○ | ✓ |
| domain-inference.md | ○ | ✓ |
| const-eval.md | ○ | ✓ |
| sv-mapping.md | ○ | ✓ |
| cli-contract.md | ○ | ✓ |

✓ done  ○ pending

## Rules

1. English is canonical. Turkish is a translation.
2. Code blocks are **identical** in both — never translated.
3. Error codes, keywords, function names never translated.
4. Follow `GLOSSARY.md` for all terminology.
5. Section numbers must match across languages (§3.2 ↔ §3.2).
6. When updating: change EN first, then TR in the same PR.
