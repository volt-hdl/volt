# Volt HDL — Mimari Kararlar, Tasarım İlkeleri ve CLAUDE.md

---

## Bölüm 1 — Karar Türleri ve Maliyetleri

Her karar eşit değildir. Yanlış tür bir kararda ilerlemek
farklı maliyetler yaratır:

```
DEĞİŞTİRME MALİYETİ TABLOSU

Değişken ismi yanlış          → git mv, 5 dakika
Fonksiyon imzası yanlış       → refactor, 1 saat
Crate sınırı yanlış           → yeniden yapılandırma, 2 gün
Hata kodu formatı yanlış      → geriye uyumluluk kırılır, 1 hafta
HIR düğüm yapısı yanlış       → tip sistemi yeniden yazılır, 3 hafta
CDC semantiği yanlış          → kullanıcı güveni kaybı, telafi edilemez
Lisans yanlış                 → hukuki sorun, telafi edilemez

KURAL: Değiştirme maliyeti yüksekse → daha fazla düşün
       Değiştirme maliyeti düşükse  → hızlıca karar ver, ilerle
```

---

## Bölüm 2 — Üç Karar Seviyesi

### Seviye 1 — "Sadece Yap" (Belgeleme Gerektirmez)

```
Kriter: Kodu okuyunca açık, değiştirmesi ucuz

Örnekler:
  Değişken isimlendirmesi (domain_check, not dc)
  Fonksiyon böl / birleştir kararı
  Yorum satırı ekle / çıkar
  Test yardımcı fonksiyonu ekle
  Hata mesajındaki virgül yeri

Nasıl karar verilir:
  "Bunu bir saat sonra değiştirmem gerekirse ne olur?"
  → "5 dakika sürer" → Yap
  → "2 gün sürer" → Seviye 2'ye çık
```

### Seviye 2 — "Satır İçi Yorum" (Kodda Belgelenir)

```
Kriter: Neden böyle yapıldığı açık değil, ama sınırlı etki

Örnekler:
  Algoritma seçimi (neden bu O(n), O(n²) değil?)
  Özel durum işleme ("CDC kontrolünü neden burada değil orada?")
  Performans tercihi ("arena yerine Vec çünkü...")

Format:
  // KARAR: [ne yapıldı]
  // NEDEN: [gerekçe]
  // ALTERNATİF: [değerlendirilen diğer seçenek ve neden reddedildi]

Örnek:
  // KARAR: Domain karşılaştırmasını HIR lowering'de değil
  //        type_check aşamasında yapıyoruz.
  // NEDEN: Tip bilgisi bu aşamada tamamlandı, lowering'de eksik.
  // ALTERNATİF: Lowering'de kontrol — ama o aşamada domain
  //             bilgisi kısmen silinmiş oluyor.
```

### Seviye 3 — "ADR Gerekli" (Mimari Karar Kaydı)

```
Kriter: Geniş etki, değiştirmesi pahalı, gelecek kararları etkiler

Trigger'lar (bu soruların cevabı "evet" ise ADR yaz):
  □ Bu karar başka bir crate'i etkiliyor mu?
  □ Kullanıcıya görünür mi? (sözdizimi, hata kodu, araç davranışı)
  □ Lisans veya güvenlik ile ilgili mi?
  □ Başka mimari kararların üzerine inşa ediliyor mu?
  □ Değiştirmek 2 günden fazla sürer mi?

ADR şablonu: docs/adr/ADR-NNNN-başlık.md
```

---

## Bölüm 3 — Mimari Karar Kriterleri

Her ADR bu soruları yanıtlamalı:

### Soru 1 — Hangi Sorunu Çözüyor?

```
İyi:
  "CDC ihlali çalışma zamanında değil derleme zamanında
   yakalanmalı. Bunun için tip sistemine saat alanı kavramı
   entegre edilmeli."

Kötü:
  "Tip sistemi çok önemli, iyi olmalı."

Sorun net tanımlanmazsa karar da net olmaz.
```

