//! Tarayıcı arayüzü: tek fonksiyon, JSON metni döner.

use wasm_bindgen::prelude::wasm_bindgen;

/// `source`'u derler; cli-contract.md §5 zarfı + `sv` + `rendered`, JSON
/// metni olarak.
#[wasm_bindgen]
pub fn compile_json(source: &str, lang: &str) -> String {
    crate::compile(source, crate::parse_lang(lang))
        .to_json()
        .to_string()
}

/// Derleyici sürümü (sayfada gösterilir; SV başlığındakiyle aynı).
#[wasm_bindgen]
pub fn version() -> String {
    volt_sv_emit::VOLT_VERSION.to_string()
}
