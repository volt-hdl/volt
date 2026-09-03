# Volt HDL — Kodlamaya Başlamadan Önce Atılacak Kritik Adımlar

> Bu belge, ilk `cargo new` komutundan önce tamamlanması gereken
> her şeyi kapsar. Sonradan değiştirilmesi pahalı olan kararlar,
> kurulması gereken altyapı ve kilitlenmesi gereken kurallar.

---

## Neden Önce Bunlar

```
Bir kural değişikliği ne kadar pahalı:

  Dosya ismi typo → git mv → 2 dakika
  Crate sınırı yanlış → yeniden yapılandırma → 2 gün
  Hata kodu formatı yanlış → geriye uyumluluk kırılır → 2 hafta
  CDC semantiği yanlış → tip sistemi yeniden yazılır → 2 ay
  Lisans yanlış → her dosya değişir → hukuki sorun

Erken kilitlenecek kararlar = ilerleyen değişiklik maliyeti.
```

---

## Adım 1 — Yasal Zemin

### 1.1 Lisans Seçimi

```
Karar: Apache 2.0 (birincil) + MIT (opsiyonel dual)

Neden Apache 2.0:
  ✓ Açık patent izni — MIT'de yok, önemli fark
  ✓ Kurumsal güven — "MIT verir ama patent ne olacak?" sorusu kalkar
  ✓ CIRCT de Apache 2.0 → ekosistem tutarlılığı
  ✓ Filament, DAHLIA de Apache 2.0

Neden MIT de ekle (dual):
  ✓ Bazı projeler Apache uyumsuz bağımlılık zorlar
  ✓ "Apache OR MIT" seçim hakkı verir

Stdlib için: MIT veya Apache 2.0 SEÇIM
  → Spade'in stdlib'i permissive ayrı
  → Volt stdlib de MIT olsun (kullanımda sürtünme sıfır)

Yapılacak:
  LICENSE-APACHE → /volt dizinine ekle
  LICENSE-MIT    → /volt dizinine ekle
  Her .rs dosyasına başlık:
  // SPDX-License-Identifier: Apache-2.0 OR MIT
```

### 1.2 Telif Hakkı Politikası

```
Karar: Katkıcı lisans anlaşması (CLA) YOK — başlangıçta

Neden CLA yok:
  CLA katkı engelidir (imza + bürokratik süreç)
  Apache 2.0 + DCO (Developer Certificate of Origin) yeterli

DCO nedir:
  Her commit'in altına: "Signed-off-by: Ad Soyad <email>"
  git commit -s  →  otomatik ekler
  "Bu katkıyı projenin lisansı altında yapma hakkım var"

CONTRIBUTING.md'de yazılacak:
  "Tüm katkılar Apache-2.0 OR MIT lisansı altında."
  "Her commit -s ile imzalanmalı (DCO)."

İleride:
  Volt Foundation kurulursa → CLA gündeme gelebilir
  Şimdilik: DCO yeterli ve katkı dostu
```

### 1.3 İsim ve Marka

```
"Volt" adı:
  Çakışma araştırması: "Volt" marka tescili var mı?
  Google: "Volt HDL" + "Volt programming language" ara
  Volt-lang.org domain müsait mi kontrol et
  
  Risk: başka Volt HDL yoksa güvenli
  Backup isimler düşün (değiştirmek pahalı!)

Domain:
  volt-lang.org → şimdi al (ucuz, değişimi pahalı)
  Alternatif: volt-hdl.org

GitHub organizasyonu:
  github.com/volt-lang → şimdi aç
  (sonra "volt-hdl" alınmış olabilir)
```

---

## Adım 2 — Repository Yapısı

### 2.1 Workspace Organizasyonu

