> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Volt İçin Faydalı GitHub Repoları — Kapsamlı Rehber

> Araştırma tarihi: Ağustos 2026
> Kategorilere göre, kullanım amacı ve öncelik sırasıyla

---

## KATEGORİ 1 — DOĞRUDAN BAĞIMLILIK (Cargo.toml'a girecek)

Bunlar Volt'un derleyicisinde kullanılacak crate'ler.

### 1.1 Sözdizim Katmanı

```
domenicquirl/cstree                          ★★★★★ KRİTİK
  Concrete Syntax Tree kütüphanesi
  rowan'ın fork'u (rust-analyzer yazarlarından)

  Volt için neden ideal:
    Kırmızı düğümler Send + Sync → thread'ler arası paylaşım
    SyntaxNode özel veri tutabiliyor
    Interned string üzerine kurulu → bellek verimli
    no_std uyumlu

  Kullanım: F1'de parser çıktısı
  Lisans: MIT/Apache-2.0

rust-analyzer/rowan                          ★★★★☆ alternatif
  cstree'nin atası, daha yaygın
  Fark: kırmızı katman her gezinmede yeniden oluşuyor
  → cstree tercih edilmeli (persistent, Sync)

maciejhirsz/logos                            ★★★★★ KRİTİK
  Derive makrosu ile lexer üretimi
  #[derive(Logos)] ile enum → hızlı lexer
  Kullanım: F0-F1'de token tanıma

rust-analyzer/ungrammar                      ★★★☆☆
  Gramer tanımından AST tipi üretimi
  rust-analyzer bunu kullanıyor
  Volt için: EBNF'den AST kodu otomatik üretilebilir
```

### 1.2 Hata Mesajları

```
brendanzab/codespan                          ★★★★★ KRİTİK
  codespan-reporting: rustc tarzı hata gösterimi
  Kaynak snippet + altı çizili + öneri satırı
  → UX Anayasası'ndaki 5 parçalı format için temel

  Alternatif: zesterer/ariadne
    Daha güzel görsel çıktı, renkli
    Değerlendirilmeli

dtolnay/thiserror                            ★★★★☆
  Hata tipi türetme (#[derive(Error)])
  VoltError enum'u için

dtolnay/anyhow                               ★★★☆☆
  Uygulama seviyesi hata (volt-driver'da)
  Library'de thiserror, binary'de anyhow
```

### 1.3 Artımlı Hesaplama

```
salsa-rs/salsa                               ★★★★☆ V1'de
  Sorgu tabanlı artımlı hesaplama
  rust-analyzer'ın temeli

  Volt için:
    Modül arayüzü değişmediyse yeniden derleme yok
    LSP'de anlık analiz için şart

  Not: rust-analyzer 2025'te yeni salsa sürümüne geçti
       → paralel değerlendirme ve kalıcılık mümkün oldu

  Kullanım: F5/V1 (MVP'de gerekmez)
```

### 1.4 LSP

```
ebkalderon/tower-lsp                         ★★★★★
  Async LSP framework
  Kullanım: F5'te LSP sunucusu

rust-lang/rust-analyzer                      ★★★★★ REFERANS
  Kod okumak için, bağımlılık değil
  crates/parser, crates/hir-def, crates/ide
  → Mimari deseni buradan öğren
```

### 1.5 CIRCT Köprüsü

```
raviqqe/melior                               ★★★★★ KRİTİK
  MLIR için Rust wrapper
  Sıfırdan FFI yazmaktan kurtarıyor
  Kullanım: F3'te volt-lower crate'i

llvm/circt                                   ★★★★★ ZORUNLU
  Donanım için MLIR dialect'leri
  hw, seq, comb, sv, firrtl, handshake
  Volt'un arka ucu

  İzlenecek dialect'ler:
    seq → register (seq.firreg)
    hw  → modül yapısı
    comb → kombinasyonel mantık
    sv  → SystemVerilog özel yapılar
```

### 1.6 Test Altyapısı

```
mitsuhiko/insta                              ★★★★★
  Snapshot testleri
  cargo insta review ile onaylama
  → SV çıktısı testleri için ideal

BurntSushi/quickcheck veya proptest          ★★★☆☆
  Property-based testing
  "Her geçerli Volt kodu geçerli SV üretir"
```

