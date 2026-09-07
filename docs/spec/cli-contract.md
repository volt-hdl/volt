# Volt CLI Sözleşmesi

> STATÜ: BAĞLAYICI
> İlgili: UX Anayasası, `error-recovery.md`
> Aşama: F0'dan itibaren — CI için ÖNKOŞUL

---

## 0. Tasarım İlkeleri

```
İ1. UNIX GELENEĞİ
    Başarıda sessiz, hatada gürültülü.
    stdout = veri, stderr = tanılar.

İ2. MAKİNE + İNSAN
    Aynı bilgi iki formatta: renkli metin ve JSON.
    CI JSON okur, insan metin okur.

İ3. ÖNGÖRÜLEBİLİR ÇIKTI
    Aynı girdi → aynı çıktı (--release modunda bit-bit).

İ4. TEK KOMUT YETERLİ
    volt run design.volt → derle + simüle + sonuç göster.
    Kurulum, yapılandırma, ön adım yok.
```

---

## 1. Komut Listesi

```
volt new <isim>              Yeni proje oluştur
volt init                    Mevcut dizinde proje başlat

volt build [dosya]           Derle → SystemVerilog
volt check [dosya]           Sadece kontrol (çıktı üretme)
volt run [dosya]             Derle + simüle
volt test [filtre]           Test çalıştır
volt fmt [dosya]             Biçimlendir
volt explain <KOD>           Hata kodunu açıkla

volt sim [dosya]             Simülasyon                      [F4]
volt verify [dosya]          Formal doğrulama                [F4]
volt doc [dosya]             Belge üret                      [F5]
volt add <paket>             Bağımlılık ekle                 [V1]
volt diff <v1> <v2>          Anlamsal fark                   [V1]
volt migrate <dosya.v>       Verilog → Volt                  [V1]
```

---

## 2. Çıkış Kodları

**Kritik:** CI bu kodlara güveniyor.

```
KOD  ANLAM                          NE ZAMAN
────────────────────────────────────────────────────────────
0    Başarı                         Hata yok (uyarı olabilir)
1    Derleme hatası                 E-kodu üretildi
2    Kullanım hatası                Geçersiz argüman/bayrak
3    G/Ç hatası                     Dosya bulunamadı, yazma hatası
4    Yapılandırma hatası            Volt.toml bozuk/eksik
5    Test başarısızlığı             Testler çalıştı, bazıları kaldı
6    Doğrulama başarısızlığı        Formal karşı örnek buldu
101  İç hata (panic)                Derleyici hatası — bug report
────────────────────────────────────────────────────────────

Uyarılar çıkış kodunu ETKİLEMEZ (--deny-warnings hariç).
```

```rust
#[repr(i32)]
pub enum ExitCode {
    Success       = 0,
    CompileError  = 1,
    UsageError    = 2,
    IoError       = 3,
    ConfigError   = 4,
    TestFailure   = 5,
    VerifyFailure = 6,
    InternalError = 101,
}
```

---

## 3. Genel Bayraklar

Tüm komutlarda geçerli:

```
-h, --help              Yardım göster
-V, --version           Sürüm bilgisi
-v, --verbose           Ayrıntılı çıktı (-vv daha ayrıntılı)
-q, --quiet             Sadece hatalar
    --format=<f>        Çıktı formatı: human | json | short
    --lang=<l>          Tanı dili: en | tr (öncelik: bayrak > VOLT_LANG > Volt.toml [ui] lang > en)
    --color=<c>         Renk: auto | always | never
    --no-color          --color=never kısayolu
-j, --jobs=<N>          Paralel iş sayısı (varsayılan: CPU sayısı)
    --manifest=<yol>    Volt.toml konumu
    --target-dir=<yol>  Çıktı dizini (varsayılan: build/)
```

### Renk Davranışı

```
--color=auto (varsayılan):
  stdout terminal ise → renkli
  boru/dosya ise      → renksiz
  NO_COLOR ortam değişkeni varsa → renksiz
  CI=true ise         → renksiz
```

---

## 4. Çıktı Dizini Yapısı