### Soru 2 — Alternatifler Neler?

```
Her mimari karar için en az 2 alternatif değerlendirilmeli.

Örnek (CDC temsili için):
  Seçenek A — Phantom tip parametresi (Clash yolu):
    Signal<Domain, Type>
    Artı: Haskell tip sistemi ile mükemmel
    Eksi: Rust'ta verbose, her sinyal için tip parametresi
  
  Seçenek B — Anotasyon (Volt'un seçimi):
    data @Domain
    Artı: Görünür, yerel kontrol, Rust'a uygun
    Eksi: Derleme zamanı garantisi daha zor
  
  Seçenek C — Örtük (Verilog yolu):
    Saat bağlantısı elle yapılır
    Artı: Mevcut tasarımcıya tanıdık
    Eksi: CDC hatası sessizce geçer

"Sadece en iyisini seç" değil, "neden diğerleri reddedildi?"
```

### Soru 3 — Hangi Değerleri Önceliyor?

```
Volt'un değer hiyerarşisi (öncelik sırasıyla):

1. Doğruluk garantisi
   "CDC hatası asla sessizce geçmemeli"
   Bu değer diğer her şeyin üstünde.

2. Öngörülebilirlik
   "Aynı Volt kodu → aynı SV çıktısı"
   Sürpriz davranış kabul edilemez.

3. Hata mesajı kalitesi
   "Kullanıcı hatanın nedenini ve çözümünü görür"
   Kriptik hata → kullanıcı kaybı.

4. Geliştirici verimliliği
   "Az kod, çok anlam"
   Ama doğruluğu feda etmeden.

5. Performans
   "Yeterince hızlı" — ölçülmeden optimize etme.

Karar verirken: "Bu seçenek hangi değeri kırar?"
Değer 1 veya 2 kırılıyorsa → reddedilir.
```

### Soru 4 — Emsal Var mı?

```
Volt kararlarında referans alınan emsal listesi:

Rust (derleyici implementasyonu):
  rust-analyzer: arena tabanlı HIR, Salsa artımlı hesaplama
  rustc: hata kodu formatı (E0308), mesaj kalitesi
  cranelift: CLIF IR tasarımı

Modern HDL'ler:
  Clash: Signal<dom,a> CDC temsili
  Spade: Lineer tipler (port güvenliği)
  Filament: Timeline tipler (L2 referansı)
  Arch: todo! mekanizması, LL(1) gramer
  Veryl: Okunabilir SV çıktısı hedefi

Karar: "Bu seçenek emisal ile tutarlı mı?
         Tutarsızsa neden daha iyi?"
```

### Soru 5 — Geri Alınabilir mi?

```
Geri alınabilirlik matrisi:

                 Düşük etki    Yüksek etki
Geri alınabilir: Hızlıca karar ADR yaz, dikkatli ol
Geri alınamaz:   Yine de ADR   Uzun süre düşün,
                               birden fazla kişi onaylasın

"Bu kararı 6 ay sonra değiştirmem gerekirse ne olur?"
→ "1 günden az" → hızlı karar
→ "1 haftadan fazla" → ADR + dikkatli değerlendirme
```

---

## Bölüm 4 — Volt'un Temel Mimari Kararları (Referans)

Bunlar kilitlenmiş, değiştirmek için yeni ADR gerekir:

### K1 — CDC Temsili: @Domain Anotasyonu

```
Karar: Sinyal tipinde değil, anotasyon olarak
  data : u8 @Fast   ← tercih edilen
  
Neden phantom tip değil:
  Rust'ta her generic parametre her yerde taşınmalı
  Bu aşırı verbose üretir: Vec<Signal<Fast, u8>>
  Yerine: Vec<u8> ama kontrol ayrı katmanda

Neden örtük değil:
  CDC hatası sessizce geçmez
  Geliştiricinin domain farkındalığı şart

Değiştirmek ne kadar pahalı: Çok pahalı (tip sistemi yeniden)
ADR: docs/adr/ADR-0002-cdc-semantik.md
```

