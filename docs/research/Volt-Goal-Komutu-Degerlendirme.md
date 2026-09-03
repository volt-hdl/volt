> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Claude Code /goal Komutu — Volt Projesi Uygunluk Değerlendirmesi

> /goal: Claude Code v2.1.139+ ile gelen otonom ajan modu.
> "Ne yap" değil, "bittiğinde ne doğru olacak" söylersin.
> Bağımsız bir doğrulayıcı model her turda sonucu kontrol eder.

---

## /goal Nedir: Özet

```
Normal Claude Code:
  Sen sordun → Claude yaptı → Sen baktın → Sen sordun → ...
  (20+ tur, her seferinde senden girdi)

/goal ile:
  Sen bitişi tanımladın → Claude otonom çalışır →
  Doğrulayıcı model her turda "bitti mi?" sorar →
  Bitti ise → Claude durur, kontrolü sana verir

Temel şart: Bitiş koşulu ölçülebilir ve çalıştırılabilir olmalı.
  ✓ "cargo test exits 0"
  ✓ "verilator --lint-only exits 0"
  ✗ "kod temiz görünüyor"
  ✗ "tip sistemi iyi tasarlanmış"
```

---

## Volt Projesi için Uygunluk Analizi

### Neden Volt için Özellikle Uygun

```
Volt geliştirme döngüsü:
  Kod yaz → cargo test çalıştır → hataları düzelt → tekrar

Bu döngü tam /goal için biçilmiş kaftan:
  ✓ "cargo test --all exits 0" → mükemmel doğrulayıcı koşul
  ✓ "verilator --lint-only counter.sv exits 0" → mükemmel
  ✓ UI testler (pass/fail klasörleri) → sayılabilir ve ölçülebilir
  ✓ Her Volt özelliği "bu test geçene kadar" şeklinde tanımlanabilir
  ✓ Derleyici hatası mesajı kesin ve binary (ya geçiyor ya geçmiyor)

Volt'ta /goal'un en güçlü olduğu yer:
  İmplementasyon görevleri (karar görevleri değil)
  "X özelliğini yaz, testler yeşile dönene kadar"
```

### Volt'ta /goal KULLANILMAYACAK Durumlar

```
Karar gerektiren görevler için /goal kullanma:

  ✗ CDC semantik tasarımı
    ("@Domain nasıl çalışmalı?" → insan kararı)

  ✗ Gramer kararları
    ("pipeline sözdizimi nasıl olmalı?" → ADR gerekiyor)

  ✗ HIR düğüm mimarisi
    ("Arena mı, Rc mı?" → mimari karar)

  ✗ CIRCT dialect seçimi
    ("seq.firreg mı, hw.reg mi?" → teknik araştırma)

  ✗ Hata mesajı metni
    ("E3001 mesajı ne desin?" → kullanıcı deneyimi kararı)

Bu görevler subjektif veya mimari.
Doğrulayıcı model "yeterince iyi mi?" sorusunu cevaplayamaz.
```

---

## Volt'ta /goal Kullanım Haritası

```
F0 — Counter → SV → Verilator → CI
   ████████████ MÜKEMMEL (çift doğrulayıcı: cargo + verilator)

F1 — Lexer implementasyonu
   █████████░░ ÇOK İYİ (token testleri binary)

F2a — Temel tipler (u8, bool, bits<N>)
   ████████░░░ İYİ (tip testi binary)

F2b — Trit tipi
   ████████░░░ İYİ (Trit semantik testleri)

F2c — CDC alan tipleri ← DIKKAT
   ████░░░░░░░ SINIRLIMDI (semantik ÖNCE elle, sonra /goal)
   Not: Semantik karar (ADR) manuel. Uygulama /goal ile.

F3 — CIRCT lowering
   █████░░░░░░ ORTA (CIRCT bilgisi şart, adım adım)

F4 — SymbiYosys formal
   ████████░░░ İYİ (formal test binary: sat/unsat)

F5 — LSP diagnostics
   ██████░░░░░ İYİ (snapshot testler doğrulanabilir)

Stdlib — TwoFlop, FIFO, AXI
   █████████░░ ÇOK İYİ (entegrasyon testleri binary)

UI Testleri — pass/fail klasörleri
   ████████████ MÜKEMMEL (en ölçülebilir görev)
```