```
build/
├── rtl/
│   ├── counter.sv
│   └── uart.sv
├── sim/                          [F4]
│   ├── counter.vcd
│   └── counter_tb
├── formal/                       [F4]
│   ├── counter.sby
│   └── counter.sva
├── constraints/                  [V1]
│   ├── design.sdc
│   └── design.xdc
├── sw/                           [V1]
│   ├── driver.rs
│   └── driver.h
├── docs/                         [F5]
│   └── design_spec.md
└── .volt-cache/                  artımlı derleme
```

`--target-dir` ile değiştirilebilir. `.gitignore`'a eklenmeli.

---

## 5. `volt build`

```bash
volt build [SEÇENEKLER] [DOSYA]
```

```
SEÇENEKLER:
    --release             Deterministik build + volt.lock
    --emit=<tür>          sv | json-ast | json-hir | none
    --target=<hedef>      generic | fpga-xilinx | fpga-intel |
                          asic-generic | asic-sky130
    --optimize=<seviye>   structure | aggressive
    --out=<dosya>         Tek dosya çıktısı
    --deny-warnings       Uyarıları hata say
```

### İnsan Çıktısı — Başarı

```
$ volt build counter.volt
   Derleniyor counter.volt
    Tamamlandı 0.12s
     Çıktı build/rtl/counter.sv (34 satır)
```

### İnsan Çıktısı — Hata

```
$ volt build design.volt
   Derleniyor design.volt

error[E3001]: iki farklı saat alanı doğrudan bağlanamaz
  ┌─ design.volt:12:14
   │
12 │     result = data
   │              ^^^^ 'data' → fast_clk alanında (satır 4)
   │     ^^^^^^ 'result' → slow_clk alanında (satır 5)
   │
   = neden: sinyal kararsız bir anda yakalanabilir
   = çözüm: result = sync(data, slow_clk)
   = daha fazla: volt explain E3001

warning[W1001]: kullanılmayan sinyal: 'temp'
  ┌─ design.volt:8:9
   │
 8 │     let temp = a + b
   │         ^^^^
   │
   = çözüm: '_temp' olarak yeniden adlandırın

     Hata: 1 hata, 1 uyarı nedeniyle derleme başarısız
```

Çıkış kodu: 1

### JSON Çıktısı

```bash
volt build --format=json design.volt
```

```json
{
  "version": "1",
  "command": "build",
  "success": false,
  "diagnostics": [
    {
      "code": "E3001",
      "severity": "error",
      "message": "iki farklı saat alanı doğrudan bağlanamaz",
      "spans": [
        {
          "file": "design.volt",
          "start": { "line": 12, "col": 14, "byte": 245 },
          "end":   { "line": 12, "col": 18, "byte": 249 },
          "label": "'data' → fast_clk alanında",
          "primary": true
        },
        {
          "file": "design.volt",
          "start": { "line": 12, "col": 5, "byte": 236 },
          "end":   { "line": 12, "col": 11, "byte": 242 },
          "label": "'result' → slow_clk alanında",
          "primary": false
        }
      ],
      "notes": [
        { "kind": "reason", "text": "sinyal kararsız bir anda yakalanabilir" }
      ],
      "help": "result = sync(data, slow_clk)",
      "suggestions": [
        {
          "span": {
            "file": "design.volt",
            "start": { "line": 12, "col": 14, "byte": 245 },
            "end":   { "line": 12, "col": 18, "byte": 249 }
          },
          "replacement": "sync(data, slow_clk)",
          "applicability": "machine-applicable"
        }
      ],
      "explain_url": "https://volthdl.org/errors/E3001"
    }
  ],
  "summary": { "errors": 1, "warnings": 1 },
  "artifacts": [],
  "duration_ms": 118
}
```

**`applicability` değerleri:**
```
machine-applicable  → otomatik uygulanabilir (LSP quick-fix)
maybe-incorrect     → öneri, kullanıcı kontrol etmeli
has-placeholders    → şablon, doldurulması gerekiyor
unspecified         → sadece bilgi
```

### Short Format

CI logları için tek satır:

```bash
volt build --format=short design.volt
```
```
design.volt:12:14: error[E3001]: iki farklı saat alanı doğrudan bağlanamaz
design.volt:8:9: warning[W1001]: kullanılmayan sinyal: 'temp'
```

---

## 6. `volt check`

Çıktı üretmeden sadece doğrulama. `build`'den hızlı.