### 1.7 CLI ve Yapı

```
clap-rs/clap                                 ★★★★★
  Argüman ayrıştırma (derive API)

casey/just                                   ★★★★☆
  Make alternatifi, komut runner
  justfile: just test, just weekly, just ui

serde-rs/serde + toml                        ★★★★★
  Volt.toml ayrıştırma
```

---

## KATEGORİ 2 — REFERANS UYGULAMALAR (Kod okuma)

Bağımlılık değil, öğrenme kaynağı. **Bunları okumak haftalar kazandırır.**

### 2.1 Rakip/Kardeş HDL'ler

```
spade-lang/spade (GitHub mirror)             ★★★★★ EN ÖNEMLİ
  Rust ile yazılmış bağımsız HDL
  Lisans: EUPL-1.2 (dikkat — GPL benzeri copyleft)

  Ne öğrenilecek:
    Lineer tip implementasyonu (Volt'a eklenecek)
    Pipeline stage referansları
    Swim build sistemi tasarımı
    Hata mesajı kalitesi

  UYARI: EUPL-1.2 → kodu kopyalama, sadece OKU
         Cleanroom uygulama yap

veryl-lang/veryl                             ★★★★★
  Lisans: Apache-2.0 OR MIT ← GÜVENLİ
  938 yıldız, çok aktif

  Ne öğrenilecek:
    Okunabilir SV üretimi (Volt'un hedefi)
    Paket yöneticisi tasarımı
    LSP implementasyonu
    Playground (WASM) yapısı
    if_reset soyutlaması

  Bu repo Volt'un en yakın teknik emsali

pc2/sus-compiler                             ★★★★☆
  Rust ile HDL, latency counting
  Lisans: kontrol edilmeli

clash-lang/clash-compiler                    ★★★☆☆
  Haskell ile, CDC tip sisteminde
  Ne öğrenilecek: Signal<dom,a> semantiği
  Kod okumak zor (Haskell)

cucapra/filament                             ★★★★★
  Lisans: Apache-2.0 ← GÜVENLİ, kullanılabilir
  Timeline tipler (Volt L2'nin temeli)
  PLDI 2023

  Ne öğrenilecek:
    Interval tip sistemi
    Kaynak çakışması analizi
    Calyx'e lowering

calyxir/calyx                                ★★★★☆
  Lisans: MIT ← GÜVENLİ
  Orta seviye IR, veri yolu + kontrol ayrımı
  ASPLOS 2021
  → Volt HIR tasarımı için referans
```

### 2.2 Derleyici Mimarisi

```
rust-lang/rust-analyzer                      ★★★★★
  Okunacak dosyalar:
    docs/dev/architecture.md    ← ÖNCE BUNU OKU
    crates/parser/              ← parser deseni
    crates/syntax/              ← CST kullanımı
    crates/hir-def/             ← arena + Idx<T>
    crates/ide-diagnostics/     ← hata sunumu

  Mimari değişmezler (Volt'a uyarlanabilir):
    "syntax crate tamamen bağımsız"
    "syntax tree bir değer tipi"
    "syntax tree tek dosya için inşa edilir"
    "syntax tree tasarım gereği eksik, iyi biçimliliği zorlamaz"

rust-lang/rust (compiler/)                   ★★★☆☆
  Hata kodu sistemi (E0308 formatı)
  compiler/rustc_errors/

bytecodealliance/cranelift                   ★★★☆☆
  IR tasarımı, kod üretimi
  Volt'un IR'ı için ilham
```

---

## KATEGORİ 3 — EKOSİSTEM ARAÇLARI (Entegrasyon hedefi)

Volt'un çıktısını tüketecek veya birlikte çalışacak araçlar.

