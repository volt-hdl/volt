//~ E2025
// #418: `bits<65536>` passed E2025 (its range was 1..=65536) while the type
// checker accepted widths up to 65535 only and turned the port into the
// error type without a diagnostic. The assignment to `bits<8>` then
// compiled with no E2001 and the SV truncated it (`assign y = 8'(x);`),
// dropping 65528 bits. The maintainer's decision in #418: one limit,
// 65535, in one shared constant read by the constant evaluator and the
// type checker; `bits<65536>` is E2025 (const-eval.md, E2025). Fixed by
// the pull request that closes #418.

module W {
    in  x : bits<65536>
    out y : bits<8>
    y = x
}