---

## Volt CLAUDE.md: /goal Oturumları İçin

```markdown
# Volt HDL — Claude Code Yapılandırması

## Proje Yapısı
Dil: Rust (Cargo workspace)
Derleyici komutları:
  cargo build --all          # derleme
  cargo test --all           # tüm testler
  cargo test -p volt-syntax  # sadece lexer/parser testleri
  cargo test -p volt-hir     # sadece HIR/tip testleri
  cargo clippy --all -- -D warnings
  cargo fmt --all --check

## /goal Oturumu Kuralları (DAIMA UYGULANIR)

1. Test kuralları:
   - Hiçbir testi silme, atlama veya yorum satırına alma
   - tests/ui/pass/ → derlenmeli, geçmeli
   - tests/ui/fail/ → tam hata kodu üretmeli (# E3001 gibi)
   - Test sayısı başlangıç değerinin altına düşmemeli

2. CDC kuralı (KRİTİK):
   - @Domain anotasyonu semantiği docs/spec/type-system.md'de tanımlı
   - Bu dosyayı değiştirme — semantik ADR ile değişir
   - Sadece implementasyon yaz, semantik değil

3. Hata kodu kuralları:
   - Hata kodları docs/spec/error-codes.md'de tanımlı
   - Yeni hata kodu ekleme — önce dokümantasyonu güncelle
   - E3xxx = CDC hataları, E2xxx = tip hataları, E4xxx = sürücü hataları

4. SV çıktı kuralları:
   - Üretilen SV, Verilator lint'ten geçmeli
   - Sinyal isimleri Volt kaynak kodundaki ile aynı olmalı
   - always_ff veya always_comb, asla always

5. Yasaklı işlemler:
   - docs/spec/ altındaki herhangi bir dosyayı değiştirme
   - ADR dosyalarını değiştirme
   - Cargo.toml'a yeni bağımlılık ekleme (PR gerektirir)
   - unsafe kod yazma

## Test Çalıştırma
cargo test --all                    # hızlı kontrol
cargo test --all 2>&1 | tail -20   # son 20 satır (hata özeti)
verilator --lint-only [dosya.sv]   # SV doğrulama

## Mimari Referanslar
CDC semantik: docs/spec/type-system.md (Kural T2)
Hata kodları: docs/spec/error-codes.md
Gramer: docs/spec/grammar.ebnf
ADR'lar: docs/adr/*.md
```

---

## F0 Aşaması İçin /goal Şablonları

### F0-A: Workspace Kurulumu

```
/goal Volt Cargo workspace'ini kur.

Tamamlanma koşulu:
  cargo build --all exits 0
  cargo test --all exits 0 (0 test bile olsa)
  cargo clippy --all -- -D warnings exits 0
  cargo fmt --all --check exits 0

Kapsam: Cargo.toml (kök ve her crate), her crate'te boş lib.rs
Dokunma: docs/ klasörü altındaki hiçbir şeyi değiştirme

Dur: 10 tur sonra raporla.
```

### F0-B: Lexer (İlk Tokenlar)

```
/goal volt-syntax crate'inde logos ile Volt lexer'ını implement et.

Hedef tokenlar: module, in, out, reg, on, let, bool, Trit,
u8, u16, u32, u64, i8, i16, i32, i64, bits, domain,
true, false, identifiers, integer literals, operators
(<=, =, +, -, *, /, ==, !=, <, >, &&, ||, !, @, :, ;, {, }, (, ), [, ], <, >)

Tamamlanma koşulu:
  cargo test -p volt-syntax exits 0
  Tüm anahtar kelimeler için token testleri yeşil
  (tests/unit/lexer_test.rs'de en az 30 test)

Kapsam: crates/volt-syntax/src/lexer.rs ve tests/
Dokunma: Diğer crate'lere dokunma, docs/spec/grammar.ebnf'i değiştirme

Dur: 20 tur sonra raporla.
```