```
Kodlamadan önce bu yapıya karar ver ve oluştur:

volt/                           ← kök workspace
├── Cargo.toml                  ← workspace tanımı
├── Cargo.lock                  ← sürüm kilidi (commit edilir!)
├── Veryl.toml                  ← hayır, Volt.toml
├── LICENSE-APACHE
├── LICENSE-MIT
├── README.md
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── CHANGELOG.md
├── docs/
│   ├── adr/                    ← Mimari Karar Kayıtları
│   │   ├── ADR-0001-lisans.md
│   │   ├── ADR-0002-cdc-semantik.md
│   │   └── ...
│   ├── spec/                   ← Dil spesifikasyonu
│   │   ├── grammar.ebnf        ← Resmi gramer
│   │   ├── type-system.md      ← Tip sistemi kuralları
│   │   └── error-codes.md      ← Hata kodu kataloğu
│   └── dev/                    ← Geliştirici rehberleri
├── crates/
│   ├── volt-syntax/            ← Lexer + Parser + CST
│   ├── volt-diagnostics/       ← Hata mesajı altyapısı
│   ├── volt-ast/               ← AST düğüm tanımları
│   ├── volt-hir/               ← Yüksek seviye IR
│   ├── volt-lower/             ← CIRCT entegrasyonu (izole!)
│   ├── volt-sim/               ← Simülatör (F4)
│   ├── volt-formal/            ← Formal köprü (F4)
│   ├── volt-lsp/               ← LSP sunucusu (F5)
│   └── volt-driver/            ← CLI ana giriş noktası
├── tests/
│   ├── ui/                     ← Kullanıcı arayüzü testleri
│   │   ├── pass/               ← Derlenmesi gereken
│   │   └── fail/               ← Hata vermesi gereken
│   ├── integration/            ← Uçtan uca testler
│   └── fixtures/               ← Örnek tasarımlar
└── tools/
    └── volt-migrate/           ← Verilog→Volt çevirici (v1)

Neden bu yapı:
  volt-lower izole → CIRCT değişirse sadece burası
  tests/ui ayrı → her özellik pass + fail test
  docs/adr koddan önce dolar → kararlar belgelenir
```

### 2.2 Cargo.toml Workspace

```toml
# Kök Cargo.toml
[workspace]
members = [
    "crates/volt-syntax",
    "crates/volt-diagnostics",
    "crates/volt-ast",
    "crates/volt-hir",
    "crates/volt-lower",
    "crates/volt-sim",
    "crates/volt-formal",
    "crates/volt-lsp",
    "crates/volt-driver",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "Apache-2.0 OR MIT"
repository = "https://github.com/volt-lang/volt"
rust-version = "1.76"  # minimum desteklenen Rust sürümü

[workspace.dependencies]
# Tüm crate'ler bu merkezi listeden sürüm alır
logos       = "0.14"      # lexer
cstree      = "0.12"      # hata-toleranslı CST
codespan-reporting = "0.11"  # hata mesajı biçimi
tower-lsp   = "0.20"      # LSP framework (F5)
clap        = { version = "4.5", features = ["derive"] }
serde       = { version = "1.0", features = ["derive"] }
tracing     = "0.1"

[workspace.metadata.volt]
# Proje meta verisi
```

### 2.3 Branch Koruma Kuralları

```
GitHub Branch Protection → main dal için:

  ✓ "Require pull request reviews before merging"
    → En az 1 onay (veya başlangıçta 0, yalnız çalışıyorsa)

  ✓ "Require status checks to pass before merging"
    → CI/CD geçmeli

  ✓ "Require branches to be up to date before merging"
    → Eski dal merge edilemez

  ✓ "Do not allow force pushes"
    → Tarih korunur

  main: kararlı, her zaman çalışan
  dev: aktif geliştirme (opsiyonel)
  feat/*: özellik dalları
```

---

## Adım 3 — CI/CD Altyapısı

### 3.1 GitHub Actions Temel Pipeline

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, dev]
  pull_request:
    branches: [main]

