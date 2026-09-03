# ADR-0024: SV Çıktı Dosyası Adlandırması — DECLFILENAME Çakışması

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-03
> Etkilenen: cli-contract.md §5, sv-mapping.md §12, volt-driver, volt-sv-emit
> Uygulama aşaması: F1 sonu

## Sorun

Verilator `--lint-only -Wall` doğrulaması (sv-mapping.md §13) DECLFILENAME
uyarısı üretiyor: bir SV dosyasının adı, içinde bildirilen modülün adıyla
eşleşmediğinde tetiklenir. Mevcut iki spec kuralı bu uyarıyı kaçınılmaz
kılıyor:

1. **cli-contract.md §5**: çıktı dosyası adı **kaynak dosyadan** türetilir —
   `volt build counter.volt` → `build/rtl/counter.sv`.
2. **sv-mapping.md İ1 (İsim Korunumu)**: modül adı **Volt kaynağından** aynen
   korunur — `module Counter { ... }` → `module Counter;`.

Sonuç: `counter.sv` dosyası içinde `Counter` modülü bulunur; Verilator
DECLFILENAME uyarısı verir ve "sıfır uyarı" hedefi (sv-mapping.md §13)
tutturulamaz. Ayrıca tek kaynak dosyada birden fazla modül bildirildiğinde
"hangi modül dosya adını belirler" sorusu tanımsız kalıyor.

## Seçenekler

**Seçenek A — Dosya adı modülden türesin (tek dosya korunur)**
`volt build counter.volt` çıktısı `build/rtl/Counter.sv` olur. Tek modüllü
dosyalarda DECLFILENAME'i çözer; ancak çok modüllü dosyalarda sorun sürer
(ikinci modül yine yabancı dosyada kalır) ve çıktı adı kaynak adından
kopar — kullanıcı `counter.volt` derleyip `Counter.sv` aramak zorunda kalır.

**Seçenek B — Modül başına bir dosya**
Her Volt modülü, modülün adını taşıyan kendi SV dosyasına yazılır:
`module Counter` → `build/rtl/Counter.sv`, `module UartTx` →
`build/rtl/UartTx.sv`. Dosya adı = modül adı olduğundan DECLFILENAME her
durumda susar; çok modüllü kaynak dosyalar da doğal biçimde çözülür.

**Seçenek C — Lint pragma ile bastır**
Üretilen dosyanın başına `/* verilator lint_off DECLFILENAME */` yazılır.
Uyarı susturulur ama neden ortadan kalkmaz; yasak listeye (sv-mapping.md
§11 ruhu) aykırı biçimde üretilen koda araç-özel pragma sızar ve diğer
araçlar (yosys, iverilog, ticari lintler) için sorun çözülmemiş olur.

## Karar: Seçenek B — Modül Başına Bir Dosya

Her Volt modülü, adı modül adıyla birebir aynı olan ayrı bir SV dosyasına
yazılır: `module Counter` → `build/rtl/Counter.sv`.

Gerekçe:

- DECLFILENAME kökten çözülür; pragma/bastırma gerekmez (Seçenek C'nin
  aksine neden gideriliyor, belirti değil).
- sv-mapping.md İ2 (1:1 modül eşlemesi) doğal uzantısını bulur:
  bir Volt modülü → bir SV modülü → **bir SV dosyası**.
- İ1 (isim korunumu) bozulmaz: modül adı da dosya adı da Volt kaynağındaki
  tanımlayıcıdan gelir; SDC/XDC kısıt dosyaları çalışmaya devam eder.
- Çok modüllü kaynak dosyalar belirsizlik olmadan desteklenir.
- Verilator, yosys ve ticari araçların dosya-başına-modül beklentisiyle
  (endüstri geleneği) hizalanır.

## Sonuçlar

- **cli-contract.md §5**: "Çıktı" satırı modül başına bir dosya listeler;
  `volt build counter.volt` çıktısı `build/rtl/Counter.sv` olur (kaynak
  `Counter` modülü içeriyorsa). `--out=<dosya>` bayrağı tek modüllü
  kaynaklarda çalışmaya devam eder; çok modüllü kaynakta `--out` kullanımı
  hata olur (davranış F1 sonunda cli-contract §5 güncellemesiyle netleşir).
- **sv-mapping.md §12**: dosya başlığındaki `// Kaynak:` satırı kaynak
  .volt dosyasını göstermeye devam eder; başlığa dosyanın hangi modülü
  içerdiği bilgisi eklenir.
- **volt-driver / volt-sv-emit**: emit aşaması modül listesi üzerinden
  dosya-başına-modül döngüsüne geçer. Uygulama F1 sonunda yapılır;
  bu ADR yalnız kararı kaydeder.
- **Testler**: `tests/fixtures/counter.expected.sv` ve CLI çıktı
  beklentileri F1 sonu uygulamasıyla birlikte güncellenir.

## Emsal

VHDL/Verilog ekosisteminde dosya-başına-modül fiili standarttır: Verilator
DECLFILENAME lint'i, Vivado'nun `read_verilog` akışı ve çoğu şirket lint
kuralı (ör. lowRISC style guide "one module per file") bu düzeni varsayar.
Java'nın "public class dosya adıyla eşleşir" kuralı aynı ilkenin kanıtlanmış
bir örneğidir.
