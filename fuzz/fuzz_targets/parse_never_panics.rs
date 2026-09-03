//! error-recovery.md §8.2: parser hiçbir girdide panik etmemeli.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        // ASLA panik etmemeli
        let _ = volt_syntax::parser::parse(volt_span::FileId(0), source);
    }
});
