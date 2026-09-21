# Rekabet araştırması — HDL ekosistemi (Eylül 2026)

- **Tarih:** 2026-09-21. Aşağıdaki tüm kaynaklara bu tarihte erişildi.
- **Volt kaynağı:** depo `HEAD 2846bb9` (2026-09-21).
- **Kapsam:** 22 rakip ve Volt, 15 boyut (A–O) ve olgunluk.

**Gerekçe.** README'deki "Why not an existing HDL?" bölümü doğrulanmamış bilgiye dayanıyordu. Bu belge o bölümün yerine geçecek karşılaştırmanın kanıt tabanıdır. Farklılaşma kararı bu belgeye göre verilecek.

## Yöntem ve kanıt kuralı

**Hücre durumları.** Her hücre şu üç durumdan biridir:

| Kısaltma | Anlamı |
|---|---|
| `VAR` | Birincil kaynakta uygulama kanıtı var: belge, kaynak kod ya da örnek. |
| `VAR°` | Kısmi destek. Sınırı ayrıntı tablosunda yazılı. |
| `YOK` | Kaynak **açıkça** "desteklenmiyor", "kapsam dışı" ya da "sınırlama" diyor. Alıntı ayrıntı tablosunda. |
| `?` | BULUNAMADI. Arandı ama kanıt çıkmadı. Nerede arandığı ayrıntı tablosunda yazılı. `?` hiçbir zaman "yok" anlamına gelmez. |

Köşeli parantezdeki sayılar §7 Kaynakça'daki numaralardır.

**Araştırmanın yapılışı.** Araştırma bu oturumda paralel alt ajanlarla yürütüldü. Her ajan kendi grubu için:

- resmi belgeleri okudu,
- depoları sığ klonlayıp `grep` ile taradı,
- `gh api` ve GitLab API ile sürüm ve katkıcı sayılarını çekti,
- makale PDF'lerini sorguladı.

Hafızadan bilgi yazılmadı. Sonuçlar bu belgede birleştirildi ve boyut tanımları tüm rakiplere aynı biçimde uygulandı.

**Birleştirme sırasında uygulanan tutarlılık kuralları:**

- **I (formal akış):** Yalnız tasarımın **işlevsel** özelliklerini doğrulayan bir akış sayıldı. Güvenlik ispatı ya da tip denetimi için yapılan SMT çağrıları I değil, E veya F kapsamında değerlendirildi (Filament, PDL, SecVerilog, ChiselFlow, Caisson).
- **K (RTL–sürücü tutarlılığı):** Yalnız üretilen RTL ile üretilen sürücüyü **karşılaştıran** otomatik bir denetim sayıldı. "Aynı kaynaktan üretiliyor" ifadesi yapısal bir güvencedir, denetim değildir; bu yüzden `?` yazıldı ve notta belirtildi.
- **F (bilgi akışı):** Register erişim bitleri IFC sayılmadı. Bu kapsamdaki örnekler SpinalHDL `Secure` ve IP-XACT `accessPolicies`.
- **Birleşmemiş PR'lar:** "Yayında var" sayılmadı (Anvil PR #90/#91).

**Genel sınır.** Rakip derleyicilerin neredeyse hiçbiri çalıştırılmadı; kanıtlar belge ve kaynak koddan geliyor. Bu nedenle "hata mesajı var" türündeki iddialar kaynak koddaki tanım düzeyindedir. Ayrıntı için §8'e bakın.

---

## 1. Özet

### Volt'un gerçekten ayrıldığı yerler

Taranan 22 araç ve dil içinde ayrışma zayıftır. Tek başına bir boyutta ayrışma bulunamadı. Aday ayrışmalar iki tanedir ve ikisinin de güveni **orta** düzeydedir:

1. **Üretim odaklı bir HDL'de derleme zamanı IFC (F) ile derleme zamanı CDC'nin (A) bir arada olması.**
   - F yalnız akademik dillerde `VAR`: SecVerilog, ChiselFlow, Caisson, Sapper.
   - Bu akademik dillerin hiçbirinde CDC kanıtı bulunamadı. Son commit'leri 2011–2023 arasında. Sapper'ın kodu bulunamadı.
   - Üretim ve topluluk dillerinde F'nin hepsi `?`: Clash, Bluespec, Veryl, Arch, Spade, Chisel, Amaranth, SpinalHDL, Hardcaml, TL-Verilog, PipelineC ve ROHD, toplam 12 dil.
   - Ancak hiçbir kaynak IFC'yi açıkça "kapsam dışı" demiyor. Bu yüzden güven yüksek değil, orta.
   - Ayrıca akademik diller IFC'de Volt'tan daha ileridedir: bağımlı etiketler ve zamanlamaya duyarlı noninterference ispatları var.

2. **RTL ile sürücünün tutarlılığını denetleyen otomatik test (K).**
   - Taranan hiçbir araçta bulunamadı. En yakın olanlar:
     - PeakRDL-cheader ve Cheby: header'ın kendi içinde tutarlılık testi.
     - airhdl: kullanıcının kurduğu bir revizyon register'ı.
   - Volt'taki karşılığı yalnız **derleyicinin kendi test paketinde**, 3 sabit tasarım üzerinde çalışıyor. Kullanıcıya bir komut ya da derleme adımı olarak sunulmuyor.
   - Yani bu ayrışma şu an bir özellik değil, bir iç test.

### Volt'un bulunduğu ama ayrışmadığı alanlar

Bu boyutlarda Volt'ta `VAR` ya da `VAR°` var, ancak en az bir rakip eşit ya da daha ileri:

| Boyut | Eşit ya da daha ileri olan rakipler |
|---|---|
| A. CDC | Clash, Bluespec, Veryl, Arch, SpinalHDL; kısmi olarak Chisel 7 ve Hardcaml |
| C. Tek saatte anotasyonsuz çalışma | Neredeyse hepsi |
| D. Pipeline | Arch, Spade, SpinalHDL, TL-Verilog, PipelineC, ROHD, PDL |
| E. Gecikme tipleri | Clash `DSignal`, Filament, Anvil (ispatlı); kısmi olarak Arch |
| G. Kontrat / assertion | Chisel `FormalContract` Require/Ensure; Arch otomatik SVA |
| H. Ardışık kontrat | Chisel LTL, Arch, SpinalHDL |
| I. Formal akış | SpinalHDL (SymbiYosys), Arch (kendi SMT BMC motoru) |
| J. Register haritası | PeakRDL, rggen, SpinalHDL RegIf |
| L. SDC/XDC üretimi | Amaranth: `create_clock` ve CDC ilkelleri için false path / max delay; kısmi olarak SpinalHDL, PipelineC ve Clash |

### Volt'un geride kaldığı yerler

- **RDC:** Volt'ta `YOK`. Arch'ta 5 fazlı RDC denetimi var; Bluespec ve Clash'te kısmi.
- **Olgunluk:** Volt 18 günlük. Yazar kimliği 2, dış kullanıcı ya da silikon kanıtı yok, makale yok, kayıt defterinde (registry) yayınlanmış sürüm yok.
- **Simülatör:** Yerleşik simülatör yok, Verilator'a bağımlı. Clash, Bluespec, Veryl, Amaranth, SpinalHDL ve Hardcaml'da yerleşik simülatör var.
- **Çıktı:** Yalnız SV çıktısı var ve bazı yapılar emit edilemiyor (E0003). Clash, SpinalHDL ve Hardcaml VHDL de üretiyor.
- **Formal ve ardışık özellikler:** Bu alanda Arch, Chisel ve SpinalHDL daha geniş kapsamlı. Volt'ta yalnız `prev()` var; sequence yok, liveness yok, motor yalnız smtbmc.

### README için sonuç

"Why not an existing HDL?" bölümündeki dört iddiadan ikisi yanlış:

- **Veryl:** Derleme zamanı CDC denetimi **var** [26][28].
- **Arch:** `arch formal` bugün **var** [41][42]. Ayrıca RDC denetimi de var [38].

Diğer iki iddianın durumu:

- **Clash:** İddia kanıtla uyumlu. Tip seviyesinde CDC var [1].
- **Spade:** İddia büyük ölçüde doğru. Yerel saat alanı "Future Work" [50]. Formal akış bulunamadı.

---

## 2. Büyük tablo — rakipler × boyutlar

**Boyut kısaltmaları:**