```
YosysHQ/yosys                                ★★★★★ ZORUNLU
  Sentez. Volt → SV → Yosys → netlist
  Yüzlerce akademik ve ticari tape-out'ta kullanıldı

verilator/verilator                          ★★★★★ ZORUNLU
  Hızlı SV simülatör
  Google, Tesla ve çok sayıda yarı iletken şirketinde
  ölçekli RTL simülasyonu için kullanılıyor
  → F0'dan itibaren CI'da

YosysHQ/sby (SymbiYosys)                      ★★★★★
  Formal doğrulama front-end
  Volt assert/invariant → SVA → sby
  Çok milyon döngülük simülasyon paketlerinden
  kaçan hataları yakalıyor

cocotb/cocotb                                ★★★★★
  Python testbench framework
  Büyük IP satıcıları ve EDA şirketleri tarafından
  benimsendi — Python testbench'ler birçok doğrulama
  görevi için UVM'den daha üretken

  Volt entegrasyonu: tip-farkında köprü (Spade modeli)

The-OpenROAD-Project/OpenROAD               ★★★★☆
  RTL-to-GDS akışı
  Volt → SV → Yosys → OpenROAD → GDSII

efabless/openlane2                           ★★★★☆
  OpenROAD tabanlı otomatik ASIC akışı
  Tiny Tapeout bunu kullanıyor

gtkwave/gtkwave                              ★★★☆☆
  Dalga formu görüntüleyici
  Volt VCD/FST üretiyor

ekiwi/surfer (veya gitlab)                    ★★★★☆
  Spade'in doğurduğu dalga formu görüntüleyici
  Chisel topluluğu da benimsedi
  → Volt desteği eklenebilir (tip-farkında gösterim)
```

---

## KATEGORİ 4 — KEŞİF LİSTELERİ (Awesome repos)

Bu listeler yeni araç bulmak için tarama noktası:

```
drom/awesome-hdl                             ★★★★★
  HDL'ler için detaylı liste
  → Yeni rakip/emsal takibi

TM90/awesome-hwd-tools                       ★★★★☆
  Açık kaynak donanım tasarım araçları
  Kategoriler: sentez, simülasyon, layout, PDK

ben-marshall/awesome-open-hardware-verification ★★★★★
  Doğrulama araçları ve framework'leri
  cocotb-coverage, PyVSC, riscv-formal

aolofsson/awesome-opensource-hardware        ★★★★☆
  Andreas Olofsson (Adapteva/Zero ASIC)
  Jeneratörler ve yeniden kullanılabilir tasarımlar

jpc-lip6/awesome-hardware-tools              ★★★☆☆
  LIP6 laboratuvarından, akademik odaklı

garden-of-eda.com                            ★★★★☆
  Web sitesi + repo listesi
  Aktif güncellenen EDA ekosistemi haritası
```

---

## KATEGORİ 5 — VOLT'A ÖZGÜ FIRSATLAR

Konuşmadaki vizyonla doğrudan bağlantılı repolar:

### 5.1 Ternary / Düşük Kesinlik

```
microsoft/BitNet                             ★★★★★
  b1.58 ternary modeller + bitnet.cpp
  Volt'un Trit tipini gerçek modelle test etmek için
  → Referans tasarım için altın vektör kaynağı

ggerganov/llama.cpp                          ★★★★☆
  GGUF format, kuantizasyon
  Q2_K ≈ ternary benzeri
  → Model → donanım akışı için referans
```

### 5.2 RISC-V ve Formal

```
YosysHQ/riscv-formal                         ★★★★☆
  RISC-V CPU tasarımları için yeniden kullanılabilir
  formal doğrulama framework'ü
  Yosys/SymbiYosys kullanıyor
  → Volt formal entegrasyonu için desen

riscv/sail-riscv                             ★★★★☆
  RISC-V'nin formal spesifikasyonu (Sail dili)
  → ISA formal semantiği örneği
  → Volt'un "yürütülebilir spec" vizyonu için emsal

chipsalliance/rocket-chip                     ★★★☆☆
  Chisel ile RISC-V jeneratörü
  → Parametrik tasarım deseni
```

### 5.3 Nöromorfik

```
lava-nc/lava                                 ★★★☆☆
  Intel Loihi framework'ü
  → Spike<T> tasarımı için semantik referans

BrainChip-inc (varsa açık repolar)           ★★☆☆☆
  Akida SDK
```

---

## KATEGORİ 6 — CI/CD VE ALTYAPI