### F0-C: Counter Örneği → SV Üretimi

```
/goal tests/fixtures/counter.volt dosyasındaki Counter modülünü
string template ile SystemVerilog'a çeviren minimal codegen yaz.

Counter'ın Volt kodu: tests/fixtures/counter.volt (hazır)
Beklenen SV çıktısı: tests/fixtures/counter.expected.sv (hazır)

Tamamlanma koşulu:
  cargo test --all exits 0
  verilator --lint-only tests/fixtures/out/counter.sv exits 0
  üretilen SV, counter.expected.sv ile eşleşiyor (snapshot test)

Kapsam: crates/volt-driver/src/ ve crates/volt-sv-emit/src/
CIRCT kullanma — doğrudan string biçimlendirme yeterli

Dur: 15 tur sonra raporla.
```

### F0-D: CI Pipeline

```
/goal .github/workflows/ci.yml dosyasını oluştur.

Tamamlanma koşulu:
  GitHub Actions workflow valid YAML
  cargo fmt --check, cargo clippy, cargo test, cargo build adımları var
  Verilator kurulumu (ubuntu-latest apt-get) dahil
  counter.volt → SV → verilator lint entegrasyon adımı var

Kapsam: .github/workflows/ klasörü
Dokunma: Rust kaynak dosyalarını değiştirme

Dur: 10 tur sonra raporla.
```

---

## F2 Aşaması İçin /goal Şablonları

### F2-A: Temel Tip Çıkarımı

```
/goal volt-hir crate'inde u8-u64 ve i8-i64 için tip çıkarım
algoritmasını implement et.

Tamamlanma koşulu:
  cargo test -p volt-hir exits 0
  Aşağıdaki test durumları geçiyor:
    u8 + u8 → u9 (taşma genişlemesi)
    u8 + u16 → E2001 hatası (örtük genişleme yok)
    -5i8 → negatif literal tanınıyor
    bits<4> + bits<4> → bits<5>
  (tests/ui/pass/ ve tests/ui/fail/ altında her durum için test var)

Kapsam: crates/volt-hir/src/type_inference.rs
docs/spec/type-system.md'deki Kural T1'i referans al (değiştirme)

Dur: 25 tur sonra raporla.
```

### F2-B: Trit Tipi Implementasyonu

```
/goal Volt tip sistemine Trit tipini ekle.

Trit semantiği (docs/spec/type-system.md Kural T4'te tanımlı):
  Trit: {-1, 0, +1}
  Trit * Trit → Trit (kapalı)
  Trit * i8  → i8
  Trit + Trit → i2 (taşma: +1+1=+2)
  Trit depolama: i2 kısıtlı

Tamamlanma koşulu:
  cargo test --all exits 0
  tests/ui/pass/trit_basic.volt → derliyor ve doğru tip çıkarıyor
  tests/ui/pass/trit_multiply.volt → Trit * Trit → Trit
  tests/ui/pass/trit_mixed.volt → Trit * i8 → i8
  tests/ui/fail/trit_overflow.volt → E2xxx uyarısı (Trit + Trit aralık uyarısı)

Kapsam: crates/volt-hir/src/ (tip tanımı ve çıkarım)
Dokunma: Kural T4 semantiğini değiştirme

Dur: 20 tur sonra raporla.
```

### F2-C: CDC Hata Tespiti (Semantik Hazırsa)

