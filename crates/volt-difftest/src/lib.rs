//! Volt aritmetik anlamı ile üretilen SystemVerilog arasında fark ve
//! metamorfik test (ADR-0108).
//!
//! - [`model`]: spesifikasyondan doğrudan yazılmış referans model;
//!   derleyiciden bağımsız (bu crate'in bağımlılığı yok).
//! - [`gen`]: tip yönlü rastgele ifade üreteci (`+ - * ~ & | ^ << >>`,
//!   karşılaştırma, bit/aralık/parça seçimi, `as`, işaretli/işaretsiz,
//!   1–64 bit tam sayı, 1–70 bit `bits<N>`, `Trit`, iç içelik 1–5).
//! - [`harness`]: ifadeleri satır içi, `let`'te, karşılaştırmada ve
//!   sözleşmede; let'e çıkarılmış, ek parantezli ve yer değiştirmiş
//!   biçimleriyle tek modülde toplar; testbench ve vektörler.
//! - [`hygiene`]: R biçimi — ifade bir fn gövdesinde, gövdenin adları
//!   çağıranın döngü değişkeni ve yerelleriyle çakışır (#128).
//! - [`coverage`]: üretecin ürettiği tipler, işlemler ve biçimler; dilin
//!   ifade düzeyindeki listesi ve hiç üretilmeyenler (#458).
//! - [`reject`]: beklenen ret ölçütü — her tipin tanımsız işlemleri tek
//!   tabloda, beklenen tanı koduyla; yalnız `volt check`, Verilator'suz
//!   (#458). Komut: `cargo run -p volt-difftest --example
//!   volt-difftest-reject -- --volt <yol>`.
//! - [`run`]: `volt check` → `volt build` → Verilator → karşılaştırma,
//!   farkların küçültülmesi ve rapor.

pub mod ast;
pub mod coverage;
pub mod gen;
pub mod harness;
pub mod hygiene;
pub mod model;
pub mod print;
pub mod regress;
pub mod reject;
pub mod rng;
pub mod run;
pub mod value;

pub use run::{run, Config, Report, Tools};