env:
  RUST_VERSION: "1.76"
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: ${{ env.RUST_VERSION }}
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all --check
      - run: cargo clippy --all-targets --all-features -- -D warnings
      - run: cargo test --all
      - run: cargo build --release

  # F0 tamamlanınca:
  integration:
    name: Integration (Verilator)
    needs: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: sudo apt-get install -y verilator
      - run: cargo run --bin volt -- build tests/fixtures/counter.volt
      - run: verilator --lint-only tests/fixtures/out/counter.sv

Neden şimdi kur:
  CI olmadan F0 yapma — daha ilk günden alışkanlık
  "Yeşil CI" projenin temel güvencesi
```

### 3.2 Kod Kalitesi Araçları

```toml
# rustfmt.toml
edition = "2021"
max_width = 100
use_small_heuristics = "Default"
imports_granularity = "Crate"
group_imports = "StdExternalCrate"

# .clippy.toml veya Cargo.toml [lints]
[workspace.lints.rust]
unsafe_code = "forbid"   # unsafe YASAKLI (başlangıçta)
missing_docs = "warn"

[workspace.lints.clippy]
pedantic = "warn"
```

```bash
# pre-commit hook (.git/hooks/pre-commit)
#!/bin/sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```

---

## Adım 4 — Mimari Karar Kayıtları (ADRs)

Her önemli karar kod yazmadan önce bir ADR dosyasında belgelenir.
Sonraki katılımcı "neden böyle?" sorusunu sorduğunda cevap burada.

### ADR Format

```markdown
# ADR-NNNN: [Karar Başlığı]

## Durum
Kabul Edildi | Tartışılıyor | Reddedildi | Değiştirildi (→ ADR-MMMM)

## Bağlam
Bu karara neden ihtiyaç var?

## Karar
Ne yapılacağına karar verildi?

## Gerekçe
Neden bu yol seçildi?

## Alternatifler
Değerlendirilen diğer seçenekler neydi?

## Sonuçlar
Bu kararın etkileri neler?
```

### Kodlamadan Önce Yazılması Gereken ADRlar

```
ADR-0001: Lisans Politikası
  Karar: Apache-2.0 OR MIT, stdlib sadece MIT
  Bu dosya hazır ↑ (Adım 1'de alındı)

ADR-0002: CDC Saat Alanı Semantiği
  Karar: @Domain anotasyonu, fantom tip değil
  İçerik:
    - "@Sys" ne anlama gelir
    - Domain tanımı nasıl yapılır
    - İki domain arası bağlantı nasıl hata üretir
    - TwoFlop<T> stdlib CDC primitifi
  Neden şimdi: tip sistemi bunu referans alacak

ADR-0003: Trit Tipi Tanımı
  Karar: Trit = {-1, 0, +1}, depolama i2
  İçerik:
    - Trit tipi olarak mı, kısıtlı i2 olarak mı?
    - Trit × i8 çarpımı tipi ne olur?
    - Trit dizisi Memory düzeni (ANS hazırlık)
  Neden şimdi: tip çıkarım bu kararı referans alır

ADR-0004: Hata Kodu Formatı
  Karar: E[DDDD] (E0401 gibi, 4 haneli)
  İçerik:
    - Hata kategorileri (E0xxx: sözdizimi, E1xxx: tip...)
    - Uyarı kodu formatı (W[DDDD])
    - Hata mesajı şablonu (codespan + öneri satırı)
  Neden şimdi: diagnostics crate bu kodu uygular

ADR-0005: Workspace Crate Sınırları
  Karar: Yukarıdaki yapı
  İçerik:
    - Her crate'in sorumluluğu
    - Bağımlılık yönü (alt → üst, döngü yok)
    - volt-lower neden izole
  Neden şimdi: wrong crate boundary → yeniden yapılandırma

ADR-0006: HIR Düğüm Stratejisi
  Karar: Arena tabanlı, Idx<T> handle deseni
  İçerik:
    - Düğüm tipleri (Module, Port, Signal, Domain, Stmt...)
    - Span bilgisi her düğümde mi, ayrı tabloda mı?
    - Döngüsel referans nasıl çözülüyor?
  Neden şimdi: HIR tasarımı sonradan değişirse pahalı

ADR-0007: L0/L1/L2 Zamanlama Seviyeleri
  Karar: L0 varsayılan, L1 opt-in, L2 #[timeline]
  İçerik:
    - L0 ne garantiliyor (pipeline hizalama otomatik)
    - L1 Delayed<T,N> semantiği
    - L2 timeline tipler (Filament referansı)
    - Kullanıcı hangi seviyeyi seçer
  Neden şimdi: parser bu anahtar kelimeleri tanımalı

ADR-0008: SV Çıktı Stili
  Karar: Okunabilir, FF seviyesinde değiştirilebilir
  İçerik:
    - Veryl'in çıktısı referans kalitesi
    - Sinyal ismi korunur (Volt ismi = SV ismi)
    - Yorum korunur (belgeleme yorumları SV'ye geçer)
    - Constraints dosyası (XDC/SDC) ile uyum
  Neden şimdi: codegen bu kararı uygular

ADR-0009: Test Dosya Formatı
  Karar: tests/ui/pass/*.volt ve tests/ui/fail/*.volt
  İçerik:
    - Fail testlerde beklenen hata kodu: // E0401
    - Pass testlerde beklenen SV snippet kontrolü
    - Fixture dosyaları nasıl organize
  Neden şimdi: ilk test bu yapıda yazılacak

ADR-0010: CIRCT Dialect Seçimi
  Karar: hw + seq + comb + sv dialect
  İçerik:
    - reg(Sys) → seq.firreg veya hw.reg?
    - module → hw.module
    - always_ff → sv.always_ff
    - Hangle bağlantı noktaları
  Neden şimdi: volt-lower bu kararı uygular
```

