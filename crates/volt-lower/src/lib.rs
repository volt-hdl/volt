//! SystemVerilog üretimi: docs/spec/sv-mapping.md eşlemesine göre alçaltma.

// ADR-0098: enum'a yeni varyant eklenince ele alınmayan her yer derleyici uyarısıyla görünsün.
#![warn(clippy::wildcard_enum_match_arm)]