```bash
$ volt check design.volt
    Kontrol design.volt
    Tamamlandı 0.08s
       Sonuç 0 hata, 2 uyarı

  todo! bulundu (2):
    design.volt:14  "LRU mu FIFO mu?"       [tip: bits<3>]
    design.volt:22  "politika belirlenecek" [kontrat]
```

**`todo!` listesi burada gösteriliyor** — `build --release`
bunları hata sayıyor (E9001).

---

## 7. `volt run`

```bash
$ volt run counter.volt
   Derleniyor counter.volt
   Simüle ediliyor (100 döngü)

   döngü  enable  count
   -----  ------  -----
       0       0      0
       1       1      1
       2       1      2
       ...

    Tamamlandı 0.34s
```

```
SEÇENEKLER:
    --cycles=<N>      Simülasyon döngü sayısı (varsayılan: 100)
    --vcd=<dosya>     Dalga formu kaydet
    --top=<modül>     Üst modül (varsayılan: tek modül)
```

---

## 8. `volt test`

```bash
$ volt test
   Derleniyor 12 test dosyası

running 39 tests
test ui::pass::01_minimal_module ... ok
test ui::pass::02_register_basic ... ok
test ui::fail::01_cdc_violation ... ok
test ui::fail::02_width_mismatch ... FAILED

failures:

---- ui::fail::02_width_mismatch ----
  beklenen: E2001
  alınan:   E2003
  konum:    tests/ui/fail/02_width_mismatch.volt:10

test result: FAILED. 38 passed; 1 failed
```

Çıkış kodu: 5

```
SEÇENEKLER:
    --filter=<desen>   Sadece eşleşen testler
    --update-snapshots Snapshot güncelle (dikkatli!)
    --nocapture        Test çıktısını göster
```

---

## 9. `volt explain`

UX Anayasası'nın "= daha fazla" satırının hedefi:

```bash
$ volt explain E3001
```

```
E3001: Saat Alanı Geçişi (CDC) İhlali

İki farklı saat alanındaki sinyaller doğrudan bağlanamaz.

NEDEN SORUN

Hedef flip-flop, kaynak sinyali kurulum (setup) veya tutma
(hold) penceresi içinde yakalarsa metastabil duruma girer.
Çıkış bir süre kararsız kalır ve sonra rastgele 0 veya 1'e
yerleşir.

Bu hata Verilog'da sessizce derlenir ve genellikle silisyumda
ortaya çıkar — hata ayıklaması en pahalı noktada.

ÖRNEK

  domain Fast { clock = posedge }
  domain Slow { clock = posedge }

  module Bad {
      in  data   : u8 @Fast
      out result : u8 @Slow

      result = data          // ✗ E3001
  }

ÇÖZÜM

  result = sync(data, slow_clk)    // ✓ iki flip-flop

ÇOK BİTLİ VERİ

sync() her biti bağımsız senkronize eder. 8-bit veri için
bitler farklı saat kenarlarında yakalanabilir:

  0b11111111 → 0b11110000 (geçersiz ara değer)

Çok bitli veri için:
  - Gray kodlama (sayaçlar için)
  - AsyncFifo (veri akışı için)
  - Handshake protokolü (kontrol için)

DAHA FAZLA
  https://volthdl.org/errors/E3001
  https://volthdl.org/guide/cdc
```

---

## 10. Ortam Değişkenleri

```
VOLT_LOG=<seviye>       error | warn | info | debug | trace
VOLT_BACKTRACE=1        Panik durumunda yığın izi
VOLT_TARGET_DIR=<yol>   Varsayılan çıktı dizini
VOLT_COLOR=<mod>        --color varsayılanı
NO_COLOR=1              Renk kapalı (standart)
CI=true                 CI modu: renksiz, ilerleme çubuğu yok
```

---

## 11. stdout / stderr Ayrımı

```
stdout:
  Üretilen veri (--emit=json-ast çıktısı)
  Test sonuç özeti
  volt explain metni
  --out=- ile SV çıktısı

stderr:
  Tanılar (hata + uyarı)
  İlerleme mesajları
  "Derleniyor...", "Tamamlandı"

Neden:
  volt build --emit=json-ast x.volt | jq '.items'
  → Tanılar boruyu kirletmiyor
```

---

## 12. İç Hata (Panic) Davranışı

