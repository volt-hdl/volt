//! error-recovery.md §8.2: parser hiçbir girdide panik etmemeli.
//!
//! İş parçacığı açmadan, libFuzzer'ın ana iş parçacığında ayrıştırır
//! (`parse_on_current_stack`, ADR-0080 §6): `parse` her çağrıda 64 MB'lık
//! bir iş parçacığı açar (Linux ~160 µs — küçük bir girdinin kendisinden
//! pahalı). Ana iş parçacığı Linux'ta 8 MB; sınırdaki girdi debug'da bile
//! 2 MB'tan azını ister. Bu yüzden derinlik korumasındaki bir gerileme
//! burada 64 MB'ın arkasına saklanmaz, fuzz'da yığın taşması olarak çıkar.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        // ASLA panik etmemeli
        let _ = volt_syntax::parse_on_current_stack(volt_span::FileId(0), source);
    }
});