### K2 — HIR: Arena Tabanlı

```
Karar: typed-arena veya id-arena, Idx<T> handle deseni

Neden Rc<RefCell<>> değil:
  Döngüsel referanslar sorun (modül → port → modül)
  RefCell çalışma zamanı panik riski
  Salsa ile entegrasyon karmaşık

Neden Box<> ağacı değil:
  Döngüsel referanslar imkânsız
  Ama CFG (kontrol akış grafiği) döngülü → Box yetersiz

Neden arena:
  Tek allocation → cache dostu
  Döngüsel referans mümkün (sadece Idx<T> sakla)
  rust-analyzer'ın kanıtlanmış yaklaşımı
  Salsa ile entegre

Değiştirmek ne kadar pahalı: Çok pahalı
ADR: docs/adr/ADR-0006-hir-dugum-stratejisi.md
```

### K3 — Hata Kodu Formatı: E[cat][num]

```
Karar: E3001 (E + kategori basamağı + sıra numarası)

Kategori haritası:
  E0xxx: Sözdizimi hataları
  E1xxx: İsim çözümleme
  E2xxx: Tip hataları
  E3xxx: CDC / alan hataları ← Volt'un özgün katkısı
  E4xxx: Sürücü hataları
  W1xxx: Uyarılar

Neden sadece sıralı (E0001, E0002...) değil:
  Kategori koddan görünür
  "E3xxx = CDC" akılda kalıcı
  Araçlar kategoriye göre filtreleyebilir

Değiştirmek ne kadar pahalı: Pahalı (geriye uyumluluk)
ADR: docs/adr/ADR-0004-hata-kodu-format.md
```

### K4 — Çıktı: CIRCT → Okunabilir SV

```
Karar: CIRCT MLIR arka ucu, hedef: IEEE 1800-2017 SV

Neden doğrudan SV değil:
  String template kırılgan
  CIRCT: çok hedef (SV, VHDL, FIRRTL)
  CIRCT'in optimizasyon geçişleri değerli

Neden LLHD değil:
  Daha az topluluk, daha az araç
  CIRCT zaten LLHD fikirlerini absorbe etti

SV okunabilirlik şartı (Veryl referansı):
  Sinyal isimleri korunur
  always_ff/always_comb kullanılır
  FF seviyesinde elle düzenlenebilir

Değiştirmek ne kadar pahalı: Orta (volt-lower izole)
ADR: docs/adr/ADR-0010-circt-dialect-secimi.md
```

### K5 — Trit: Kısıtlı i2

```
Karar: Trit = i2 restricted to {-1, 0, +1}
  Aritmetik: {-1,0,+1} × {-1,0,+1} ⊆ {-1,0,+1} (kapalı)
  Depolama: 2 bit, işaretli
  
Neden enum değil:
  Donanım seviyesinde 2 bit depolama şeffaf olmalı
  Aritmetik native çalışmalı

Neden i3 veya özel tip değil:
  i2 Rust'ta doğal (aligned, LLVM dostu)
  Kısıtlama derleme zamanı kontrol edilir

Değiştirmek ne kadar pahalı: Orta
ADR: docs/adr/ADR-0003-trit-tipi.md
```

---

## Bölüm 5 — CLAUDE.md: Tam Volt Yapılandırması

Bu dosya proje kökünde yaşar. /goal oturumlarının temeli.