```
actions-rs/toolchain (veya dtolnay/rust-toolchain) ★★★★★
  GitHub Actions Rust kurulumu

Swatinem/rust-cache                          ★★★★★
  CI'da cargo cache → 5-10× hızlanma

taiki-e/cargo-llvm-cov                       ★★★★☆
  Kod kapsamı ölçümü

rustsec/rustsec (cargo-audit)                ★★★★★
  Güvenlik açığı taraması (haftalık)

est31/cargo-udeps                            ★★★☆☆
  Kullanılmayan bağımlılık tespiti

vmunoz82/eda_tools                           ★★★★☆
  Docker imajı: Yosys, SymbiYosys (Z3, boolector,
  yices2), nextpnr, Amaranth, Silice, Verilator
  → CI'da EDA araçları için hazır ortam
```

---

## ÖNCELİK SIRALI OKUMA PLANI

```
BU HAFTA (F0 öncesi):
  1. rust-analyzer/docs/dev/architecture.md      (2 saat)
  2. veryl-lang/veryl → src/ yapısı              (3 saat)
  3. domenicquirl/cstree → README + örnekler     (1 saat)
  4. maciejhirsz/logos → derive örnekleri        (1 saat)

F1 ÖNCESİ:
  5. rust-analyzer/crates/parser/                (4 saat)
  6. veryl-lang/veryl → parser implementasyonu   (3 saat)
  7. spade-lang/spade → hata mesajları           (2 saat)

F2 ÖNCESİ:
  8. cucapra/filament → timeline tipler          (4 saat)
  9. calyxir/calyx → IR tasarımı                 (3 saat)
  10. spade → lineer tip implementasyonu         (3 saat)

F3 ÖNCESİ:
  11. llvm/circt → seq/hw/comb dialect docs      (8 saat)
  12. raviqqe/melior → API örnekleri             (3 saat)
  13. filament → Calyx lowering kodu             (4 saat)

F4 ÖNCESİ:
  14. YosysHQ/sby → SVA entegrasyonu             (2 saat)
  15. cocotb → Python köprüsü                    (2 saat)
  16. YosysHQ/riscv-formal → formal desen        (2 saat)
```

---

## LİSANS UYARI TABLOSU

```
REPO                    LİSANS              VOLT'TA KULLANIM
─────────────────────────────────────────────────────────────────
cstree, logos           MIT/Apache-2.0      ✓ Doğrudan bağımlılık
codespan                Apache-2.0          ✓ Doğrudan
melior                  Apache-2.0          ✓ Doğrudan
salsa                   MIT/Apache-2.0      ✓ Doğrudan
tower-lsp               MIT                 ✓ Doğrudan
insta                   Apache-2.0          ✓ Doğrudan

veryl                   Apache-2.0 OR MIT   ✓ Kod referansı OK
filament                Apache-2.0          ✓ Kod referansı OK
calyx                   MIT                 ✓ Kod referansı OK
rust-analyzer           MIT/Apache-2.0      ✓ Kod referansı OK

spade                   EUPL-1.2            ⚠ COPYLEFT
                                            → SADECE OKU
                                            → Cleanroom uygula
                                            → Kod kopyalama

clash                   BSD-2               ✓ Ama Haskell
circt                   Apache-2.0 + LLVM   ✓ Bağımlılık OK
yosys                   ISC                 ✓ Araç olarak
verilator               LGPL-3 / Artistic   ✓ Araç olarak (link yok)
cocotb                  BSD-3               ✓ Araç olarak
```

---

## ÖZET: EN KRİTİK ON REPO

```
1.  llvm/circt                 → Arka uç, zorunlu
2.  raviqqe/melior             → CIRCT köprüsü
3.  domenicquirl/cstree        → Sözdizim ağacı
4.  maciejhirsz/logos          → Lexer
5.  rust-lang/rust-analyzer    → Mimari referans (kod okuma)
6.  veryl-lang/veryl           → En yakın emsal (Apache/MIT)
7.  cucapra/filament           → Timeline tipler (Apache)
8.  verilator/verilator        → CI'da doğrulama
9.  YosysHQ/sby                → Formal doğrulama
10. brendanzab/codespan        → Hata mesajı formatı

Bu on repo Volt'un teknik temelinin %80'ini oluşturuyor.
```