---

## Adım 5 — Dil Spesifikasyonu Kilitlenmesi

### 5.1 Resmi EBNF Gramer (docs/spec/grammar.ebnf)

Kodlamadan önce bu dosya var olmalı.
Parser yazılırken referans.
Değişirse ADR güncellenir.

```ebnf
(* docs/spec/grammar.ebnf — MVP alt kümesi *)
(* Tam dil değil, F0-F2 arası çalışan subset *)

Program     = { Item } ;
Item        = ModuleDecl | DomainDecl ;

DomainDecl  = "domain" Ident "{" DomainBody "}" ;
DomainBody  = (* clock, reset tanımları *) ;

ModuleDecl  = "module" Ident "{" { Port } { Stmt } "}" ;

Port        = PortDir Ident ":" Type [ "@" Domain ] ;
PortDir     = "in" | "out" ;

Type        = "bool"
            | "u8" | "u16" | "u32" | "u64"
            | "i8" | "i16" | "i32" | "i64"
            | "bits" "<" Expr ">"
            | "Trit"
            | Ident [ "<" TypeArgs ">" ] ;

Domain      = Ident ;

Stmt        = RegDecl | OnBlock | Assign | LetBind ;

RegDecl     = "reg" "(" Domain ")" Ident ":" Type "=" Expr ;
OnBlock     = "on" Domain "{" { Stmt } "}" ;
Assign      = Expr "<=" Expr                    (* non-blocking *)
            | Expr "=" Expr ;                   (* continuous *)
LetBind     = "let" Ident "=" Expr ;

Expr        = (* operatörler, literaller, çağrılar... *) ;

(* Henüz dahil edilmeyenler (F0 için yeterli): *)
(* pipeline, fn, formal, generic, match, enum... *)
```

### 5.2 Tip Sistemi Kuralları (docs/spec/type-system.md)