```markdown
# Volt HDL — Claude Code Yapılandırması

Son güncelleme: [tarih]
ADR referansı: docs/adr/ (tüm mimari kararlar burada)

---

## Proje Özeti

Volt: Saat alanı geçişlerini (CDC) derleme zamanında
      garantileyen, CIRCT tabanlı bağımsız HDL.

Derleyici: Rust (Cargo workspace)
Çıktı: IEEE 1800-2017 SystemVerilog
Arka uç: CIRCT/MLIR (melior crate üzerinden)
Lisans: Apache-2.0 OR MIT

---

## Derleme ve Test Komutları

# Hızlı kontrol (geliştirme sırasında)
cargo build --all
cargo test --all
cargo test -p volt-syntax      # sadece lexer/parser
cargo test -p volt-hir         # sadece HIR ve tip sistemi
cargo test -p volt-lower       # sadece CIRCT lowering

# Kalite kontrol (commit öncesi)
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings

# Entegrasyon testi (F0 sonrası)
cargo run --bin volt -- build tests/fixtures/counter.volt
verilator --lint-only tests/fixtures/out/counter.sv

# Snapshot testleri güncelle (kasıtlı değişiklik sonrası)
cargo insta review

---

## KESİN KURALLAR — Asla İhlal Etme

### Doküman Kilidi
- docs/spec/grammar.ebnf → OKUMA ONLY, değiştirme
- docs/spec/type-system.md → OKUMA ONLY, değiştirme
- docs/spec/error-codes.md → OKUMA ONLY, yeni kod ekleme
- docs/adr/*.md → OKUMA ONLY, değiştirme
Bu dosyalar ADR süreciyle değişir, /goal oturumunda değil.

### Test Kuralları
- Hiçbir testi silme, atlama, yorum satırına alma
- Test sayısı başlangıç değerinin ALTINA düşmemeli
- tests/ui/fail/ dosyaları: ilk satır hata kodunu içermeli
  Örnek: // E3001
- tests/ui/pass/ dosyaları: derlemeli VE SV üretmeli
- Testleri geçirmek için mantığı kısaltma veya atlatma

### Güvenlik
- unsafe kod YOK (library code'da)
  Sadece volt-driver'da ve #[allow(unsafe_code)] ile
- panic! YOK library crate'lerde
  Kullan: Result<T, VoltError> döndür
- unwrap() YOK (test kodu hariç)
  Kullan: expect("açıklayıcı mesaj") veya ?

### Bağımlılık
- Cargo.toml'a yeni bağımlılık EKLEME (PR gerektirir)
- cargo audit'ten geçmeyen bağımlılık YOK
- GPL lisanslı bağımlılık YOK (viral)

### CDC Semantiği
- @Domain anotasyonu semantiği: ADR-0002 referansı
  Bu semantiği değiştirme, sadece uygula
- TwoFlop<T>: standart CDC primitifi, stdlib'de
  Alternatif yol oluşturma

### Hata Kodları
- Yeni hata kodu EKLEME (docs/spec/error-codes.md güncellenmeden)
- Mevcut hata kodunu DEĞIŞTIRME veya SILME
- Hata mesajı formatı: codespan-reporting stili

---

## TERCIHLER — Mümkünse Uygula

### Kod stili
- İfadeler kısa: tek fonksiyon tek iş
- Struct field'ları anlamlı: src_domain değil sd
- Enum varyantları tam kelime: CdcViolation değil CdcV
- Error türü: VoltError enum, thiserror ile

### Hata mesajları (kullanıcıya gösterilen)
Her hata şunları içermeli:
  1. Hata kodu (E3001)
  2. Kısa başlık ("Saat alanı uyumsuzluğu")
  3. Kaynak konumu (dosya:satır:sütun)
  4. Offending kod snippet'i (codespan)
  5. Neden yanlış (1 cümle)
  6. Nasıl düzeltilir (help: satırı)
  7. Bakınız (ADR veya dokümantasyon linki)

### SV çıktı
- Sinyal isimleri: Volt kaynakla aynı (değiştirme)
- always_ff kullan, always değil
- Her modül açıklama yorumuyla başlar
- Okunabilirlik öncelikli (Veryl çıktısı referans)

### Test yazma
- Her yeni özellik: en az 1 pass + 1 fail testi
- pass testleri: beklenen SV snippet snap testi
- fail testleri: tam hata kodu (// E3001 ilk satırda)
- Test isimleri: ne test ettiğini açıklar
  iyi: test_cdc_violation_direct_assignment
  kötü: test1, test_error

---

## MİMARİ REFERANSLAR

### HIR Düğüm Yapısı (ADR-0006)
Arena tabanlı, Idx<T> handle deseni
  use volt_hir::arena::{Arena, Idx};
  let module_id: Idx<Module> = arena.alloc(Module { ... });
  // Idx<T> kopyalanabilir, Clone derive, PartialEq, Eq

### CDC Tip Kontrolü (ADR-0002)
@Domain anotasyonu → HIR'da DomainTag
CDC kontrolü: type_check aşamasında
  Signal { ty: Type, domain: Option<DomainId> }
  Farklı domain → E3001

### Hata Üretimi (ADR-0004)
  use volt_diagnostics::{Diagnostic, ErrorCode, Severity};
  Diagnostic::new(ErrorCode::E3001)
    .with_span(span)
    .with_message("Saat alanı uyumsuzluğu")
    .with_help("TwoFlop<T> kullanın")
    .emit();

### Crate Bağımlılık Yönü (ADR-0005)
  volt-driver → volt-lsp, volt-formal, volt-sim
           ↓
       volt-lower (CIRCT buraya izole)
           ↓
       volt-hir (tip sistemi, HIR)
           ↓
       volt-ast (AST düğümleri)
           ↓
  volt-syntax (lexer + parser)
           ↓
  volt-diagnostics (hata altyapısı)
           ↓
  volt-span (kaynak konumu)

Döngüsel bağımlılık YASAKTIR.

---

## /goal OTURUM TAMPONLARı

/goal başlatmadan önce kontrol et:
  git status → "nothing to commit" olmalı
  cargo test --all → baseline test sayısını not al
  verilator --version → kurulu mu kontrol et

/goal sonrasında kontrol et:
  git diff → her değişikliği gözden geçir
  cargo test --all → bağımsız çalıştır (Claude'dan değil)
  Test sayısı düştü mü? → kabul edilemez
  docs/spec/ değişti mi? → kabul edilemez
  Yeni bağımlılık eklendi mi? → geri al

/goal için ideal bitiş koşulu şablonu:
  /goal [GÖREV]
  
  Tamamlanma koşulu:
    cargo test --all exits 0
    [Volt-özgü koşul: verilator, snapshot, UI test sayısı]
  
  Kapsam: [hangi crate'ler ve dosyalar]
  DOKUNMA: docs/spec/, docs/adr/, Cargo.toml
  
  Dur: [N] tur sonra raporla.

---

## SIKÇA SORULAN DURUMLAR

S: "Yeni bir tip kuralı gerekiyor ama docs/spec/type-system.md'de yok"
C: Dur. /goal'u bitir. ADR yaz. Sonra yeni /goal başlat.

S: "Bu testi geçmek için özel durum ekliyorum"
C: Dur. Özel durum = semantik karar = ADR gerekiyor.

S: "Hata kodu formatı bu durumda uymuyor"
C: docs/spec/error-codes.md'ye bak. Yoksa ADR aç.

S: "CIRCT'te bunu yapmak zor, başka yol var"
C: volt-lower crate'i tampondur. CIRCT sorununu orada çöz.
   Alternatif: FIRRTL fallback (volt-lower'da hazır).

S: "Bu özellik için yeni bağımlılık gerekiyor"
C: Dur. Cargo.toml değiştirme. PR aç, tartış, sonra uygula.
```