| Kod | Boyut |
|---|---|
| A | Derleme zamanı CDC denetimi |
| B | RDC (reset alanı) denetimi |
| C | Tek saatte anotasyonsuz çalışma |
| D | Pipeline sözdizimi (otomatik aşama register'ı) |
| E | Tip seviyesinde gecikme/zamanlama (hizalama hatası) |
| F | Bilgi akışı / güven seviyesi |
| G | Dilde kontrat / assertion |
| H | Ardışık kontrat (`$past` benzeri) |
| I | Otomatik formal doğrulama akışı |
| J | Register haritasından RTL ve sürücü üretimi |
| K | RTL ile sürücü tutarlılık denetimi |
| L | SDC/XDC kısıt üretimi |
| M | Yerleşik simülasyon / test dili |
| N | LSP / editör desteği |
| O | Çıktı biçimi |

**Satır grupları:**

- **Üretim ve topluluk dilleri:** Volt, Clash, Bluespec, Veryl, Arch, Spade, Chisel, Amaranth, SpinalHDL, Hardcaml, TL-Verilog, PipelineC, ROHD
- **Akademik diller:** Filament, Anvil, PDL, SecVerilog, ChiselFlow, Caisson, Sapper
- **Register haritası araçları:** PeakRDL, IP-XACT/Kactus2, rggen

| | A | B | C | D | E | F | G | H | I | J | K | L | M | N | O |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **Volt** | VAR° [V] | YOK [V] | VAR [V] | VAR° [V] | VAR° [V] | VAR° [V] | VAR [V] | VAR° [V] | VAR° [V] | VAR° [V] | VAR° [V] | VAR° [V] | VAR° [V] | VAR° [V] | SV° [V] |
| Clash | VAR [1][2] | VAR° [1] | VAR° [3] | ? | VAR [4] | ? | VAR° [5] | VAR° [5] | VAR° [6] | VAR° [7] | ? [7] | VAR° [8] | VAR [3][9] | VAR° [10] | VHDL/V/SV [11] |
| Bluespec | VAR [15][16] | VAR° [15] | VAR [17] | ? | ? | ? | VAR° [18] | VAR° [18] | ? | VAR° [19] | ? | ? | VAR [20] | VAR° [21] | Verilog [20] |
| Veryl | VAR [26][27][28] | ? | VAR [26][29] | ? | ? | ? | VAR° [30] | VAR° [31] | ? | VAR° [32] | ? | ? | VAR [33] | VAR [34] | SV [35] |
| Arch | VAR [38][39] | VAR [38] | VAR° [39][40] | VAR [39] | VAR° [39] | ? | VAR [39][41] | VAR° [39] | VAR [41][42] | VAR° [43] | ? | VAR° [44] | VAR° [39][41] | VAR° [45] | SV [41] |
| Spade | YOK [50][52] | ? | YOK [54][53] | VAR [55] | VAR° [55] | ? | VAR° [54] | ? | ? | ? | ? | ? | VAR° [56] | VAR [57] | SV [58] |
| Chisel (+CIRCT) | VAR° [74][75] | ? | VAR [77] | VAR° [78] | ? | ? | VAR [79] | VAR [79] | YOK [80] | VAR° [81] | ? | ? | VAR [82] | ? | SV [84] |
| Amaranth | ? | ? | VAR [65] | ? | ? | ? | VAR° [64] | YOK [67] | YOK [68] | VAR° [69] | ? | VAR [70] | VAR [71] | ? | Verilog/RTLIL [72] |
| SpinalHDL | VAR [88] | ? | VAR [89] | VAR [90] | ? | ? [91] | VAR [92] | VAR [92] | VAR [92] | VAR [93] | ? | VAR° [94] | VAR [95] | ? [96] | VHDL/V/SV [97] |
| Hardcaml | VAR° [154] | ? | VAR [155] | VAR° [155] | ? | ? | VAR [156] | VAR° [156] | VAR° [157] | VAR° [158] | ? | VAR° [159] | VAR [160] | VAR° [160] | V/VHDL/SV° [160] |
| TL-Verilog | YOK [146] | ? | VAR [145] | VAR [145][147] | VAR° [145][147] | ? | VAR° [146] | VAR° [145][146] | ? | ? | ? | ? | VAR° [151] | VAR° [151] | SV/V [146][148] |
| PipelineC | VAR° [163] | ? | VAR° [164][165] | VAR [164] | VAR° [164] | ? | VAR° [166] | ? | ? | VAR° [167] | ? | VAR° [165] | VAR [166][168] | ? | VHDL (+V) [168] |
| ROHD | ? [170] | ? | VAR [171] | VAR [171] | ? | ? | VAR° [172] | ? | ? | VAR° [173] | ? | ? | VAR [171][175] | VAR° [174] | SV [172] |
| Filament | YOK [101] | ? | VAR [101] | YOK [101] | VAR [101] | ? | VAR° [103] | ? | ? | ? | ? | ? | VAR° [103] | VAR° [103] | Verilog/Calyx [103] |
| Anvil | ? | ? | VAR [105] | YOK [106] | VAR [104] | ? | ? [105] | ? | ? [105] | ? | ? | ? | VAR° [105] | VAR° [107] | SV [104] |
| PDL | ? | ? | VAR [108] | VAR [108] | YOK [108] | ? [109] | ? | ? | ? | ? | ? | ? | VAR° [109] | ? | BSV [108] |
| SecVerilog | ? | ? | VAR° [110] | ? | ? | VAR [110][113][114] | ? | ? | ? | ? | ? | ? | ? | ? | Verilog [111] |
| ChiselFlow | ? | ? | VAR [115][116] | ? | ? | VAR [115][116] | VAR° [116] | ? | ? | ? | ? | ? | VAR° [116] | ? | Verilog [115][117] |
| Caisson | ? | ? | VAR [118] | ? | ? | VAR [118] | ? | ? | ? | ? | ? | ? | ? | ? | Verilog [118][119] |
| Sapper | ? | ? | VAR [120] | ? | ? | VAR [120] | ? | ? | ? | ? | ? | ? | ? | ? | Verilog [120] |
| SystemRDL + PeakRDL | YOK [123] | ? | VAR [124] | ? | ? | ? | YOK [125][126] | ? | ? | VAR [128] | ? [129][130] | ? | VAR° [130] | VAR° [131] | SV/VHDL/C/UVM/… [132] |
| IP-XACT / Kactus2 | ? | ? | ? | ? | ? | ? [134] | ? | ? | ? | VAR° [135][137] | ? | ? [134][138] | YOK [135] | VAR° [135] | IP-XACT/C/SVD/… [137] |
| rggen | ? | ? | VAR [142] | ? | ? | ? | ? | ? | ? | VAR [141][143] | ? [143] | ? | VAR° [141] | VAR° [144] | SV/V/Veryl/VHDL/C [141] |

**Tablo notları.** Ayrıntılı açıklama, sınırlar ve aranan yerler §2a'da.

- **Volt:** Kanıtlar depo dosya yollarıdır [V]; bkz. §2a.
- **Register haritası araçları:** Bu satırlar HDL değil; A–I boyutlarının çoğu bu araçlar için anlamsız ya da kapsam dışı.
- **Sınırlı taranan diller:** HazardFlow, SecChisel, SUS, XLS/DSLX, Kanagawa, Silice, MyHDL ve RHDL tabloya alınmadı; bkz. §2b.

**Satır sayısı:** 22 rakip ve Volt, toplam 23 satır × 15 boyut.

### 2a. Hücre ayrıntıları

Biçim: `Boyut — Durum — not — kaynak / aranan yer`.

#### Volt [V] (depo, HEAD 2846bb9)

- **A — VAR°.**
  - Derleme zamanında sinyal-alan çıkarımı yapılıyor. Hata kodları E3001, E3002, E3009–E3014.
  - Kaynaklar: `crates/volt-hir/src/domain/assign.rs:150`, `domain/sync.rs`, `tests/ui/fail/01_cdc_violation.volt`; ADR-0002, 0027, 0047, 0049.
  - Onaylı geçiş yolu yalnız `sync()`/`sync3()` ve 4 yerleşik çift saatli primitif. Kullanıcının kendi yazdığı senkronizör tanınmıyor.
  - Çok bitli `sync()` hata değil, yalnız uyarı veriyor (W3003). Gray kod doğrulanmıyor.
  - `sync()` için bazı biçimler SV emit aşamasında E0003 veriyor (`volt-sv-emit/src/lib.rs:913-959`).
- **B — YOK.**
  - `README.md:352`: "Reset-domain (RDC) checks — E3003 reserved, not enforced". ADR-0002:7.
  - `crates/` altında E3003 üreten kod yok.
- **C — VAR.**
  - Kaynaklar: `docs/spec/domain-inference.md:70-113` (K2); golden test `tests/fixtures/counter.volt`.
  - İki saat varken anotasyonsuz sinyal E3010 veriyor.
- **D — VAR°.**
  - ADR-0038 ve `examples/riscv_pipeline.volt`. Aşama register'ı ve stall/flush telleri otomatik üretiliyor.
  - Aşamalar arası yalnız skaler değer taşınıyor; N bir literal olmalı, generic yok.
  - Stall/flush ayrıklığı doğrulanmıyor.
  - `README.md:353` hâlâ "reserved" diyor; bu satır bayat.
- **E — VAR°.**
  - ADR-0037: `Delayed<T,N>`, `@strict_timing`, E5010. Kaynak `crates/volt-hir/src/timing.rs`.
  - Opt-in. Geri besleme, kontrol sinyalleri, instance portları ve kontratlar denetlenmiyor.
  - Açık anotasyonlu `let` bilinen bir kaçış yolu.
- **F — VAR°.**
  - ADR-0052 ve `crates/volt-hir/src/trust.rs`: `public < confidential < secret` sıralaması, örtük akış izleme, `declassify(expr,"gerekçe")`.
  - Alt modül özeti tutucu. Zamanlama ve güç yan kanalları kapsam dışı (ADR-0052:172-188).
  - Kanıtlanmış bir noninterference teoremi yok.
- **G — VAR.**
  - ADR-0011 ve ADR-0014: `requires`/`ensures`/`invariant`/`cover`/`assert`/`assume`. SVA çıktısı `volt-sv-emit/src/sva.rs` üzerinden.
  - Kontratlar simülasyonda koşmuyor (ADR-0040:131-132).
- **H — VAR°.**
  - ADR-0040: `prev(x,N)`, kaynak `volt-sv-emit/src/past.rs`.
  - Sequence, `##` ve liveness yok. Arama `grammar-full.ebnf`, ADR-0011 ve ADR-0040 üzerinde yapıldı.
- **I — VAR°.**
  - `volt verify`, kaynaklar `crates/volt-driver/src/verify.rs` ve `volt-sv-emit/src/sby.rs`. SymbiYosys, Yosys ve SMT çözücü dış araç olarak gerekli; Windows'ta Docker ya da WSL.
  - Motor yalnız smtbmc. EBMC bulunamadı.
  - CI'daki formal işi `continue-on-error` (`.github/workflows/ci.yml:53-58`).
- **J — VAR°.**
  - ADR-0044 ve ADR-0053: `@mmio` tanımından AXI4-Lite RTL ile Rust, C, JSON ve Markdown çıktısı. Kaynak `crates/volt-sw-emit`.
  - Yalnız AXI4-Lite ve 32 bit. Reset değeri her zaman 0. SVD ve IP-XACT çıktısı yok.
- **K — VAR°.**
  - Test `crates/volt-driver/tests/sw_emit_tests.rs::regmap_offsets_match_rtl_address_decode_for_every_design` (ADR-0053 §7).
  - Yalnız derleyicinin test paketinde ve 3 sabit tasarımda çalışıyor. Kullanıcı tasarımı için bir komut ya da derleme adımı yok.
- **L — VAR°.**
  - ADR-0054, `--emit=sdc,xdc`. `create_clock`, `set_clock_groups -asynchronous`, sync köprüleri için `set_false_path` ve XDC `ASYNC_REG` üretiliyor.
  - OpenSTA ile tek seferlik, yerel bir okuma yapıldı; CI'da çalışmıyor. XDC Vivado'da denenmedi.
- **M — VAR°.**
  - ADR-0033 ve ADR-0058: `volt test`, test dili C++ testbench'e çevriliyor.
  - Yerleşik simülatör yok, Verilator dış bağımlılık. Kaynak `crates/volt-driver/src/sim/mod.rs:1-4`.
- **N — VAR°.**
  - `crates/volt-lsp/src/lib.rs:100-124`: diagnostics, hover, completion, definition, document symbol.
  - Rename, references ve formatting yok. VS Code eklentisi yayımlanmamış (`README.md:332-333`).
- **O — VAR°.**
  - Çıktı okunabilir SV (ADR-0008, ADR-0012).
  - E0003 veren yapılar: `Trit`, açık reset portu, blok içi `let`, match guard, dosya dışı instance. Kaynak `volt-sv-emit/src/lib.rs`, satırlar 1052–1535.
  - ADR-0057: 2026-09-20'ye kadar sessiz bir öncelik hatası vardı; düzeltildi.

#### Clash

Kaynak kısaltması: `CC` = https://github.com/clash-lang/clash-compiler/blob/65055b63ec4d223260bdda0deba445a5fb430ee3/

- **A — VAR.**
  - `Signal dom a` tipindeki alan parametresi alanları ayırıyor. Geçiş `unsafeSynchronizer` ve senkronlayıcılar üzerinden [1][2].
  - `unsafeSynchronizer`'ın senkronlayıcı olmadan doğrudan kullanımı tip denetiminden geçiyor.
- **B — VAR°.**
  - `Reset dom`, reset türü ve polaritesini alanda taşıyor [1].
  - Belge: aynı alandaki iki asenkron reset için "no guarantee that they actually originate from the same source".
- **C — VAR°.** `HiddenClockResetEnable` kısıtı tip imzasında görünüyor. Top entity saati açıkça alıyor [3].
- **D — ?** Aranan: `clash-prelude/src` içinde `pipelin`/`retim`.
- **E — VAR.**
  - `DSignal dom delay a`; `delayed`/`delayN` gecikme tipini değiştiriyor [4].
  - Tip hatası derlenerek denenmedi.
- **F — ?** Aranan: prelude, lib, docs içinde `information flow`/`taint`/`noninterference`.
- **G — VAR°.**
  - `Clash.Verification`: `assert`, `cover`, `assume` [5].
  - "experimental"; "Clash simulation support not yet implemented".
- **H — VAR°.** `next`, `nextN`, `timplies`, `eventually` var; `$past` karşılığı bulunamadı [5].
- **I — VAR°.**
  - `RenderAs YosysFormal` SymbiYosys'e uygun çıktı veriyor [6].
  - `sby` yalnız derleyicinin kendi test paketinde çalıştırılıyor.
- **J — VAR°.**
  - Harici paket `clash-protocols-memmap`: harita devreden türetiliyor, JSON üzerinden C ve Rust üretiliyor [7].
  - Çekirdek derleyicide yok.
- **K — ?** Pakette adres doğrulayıcısı var (`AbsAddress.hs`), ama RTL ile sürücüyü karşılaştıran bir denetim yok [7].
- **L — VAR°.** Yalnız `create_clock` satırları üretiliyor; kod yorumu: "Currently this limited to the names and periods of clocks" [8].
- **M — VAR.** GHCi simülasyonu ve `stimuliGenerator`/`outputVerifier` [3][9].
- **N — VAR°.** HDL'ye özgü bir LSP yok; Haskell dil sunucusu HLS kullanılıyor [10].
- **O — VAR.** VHDL, Verilog ve SystemVerilog; üretilen adlar `c$case_alt_24` biçiminde [11].

#### Bluespec (BSV ve BH, `bsc`)

Kaynak kısaltması: `BSC` = https://github.com/B-Lang-org/bsc/blob/3ea4b2baa4daaceefcbedc2c14d36e933662f24e/

- **A — VAR.** G0007 "Reference across clock domain" [15]; Clocks kütüphanesinde senkronlayıcılar var [16].
- **B — VAR°.**
  - Reset'in alanlar arası kullanımı hata veriyor [15].
  - Birden fazla reset'in karışması yalnız uyarı: G0043 ve G0083 [15].
- **C — VAR.** "Each module has an implicit clock and reset" [17].
- **D — ?** Aranan: BSV_lang.tex ve BH_lang.tex içinde `pipelin`, user_guide, LibDoc.
- **E — ?** Aranan: referans kılavuzları ve Error.hs tanıları.
- **F — ?** Aranan: doc ve src/comp içinde `taint`/`information flow`.
- **G — VAR°.**
  - `staticAssert` ve `dynamicAssert` var; `-check-assert` bayrağı gerekiyor [18].
  - OVL sarmalayıcıları var [18].
- **H — VAR°.** OVL üzerinden `bsv_assert_next`, `cycle_sequence` [18]. Dilde `$past` karşılığı bulunamadı.
- **I — ?** Aranan: doc, src, README içinde `symbiyosys`/`smtbmc`/`ebmc`.
- **J — VAR°.** CBus register erişim RTL'i üretiyor; sürücü üretimi bulunamadı [19].
- **K — ?** Aranan: doc tamamı, CBus.tex.
- **L — ?** Aranan: user_guide içinde `sdc`/`xdc`/`timing constraint`.
- **M — VAR.** Bluesim ve StmtFSM [20].
- **N — VAR°.** Depoda emacs ve vim modları var; LSP'ler üçüncü parti ve erken aşamada [21].
- **O — VAR.** Verilog ve Bluesim C++ [20].

#### Veryl

- **A — VAR.**
  - `'a`/`'b` saat alanı anotasyonu, `mismatch_clock_domain` hatası ve `unsafe (cdc) {}` bloğu [26][27][28].
  - Sınırlar:
    - Anotasyonsuz (`None`) alan her alanla uyumlu sayılıyor (`symbol.rs`).
    - `unsafe (cdc)` bloğunun içeriği denetlenmiyor.
    - Çok bitli geçiş denetimi bulunamadı.
- **B — ?** Aranan: kitapta `reset.?domain`/`rdc`, `crates/**`, `gh search issues "reset domain"`.
- **C — VAR.** "If there is single clock only in a module, the annotation can be omitted" [26]; ayrıca [29].
- **D — ?** Aranan: `veryl.par` anahtar kelimeleri, kitapta `pipeline` (0 sonuç).
- **E — ?** Aranan: kitapta `latency`, parser anahtar kelimeleri.
- **F — ?** Aranan: kitapta `information flow`/`taint`/`security`.
- **G — VAR°.**
  - `$assert` yalnız `#[test]` modüllerinde kullanılabiliyor [30].
  - Issue #1812 "Compile-time assertions" açık.
- **H — VAR°.** `$past`/`$rose` SV'ye aynen geçiriliyor; yerleşik bir property yapısı yok [31].
- **I — ?** Aranan: kitapta `formal`/`symbiyosys`/`sby`, kaynak, issue'lar.
- **J — VAR°.** Üçüncü taraf RgGen (`rggen-veryl`) C header da üretiyor [32].
- **K — ?** Aranan: kitap, kaynak, RgGen README.
- **L — ?** Aranan: kitapta ve kaynakta `sdc`/`xdc`. `[synth]` bölümündeki frekans yalnız kestirim için.
- **M — VAR.** `#[test]` ile yerleşik simülatör; arka uçlar cc, cranelift ve interpret [33].
- **N — VAR.** `veryl-ls` [34].
- **O — VAR.** Okunabilir SV ve source map [35].

#### Arch (`arch-hdl-lang/arch-com`)

- **A — VAR.**
  - `Clock<Domain>` tipi. Denetimler: seq→seq, comb→seq, instance sınırı ve yakınsayan senkronizör [38][39].
  - Sınırlar:
    - Denetim yalnız en az 2 alan bildiren modülde çalışıyor.
    - `pragma cdc_safe` bütün modülü muaf tutuyor ve tanı basmıyor.
    - v0.72.2 ve öncesinde `rdc_safe` CDC denetimini de kapatıyordu; PR #1025 ile düzeltildi [47].
- **B — VAR.**
  - 5 faz; `tests/rdc` altında 48 senaryo [38].
  - Senkron resetin alanlar arası kullanımı işaretlenmiyor.
- **C — VAR°.** `domain` bloğu gerekmiyor, ama `Clock<SysDomain>` yazmak zorunlu [39][40].
- **D — VAR.** Birinci sınıf `pipeline`; stall, flush ve forward bildirimsel [39].
- **E — VAR°.**
  - `pipe_reg<T,N>` ve pipelined operatörlerde, farklı çevrimdeki işlenenler hata veriyor [39].
  - Genel bir gecikme tip sistemi olduğuna dair kanıt bulunamadı.
- **F — ?** Aranan: spec'te `information.flow`/`taint`/`declassif`/`trust`.
- **G — VAR.**
  - `assert`/`cover`/`assume` ve FIFO, counter, FSM, indeks ve handshake için otomatik SVA [39][41].
  - `arch sim` assert'leri çalışma zamanında denetlemiyor.
- **H — VAR°.**
  - `past`, `rose`, `fell`, `|=>`, `##N` var; yalnız assert içinde kullanılabiliyor [39].
  - Sequence kompozisyonu ve liveness bilerek kapsam dışı.
- **I — VAR.**
  - `arch formal`: SMT-LIB2 ile BMC; z3, boolector ve bitwuzla destekleniyor [41][42].
  - Sınırlı (bounded) doğrulama; `.sby` üretimi yok.
  - README "no sub-inst" diyor, COMPILER_STATUS "one level" diyor; iki kaynak çelişiyor.
- **J — VAR°.** `rdl2arch` SystemRDL'den `.arch` RTL üretiyor; sürücü üretimi bulunamadı [43].
- **K — ?** Aranan: rdl2arch README, spec'te `csr|register map`.
- **L — VAR°.** Yalnız `set_multicycle_path` üretiliyor; `create_clock` yok [44].
- **M — VAR°.**
  - `arch sim` C++ model üretiyor; test tezgahı C++ ya da Python ile yazılıyor [41].
  - README "no built-in testbench constructs" diyor, spec §20.5 `testbench` tanımlıyor; iki kaynak çelişiyor [39].
- **N — VAR°.** Yalnız sözdizimi renklendirme ve bir MCP sunucusu var; LSP bulunamadı [45].
- **O — VAR.** Okunabilir SV [41].

#### Spade

- **A — YOK.**
  - TRETS makalesi "Native clock domains … prime candidate" diyerek bunu Future Work'e koyuyor [50].
  - Derleyicide `domain` geçen bir kod yok. RFC tasarımı [52] uygulanmamış. CDC yalnız stdlib ilkelleriyle [51].
- **B — ?** Aranan: `git grep` domain/reset, book, CHANGELOG, issue'lar.
- **C — YOK.**
  - Söz dizimi `reg(clk)` biçiminde, saat zorunlu [54].
  - Örtük saat önerisi (issue #118) 2022'den beri açık [53].
- **D — VAR.** `pipeline(N)`, `reg;` ve `stage()`; dinamik pipeline'lar da var [55].
- **E — VAR°.** "Pipeline depth mismatch" ve "use before it is ready" hataları var; yalnız pipeline içinde geçerli [55].
- **F — ?** Aranan: `git grep taint|secret|declassif`, book, makale.
- **G — VAR°.** `assert` yalnız simülasyonda ve yalnız Icarus ile çalışıyor [54].
- **H — ?** Aranan: `git grep \bpast\b`, book, CHANGELOG.
- **I — ?** Aranan: swim `src/` içinde `formal|sby|bmc`, issue'lar, makale.
- **J, K — ?** Aranan: `git grep register map|regmap|mmio|svd`, book, swim.
- **L — ?** Aranan: spade ve swim içinde `create_clock|.sdc|.xdc`, TRETS makalesi.
- **M — VAR°.** swim testleri var, ama "Tests are not written in Spade itself", yani test dili Spade değil [56].
- **N — VAR.** `spade-language-server`; v0.20'de otomatik tamamlama eklendi [57].
- **O — VAR.** Tek dosya `build/spade.sv` [58].

#### Chisel (+CIRCT/firtool)

- **A — VAR°.**
  - Chisel 7 alan sistemi: `associate` ve `unsafeCast` [74]. firtool `InferDomains` "illegal domain crossing" hatası veriyor [75].
  - firtool'da `-domain-mode` varsayılanı `Strip`, yani denetim varsayılan olarak kapalı [75].
  - Alanları kullanıcı elle bağlıyor.
- **B — ?** Aranan: chisel ve circt içinde `rdc|resetdomain`, reset.md.
- **C — VAR.** Örtük saat ve reset [77].
- **D — VAR°.** Yalnız `util.Pipe` ve `ShiftRegister`; otomatik bir pipeline oluşturucu yok [78].
- **E — ?** Aranan: `latency|retim`; yalnız CIRCT Arc geçişi çıktı, o da tip değil.
- **F — ?** Aranan: `taint|secur|information.?flow`.
- **G — VAR.** `FormalContract { RequireProperty; EnsureProperty }` ve layers [79].
- **H — VAR.** `chisel3.ltl`: `past`, `Delay`, `|->`, `|=>` [79].
- **I — YOK (Chisel 7 çekirdeği).**
  - Alıntı: "Formal verification is currently unsupported in this compatibility layer" [80].
  - Eski chiseltest arşivlendi. `circt-bmc` ayrı bir araç [80].
- **J — VAR°.** rocket-chip regmapper RTL ve JSON üretiyor; C header bulunamadı [81].
- **K — ?** Aranan: chisel ve regmapper.
- **L — ?** Aranan: chisel ve circt içinde `sdc|xdc|timing.?constraint`.
- **M — VAR.** ChiselSim; arka uç Verilator ya da VCS [82].
- **N — ?** Chisel'e özgü LSP bulunamadı. CIRCT'deki LSP SV içindir [83]. Aranan: installation.md, `gh search code`.
- **O — VAR.** FIRRTL ve firtool üzerinden SV [84].

#### Amaranth

- **A — ?**
  - Çoklu alan birinci sınıf, ama alanlar arası kullanım denetimi bulunamadı.
  - CDC yalnız kütüphane düzeyinde [62]. Elaborasyon hataları alan ya da sürücü çakışmasıyla ilgili [63].
  - Aranan: docs içinde `cross|cdc`, `_ir.py` raise satırları, issue #317/#328/#212/#227.
- **B — ?**
  - Belge sorumluluğu kullanıcıya bırakıyor: "must be synchronous to the domain" [64].
  - Aranan: guide, `_ir.py`, issue'lar.
- **C — VAR.** `m.d.sync`; eksik `sync` alanı otomatik oluşturuluyor [65].
- **D — ?** `lib.stream` register eklemiyor. Issue #213 (2019) açık [66].
- **E, F — ?** Aranan: guide, reference, stdlib, changes.
- **G — VAR°.** `Assert`/`Assume`/`Cover` var; kontrat sözdizimi yok [64].
- **H — YOK.** Alıntı: "Removed: … ast.Past, ast.Stable, ast.Rose, ast.Fell" (0.5) [67].
- **I — YOK.**
  - Bakımcının ifadesi: "the entire scope of formal verification … is 'it can generate verilog or rtlil with assert/assume statements'" (issue #505) [68].
  - İfade 2020 tarihli ve issue hâlâ açık. Daha yeni bir kanıt bulunamadı.
- **J — VAR°.** `amaranth-soc` CSR RTL'i üretiyor; sürücü üretimi bulunamadı [69].
- **K — ?** Aranan: amaranth, amaranth-soc.
- **L — VAR.** Platform katmanı `create_clock`, CDC ilkeleri için `set_false_path`/`set_max_delay` ve XDC/SDC üretiyor [70].
- **M — VAR.** `amaranth.sim` [71].
- **N — ?** Aranan: web araması "Amaranth HDL language server", org sayfası.
- **O — VAR.** Yosys üzerinden Verilog; RTLIL ve CXXRTL [72].

#### SpinalHDL

- **A — VAR.**
  - Elaborasyon zamanında, varsayılan olarak açık: "CLOCK CROSSING VIOLATION" (`PhaseCheckCrossClock`) [88].
  - İstisna yolları: `crossClockDomain` etiketi, `setSynchronousWith`, `BufferCC`.
- **B — ?** Aranan: `gh search code "RESET CROSSING"`, `crossResetDomain`, Phase.scala.
- **C — VAR.** "By default, a ClockDomain is applied to the whole design" [89].
- **D — VAR.** `spinal.lib.misc.pipeline`: `StageLink` register'ı kendisi ekliyor [90].
- **E — ?**
  - Tip seviyesinde karşılık bulunamadı. Pipeline API'si hizalamayı yapı yoluyla sağlıyor.
  - Aranan: pipeline introduction, `latency` grep.
- **F — ?**
  - `regif.Secure` bir erişim biti, IFC değil [91].
  - Aranan: `taint|secur|information.?flow`.
- **G — VAR.** `assert`/`assume`/`cover` ve `formalContext` [92].
- **H — VAR.** `past`, `rose`, `fell`, `stable`, `pastValid` [92].
- **I — VAR.** `FormalConfig.withBMC/withProve/withCover` SymbiYosys'i çalıştırıyor [92].
- **J — VAR.** RegIf: RTL ile birlikte HTML, C header, JSON, RALF (UVM) ve SystemRDL üretiyor [93].
- **K — ?** Aynı `busif` tanımından üretiliyor, ama bir denetim belgelenmemiş. Aranan: regIf.rst, regif testleri.
- **L — VAR°.**
  - `TimingExtractorXdc` false path, max delay ve saat XDC'si üretiyor [94].
  - Yalnız Xilinx için; resmi belgede yok.
- **M — VAR.** SpinalSim; arka uçlar Verilator, GHDL ve Icarus [95].
- **N — ?** Özel LSP yok; Metals ve IntelliJ öneriliyor [96]. Aranan: SpinalDoc içinde `lsp`.
- **O — VAR.** VHDL, Verilog ve SV [97].

#### Hardcaml

- **A — VAR°.**
  - `Clocked_signal` elaborasyon zamanında "raises if they're not compatible" [154].
  - Yalnız master dalında (2026-07-10); v0.17.x etiketlerinde yok. Tip düzeyinde değil.
- **B — ?** Aranan: ağaç, docs (63 dosya), `async_fifo.mli`.
- **C — VAR.** Düz `Signal` API'si anotasyon istemiyor [155].
- **D — VAR°.** Kütüphane düzeyinde `Signal.pipeline` ve `Stages` var; sözdizimi yok [155].
- **E — ?** Aranan: docs içinde `latency`, hardcaml_circuits, hardcaml_handshake.
- **F — ?** Aranan: ağaç ve docs içinde `security|taint`.
- **G — VAR.** `Assertions.add` [156].
- **H — VAR°.** `Property.LTL` ve CTL var, NuSMV hedefli; `$past` karşılığı görülmedi [156].
- **I — VAR°.** `hardcaml_verify` NuSMV kullanıyor; `Sec` yalnız kombinasyonel eşdeğerlik denetliyor [157].
- **J — VAR°.** `hardcaml_axi`: `Register_bank` ve `C_register_interface` C struct üretiyor [158].
- **K — ?** Aranan: hardcaml_axi ağacı; herkese açık bir test dizini yok.
- **L — VAR°.** Yalnız rapor akışı için `create_clock` [159].
- **M — VAR.** Cyclesim, event-driven simülasyon, expect testleri [160].
- **N — VAR°.** OCaml araçları kullanılıyor; "Reuse standard OCaml tooling" [160].
- **O — VAR.** Verilog, VHDL ve "basic Systemverilog support" [160].

#### TL-Verilog

- **A — YOK.** Alıntı: "SandPiper operates internally to clock and power domains. Your approach to clock and power domain crossings remains unchanged." [146]
- **B — ?**
  - ICCD makalesi: "asynchronous reset conditions are outside the scope of timing-abstract modeling" [147]. Bu, RDC denetimini açıkça reddetmiyor.
  - Aranan: SPEC, ICCD, FAQ.
- **C — VAR.** Saat HDL bölgesinden geliyor; TLV kodunda saat yazılmıyor [145].
- **D — VAR.** `|pipe` ve `@stage`: "Flip-flops are implied where pipesignals cross pipestage boundaries" [145][147].
- **E — VAR°.**
  - Hizalama `>>N` ile yapılıyor; pipeline'lar arası referansta açık hizalama zorunlu.
  - SPEC §17 "What's Missing" bölümü "Special types for alignments" diyor, yani tip düzeyinde değil [145][147].
- **F — ?** Aranan: SPEC, M5 kılavuzu, FAQ, Redwood sitesi.
- **G — VAR°.** SVA sözdizimiyle; ayrı bir TLV yapısı yok [146].
- **H — VAR°.** SVA ile yazılıyor; `>>1` ve `$RETAIN` önceki değere erişiyor [145][146].
- **I — ?**
  - WARP-V projesi riscv-formal ile doğrulanmış [149]. Bu dil ya da araç özelliği değil.
  - Aranan: SPEC, FAQ, tlvflows.
- **J, K — ?** Aranan: SPEC, docs_for_LLMs, TL-X-org depoları.
- **L — ?**
  - tlvflows'taki SDC dosyaları elle yazılmış [150].
  - Aranan: tlvflows, SPEC, FAQ.
- **M — VAR°.** Makerchip IDE ve Verilator; testler SV sinyalleriyle yazılıyor [151].
- **N — VAR°.** VS Code eklentisi: semantic tokens ve hover; LSP sunucusu yok [151].
- **O — VAR.** SV ve Verilog [146][148].

#### PipelineC

- **A — VAR°.**
  - İki alandan kullanılan paylaşılan global tel derleme hatası veriyor [163].
  - `#pragma ASYNC_WIRE` denetimi kapatıyor.
  - Pypeline'da çoklu saat "Not supported".
  - Hata tetiklenerek denenmedi.
- **B — ?** Aranan: wiki, src ve md içinde `reset domain`.
- **C — VAR°.** `#pragma MAIN_MHZ` zorunlu [164][165].
- **D — VAR.** "HLS-like automatic pipelining" [164].
- **E — VAR°.**
  - Hizalamayı derleyici ekliyor.
  - Pypeline'da `latency=` kısıtı tutmazsa derleme duruyor [164].
- **F — ?** Aranan: wiki, src, md.
- **G — VAR°.** Pypeline `sim_assert` ("Simulation-only assertion") [166].
- **H, I — ?** Aranan: `past`, `SymbiYosys|sby|smtbmc`.
- **J — VAR°.** Ortak C başlığı `mem_map.h` hem donanımda hem yazılımda kullanılıyor [167].
- **K — ?** mem_map.h içinde "Needs to match link.ld (TODO …)" notu var [167].
- **L — VAR°.**
  - `create_clock`, `set_clock_groups -asynchronous` [165].
  - Kod yorumu: iç zamanlama döngüsü için, "Rely on … board provided constraints for real hardware".
- **M — VAR.** `--sim` seçeneği ve Pypeline'ın Python içindeki simülatörü [166][168].
- **N — ?** Aranan: tüm depoda `lsp|vscode|tree-sitter`.
- **O — VAR.** VHDL; Verilog, GHDL ve Yosys üzerinden [168].

#### ROHD (Intel)

- **A — ?**
  - Yalnız `Synchronizer` ve `AsyncFifo` bileşenleri var [170].
  - Aranan: rohd, rohd-hcl, issue'lar.
- **B — ?** Aranan: rohd, rohd-hcl, rohd-vf.
- **C — VAR.** `Sequential(clk, …)` [171].
- **D — VAR.** `Pipeline`: "automatically pipelined" [171].
- **E, F — ?** Aranan: A14, pipeline.dart, `security|taint`.
- **G — VAR°.** Yalnız Dart `assert`; SVA üretimi yok [172].
- **H, I — ?** Aranan: lib/src, `formal|sby`, issue'lar.
- **J — VAR°.** ROHD-HCL `Csr`/`CsrBlock` yalnız RTL üretiyor [173].
- **K, L — ?** Aranan: csr.md, lib/src/memory/csr, `sdc|xdc`.
- **M — VAR.** Dart simülatörü ve ROHD-VF [171][175].
- **N — VAR°.** Dart dil sunucusu, VS Code ve DevTools eklentileri [174].
- **O — VAR.** SV; 0.6.11'den beri Yosys JSON da [172].

#### Filament

- **A — YOK.** Alıntı: "All event variables operate in the same clock domain, but this limitation can be removed in the future." (PLDI 2023, dipnot 2) [101]
- **B — ?** Aranan: makale, docs, repo `reset`.
- **C — VAR.** Olay tabanlı; saat yazılmıyor [101].
- **D — YOK.** Alıntı: "computation must be explicitly mapped onto hardware"; register'lar elle konuyor [101].
- **E — VAR.** Zaman çizelgesi (timeline) tipleri; "Available for [G+2, G+3) but required during [G, G+1)" [101].
- **F — ?** Aranan: makale, site, repo `label|secret`.
- **G — VAR°.** `assert`/`assume` yalnız derleme zamanı parametre kısıtları; sinyal assertion'ı değil [103].
- **H — ?** Aranan: makale, docs, gramer.
- **I — ?** z3 ve cvc5 yalnız tip ve zaman kısıtları için kullanılıyor; işlevsel formal akış yok [103].
- **J, K, L — ?** Aranan: makale, repo `register map|csr|sdc`.
- **M — VAR°.** fud ve cocotb ile transaction testleri [103].
- **N — VAR°.** Tree-sitter ve vim/vscode sözdizimi; çalışan LSP yok [103].
- **O — VAR.** Verilog ve Calyx [103].

#### Anvil

- **A — ?**
  - Kod üretici tek `clk_i` üretiyor (`codegenPort.ml:45`) [105].
  - Açıkça "desteklenmiyor" diyen bir ifade yok.
  - Aranan: langref, lib grep, makale.
- **B — ?** Aranan: aynı yerler.
- **C — VAR.** Saat derleyici tarafından ekleniyor [105].
- **D — YOK.** Alıntı: "does not specifically target specific types of pipelined designs. The designer can express pipelined designs by explicitly modelling stages as concurrent modules" [106].
- **E — VAR.**
  - Lifetime ve loan time kavramları, kanal zamanlama kontratları, dinamik gecikme (`dyn`) [104].
  - Biçimsel kanıt makalenin Ek C–D bölümlerinde.
- **F — ?** Aranan: lib grep, makale, langref.
- **G — ?**
  - Ana dalda yalnız `dprint`/`dfinish` var.
  - `assert`, açık ve birleşmemiş PR #90'da [105].
- **H — ?** Aranan: langref, PR #90.
- **I — ?** EBMC akışı yalnız açık PR #90 ve #91'de [105].
- **J, K, L — ?** Aranan: README, langref, lib.
- **M — VAR°.** Verilator ile `.test` golden dosyaları [105].
- **N — VAR°.** Deneysel `anvil-lsp` deposu [107]; issue #73'te LSP talebi [105].
- **O — VAR.** Sentezlenebilir SV [104].

#### PDL

- **A, B — ?** Aranan: makale, README, docs, repo grep.
- **C — VAR.** `---` aşama ayırıcısı [108].
- **D — VAR.** "automatically generates the registers and control logic" [108].
- **E — YOK.** Alıntı: "PDL does not reason about concrete timing of stage execution." [108]
- **F — ?**
  - `Security.scala` içinde bir kafes var ama `flowsTo` hiçbir yerde çağrılmıyor [109].
- **G, H — ?** Aranan: makale, docs.
- **I — ?** Z3 yalnız kilit ve spekülasyon denetimi için kullanılıyor [108].
- **J, K, L — ?** Aranan: makale, repo.
- **M — VAR°.** `interpret` komutu ve BSV testbench'i [109].
- **N — ?** README'de yalnız bash tamamlaması var.
- **O — VAR.** BSV [108].

#### SecVerilog

- **A — ?** Tehdit modeli "fixed-frequency clock" varsayıyor, ama çoklu saati açıkça reddetmiyor [110].
- **B — ?** Aranan: iki makale, repo.
- **C — VAR°.** Etiket bildirimleri gerekli, saat anotasyonu gerekmiyor [110].
- **D, E — ?** Aranan: makale, proje sayfası.
- **F — VAR.**
  - Statik tip denetimi, bağımlı etiketler, timing-sensitive noninterference ispatı [110].
  - Etiket değişince register'ı sıfırlayan kod dinamik olarak ekleniyor [110].
  - `downgrade` SecVerilogBL (2017) ile geldi [113]; `declassify` 2023 çatalında var [114].
- **G–N — ?** Aranan: makaleler, aferr/secverilog (Icarus 0.9.6 tabanlı).
- **I — ?** Z3 yalnız güvenlik ispatı için [112].
- **O — VAR.** "security labels are removed, resulting in standard Verilog" [111].

#### ChiselFlow

- **A, B — ?** Aranan: makale, repo `withClock`.
- **C — VAR.** Örtük saat ve reset [116]; etiket çıkarımı var [115].
- **D, E — ?** Aranan: makale, repo.
- **F — VAR.**
  - Statik denetim; bağımlı etiketler; `decl` ve `endo` downgrade yapıları [115][116].
  - Nonmalleable IFC ispatı "future work".
- **G — VAR°.** Chisel3'ten gelen `assert` [116].
- **H–L — ?** Z3 yalnız güvenlik yükümlülükleri için.
- **M — VAR°.** Chisel3 ve Verilator mirası [116].
- **N — ?**
- **O — VAR.** SIRRTL ve FIRRTL üzerinden Verilog [115][117].

#### Caisson

- **A, B — ?** Senkron tasarım varsayımı var [118].
- **C — VAR.** FSM tabanlı; bir geçiş = bir çevrim [118].
- **F — VAR.** Statik etiketler; timing-sensitive noninterference ispatı; declassification bulunamadı [118].
- **D, E, G–N — ?** Aranan: makale, repo (7 Scala dosyası).
- **O — VAR.** Verilog [118][119].

#### Sapper

- **C — VAR.** "All writes to registers will be automatically compiled into a synchronous block" [120].
- **F — VAR.**
  - Karma yaklaşım: statik analiz ve donanıma eklenen dinamik denetim [120].
  - Alıntı: "Explicit declassification is not supported".
- **A, B, D, E, G–N — ?**
  - Aranan: makale.
  - Kod bulunamadı; GitHub, Exa ve yazar sayfasında arandı.
- **O — VAR.** Verilog [120].

#### SystemRDL 2.0 + PeakRDL

- **A — YOK.** Alıntılar [123]:
  - "Only supported for signals used as resets … Ignored in all other contexts"
  - "I have zero interest in implementing resynchronizers"
- **B — ?** Aranan: regblock docs, `builtin.py`.
- **C — VAR.** Tek `clk` [124].
- **D — ?** Yalnız üretici seçeneği olarak "pipelining options" var [124].
- **E — ?** Aranan: SystemRDL PDF, regblock docs.
- **F — ?**
  - 84 yerleşik özellikte secure ya da privilege kavramı yok [125].
  - `pprot` bildiriliyor ama kullanılmıyor [133].
- **G — YOK.**
  - Standart: "Expressions defined in constraint components are not evaluated by SystemRDL compilers" [126].
  - Derleyici: "`constraint` blocks are not implemented yet" [125].
- **H, I — ?** Aranan: SystemRDL PDF, regblock docs, tests/README [127].
- **J — VAR.** SV ve VHDL RTL, C header, UVM, HTML, IP-XACT [128].
- **K — ?**
  - cheader `testcase` seçeneği header'ın kendi içinde test üretiyor; RTL'e karşı değil [129].
  - UVM RAL'i RTL'e karşı koşmak kullanıcıya kalıyor [130].
- **L — ?** Aranan: tüm klonlarda `sdc|xdc|create_clock`.
- **M — VAR°.** UVM RAL [130].
- **N — VAR°.**
  - Resmi eklenti yalnız sözdizimi renklendiriyor.
  - LSP üçüncü parti `systemrdl-pro`'da [131].
- **O — VAR.** SV, VHDL, C, UVM, HTML, IP-XACT, SVD [132].

#### IP-XACT (IEEE 1685-2022) + Kactus2

- **A–E — ?**
  - Aranan: UG PDF [134], Kactus2 Help ve Plugins [136].
- **F — ?** `accessPolicies/modeRef` moda bağlı erişim tanımlıyor [134]; bu IFC değil.
- **G, H, I — ?** Aranan: Kactus2 Help, README.
- **J — VAR°.**
  - Yalnız sürücü tarafı: C header, SVD, device tree [137].
  - README: "Neither Kactus2 nor IP-XACT handles module implementations" [135].
- **K — ?** Doğrulayıcılar IP-XACT belgesinin kendi bütünlüğünü denetliyor [138].
- **L — ?**
  - Format sentez kısıtlarını tutabiliyor [134], ama SDC üreten bir eklenti bulunamadı [138].
- **M — YOK.** Alıntı: "Synthesis or simulation: These require tools that are specificly created for the purpose" [135].
- **N — VAR°.** GUI editörü [135].
- **O — VAR.** IP-XACT XML, C, SVD, Verilog/VHDL başlıkları [137].

#### rggen

- **A, B, D–I — ?** Aranan: README, wiki (12 sayfa), DVCon JP 2026 makalesi.
- **C — VAR.** [142]
- **J — VAR.** SV, Verilog, Veryl, VHDL, UVM RAL, C header, Markdown [141][143].
- **K — ?**
  - Makale "consistent with the specification" diyor; bu tek kaynaktan üretim [143].
  - Otomatik bir karşılaştırma bulunamadı.
- **L — ?** Aranan: `sdc|xdc|create_clock`.
- **M — VAR°.** UVM RAL [141].
- **N — VAR°.** Web UI [144].
- **O — VAR.** [141]

### 2b. Sınırlı taranan ya da yalnız varlığı doğrulanan diller

Bu diller tabloya alınmadı. Aşağıdaki bilgiler karşılaştırma için değil, ileride taranacak adayları kaydetmek içindir.

- **HazardFlow** (KAIST, PLDI 2024):
  - D için kılavuz "hazard interfaces and combinators" diyor; çıktı Verilog.
  - Son commit 2024-12-05.
  - Kaynak: https://kaist-cp.github.io/hazardflow/book/
- **SecChisel** (HASP 2019):
  - Chisel üzerinde güvenlik etiketleri ve Z3 kullanıyor.
  - Kod erişimi araştırılmadı.
  - Kaynak: https://eprint.iacr.org/2017/193
- **Varlığı doğrulanan, A–O taraması yapılmayan diller:**
  - SUS: https://github.com/pc2/sus-compiler (latency counting)
  - Google XLS/DSLX: https://github.com/google/xls
  - Kanagawa: https://github.com/microsoft/kanagawa
  - Silice: https://github.com/sylefeb/Silice
  - MyHDL: https://github.com/myhdl/myhdl
  - RHDL: https://github.com/samitbasu/rhdl
- **Diğer register haritası araçları:**
  - **Corsair:** 1.x donduruldu; A–O tablosu scratchpad raporunda var, burada özetlendi.
  - **Cheby (CERN):** `--gen-c-check-layout` yalnız C layout'unu denetliyor, RTL'e karşı değil.
  - **airhdl:** ticari; revizyon register'ı kullanıcı tarafından kuruluyor.
  - **svd2rust:** yalnız sürücü üretiyor.

---

## 3. Olgunluk tablosu

"Aktif geliştirici" = 2025-09-21'den sonra varsayılan dala commit atan farklı yazar sayısı (botlar hariç). Kimlik birleştirme yapılmadı.

| | Son sürüm (tarih) | Aktif geliştirici (12 ay) | Silikon / tape-out kanıtı | Makale | Lisans |
|---|---|---|---|---|---|
| **Volt** | Etiket `v0.3.0-f4` (2026-09-07); `Cargo.toml` 0.1.0 [V] | 2 yazar kimliği; ilk commit 2026-09-03, 73 commit [V] | Bulunamadı (depoda arandı) [V] | Bulunamadı [V] | MIT OR Apache-2.0 [V] |
| Clash | v1.10.2 (2026-09-07) [12] | ~14 [12] | Tiny Tapeout 2 "clash cpu" (SKY130; hobi ölçeği) [13] | DSD 2010 [14] | BSD-2-Clause [CC] |
| Bluespec | 2026.01 (2026-05-01) [22] | 15; commit'lerin %62'si tek kişiden [22] | SHAKTI C-Class, 22nm/180nm, 3 test tape-out [23] | MEMOCODE 2004 [24] | BSD-3-Clause (çoğunlukla) [25] |
| Veryl | v0.21.0 (2026-09-02) [36] | 22; çekirdek ~3 [36] | Bulunamadı | DVCon Japan 2024 (arXiv 2411.12983), OSCAR@ISCA 2025 slaytları, IEICE 2026 [37] | MIT OR Apache-2.0 [36] |
| Arch | v0.72.5 (2026-09-20); depo 2026-04-10'da açıldı [47] | Pratikte 1 (`nogate-ai` 2067 commit) [47] | Bulunamadı | arXiv 2604.05983 (hakemsiz) [48] | LGPL-3.0; CLA zorunlu [49] |
| Spade | v0.20.0 (2026-08-20) [59] | ~12 [59] | Tiny Tapeout 3'te 1 proje (çalıştığı doğrulanmadı) [60] | ACM TRETS 2026, FPL 2022, OSDA 2023 [50][61] | Derleyici EUPL-1.2; stdlib MIT/Apache [59] |
| Chisel | v7.15.0 (2026-09-01); firtool 1.159.0 [85] | ~20 [85] | Raven 28nm (Berkeley) [86] | DAC 2012 [87] | Apache-2.0 [85] |
| Amaranth | v0.5.10 (2026-09-10) [73] | 14 (38 commit) [73] | Tiny Tapeout 2'de 5, TT03'te 7 proje (çalıştıkları doğrulanmadı) [13][60] | Hakemli makale bulunamadı | BSD-2-Clause [72] |
| SpinalHDL | v1.15.0 (2026-09-02) [98] | 28 yazar kimliği [98] | Carbon1, IHP 130nm SG13S [99] | Hakemli makale bulunamadı; FOSDEM 2017 [100] | Çekirdek LGPL-3; lib MIT [98] |
| Hardcaml | Etiket v0.17.1 (2025-10); master v0.18 önizleme (2026-07-10) | Ölçülemedi (dışa aktarım botu) | Jane Street: "FPGA and ASIC designs" (blog 2026-09-10; proses belirtilmemiş) [162] | arXiv 2312.15035 [161] | MIT |
| TL-Verilog | Derleyici kapalı; `sandpiper-saas` 1.1.0 (2024-08-26) [152] | Açık ekosistemde 5 | TT şablonları var; silikonda doğrulanmış proje bulunamadı [153] | ICCD 2017 [147], arXiv 1811.12474 [149] | SandPiper ticari; kütüphaneler BSD-3 [146] |
| PipelineC | Etiket yok; son commit 2026-09-21 [164] | Pratikte 1 (561 commit) | Bulunamadı | LATTE '23 (resmi yayın durumu doğrulanmadı) [169] | GPL-3.0 |
| ROHD | v0.6.11 (2026-09-18) [172] | 7 | Bulunamadı | DAC 2022, LATTE '24 [176] | BSD-3-Clause |
| Filament | Sürüm yok; son commit 2026-07-23 | 1 (1 commit) | Bulunamadı | PLDI 2023 [101] | MIT |
| Anvil | Sürüm yok; son commit 2026-09-12 [105] | ~7 (96 commit) [105] | Bulunamadı; yalnız 22nm sentez [104] | ASPLOS 2026 (arXiv 2503.19447) [104] | MIT |
| PDL | v1.0 (2022-02-28) [109] | 0 | Bulunamadı; 45nm sentez [108] | PLDI 2022 [108] | MIT |
| SecVerilog | Sürüm yok; resmi depo 2015, çatal 2023 [112][114] | 0 | Bulunamadı; 90nm sentez [113] | ASPLOS 2015, 2017 [110][113] | GPLv2 (Icarus) |
| ChiselFlow | v1.0 (2019-11-07) [116] | 0 | Bulunamadı | CCS 2018 [115] | BSD tarzı (Chisel3) |
| Caisson | Sürüm yok; 2011-06-03 [119] | 0 | Bulunamadı; Stratix II ve 90nm sentez [118] | PLDI 2011 [118] | BSD tarzı |
| Sapper | Kod bulunamadı | — | Bulunamadı | ASPLOS 2014 [120] | — |
| PeakRDL | regblock v1.3.1 (2026-03-28); PeakRDL v1.5.0 (2025-10-03) | regblock 8; ağırlık tek bakımcıda | Açık IP'lerde kullanım (pulp, opentitan bağımlılığı); tape-out kanıtı bulunamadı | Standart: SystemRDL 2.0 (2018) [126] | PeakRDL-* LGPL-3.0; compiler MIT |
| Kactus2 | v3.14.2 (2026-09-09) | 2 | Bulunamadı | JOSS 2017 [139]; IEEE 1685-2022 [140] | GPL-2.0 |
| rggen | v0.36.1 (2026-04-19) | 1 | PEZY'de 30'dan fazla blokta üretim kullanımı (yazarın kendi beyanı) [143] | DVCon Japan 2026 [143] | MIT |

---

## 4. Boyut notları: kim en iyi yapıyor, nasıl

- **A. CDC**
  - **En kapsamlı:** Arch. Beş çeşit denetimi var ve yakınsayan senkronizörü de tespit ediyor [38].
  - **En köklü:** Clash. Alan tipte taşınıyor [1].
  - **Varsayılan olarak açık:** SpinalHDL. Elaborasyon hatası veriyor ve kombinasyonel yolu kaynak satırıyla raporluyor [88].
  - **Anotasyonla çalışanlar:**
    - Veryl: anotasyon ve `unsafe (cdc)` [26].
    - Chisel 7: alan sistemi var ama varsayılan olarak kapalı [75].
  - **Açıkça kapsam dışı sayanlar:** TL-Verilog [146] ve Filament [101].
  - **Volt:** Denetim çıkarımla ve derleme zamanında çalışıyor. Onaylı geçiş yolları dar, yakınsama tespiti yok.
- **B. RDC**
  - **Belgelenmiş RDC denetimi olan tek araç:** Arch (5 faz, 48 test) [38].
  - **Kısmen:** Bluespec (hata ve uyarı) [15], Clash (alan tipinde reset) [1].
  - **Volt:** Açıkça YOK.
- **C. Tek saatte anotasyonsuz çalışma**
  - Çoğu dilde var.
  - **Açıkça yok:** Spade, çünkü `reg(clk)` zorunlu [54].
  - **Kısmen:** Arch, çünkü `Clock<SysDomain>` yazmak zorunlu [40].
- **D. Pipeline sözdizimi**
  - **En zengin:** Arch (stall, flush, forward, değişken gecikme) [39] ve TL-Verilog (aşama ve hizalama modeli) [145].
  - **Otomatik bölme:** PipelineC, zamanlama geri bildirimiyle [164].
  - **Diğerleri:** Spade, SpinalHDL, ROHD, PDL.
  - **Açıkça hedeflemeyenler:** Filament [101] ve Anvil [106].
- **E. Tip seviyesinde gecikme / zamanlama**
  - **Biçimsel ispatlı:** Filament timeline tipleri [101] ve Anvil lifetime ile kanal kontratları [104].
  - **Üretim dillerinde en güçlüsü:** Clash `DSignal` [4].
  - **Yalnız pipeline yapısı içinde:** Arch ve Spade.
  - **Volt:** Opt-in ve dar kapsamlı.
- **F. Bilgi akışı / güven seviyesi**
  - **Akademik diller (Volt'tan daha ileri):**
    - SecVerilog: bağımlı etiketler ve timing-sensitive noninterference ispatı [110].
    - ChiselFlow: robust declassification [115].
  - **Üretim ve topluluk dillerinde:** Taranan 12 dilin hiçbirinde bulunamadı.
- **G. Kontrat / assertion**
  - **Kontrat biçimi:** Chisel `FormalContract` Require/Ensure ve cutpoint semantiği [79].
  - **Otomatik SVA:** Arch; FIFO, sayaç ve FSM için [39].
  - **Volt:** `requires`/`ensures`/`invariant` sözdizimi var; ne otomatik SVA ne cutpoint var.
- **H. Ardışık kontrat**
  - **En zengin:** Chisel `chisel3.ltl` (`past`, `Delay`, implikasyon) [79].
  - **Ardından:** SpinalHDL (`past`, `rose`, `fell`, `stable`) [92] ve Arch (`|=>`, `##N`) [39].
  - **Kaldırılan:** Amaranth 0.5'te `Past` kaldırıldı [67].
  - **Volt:** Yalnız `prev`.
- **I. Otomatik formal akış**
  - **SymbiYosys ile:** SpinalHDL `FormalConfig` [92].
  - **Harici araçsız:** Arch, kendi SMT motoruyla BMC yapıyor [41].
  - **Açıkça yok:** Chisel 7 çekirdeği [80] ve Amaranth [68].
  - **Volt:** SpinalHDL'e benzer, dış araç gerektiriyor; CI'da zorunlu değil.
- **J. Register haritasından RTL ve sürücü**
  - **Özel araçlar:** PeakRDL ve rggen en geniş kapsamlı (çok sayıda cpuif, UVM, C) [128][141].
  - **HDL içinde:** SpinalHDL RegIf; beş çıktı ve UVM RALF [93].
  - **Volt:** Tek arayüz (AXI4-Lite). Rust sürücü çıktısı var, bu nadir: Rust çıktısı başka yalnız Clash-memmap'te (harici paket) [7] ve PeakRDL topluluk eklentilerinde [128] var.
- **K. RTL ile sürücü tutarlılığı**
  - Hiçbir rakipte otomatik karşılaştırma bulunamadı.
  - **Volt:** Karşılaştırma derleyicinin kendi testinde var. Kullanıcı özelliği değil.
- **L. SDC/XDC kısıt üretimi**
  - **En olgun:** Amaranth. Platform katmanı `create_clock` ve CDC ilkeleri için false path / max delay üretiyor, birden çok üretici destekleniyor [70].
  - **Diğerleri:** Volt, SpinalHDL (belgelenmemiş) [94], PipelineC (iç kullanım) [165].
  - **Dar kapsamlı:** Clash yalnız `create_clock` [8], Arch yalnız multicycle [44].
- **M. Yerleşik simülasyon / test**
  - **Dilin kendisinde simülatör:** Clash, Bluespec (Bluesim), Veryl, Amaranth, SpinalHDL, Hardcaml, ROHD.
  - **Test dili var, simülatör harici:** Volt (Verilator'a bağlı).
- **N. LSP / editör**
  - **Özel LSP:** Veryl, Spade, Volt (kısmi, yayımlanmamış), Anvil (deneysel).
  - **Barındıran dilin LSP'si:** Chisel, SpinalHDL, Clash, Hardcaml ve ROHD, barındıran dilin araçlarını kullanıyor.
- **O. Çıktı biçimi**
  - **VHDL de üretenler:** Clash, SpinalHDL, Hardcaml, PipelineC.
  - **Okunabilir SV hedefi:** Veryl, Arch, Spade, Volt.
  - **Volt:** Birkaç yapı SV'ye emit edilemiyor.

---

## 5. Volt'un ayrıştığı aday alanlar ve güven seviyeleri

**Güven ölçeği:**

- **Yüksek:** Birden fazla rakipte açık YOK kanıtı var.
- **Orta:** Kanıt bulunamadı ama arama geniş tutuldu.
- **Düşük:** Arama sınırlı kaldı.

| # | Aday alan | Durum | Güven | Gerekçe |
|---|---|---|---|---|
| 1 | Üretim odaklı bir HDL'de derleme zamanı **bilgi akışı (F)** | Volt: VAR° | **Orta** | Aşağıda |
| 2 | **F ile A'nın aynı dilde bir arada olması** | Volt: F VAR° + A VAR° | **Orta** | Aşağıda |
| 3 | **RTL ile sürücü tutarlılık denetimi (K)** | Volt: VAR° (yalnız iç test) | **Orta** | Aşağıda |
| 4 | Register haritasından **Rust** sürücü üretiminin HDL'nin kendisinde olması | Volt: VAR | **Düşük** | Aşağıda |
| 5 | Tek dilde **A + J + L + I** bileşimi | Volt: VAR° hepsi | **Düşük** | Aşağıda |

**1. Derleme zamanı bilgi akışı (F)**

- Üretim ve topluluk dillerinin 12'sinde de `?`.
- Hiçbir kaynak IFC'yi açıkça "kapsam dışı" demiyor; bu yüzden yüksek değil, orta.
- Akademik rakipler (SecVerilog, ChiselFlow, Caisson, Sapper) F'de VAR ve Volt'tan daha güçlü: ispatlı noninterference ve bağımlı etiketler.
- Bu yüzden ayrışma "IFC" değil, "IFC'nin bakımı süren, üretim odaklı bir dilde olması". Akademik depoların son commit'leri: SecVerilog 2015/2023, ChiselFlow 2019, Caisson 2011.

**2. F ile A'nın bir arada olması**

- A'da VAR olan dillerin (Clash, Bluespec, Veryl, Arch, SpinalHDL) hiçbirinde F bulunamadı.
- F'de VAR olan akademik dillerin hiçbirinde A bulunamadı. SecVerilog tek ve sabit frekanslı bir saat varsayıyor [110].
- Açık YOK kanıtı yok.

**3. RTL ile sürücü tutarlılık denetimi (K)**

- Taranan 22 aracın hiçbirinde bulunamadı. PeakRDL, rggen, Cheby, Corsair, SpinalHDL RegIf ve Clash-memmap özellikle incelendi.
- Açık YOK kanıtı yok.
- Volt'un kendi karşılığı kullanıcıya sunulmuyor, bu yüzden şu an bir ürün ayrışması değil.

**4. HDL içinde Rust sürücü üretimi**

- Clash-memmap Rust üretiyor, ama harici bir paket ve haritayı devreden türetiyor [7].
- PeakRDL'in topluluk eklentilerinde Rust çıktısı var [128].
- Arama sınırlı kaldı; topluluk eklentileri tek tek incelenmedi.

**5. Tek dilde A + J + L + I**

- SpinalHDL bu dört boyutun hepsinde VAR ya da VAR° [88][92][93][94]. Yani bileşim tek başına bir ayrışma değil.
- Fark yalnız F ve K'da. Bunlar 1–3. satırlarda ele alındı.

**Açıkça ayrışma olmayan alanlar:**

- CDC (A)
- Pipeline (D)
- Gecikme tipleri (E)
- Kontrat sözdizimi (G)
- Formal akış (I)
- SDC üretimi (L)
- Okunabilir SV (O)

Bu alanların her birinde en az bir rakip, birincil kaynakla, eşit ya da daha kapsamlı destek sunuyor (bkz. §4).

**Yasaklı ifadeler.** Yukarıdaki güven seviyelerinin hiçbiri "Yüksek" değil. Bu nedenle "unique", "only", "first", "ilk" ve "tek" ifadeleri **kullanılamaz**. README için önerilen dil: "we found no … in <liste>, as of 2026-09".

---

## 6. Volt'un geride kaldığı alanlar

1. **RDC.**
   - Volt'ta YOK (E3003 rezerve).
   - Arch'ta belgelenmiş 5 fazlı denetim var; Bluespec ve Clash'te kısmi.
2. **Olgunluk.**
   - Proje 18 günlük; bir paket kayıt defterinde (crates.io vb.) yayınlanmış sürüm bulunamadı.
   - 2 yazar kimliği var. Bunlar ayrı kişiler mi, depodan anlaşılamıyor.
   - Tape-out, FPGA kartında çalıştırma ve makale kanıtı yok.
   - Karşılaştırma için rakiplerden örnekler:
     - Veryl: 1039 yıldız, 22 katkıcı, 5 yayın.
     - SpinalHDL: IHP 130nm silikon.
     - Chisel: 28nm silikon.
     - Bluespec: 22nm SHAKTI.
3. **Ardışık özellik dili.**
   - Volt'ta yalnız `prev`.
   - Chisel'de LTL (`Delay`, implikasyon), SpinalHDL'de `rose`/`fell`/`stable`, Arch'ta `|=>`/`##N` var.
4. **Formal akış.**
   - Volt'ta dış araç zorunlu, motor yalnız smtbmc, CI'da `continue-on-error`.
   - Arch dış araç gerektirmiyor ve boş varsayım (vacuity) denetimi yapıyor.
   - Arch ile Chisel'de otomatik ya da cutpoint semantikli kontratlar var.
5. **Otomatik SVA.** Arch FIFO, sayaç, FSM ve indeks sınırı için özellikleri kendisi üretiyor. Volt'ta bulunamadı.
6. **CDC derinliği.**
   - Volt yakınsayan senkronizörü tespit etmiyor; Arch ediyor.
   - Volt kullanıcı senkronizörünü tanımıyor.
   - Çok bitli `sync()` yalnız uyarı veriyor.
7. **Gecikme tipleri.**
   - Volt'ta opt-in, geri besleme ve kontrol sinyallerini kapsamıyor, açık bir kaçış yolu var.
   - Clash `DSignal` genel amaçlı. Filament ve Anvil'in tip sistemleri biçimsel olarak kanıtlanmış.
8. **IFC'nin titizliği.**
   - Volt'ta noninterference teoremi yok ve alt modül özeti tutucu.
   - SecVerilog ve ChiselFlow'da ispat ve bağımlı etiketler var.
9. **Simülasyon.** Volt'ta yerleşik simülatör yok, Verilator zorunlu. En az yedi rakipte dilin kendi simülatörü var (§4, M).
10. **Çıktı kapsamı.**
    - Volt yalnız SV üretiyor, VHDL yok.
    - `Trit`, açık reset ve blok içi `let` gibi yapılar SV'ye emit edilemiyor (E0003).
    - 2026-09-20'ye kadar sessiz bir öncelik hatası vardı (ADR-0057).
11. **Register haritası.**
    - Volt'ta yalnız AXI4-Lite ve 32 bit, reset değeri hep 0, UVM, SVD ve IP-XACT çıktısı yok.
    - PeakRDL ve rggen birçok veriyolu ile UVM RAL destekliyor. SpinalHDL RegIf ise RALF ve SystemRDL üretiyor.
12. **Editör desteği.** VS Code eklentisi yayımlanmamış; rename ve references yok.
13. **Depo içi belge tutarlılığı.**
    - `README.md:353` pipeline'ı "reserved" diyor; rozet 1096 test gösteriyor, README "99 diagnostic codes" diyor. Üçü de güncel durumla çelişiyor.
    - `docs/README.md:26` `research/*-Detayli-Inceleme.md` dosyalarına bağlantı veriyor, ama bu dosyalar `docs/research/` altında yok.

---

## 7. Kaynakça

Tüm kaynaklara **2026-09-21**'de erişildi.

**Volt**

- [V] Volt deposu, `HEAD 2846bb9`, yerel yol `C:\Dev\volthdl`. Hücrelerdeki dosya ve satır atıfları bu commit'e göredir.

**Clash**

`CC` kısaltması = https://github.com/clash-lang/clash-compiler/blob/65055b63ec4d223260bdda0deba445a5fb430ee3/

- [1] `CC` clash-prelude/src/Clash/Explicit/Signal.hs (L74-130, L424-441)
- [2] `CC` clash-prelude/src/Clash/Explicit/Synchronizer.hs
- [3] `CC` clash-prelude/src/Clash/Signal.hs (L601-605, L1272)
- [4] `CC` clash-prelude/src/Clash/Signal/Delayed/Internal.hs; clash-prelude/src/Clash/Explicit/Signal/Delayed.hs
- [5] `CC` clash-prelude/src/Clash/Verification.hs; clash-prelude/src/Clash/Explicit/Verification.hs
- [6] `CC` clash-prelude/src/Clash/Verification/Internal.hs; tests/shouldwork/Verification/SymbiYosys.hs
- [7] https://github.com/QBayLogic/clash-protocols-memmap (README, Check/AbsAddress.hs); https://github.com/bittide/bittide-hardware/blob/main/bittide-instances/src/Bittide/Instances/MemoryMaps.hs
- [8] `CC` clash-lib/src/Clash/Driver/Types.hs (L464-489); clash-lib/src/Clash/Driver.hs (L941-949)
- [9] `CC` clash-prelude/src/Clash/Explicit/Testbench.hs
- [10] https://github.com/clash-lang/clash-starters/blob/main/simple/README.md
- [11] https://github.com/trailofbits/clash-silicon-tinytapeout/blob/main/src/top.v
- [12] https://github.com/clash-lang/clash-compiler/releases/tag/v1.10.2; `gh api repos/clash-lang/clash-compiler/commits?since=2025-09-21`
- [13] https://tinytapeout.com/chips/tt02/
- [14] https://ieeexplore.ieee.org/document/5615430/ (DSD 2010, DOI 10.1109/DSD.2010.21)

**Bluespec**

`BSC` kısaltması = https://github.com/B-Lang-org/bsc/blob/3ea4b2baa4daaceefcbedc2c14d36e933662f24e/

- [15] `BSC` src/comp/Error.hs (L3068-3072, L3327-3349, L3510-3582)
- [16] `BSC` doc/libraries_ref_guide/LibDoc/Clocks.tex
- [17] `BSC` doc/BSV_ref_guide/BSV_lang.tex (L2365-2367, L12062-12066)
- [18] `BSC` doc/libraries_ref_guide/LibDoc/Assert.tex; OVLAssertions.tex
- [19] `BSC` doc/libraries_ref_guide/LibDoc/CBus.tex
- [20] `BSC` doc/user_guide/user_guide.tex
- [21] https://github.com/robertszafa/bsv-lsp; https://marketplace.visualstudio.com/items?itemName=runtimetantrum.blues-lsp
- [22] https://github.com/B-Lang-org/bsc/releases/tag/2026.01; `gh api repos/B-Lang-org/bsc/commits?since=2025-09-21`
- [23] https://cacm.acm.org/research/building-the-shakti-microprocessor/ (DOI 10.1145/3556632)
- [24] https://www.semanticscholar.org/paper/651534f5bd9d3c3ec4baba3461f03af264e1bbef (MEMOCODE 2004)
- [25] `BSC` COPYING

**Veryl**

- [26] https://doc.veryl-lang.org/book/05_language_reference/15_clock_domain_annotation.html
- [27] https://doc.veryl-lang.org/book/05_language_reference/15_clock_domain_annotation/01_unsafe_cdc.html
- [28] https://github.com/veryl-lang/veryl/blob/master/crates/analyzer/src/conv/checker/clock_domain.rs; .../crates/analyzer/src/symbol.rs
- [29] https://doc.veryl-lang.org/book/05_language_reference/03_data_type/04_clock_reset.html
- [30] https://doc.veryl-lang.org/book/05_language_reference/13_integrated_test.html; https://github.com/veryl-lang/veryl/issues/1812
- [31] https://github.com/veryl-lang/veryl/blob/master/crates/analyzer/src/sv_system_function.rs
- [32] https://github.com/veryl-lang/veryl#related-projects; https://github.com/rggen/rggen-veryl
- [33] https://doc.veryl-lang.org/book/06_development_environment/07_simulator.html
- [34] https://doc.veryl-lang.org/book/06_development_environment/08_language_server.html
- [35] https://doc.veryl-lang.org/book/01_introduction.html; .../12_source_map.html
- [36] https://github.com/veryl-lang/veryl/releases/tag/v0.21.0; `gh api repos/veryl-lang/veryl/commits?since=2025-09-21`
- [37] https://veryl-lang.org/docs/#publications; https://arxiv.org/abs/2411.12983

**Arch**

- [38] https://github.com/arch-hdl-lang/arch-com/blob/main/doc/cdc-rdc-evidence.md
- [39] https://github.com/arch-hdl-lang/arch-com/blob/main/doc/ARCH_HDL_Specification.md
- [40] https://github.com/arch-hdl-lang/arch-com/blob/main/doc/arch.ebnf
- [41] https://github.com/arch-hdl-lang/arch-com (README)
- [42] https://github.com/arch-hdl-lang/arch-com/blob/main/doc/COMPILER_STATUS.md
- [43] https://github.com/arch-hdl-lang/rdl2arch; https://github.com/arch-hdl-lang/rdl2arch-riscv
- [44] https://github.com/arch-hdl-lang/arch-com/blob/main/src/codegen/mod.rs (`emit_sdc`)
- [45] https://github.com/arch-hdl-lang/arch-com/tree/main/editors
- [46] https://github.com/arch-hdl-lang/harc-com
- [47] https://github.com/arch-hdl-lang/arch-com/releases; https://github.com/arch-hdl-lang/arch-com/pull/1025; `gh api repos/arch-hdl-lang/arch-com/commits?since=2025-09-21`
- [48] https://arxiv.org/abs/2604.05983
- [49] https://github.com/arch-hdl-lang/arch-com/blob/main/CLA.md

**Spade**

- [50] https://spade-lang.org/spade.typ.pdf (ACM TRETS 2026, DOI 10.1145/3793550)
- [51] https://gitlab.com/spade-lang/spade/-/blob/main/spade-compiler/stdlib/cdc.spade
- [52] https://gitlab.com/spade-lang/RFCs/-/blob/main/text/clock_domains.md
- [53] https://gitlab.com/spade-lang/spade/-/work_items/118; https://gitlab.com/spade-lang/spade/-/work_items/149
- [54] https://gitlab.com/spade-lang/book/-/blob/main/src/language_reference/statements.md
- [55] https://gitlab.com/spade-lang/book/-/blob/main/src/pipelines_hw.md; .../language_reference/dynamic_pipelines.md
- [56] https://gitlab.com/spade-lang/book/-/blob/main/src/simulation.md
- [57] https://gitlab.com/spade-lang/spade/-/blob/main/CHANGELOG.md; https://gitlab.com/spade-lang/book/-/blob/main/src/editor-setup.md
- [58] https://gitlab.com/spade-lang/swim/-/blob/main/src/spade.rs
- [59] GitLab API `projects/spade-lang%2Fspade/releases` ve `repository/commits?since=2025-09-21`; https://gitlab.com/spade-lang/spade/-/blob/main/README.md
- [60] https://tinytapeout.com/chips/tt03/
- [61] https://arxiv.org/abs/2304.03079

**Amaranth**

- [62] https://amaranth-lang.org/docs/amaranth/latest/stdlib/cdc.html
- [63] https://github.com/amaranth-lang/amaranth/blob/main/amaranth/hdl/_ir.py
- [64] https://amaranth-lang.org/docs/amaranth/latest/guide.html (#lang-controlinserter, #lang-assert)
- [65] https://github.com/amaranth-lang/amaranth/blob/main/amaranth/build/plat.py; https://amaranth-lang.org/docs/amaranth/latest/start.html
- [66] https://github.com/amaranth-lang/amaranth/issues/213
- [67] https://amaranth-lang.org/docs/amaranth/latest/changes.html
- [68] https://github.com/amaranth-lang/amaranth/issues/505; https://github.com/amaranth-lang/amaranth/issues/487
- [69] https://github.com/amaranth-lang/amaranth-soc/blob/main/amaranth_soc/csr/reg.py; .../memory.py
- [70] https://github.com/amaranth-lang/amaranth/blob/main/amaranth/vendor/_xilinx.py; https://amaranth-lang.org/docs/amaranth/latest/intro.html
- [71] https://amaranth-lang.org/docs/amaranth/latest/simulator.html
- [72] https://github.com/amaranth-lang/amaranth/blob/main/amaranth/back/verilog.py; https://github.com/amaranth-lang/amaranth/blob/main/README.md
- [73] `gh api repos/amaranth-lang/amaranth/releases` ve `commits?since=2025-09-21`

**Chisel (+CIRCT)**

- [74] https://github.com/chipsalliance/chisel/blob/main/core/src/main/scala/chisel3/domains/ClockDomain.scala; .../src/test/scala/chiselTests/DomainSpec.scala
- [75] https://github.com/llvm/circt/blob/main/test/Dialect/FIRRTL/infer-domains-infer-errors.mlir; https://github.com/llvm/circt/blob/main/lib/Firtool/Firtool.cpp
- [76] https://github.com/chipsalliance/chisel/blob/main/docs/src/explanations/multi-clock.md
- [77] https://www.chisel-lang.org/docs/explanations/sequential-circuits
- [78] https://github.com/chipsalliance/chisel/blob/main/src/main/scala/chisel3/util/Pipe.scala
- [79] https://github.com/chipsalliance/chisel/blob/main/core/src/main/scala/chisel3/FormalContract.scala; .../src/main/scala/chisel3/ltl/LTL.scala; https://www.chisel-lang.org/docs/explanations/layers
- [80] https://github.com/chipsalliance/chisel/blob/main/src/main/scala/chiseltest/formal/package.scala; .../docs/src/appendix/migrating-from-chiseltest.md; https://github.com/ucb-bar/chiseltest; https://github.com/llvm/circt/blob/main/docs/Tools/circt-bmc.md
- [81] https://github.com/chipsalliance/rocket-chip/blob/master/src/main/scala/regmapper/RegFieldDesc.scala; .../util/Annotations.scala
- [82] https://www.chisel-lang.org/docs/explanations/testing
- [83] https://github.com/llvm/circt/tree/main/lib/Tools/circt-verilog-lsp-server
- [84] https://github.com/chipsalliance/chisel/blob/main/docs/src/installation.md
- [85] `gh api repos/chipsalliance/chisel/releases/latest` ve `commits?since=2025-09-21`; `gh api repos/llvm/circt/releases/latest`
- [86] https://pdfs.semanticscholar.org/d57c/d9e36b070695aeb8eff510fa61beb5b47eaf.pdf (Raven 28nm)
- [87] https://dl.acm.org/doi/10.1145/2228360.2228584 (DAC 2012)

**SpinalHDL**

- [88] https://spinalhdl.github.io/SpinalDoc-RTD/master/SpinalHDL/Design%20errors/clock_crossing_violation.html; https://github.com/SpinalHDL/SpinalHDL/blob/dev/core/src/main/scala/spinal/core/internals/Phase.scala
- [89] https://github.com/SpinalHDL/SpinalDoc-RTD/blob/master/source/SpinalHDL/Structuring/clock_domain.rst
- [90] https://spinalhdl.github.io/SpinalDoc-RTD/master/SpinalHDL/Libraries/Pipeline/introduction.html
- [91] https://github.com/SpinalHDL/SpinalHDL/blob/dev/lib/src/main/scala/spinal/lib/bus/regif/Secure.scala
- [92] https://spinalhdl.github.io/SpinalDoc-RTD/master/SpinalHDL/Formal%20verification/index.html; https://github.com/SpinalHDL/SpinalDoc-RTD/blob/master/source/SpinalHDL/Other%20language%20features/assertion.rst
- [93] https://spinalhdl.github.io/SpinalDoc-RTD/master/SpinalHDL/Libraries/regIf.html
- [94] https://github.com/SpinalHDL/SpinalHDL/blob/dev/lib/src/main/scala/spinal/lib/eda/xilinx/TimingExtractorXdc.scala
- [95] https://spinalhdl.github.io/SpinalDoc-RTD/master/SpinalHDL/Simulation/index.html
- [96] https://github.com/SpinalHDL/SpinalDoc-RTD/blob/master/source/SpinalHDL/Getting%20Started/VSCodium.rst
- [97] https://spinalhdl.github.io/SpinalDoc-RTD/master/SpinalHDL/Other%20language%20features/vhdl_generation.html
- [98] `gh api repos/SpinalHDL/SpinalHDL/releases/latest` ve `commits?since=2025-09-21`; https://github.com/SpinalHDL/SpinalHDL (README lisans)
- [99] https://github.com/aesc-silicon/carbon1-sw
- [100] https://archive.fosdem.org/2017/schedule/event/spinal_hdl/

**Akademik diller**

- [101] https://rachit.pl/files/pubs/filament.pdf (PLDI 2023, DOI 10.1145/3591234)
- [102] https://filamenthdl.com/
- [103] https://github.com/cucapra/filament (crates/ast/src/syntax.pest, crates/filament/src/cmdline.rs, doc/docs/lang/run.md, tools/)
- [104] https://arxiv.org/abs/2503.19447 (v2; README'ye göre ASPLOS 2026)
- [105] https://github.com/AnvilHDL/anvil (lib/codegenPort.ml, README, PR #86, #90, #91, issue #73)
- [106] https://github.com/AnvilHDL/anvil-docs (docs/langref/introduction.md, channels.md, registers.md)
- [107] https://github.com/AnvilHDL/anvil-lsp
- [108] https://www.cs.cornell.edu/~dzag/assets/pdl.pdf (PLDI 2022)
- [109] https://github.com/apl-cornell/PDL (src/main/scala/pipedsl/common/Security.scala, CommandLineParser.scala)
- [110] https://www.cs.cornell.edu/andru/papers/asplos15/asplos15.pdf (ASPLOS 2015)
- [111] https://www.cs.cornell.edu/projects/secverilog/
- [112] https://github.com/aferr/secverilog
- [113] https://www.cs.cornell.edu/andru/papers/trustzone/asplos17.pdf (ASPLOS 2017)
- [114] https://github.com/dzagieboylo/secverilog/blob/main/verilog-0.9.6/parse.y
- [115] https://www.cs.cornell.edu/andru/papers/hyperflow/hyperflow.pdf (CCS 2018)
- [116] https://github.com/apl-cornell/ChiselFlow
- [117] https://github.com/apl-cornell/sirrtl
- [118] https://users.ece.utexas.edu/~tiwari/pubs/PLDI-11-caisson.pdf (PLDI 2011)
- [119] https://github.com/vineethk/Caisson
- [120] http://people.cs.uchicago.edu/~ftchong/papers/ASPLOS-14-sapper.pdf (ASPLOS 2014)
- [121] https://kaist-cp.github.io/hazardflow/book/
- [122] https://eprint.iacr.org/2017/193

**Register haritası araçları**

- [123] https://github.com/SystemRDL/PeakRDL-regblock/blob/main/docs/props/signal.rst; .../docs/props/field.rst; .../docs/dev_notes/
- [124] https://github.com/SystemRDL/PeakRDL-regblock/blob/main/docs/cpuif/internal_protocol.rst; .../docs/index.rst
- [125] https://github.com/SystemRDL/systemrdl-compiler/blob/main/src/systemrdl/properties/builtin.py; .../docs/known_issues.rst
- [126] https://www.accellera.org/images/downloads/standards/systemrdl/SystemRDL_2.0_Jan2018.pdf
- [127] https://github.com/SystemRDL/PeakRDL-regblock/blob/main/tests/README.md
- [128] https://peakrdl.readthedocs.io (gallery, community)
- [129] https://github.com/SystemRDL/PeakRDL-cheader/blob/main/docs/index.rst; .../src/peakrdl_cheader/templates/test_header.c
- [130] https://github.com/SystemRDL/PeakRDL-uvm
- [131] https://github.com/SystemRDL/vscode-systemrdl; https://github.com/seimei-d/systemrdl-pro
- [132] https://github.com/SystemRDL (organizasyon depo listesi)
- [133] https://github.com/SystemRDL/PeakRDL-regblock/blob/main/src/peakrdl_regblock/cpuif/apb4/__init__.py
- [134] https://accellera.org/images/downloads/standards/ip-xact/IPXACT-2022_user_guide.pdf
- [135] https://github.com/kactus2/kactus2dev/blob/main/README.md
- [136] https://github.com/kactus2/kactus2dev/tree/main/Help/componenteditor
- [137] https://github.com/kactus2/kactus2dev/tree/main/Plugins (MemoryMapHeaderGenerator vb.); .../Help/hwdesign/interconnectgenerator.html
- [138] https://github.com/kactus2/kactus2dev/tree/main/IPXACTmodels; .../Plugins/QuartusProjectGenerator/QuartusGenerator.cpp
- [139] https://joss.theoj.org/papers/10.21105/joss.00151
- [140] https://standards.ieee.org/ieee/1685/10583
- [141] https://github.com/rggen/rggen/blob/master/README.md
- [142] https://github.com/rggen/rggen/wiki
- [143] https://raw.githubusercontent.com/rggen/publications/master/rggen_dvcon_paper.pdf (DVCon Japan 2026)
- [144] https://rggen.github.io/rggen-webui/

**Diğer diller**

- [145] https://github.com/rweda/Makerchip-public/blob/HEAD/docs_for_LLMs/TLXSpec.txt
- [146] https://www.redwoodeda.com/details-and-faq
- [147] https://github.com/rweda/Makerchip-public/blob/HEAD/docs_for_LLMs/ICCD2017Paper.txt (ICCD 2017)
- [148] https://www.redwoodeda.com/tl-verilog
- [149] https://github.com/stevehoover/warp-v; https://arxiv.org/pdf/1811.12474
- [150] https://github.com/TL-X-org/tlvflows
- [151] https://github.com/rweda/makerchip-vscode-extension; Makerchip-public/tutorial/tlv/adder.tlv
- [152] https://pypi.org/project/sandpiper-saas/
- [153] https://github.com/stevehoover/tt08-makerchip-template
- [154] https://github.com/janestreet/hardcaml (kernel/clocked_signal.mli, kernel/clock_domain.mli, src/design_rule_checks.mli)
- [155] https://github.com/janestreet/hardcaml (docs/sequential_logic.md, docs/hardcaml_interfaces.md); https://github.com/janestreet/hardcaml_circuits (src/stages.mli)
- [156] https://github.com/janestreet/hardcaml (src/assertions.mli, src/property.mli)
- [157] https://github.com/janestreet/hardcaml_verify (src/nusmv.mli, src/sec.mli)
- [158] https://github.com/janestreet/hardcaml_axi (src/c_register_interface.mli, src/register_bank.mli)
- [159] https://github.com/janestreet/hardcaml_xilinx_reports (src/project.ml)
- [160] https://github.com/janestreet/hardcaml (docs/simulation.md, docs/why.md, docs/rtl_generation.md)
- [161] https://arxiv.org/abs/2312.15035
- [162] https://blog.janestreet.com/protocol-emulator-asic-competition/
- [163] https://github.com/JulianKemmerer/PipelineC (src/VHDL.py, docs/pipelinec_to_pypeline.md, wiki Main-Function-Clock-Crossings)
- [164] https://github.com/JulianKemmerer/PipelineC (README.md, docs/AUTO_PIPELINE_DESIGN.md)
- [165] https://github.com/JulianKemmerer/PipelineC/blob/master/src/SYN.py
- [166] https://github.com/JulianKemmerer/PipelineC (src/pypeline.py, docs/pypeline_guide.md)
- [167] https://github.com/JulianKemmerer/PipelineC/wiki/Example:-StreamSoC; examples/risc-v/gcc_test/mem_map.h
- [168] https://github.com/JulianKemmerer/PipelineC/wiki/Running-the-Tool
- [169] https://github.com/JulianKemmerer/PipelineC/blob/master/docs/LATTE23.pdf
- [170] https://github.com/intel/rohd-hcl/blob/main/lib/src/synchronizer.dart
- [171] https://github.com/intel/rohd/tree/main/doc/user_guide/_docs (A10, A14, A16, A18); https://intel.github.io/rohd/rohd/Pipeline-class.html
- [172] https://github.com/intel/rohd/blob/main/CHANGELOG.md
- [173] https://github.com/intel/rohd-hcl/blob/main/doc/components/csr.md
- [174] https://github.com/intel/rohd/tree/main/rohd_extension; https://github.com/intel/rohd/tree/main/rohd_devtools_extension
- [175] https://github.com/intel/rohd-vf
- [176] https://intel.github.io/rohd-website/blog/; https://capra.cs.cornell.edu/latte24/paper/6.pdf

---

## 8. Doğrulanamayan önemli iddialar

**Çalıştırılarak denenmeyenler.** Rakip derleyicilerin hiçbiri çalıştırılmadı. Aşağıdaki iddialar yalnız kaynak kod ve belgeye dayanıyor:

- Clash `DSignal` tip hatası
- Bluespec G0007
- Veryl `mismatch_clock_domain`
- Arch CDC/RDC ve `arch formal`
- PipelineC CDC `raise`

**Tek kaynağa ya da tarihli ifadeye dayananlar:**

- **Chisel domain denetimi:** ChiselStage'in firtool'a `-domain-mode` geçip geçmediği incelenmedi. "Varsayılan kapalı" yargısı yalnız firtool CLI varsayılanına dayanıyor.
- **Amaranth formal akışı:** "Yok" yargısı 2020 tarihli bir bakımcı ifadesine dayanıyor. 0.6 RFC'leri taranmadı.
- **Arch formal kapsamı:** README ile COMPILER_STATUS çelişiyor. `nogate-ai` hesabının kaç kişiye karşılık geldiği bilinmiyor.

**Silikon kanıtı:**

- Tiny Tapeout projelerinin (Clash, Spade, Amaranth) silikonda çalıştığı doğrulanmadı.
- Jane Street ASIC'lerinin proses düğümü doğrulanmadı.
- ROHD'nin Intel içinde kullanıldığı doğrulanmadı.
- Chisel için "11 tape-out" sayısı yalnız bir arama özetinde görüldü; bu yüzden kullanılmadı.

**Açılamayan kaynaklar:**

- Anvil ASPLOS 2026'nın ACM sayfası 403 verdi; arXiv v2 kullanıldı.
- CACM SHAKTI makalesinin tam tarihi okunamadı.

**Taranmayan alanlar:**

- `bsc-contrib` taranmadı; Bluespec'in I, J, K ve L hücreleri orada değişebilir.
- XLS, Kanagawa, Silice, MyHDL, SUS ve RHDL'de A–O taraması yapılmadı. F ve K boyutlarındaki "bulunamadı" genellemeleri bu dilleri kapsamıyor.

**Katkıcı sayıları:** Yalnız varsayılan dal sayıldı; takma ad birleştirme yapılmadı.