```
NOT: Bu /goal'u SADECE ADR-0002 tamamlandıktan ve
     docs/spec/type-system.md Kural T2 yazıldıktan sonra kullan.
     Semantiği değil, implementasyonu yaz.

/goal CDC alan uyumsuzluk hatası E3001'i implement et.

Semantik referans: docs/spec/type-system.md Kural T2 (değiştirme)

Tamamlanma koşulu:
  cargo test --all exits 0
  Tüm UI testleri geçiyor:
    tests/ui/fail/cdc_violation_direct.volt     → E3001
    tests/ui/fail/cdc_violation_assignment.volt → E3001
    tests/ui/fail/cdc_violation_port.volt       → E3001
    tests/ui/pass/cdc_correct_bridge.volt       → derleniyor
    tests/ui/pass/cdc_same_domain.volt          → derleniyor
  E3001 hata mesajı şunları içeriyor:
    - Kaynak alan adı
    - Hedef alan adı
    - Offending satır numarası
    - "TwoFlop<T> kullanın" önerisi

Kapsam: crates/volt-hir/src/type_check.rs ve diagnostics
Dokunma: docs/spec/ altındaki hiçbir şeyi değiştirme

Dur: 30 tur sonra raporla.
```

---

## UI Test Genişletme İçin /goal

Bu Volt'un en tekrarlayan ve /goal için en uygun görevi:

### UI Test Suite Genişletme

```
/goal tests/ui/ altındaki test kapsamını genişlet.

Tamamlanma koşulu:
  cargo test --all exits 0
  tests/ui/pass/ en az 15 farklı geçen senaryo içeriyor
  tests/ui/fail/ en az 15 farklı hata senaryosu içeriyor
  Her fail testinin ilk satırı beklenen hata kodunu içeriyor: // E3001
  cargo test --all çıktısı "0 failed" gösteriyor

Eklenecek test kategorileri:
  CDC ihlalleri (E3001): 5 senaryo
  Bit genişliği uyumsuzlukları (E2001): 3 senaryo
  Çift sürücü (E4001): 3 senaryo
  Sürücüsüz port (E4002): 2 senaryo
  Geçerli CDC köprüsü: 3 senaryo
  Geçerli Trit kullanımı: 4 senaryo

Kapsam: tests/ui/ klasörü (yeni .volt dosyaları ekle, mevcut değiştirme)

Dur: 20 tur sonra raporla.
```

---

## Stdlib İmplementasyonu İçin /goal

### TwoFlop<T> CDC Primitifi

```
/goal Volt standart kütüphanesine TwoFlop<T> CDC primitifini ekle.

TwoFlop<T> semantiği:
  İki flip-flop senkronizatör
  Giriş: T @SrcDomain
  Çıkış: T @DstDomain
  Metastabilite: iki döngü gecikme ile çözülüyor

Tamamlanma koşulu:
  cargo test --all exits 0
  tests/ui/pass/stdlib_twoflop.volt → derliyor
  Üretilen SV için verilator --lint-only exits 0
  Üretilen SV iki aşamalı flip-flop içeriyor (always_ff × 2)
  tests/ui/pass/cdc_bridge_usage.volt → TwoFlop ile CDC geçiyor

Kapsam: crates/volt-stdlib/src/cdc.volt
Dokunma: volt-hir tip kurallarını değiştirme

Dur: 20 tur sonra raporla.
```

---

## Hata Mesajı Kalitesi İçin /goal

```
/goal E3001 (CDC ihlali) hata mesajını iyileştir.

Mevcut mesaj: "E3001: clock domain mismatch"
Hedef mesaj formatı:
  error[E3001]: Saat alanı uyumsuzluğu
   --> dosya.volt:12:5
    |
  12 |     slow_out @Slow = fast_signal @Fast;
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |     @Fast sinyali @Slow alana doğrudan bağlanamaz
    |
  help: TwoFlop<u8>(fast_signal) kullanın
  note: Bakınız: docs/adr/ADR-0002-cdc-semantik.md

Tamamlanma koşulu:
  cargo test --all exits 0
  Tüm E3001 snapshot testleri yeni formatta eşleşiyor
  (insta crate snapshot testleri)
  Alan isimleri, satır numarası ve öneri satırı var

Kapsam: crates/volt-diagnostics/src/e3xxx.rs
Dokunma: Hata kodunu veya semantiği değiştirme

Dur: 15 tur sonra raporla.
```

---

## Doküman Üretimi İçin /goal