```
thread 'main' panicked at crates/volt-hir/src/ty.rs:142

Volt'ta bir iç hata oluştu. Bu bir derleyici hatasıdır.

Lütfen bildirin: https://github.com/volthdl/volt/issues/new

Bilgi:
  volt sürümü: 0.1.0 (a3f2b1c 2026-08-31)
  platform:    x86_64-unknown-linux-gnu
  komut:       volt build design.volt
  girdi:       design.volt (1.2 KB)

Yığın izi için: VOLT_BACKTRACE=1 ile tekrar çalıştırın
```

Çıkış kodu: 101

---

## 13. CI Entegrasyonu

```yaml
# .github/workflows/ci.yml
- name: Volt kontrol
  run: |
    volt check --format=short --deny-warnings

- name: Volt derleme
  run: |
    volt build --release
    verilator --lint-only -Wall build/rtl/*.sv

- name: Volt testleri
  run: |
    volt test --format=json > test-results.json
```

**`--deny-warnings`:** Uyarıları hata sayar, çıkış kodu 1 olur.

---

## 14. Yardım Metni

```
$ volt --help

Volt HDL — saat alanı güvenli donanım tanımlama dili

KULLANIM:
    volt <KOMUT> [SEÇENEKLER]

KOMUTLAR:
    new       Yeni proje oluştur
    build     Derle ve SystemVerilog üret
    check     Hızlı kontrol (çıktı üretmez)
    run       Derle ve simüle et
    test      Testleri çalıştır
    fmt       Kaynak kodu biçimlendir
    explain   Hata kodunu açıkla

SEÇENEKLER:
    -h, --help          Bu yardımı göster
    -V, --version       Sürüm bilgisi
    -v, --verbose       Ayrıntılı çıktı
    -q, --quiet         Sadece hatalar
        --format=<f>    human | json | short
        --color=<c>     auto | always | never

ÖRNEKLER:
    volt new my_design
    volt build counter.volt
    volt run counter.volt --cycles=50
    volt explain E3001

Daha fazla: https://volthdl.org
```

---

## 15. Uygulama Sırası

```
F0 (2 gün):
  build, check komutları
  Çıkış kodları 0/1/2/3
  --format=human ve --format=json
  --color desteği

F1 (1 gün):
  explain komutu (5 hata kodu ile başla)
  --verbose / --quiet

F4 (2 gün):
  run, test, sim komutları
  Çıkış kodu 5, 6

F5 (2 gün):
  fmt, doc, new
  Tam yardım metinleri
```

---

## 16. Test Vektörleri

```
KOMUT                                  ÇIKIŞ  ÇIKTI
──────────────────────────────────────────────────────────
volt build counter.volt                0      counter.sv
volt build cdc_violation.volt          1      E3001
volt build --bad-flag                  2      kullanım hatası
volt build yok.volt                    3      dosya yok
volt build (Volt.toml bozuk)           4      config hatası
volt test (1 test kaldı)               5      test raporu
volt verify (karşı örnek)              6      counter-example
volt check --deny-warnings (uyarı var) 1      uyarı → hata
volt explain E3001                     0      açıklama metni
volt explain E9999                     2      bilinmeyen kod
──────────────────────────────────────────────────────────
volt build --format=json | jq .success  → false
volt build --format=short               → tek satır/tanı
NO_COLOR=1 volt build                   → ANSI kodu yok
```

---

## 17. Ek Hata Kodları (Bütçe, Sürüm, Yapı)

> Taşındı: Bu kodlar önceden `docs/design/Volt-Dil-Spesifikasyonu-v3.md`
> içinde tanımlıydı; bağlayıcı tanım artık burasıdır.

```
E5xxx  Davranışsal Kontratlar
  E5004  Kontrat ifadesi Bool değil

E6xxx  Bütçe ve Zamanlama
  E6001  Kaynak bütçesi aşıldı
  E6003  @false_path kanıtlanamadı (yol gerçekten var)
  E6004  @multicycle pipeline derinliğiyle uyuşmuyor

E7xxx  Sürüm
  E7001  SemVer ihlali: kırıcı değişiklik ama MAJOR bump yok
  E7002  abi_version değişmeden arayüz değişti

E9xxx  Yapı
  E9001  todo! ile release build yapılamaz
  E9002  Determinizm ihlali
```