---

## Bölüm 6 — Tasarım Kararları İçin Pratik Kılavuz

### "Şimdi mi, Sonra mı?" Akış Şeması

```
Yeni bir karar vermek gerekiyor.

Adım 1: Bu karar geri alınabilir mi?
  Evet ve ucuz → Yap, git (Seviye 1)
  Hayır veya pahalı → Adım 2'ye geç

Adım 2: Başka bir crate veya kullanıcı etkiliyor mu?
  Hayır → Satır içi yorum yaz, devam et (Seviye 2)
  Evet → Adım 3'e geç

Adım 3: Emsal var mı? (Rust ekosistemi, mevcut HDL'ler)
  Evet, açık emsal → Emu takip et, neden'i belgele
  Hayır veya çelişen → ADR yaz (Seviye 3)

Adım 4 (ADR): 2 alternatif değerlendirdin mi?
  Hayır → Önce alternatifleri listele
  Evet → ADR'ı tamamla, onay al, sonra uygula
```

### Voltda Sık Karşılaşılan Karar Durumları

```
DURUM: "HIR'da yeni bir düğüm türü eklemem lazım"
  → ADR-0006'yı oku (HIR düğüm stratejisi)
  → Mevcut düğüm türlerine uyuyor mu?
  → Uyuyorsa: ekle, satır içi yorum yaz
  → Uyumuyorsa: ADR güncelle, tartış

DURUM: "Bu sözdizimi iki şekilde parse edilebilir"
  → docs/spec/grammar.ebnf'e bak
  → Gramer net değilse → ADR yaz, gramer güncelle
  → Asla "parser hangisini seçerse seçsin" deme

DURUM: "Bu hata mesajı nasıl görünmeli?"
  → Seviye 2 (satır içi yorum yeterli)
  → Ama format değiştiriliyorsa → snapshot testleri güncelle
  → Format standardı CLAUDE.md'de yazılı

DURUM: "CIRCT'te X dialect yerine Y dialect kullansam?"
  → ADR-0010'a bak
  → Değişiklik vault-lower izole → orta maliyet
  → Alt crate bağımlılığı var mı? Yoksa Seviye 2
  → Varsa ADR

DURUM: "Yeni stdlib modülü eklesem?"
  → Kapsam içi (stdlib özellikleri ADR-0005'te)
  → Yeni crate gerektirmiyor → Seviye 2
  → Yeni bağımlılık gerektiriyor → PR gerekir

DURUM: "Bu davranış spec ile çelişiyor"
  → ASLA "spec'i düzelt" deme
  → Önce implementasyonu düzelt
  → Hâlâ çelişiyorsa → ADR ile spec güncellemesi
```