```
/goal Tüm public Rust öğeleri için rustdoc belgeleme ekle.

Tamamlanma koşulu:
  cargo doc --no-deps --all 2>&1 | grep "^warning" | wc -l → 0 satır
  (sıfır eksik belgeleme uyarısı)
  Her public struct, enum, fn, trait: en az 1 satır açıklama
  Her public fn: @param, @returns açıklaması (///) var

Kapsam: crates/ altındaki tüm Rust dosyaları
Dokunma: Test dosyalarına (tests/) dokunma, mantık değiştirme

Dur: 25 tur sonra raporla.
```

---

## /goal için Güvenlik Kontrol Listesi

Her /goal öncesi:

```bash
# 1. Temiz Git durumu şart
git status
# "nothing to commit, working tree clean" olmalı
# Değilse:
git add . && git commit -m "checkpoint: /goal öncesi"

# 2. Baseline ölç
cargo test --all 2>&1 | grep -E "test result|FAILED|ok"
# Kaç test geçiyor şu an? Not al.

# 3. Verilator çalışıyor mu?
verilator --version  # yüklü mü?

# 4. CLAUDE.md hazır mı?
cat CLAUDE.md  # Volt kuralları var mı?
```

Her /goal sonrası:

```bash
# 1. Diff'i gözden geçir
git diff --stat   # hangi dosyalar değişti?
git diff          # tam değişiklik

# 2. Bağımsız test çalıştır (Claude'dan değil, sen çalıştır)
cargo test --all

# 3. Test sayısı düştü mü?
cargo test --all 2>&1 | grep "test result"
# Başlangıçtaki sayının altında mı?

# 4. Memnunsun → commit
git add . && git commit -m "feat(F0): counter → SV pipeline çalışıyor"

# 5. Memnun değilsin → geri al
git checkout .
```

---

## /goal ile /goal Olmadan Karşılaştırma

```
F0 Görev Örneği: Lexer implementasyonu

/goal OLMADAN (manuel):
  Sen: "logos ile keyword tanıma ekle"
  Claude: lexer.rs yazar, durur
  Sen: cargo test çalıştırır, hata görür, yapıştırır
  Claude: düzeltir, durur
  Sen: tekrar test, yeni hata, yapıştırır
  Claude: düzeltir...
  → 15-20 tur, her tur senden girdi

/goal İLE:
  Sen: "/goal Lexer'ı implement et, cargo test exits 0 olana kadar"
  Claude: lexer.rs yazar, test çalıştırır, hata görür, düzeltir,
          test çalıştırır, geçer → "bitti mi?" doğrulayıcı → EVET → durur
  → Sen sadece bitiş koşulunu tanımladın, geri kalan otonom

Volt'ta F0-F5 için /goal tasarrufu:
  F0 (3 görev): ~30 tur → ~6 tur (5× tasarruf)
  F1 (parser):  ~50 tur → ~10 tur
  F2 (tipler):  ~100 tur → ~20 tur
  F4-F5:        ~60 tur → ~12 tur
  Toplam tahmini tasarruf: 200+ tur
```

---

## Özet: Volt Projesi için /goal Haritası

```
KESİNLİKLE KULLAN:
  ✓ Workspace kurulumu
  ✓ Lexer implementasyonu
  ✓ Parser tamamlama
  ✓ Temel tip implementasyonları (u8, Trit...)
  ✓ CDC hata tespiti uygulaması (semantik sonra)
  ✓ UI test suite genişletme ← en tekrarlayan
  ✓ Stdlib primitifleri (TwoFlop, FIFO)
  ✓ Hata mesajı kalitesi
  ✓ Belgeleme tamamlama
  ✓ CI pipeline oluşturma

KULLANMA:
  ✗ CDC semantik tasarımı (ADR ile yap)
  ✗ Gramer kararları (spec ile yaz)
  ✗ CIRCT dialect seçimi (araştırma ile karar ver)
  ✗ HIR mimari kararları
  ✗ Hata mesajı metni kararları
  ✗ ADR yazımı

TEMEL KURAL:
  "Bu görev için 'bitişini' runnable bir komutla tanımlayabilir miyim?"
  Evet → /goal kullan
  Hayır → Manuel yap, ADR yaz, sonra /goal ile uygula
```