```markdown
# Volt Tip Sistemi Kuralları — MVP

## Kural T1: Bit Genişliği Uyumu
  u8 + u8 → u9 (taşma genişlemesi)
  u8 + u16 → hata (örtük genişleme yok)
  Açık dönüşüm: zext(), sext(), trunc()

## Kural T2: CDC Alan Uyumu
  Sinyal A @DomainX, Sinyal B @DomainY
  A @DomainX = B @DomainY → E0401 hatası
  A = bridge(B) ← sadece stdlib primitifi ile

## Kural T3: Port Yönü Uyumu
  "in" portu sol tarafta kullanılamaz
  "out" port sağ tarafta okunabilir
  Çifte sürücü → E0402 hatası

## Kural T4: Trit Tipi Davranışı
  Trit × Trit → Trit (kapalı: {-1,0,+1}×{-1,0,+1}⊆{-1,0,+1})
  Trit × i8  → i8  (genişleme)
  Trit + Trit → i2 (taşma mümkün: +1+1=+2)

## Kural T5: Register Alanı
  reg(D) x : T = e ← x sadece D alanında güncellenebilir
  on D { x <= new_val } ← geçerli
  on D2 { x <= new_val } ← E0403 hatası (yanlış alan)
```

### 5.3 Hata Kodu Kataloğu (docs/spec/error-codes.md)

```markdown
# Volt Hata Kodları

## E0xxx — Sözdizimi Hataları
E0001: Beklenmeyen token
E0002: Eksik kapanış `}`
E0003: Bilinmeyen anahtar kelime

## E1xxx — İsim Çözümleme Hataları
E1001: Tanımsız isim
E1002: Çift tanım
E1003: Özel modüle erişim

## E2xxx — Tip Hataları
E2001: Bit genişliği uyumsuzluğu
E2002: İşaret uyumsuzluğu
E2003: Tip bekleniyor, bulunamadı

## E3xxx — Alan/CDC Hataları ← VOLT'UN KALBİ
E3001: Saat alanı uyumsuzluğu (CDC ihlali)
E3002: Tanımsız saat alanı
E3003: Döngüsel alan bağımlılığı

## E4xxx — Sürücü Hataları
E4001: Çift sürücü (aynı sinyale iki assign)
E4002: Sürücüsüz out portu
E4003: Yazma gerektiren read-only sinyal

## W1xxx — Uyarılar
W1001: Kullanılmayan sinyal (opsiyonel: `_` ile sustur)
W1002: Eksik hizalama (L0 zaten düzeltiyor ama uyarır)
```

---

## Adım 6 — Katkı Rehberi

### 6.1 CONTRIBUTING.md (Temel)

```markdown
# Volt'a Katkıda Bulunma

## Hızlı Başlangıç
1. Depoyu fork et ve klonla
2. `cargo build` (bağımlılıkları indir)
3. `cargo test` (tüm testler yeşil olmalı)
4. Dal aç: `git checkout -b feat/benim-ozelligim`
5. Değişiklik yap + test yaz
6. `cargo fmt && cargo clippy`
7. `git commit -s -m "Kısa açıklama"`
8. PR aç

## Commit Mesajı Formatı
  feat(parser): if ifadesi desteği
  fix(cdc): domain karşılaştırma hatası düzeltildi
  test(hir): register düğümü testleri
  docs(adr): CDC semantik kararı eklendi

## DCO (Geliştirici Sertifikası)
Her commit -s bayrağıyla imzalanmalı:
  git commit -s -m "mesaj"
→ Signed-off-by: Ad Soyad <email@example.com>

## Test Yazma
  tests/ui/pass/ → derlenmesi gereken örnekler
  tests/ui/fail/ → hata üretmesi gereken örnekler
  fail testlerde ilk satır: // E3001 (beklenen hata)

## PR Kontrol Listesi
  [ ] Tüm testler geçiyor (cargo test)
  [ ] Lint temiz (cargo clippy)
  [ ] Biçimlendirilmiş (cargo fmt)
  [ ] Test eklendi (yeni davranış için)
  [ ] CHANGELOG güncellendi

## Yardım Al
  Discord: [davet linki]
  Issue: "good first issue" etiketli sorunlar
```