---

## Bölüm 7 — CLAUDE.md Versiyonlama

```
CLAUDE.md değişince ne olur?

Küçük güncelleme (yeni komut, yeni test kuralı):
  Direkt değiştir, commit mesajı: "chore(claude): X eklendi"
  Önceki /goal oturumları etkilenmez

Büyük güncelleme (kural eklendi, kural değişti):
  PR aç, tartış
  Commit mesajı: "feat(claude): CDC kuralı güncellendi"
  Tüm aktif /goal oturumlarını sonlandır

CLAUDE.md yaşayan bir belge:
  Proje büyüdükçe kurallar netleşir
  Ama her değişiklik izlenebilir olmalı
  "Kural neden değişti?" sorusu yanıtlanabilmeli

CLAUDE.md'nin OLMAYACAK kısımları:
  Spesifik implementasyon detayları (kod değişir)
  Kişisel tercihler (ekip kararları olmalı)
  "Bence X daha iyi" (kanıtlanmış kural olmalı)
```

---

## Özet: Karar Rehberi

```
Mimari karar verirken sor:

1. "Bu geri alınabilir mi?"
   → Pahalıysa: ADR yaz

2. "Hangi değeri önceliyor?"
   → Doğruluk > Öngörülebilirlik > Hata mesajı > Verimlilik

3. "Emsal ne söylüyor?"
   → rust-analyzer, Clash, Spade, Arch, Filament, Veryl

4. "Alternatifleri değerlendirdim mi?"
   → En az 2 alternatif, her birinin artısı/eksisi

5. "Değiştirme maliyeti ne?"
   → 1 günden az: git, belgele
   → 1 haftadan fazla: ADR, tartış, onay al

CLAUDE.md kuralları:
  KESİN KURALLAR: asla ihlal edilmez (/goal da dahil)
  TERCIHLER: mümkünse uygulanır, gerekçeyle sapılabilir

Temel prensip:
  "Semantik = ADR, uygulama = /goal"
  Bu ayrım korunduğu sürece proje tutarlı ilerler.
```