### 6.2 CODE_OF_CONDUCT.md

```
Contributor Covenant v2.1 kullan (standar, evrensel tanınan)
Sadece link veya metin kopyala
"Biz bekliyoruz ki..." → dostane ortam garantisi
```

---

## Adım 7 — Test Stratejisi

### 7.1 Test Türleri ve Organizasyonu

```
1. Birim Testler (her crate'te):
   crates/volt-syntax/tests/
     lexer_test.rs    → token tanıma
     parser_test.rs   → AST yapısı
   crates/volt-hir/tests/
     type_check_test.rs → tip kuralları

2. UI Testleri (davranış testleri):
   tests/ui/pass/
     counter.volt          → temel sayaç
     cdc_bridge.volt       → doğru CDC kullanımı
     trit_layer.volt       → ternary tip
   tests/ui/fail/
     cdc_violation.volt    → E3001 bekleniyor
     double_driver.volt    → E4001 bekleniyor
     wrong_width.volt      → E2001 bekleniyor

3. Entegrasyon testleri:
   Counter → SV → Verilator lint → geçmeli
   (F0 tamamlanınca)

4. Snap testleri (F3 sonrası):
   Counter Volt kodu → beklenen SV snapshot
   Değişirse: manuel onay gerekiyor
   insta crate ile
```

### 7.2 Test Çalıştırma Protokolü

```bash
# Hızlı test (geliştirme sırasında)
cargo test -p volt-syntax

# Tam test (PR öncesi)
cargo test --all

# Tek UI testi debug
cargo run --bin volt -- build tests/ui/fail/cdc_violation.volt
# Beklenti: E3001 hatası görünmeli

# Verilator entegrasyon (F0 sonrası)
make test-integration
```

---

## Adım 8 — Teknik Sınırlar Belgesi

### 8.1 MVP Kapsam Belgesi (docs/dev/scope.md)

```markdown
# Volt MVP Kapsamı

## MVP'de OLAN (F0-F5)

Dil özellikleri:
  ✓ module, in, out, reg(D), on D { }
  ✓ Temel tipler: bool, u8-u64, i8-i64, bits<N>, Trit
  ✓ @Domain anotasyonu ve CDC hata tespiti
  ✓ Basit ifadeler: +, -, *, /, if/else, match (basit)
  ✓ let bağlaması (değişmez)
  ✓ Stdlib: TwoFlop<T>, FIFO (basit), AXI_Lite başlangıç

Araçlar:
  ✓ volt build → SV üretimi
  ✓ volt check → tip kontrolü
  ✓ volt test → testbench
  ✓ volt fmt → biçimlendirme
  ✓ LSP temel (diagnostics, hover)
  ✓ Verilator entegrasyonu

## MVP'de OLMAYAN (Bilinçli kapsam dışı)

Dil özellikleri:
  ✗ Generic parametreler (v1'de)
  ✗ Trait sistemi (v1'de)
  ✗ Sum tipler / enum (v1'de)
  ✗ Higher-order fonksiyon (v1'de)
  ✗ Pipeline birimi (L1 var ama kısıtlı)
  ✗ Spike<T> nöromorfik (v2'de)
  ✗ PTrit fotonik (v3'te)
  ✗ Analog (kapsam dışı, belki hiç)
  ✗ Asenkron tasarım (kapsam dışı)
  ✗ UVM (kapsam dışı MVP için)

Bu kapsam dışı listesi katkıcılara "hayır" demek için kullanılır.
"Bu MVP kapsamında değil, v1 issue açın" yanıtı hazır.
```

### 8.2 Bağımlılık Politikası

```toml
# Kabul Edilen Bağımlılık Kriterleri:
# 1. Aktif bakım (son 6 ay içinde commit)
# 2. İyi test edilmiş
# 3. Apache/MIT lisanslı
# 4. Güvenlik sorunları olmayan

# YASAKLI bağımlılık:
# - GPL/LGPL (viral lisans)
# - Sürümü sabitlenmemiş "git = ..." bağımlılıklar
# - Çok geniş özellik seti olan ancak az kullanılan

# Bağımlılık ekleme protokolü:
# 1. ADR'de gerekçe yaz
# 2. Alternatifler değerlendirildi mi?
# 3. cargo audit çalıştır (güvenlik)
# 4. Lisans uyumu kontrol
```

---

## Adım 9 — İletişim Altyapısı

### 9.1 Kanallar (Öncelik Sırasıyla)

```
GitHub Issues (birincil, asenkron):
  Bug reports
  Feature requests (kapsam içi)
  ADR tartışmaları
  Etiketler: bug, enhancement, good-first-issue,
             adr, scope-v1, scope-v2

GitHub Discussions (ikincil, soru-cevap):
  "Nasıl yaparım?" soruları
  Tasarım tartışmaları
  Duyurular

Discord (anlık, topluluk):
  #volt-genel
  #volt-geliştirme (katkıcılar)
  #volt-yardım (kullanıcılar)
  #volt-duyuru (salt okunur)

Matrix (Discord mirror, opsiyonel):
  Açık kaynak topluluğu Matrix tercihli

NOT: Çok fazla kanal = dağınık = topluluk ölür
     Başlangıçta GitHub Issues yeterli
     Discord F0 sonrası açılır
```

### 9.2 İlk Haftalarda İletişim Tonu

```
RFC (Request for Comments) süreci — F0 öncesi HAZIR OLMALI:

RFC formatı (docs/rfcs/):
  RFC-0001-trit-tipi.md
  
  Durum: Teklif | İnceleme | Kabul | Uygulama | Tamamlandı

  Problem: Ternary ağırlıklar için tip sistemi eksik.
  Önerilen çözüm: ...
  Alternatifler: ...
  Etkilenen: volt-hir, volt-lower, stdlib

RFC ne zaman gerekli:
  Dili etkileyen karar → RFC
  Araç değişikliği → GitHub Issue yeterli
  Bug fix → PR yeterli

RFC ne zaman değil:
  İlk 6 ay tek kişi → ADR yeterli
  Topluluk 5+ kişi olunca → RFC önemli
```

---

## Adım 10 — Minimum Çalışabilir Belgeleme

### 10.1 README.md (İlk Sürüm)

```markdown
# Volt HDL

Volt, CDC güvenliğini derleme zamanında garantileyen,
bağımsız, CIRCT tabanlı bir Hardware Description Language.

## Neden Volt?

```verilog
// Verilog'da bu sessizce geçer:
always_ff @(posedge fast_clk)
    slow_reg <= fast_signal;  // CDC ihlali!
```

```volt
// Volt'ta bu derleme hatası:
slow_reg @Slow <= fast_signal @Fast;
// E3001: Saat alanı uyumsuzluğu: @Fast → @Slow
// Yardım: TwoFlop<u8>(fast_signal) kullanın
```

## Durum

⚠️ Erken alpha geliştirme. Üretime hazır değil.

F0 (sayaç → SV → Verilator) tamamlandı ✓
F1-F2 (tip sistemi) devam ediyor...

## Hızlı Başlangıç

cargo install volt-lang  # henüz yok

## Lisans

Apache-2.0 OR MIT
```

---

## Adım 11 — Ölçüm: Başarıyı Nasıl Anlarsın

### 11.1 F0 Başarı Kontrol Listesi

```
□ git clone yapılabilir
□ cargo build --release hata vermez
□ volt build tests/fixtures/counter.volt çalışır
□ Üretilen SV, Verilator lint'ten geçer
□ cargo test --all → sıfır hata
□ GitHub CI: yeşil ✓
□ README'deki örnek gerçekten çalışır

Bu listeden tek madde eksikse → F0 tamamlanmadı.
```

### 11.2 F2 (Tip Sistemi) Başarı Kontrol Listesi

```
□ CDC ihlali → E3001 hatası (doğru satır, doğru mesaj)
□ Doğru CDC → derlenir
□ Trit × i8 → i8 tipi çıkarılır
□ Port yönü yanlış → E4001
□ Bit genişliği uyumsuz → E2001
□ Her kural için tests/ui/fail/ testi var
□ Her kural için tests/ui/pass/ testi var
```

---

## Adım 12 — Başlama Kontrol Listesi

```
KRİTİK (bunlar olmadan başlama):

Yasal:
  □ LICENSE-APACHE ve LICENSE-MIT dosyaları oluşturuldu
  □ SPDX başlığı Cargo.toml'da

Repository:
  □ GitHub organizasyon/repo açıldı
  □ Branch koruma aktif
  □ Cargo workspace yapısı oluşturuldu
  □ .github/workflows/ci.yml hazır

ADR'lar:
  □ ADR-0001: Lisans
  □ ADR-0002: CDC semantiği
  □ ADR-0003: Trit tipi
  □ ADR-0004: Hata kodu formatı
  □ ADR-0005: Workspace sınırları

Spesifikasyon:
  □ docs/spec/grammar.ebnf (F0-F2 subset)
  □ docs/spec/type-system.md (MVP kuralları)
  □ docs/spec/error-codes.md (E3xxx kataloğu)

Katkı:
  □ CONTRIBUTING.md (DCO dahil)
  □ CODE_OF_CONDUCT.md

Test:
  □ tests/ui/pass/ klasörü mevcut
  □ tests/ui/fail/ klasörü mevcut
  □ Makefile veya README'de test talimatları

ÖNEMLİ (F0 bitmeden):
  □ volt-lang.org domain alındı
  □ Discord server hazır (F0 duyurusu için)
  □ İlk "good first issue" etiketi planlandı

KOD YAZMAYA BAŞLAYACAK ADIM:
  cargo new --lib crates/volt-syntax
  → Ardından lexer.rs
  → Ardından logos ile ilk tokenlar
  
  Bu listedeki kutucuklar dolu değilse,
  kodlamaya değil listeyi tamamlamaya odaklan.
```

---

## Toplam Süre Tahmini

```
Bu adımların tamamlanma süresi (tek kişi):

Yasal + Repo kurulumu:    0.5 gün
ADR yazımı (10 ADR):      2-3 gün
Spesifikasyon belgesi:    2-3 gün
CI pipeline:              0.5 gün
Katkı rehberi:            0.5 gün
Test stratejisi:          0.5 gün
───────────────────────────────
Toplam:                   6-8 tam gün

Bu yatırım neden değer:
  Sonradan değiştirilen her karar bu sürenin
  10× katı maliyete neden olur.
  8 gün şimdi = belki 80 gün sonra kazanılan.

Paralel çalışma mümkün:
  ADR yazarken spesifikasyon da yazılır
  CI kurulurken ADR beklemez
  Gerçekte 3-4 yoğun gün yeterli
```

---

## Son Not: Neden Bunlar Önemli

```
Bu konuşmada 30 belge üretildi.
Vizyon, rekabet analizi, teknoloji trendleri, yol haritası —
hepsi hazır.

Şimdi soru şu:
"Bir yıl sonra aynı belgelere bakıp
 'evet, biz bunu inşa ettik' diyecek miyiz,
 yoksa 'hâlâ planlamaya devam ediyoruz' mu diyeceğiz?"

Bu adımlar arasındaki fark:
  Plan = Belgeler + Vizyon
  Proje = Plan + Çalışan Kod + Ekip + Kullanıcı

Bu kontrol listesi, Plan'dan Proje'ye geçişin kapısı.
Kapı açılmadan içeri girilmez.
Ama kapıyı açmak 6-8 gün.
```
