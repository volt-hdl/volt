# ADR-0079: Çıktı Doğrulama Ağı — Her Çıktı Gerçek Tüketicisiyle Denetlenir

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-25
> Etkilenen: volt-driver/tests (`output_net_tests.rs` — YENİ: ağ;
> `tools/mod.rs` — YENİ: `VOLT_REQUIRE_TOOLS`; `require_tools_tests.rs` —
> YENİ; araç bağımlı testler `tools::require`'a geçti), scripts/sta
> (`sweep.py` — YENİ: her .sdc OpenSTA'ya), volt-sv-emit (`expr.rs` tekli
> işleç + boyut dönüşümü; `sva.rs` kontrol modülü lint pragmaları;
> `builtin_prim.rs` okunmayan primitif çıkışı), volt-sdc-emit (`render.rs`
> hiyerarşik pin `-through`), volt-ast (`mmio_names.rs` — YENİ: sürücü ad
> uzayı), volt-sw-emit (adlar `mmio_names`'ten; setter yereli), volt-syntax
> (`parser/mmio.rs` E1014), volt-diagnostics (E1014), tests/ui (pass 110,
> ağ işaretli 19 fixture; fail 138–141), CI (`integration`, `verify`,
> `timing` işleri)
> İlgili: ADR-0053 (HW-SW köprüsü), ADR-0063 (regmap tutarlılığı),
> ADR-0065 (SDC ikinci ağ), ADR-0070 (tanı paritesi), ADR-0071 (ui/pass
> build), ADR-0072 (örnek çıkışı susturması), ADR-0078 (ayrılmış sözcükler)

## Sorun

"Volt tamam diyor, çıktı geçersiz" sınıfı üç kez çıktı: ADR-0070 (tanı
paritesi), ADR-0078 (`ui/pass/10_explicit_cast` ve `ui/fail/54` temiz
derleniyor, SV geçersiz) ve ADR-0078'in `@mmio` yan bulgusu (`raw` alanı
Rust'ta `ctrl_raw`'ı iki kez tanımlıyor). ADR-0071 ui/pass harness'ına build
denetimi ekledi, ama yalnız Volt'un hata VERMEDİĞİNİ denetliyor; üretilen
dosyanın gerçek araçta geçerli olduğunu değil. Tek tek düzeltmek yerine
sınıfın kalıcı ağı gerekir.

Aynı temada iki açık iş: `@mmio` ad çarpışmaları (sınıf olarak) ve araçlı
CI işlerinde "araç yoksa atla" deseni — kurulum sessizce bozulursa testler
hata vermeden atlanır, CI yeşil görünür (`continue-on-error`'ın başka
biçimi).

## 1. Envanter (ADIM 1.1)

Önce = main `54216fb` (PR #37 sonrası). Sonra = bu ADR. Korpus: `tests/ui/pass`
(96), `examples/` (`*_test.volt` hariç), `tests/fixtures/` (parite
negatifleri ve testler hariç) — **122 tasarım**.

| Çıktı | Gerçek tüketici | Önce CI'da denetlenen | Sonra |
|---|---|---|---|
| SV (`build/rtl/*.sv`) | Verilator `--lint-only -Wall` | `counter.volt`; ui/pass 108–109; `volt test` fixture'ları (sim) | 122 tasarım, her modül üst (Verilator lint işi) |
| SV | Yosys `read_verilog -sv; hierarchy -check; proc; check -assert` | yalnız `volt verify`'ın formal SV'si (6 tasarım) ve STA sentezi (2) | 122 tasarım, her modül üst (Formal işi) |
| SVA ayrı (`.sva` + `bind`) | Verilator `--assert -Wall` (RTL ile) | yok (Yosys `property` okuyamaz, bkz. sby.rs) | 76 dosya |
| SVA satır içi (`--sva=inline`) | Verilator `--assert -Wall` | yok | kontratlı her tasarım |
| Formal SV + `.sby` (`volt verify`) | sby / Yosys `read -formal` | 6 tasarım (verify işi) | değişmedi |
| SDC | OpenSTA `read_sdc` (+ her `get_cells` bir hücre) | ui/pass/87 ve hybrid_accel (`run.py`) | komut içeren her .sdc, iki stil: 52 dosya (`sweep.py`, timing işi) |
| XDC | Vivado yok → Tcl söz dizimi kapısı | yok (yalnız Volt'un kendi `syntax_check`'i) | 150+ dosya, güvenli Tcl alt yorumlayıcısında Vivado komut alt kümesi |
| C başlığı | `cc -std=c99` / `c++ -std=c++17`, `-Wall -Wextra -Werror -pedantic -fsyntax-only` | 3 tasarım (sw_emit_tests; test işinde gcc) | her `@mmio` tasarımı |
| Rust sürücüsü | `rustc --crate-type lib -D warnings` | 3 tasarım (`cargo check`) | her `@mmio` tasarımı |
| regmap JSON | `volt-regmap/1` şeması + `volt check-regmap` | SV kod çözümüyle tutarlılık (sw_emit_tests) | şema (anahtar tipleri, adres = taban + offset, hizalı, ardışık alanlar ≤ 32 bit) + check-regmap üç biçim |
| regmap Markdown | insan | içerik testleri | değişmedi (makine tüketicisi yok) |
| Testbench C++ / `.vlt` | Verilator + C++ derleyicisi (`volt test`) | integration işi | değişmedi |

Her koşucunun geçersiz girdiyi gerçekten REDDETTİĞİNİ gösteren negatif
testi vardır (`*_runner_rejects_*`, `yosys_tcl_gate_rejects_bad_xdc`,
`regmap_schema_rejects_overlapping_fields`): çıktıyı yanlış ayrıştıran bir
koşucu ağı sessizce yeşil bırakamaz. Korpus ve dosya sayıları alt sınırla
denetlenir (dizin okuması boş kalırsa test düşer).

## 2. Uyarı politikası (KARAR)

- **Araç hatası her zaman düşürür**; istisna yok.
- **Verilator `-Wall` ve Yosys uyarıları da düşürür.** Yalnız hatayla
  yetinmek reddedildi: bulunan derleyici kusurlarının üçü (`.sva`
  DECLFILENAME/UNUSEDSIGNAL, okunmayan primitif çıkışı) yalnız uyarıydı
  ve ADR-0024'ten beri örnekler `-Wall` temizdir.
- İstisna yalnız **gerekçeli fixture işaretiyle** (ADR-0071 CHECK-ONLY
  deseni); işaret bayatlarsa (uyarı artık çıkmıyorsa) test yine düşer:

| İşaret | Anlam |
|---|---|
| `//~ LINT-ALLOW: <KOD>: <gerekçe>` | fixture'ın bilinçli Verilator uyarısı |
| `//~ YOSYS-ALLOW: <ileti parçası>: <gerekçe>` | fixture'ın bilinçli Yosys uyarısı |
| `//~ SYNTH-SKIP: <gerekçe>` | Yosys'e (ve SDC taramasına) verilmez |
| `//~ NET-SKIP: <gerekçe>` | hiçbir tüketiciye verilmez |

- Tasarımdan bağımsız iki Yosys bilgi notu testte gerekçeleriyle listelidir:
  "Replacing memory ... with list of registers" (dizi açılımı) ve "limited
  support for tri-state logic" (ADR-0051 inout).
- `@source` extern gövdeleri kullanıcının SV'sidir: tasarımla birlikte
  araca verilir, stil uyarıları sayılmaz, hataları sayılır.
- İşaretler dosya SONUNA yazıldı: satır numaraları ve tanı konumları
  kaymaz.

## 3. İlk koşu (ADIM 1.3) — sınıflandırma

Ağ ilk koşuda **5 geçersiz çıktı** (araç reddetti ya da kısıtı yok saydı)
ve **2 derleyici kaynaklı lint kirliliği sınıfı** yakaladı:

| Bulgu | Tüketici | Sınıf | Karar |
|---|---|---|---|
| `neg = -9'(a)` (ui/pass/12) | Yosys: "Static cast with zero or negative size" (`~9'(a)` de) | A | tekli işlecin işleneni boyut dönüşümüyse parantez: `-(9'(a))` |
| `-from [get_pins {l/pin}]` (ui/pass/68 Board, 69 OpenDrainBus; SDC + XDC) | OpenSTA: "not a valid start point" → kısıt YOK SAYILIR (Vivado da uyarır) | A | hiyerarşik pin yolun başı/sonu olamaz: `-through [get_pins …]` |
| 64'ten uzun dizi sıfırlama döngüsü (examples/riscv_sw/hello_soc) | Verilator 5.020: BLKLOOPINIT | araç sürümü | SV geçerli; CI Verilator'ı runner imajıyla kayan apt 5.020 yerine sabitlenmiş OSS CAD Suite'ten (formal/timing ile aynı önbellek, yeni indirme yok) |
| `.sva` kontrol modülü (76 dosya) DECLFILENAME + UNUSEDSIGNAL | Verilator `-Wall` | A | dosya adı `<modül>.sva` (cli-contract) ve portlar sinyalin tamamını gözler: modül başlığı lint pragmalarıyla sarılır |
| okunmayan yerleşik primitif çıkışı (`hs_busy`, ui/pass/87; 6 primitifte ölçüldü) | Verilator `-Wall` UNUSEDSIGNAL | A | ADR-0072 örnek çıkışı kuralı primitiflere: okunmayan çıkışın bildirimi susturulur (yeni ui/pass/110) |
| gövdesiz extern (ui/pass/62) | MODMISSING / Yosys "not part of the design" | C | `NET-SKIP`: fixture alan bağlamasını gösterir, gövde bilerek yok |
| `tri1` çekme ağı (ui/pass/69) | Yosys `tri1` ayrıştıramaz | C | `SYNTH-SKIP`: kart seviyesi pull-up; Verilator'da temiz |
| ayrık karışık sürücü `lanes` (ui/pass/93) | Yosys uyarısı | C | `YOSYS-ALLOW`: IEEE 1800 §6.5 ayrık bitlere izin verir (ADR-0073). Açık bulgu: bazı ticari araçlar `always_comb` değişkenine ikinci süreçten yazmayı reddeder |
| fixture'ın bilinçli uyarıları (17 fixture: kullanılmayan `clk`, yalnız kontratta okunan hayalet register, dar indeks, örtüşen kol, sabit kontrat) | Verilator `-Wall` | C | `LINT-ALLOW` + gerekçe |
| `@source` gövde dosyası DECLFILENAME (ui/pass/101, fixtures/extern_source) | Verilator | harness | extern gövdeleri araca verilir, stil uyarıları sayılmaz |

B sınıfı (fixture'ın kendisi hatalı) bulunmadı. `examples/` içinde örnek
hatası yok (BLKLOOPINIT araç sürümüdür). C, C++, Rust, JSON ve XDC'de ilk
koşuda bulgu yok.

**Golden:** geçerli tasarımların çıktısı yalnız yukarıdaki A düzeltmeleri
kadar değişti (122 tasarım, bütün `--emit` türleri, önce/sonra dökümü):
`12` SV'de bir satır; `87` SV'de iki pragma satırı; `68`/`69` SDC ve XDC'de
`-from` → `-through`; 76 `.sva` dosyasında başlık yorumu ve pragmalar;
yeni `110`.

## 4. CI süresi (ADIM 1.4)

Ölçüm PR #38 (run 36147681759) ile taban (PR #37, #36, #35):

| İş | Önce | Sonra | Ağ adımı |
|---|---|---|---|
| Verilator lint | 83–124 s | 165 s | +15 s (Verilator + cc + c++ + rustc); OSS CAD önbelleği 8 s, apt kurulumu −11 s |
| Formal verification | 46–105 s | 62 s | +7 s (Yosys + XDC Tcl) |
| Timing | 62–89 s | 122 s | +64 s (52 .sdc, iki stil) |
| Fuzz (60 s) — kritik yol | 172–189 s | 229 s | — (ağ yok; sapma cargo-fuzz kurulumu) |

**KARAR:** ağ adımlarının toplamı ~86 s (< 2 dk) ve paralel işlere dağıtılmış
durumda; hiçbir iş kritik yolu (Fuzz) aşmıyor, PR duvar süresi Fuzz'a bağlı
kalıyor. Hepsi PR CI'da kalır; gecelik ayırma gerekmedi. SDC taraması en
pahalı parçadır (Docker'da OpenSTA çağrısı başına ~1 s); büyürse ilk aday
tek konteynerde toplu koşudur.

## 5. `@mmio` ad çarpışmaları (Bölüm 2)

Sürücüler adlarını kullanıcının adlarından kurar. Taranan sınıf (Rust
`impl` tek ad uzayıdır; C'de makrolar ve işlevler dosya kapsamındadır):

| # | Yol | Örnek | Ağ yakalar mı? |
|---|---|---|---|
| 1 | alan ↔ ham sözcük erişimcisi | `ctrl.raw` → `ctrl_raw` = `ctrl_raw()` / `set_ctrl_raw` | Rust derlenmez; fixture varsa |
| 2 | register/alan yolu birleşmesi | `irq.status_rx` ↔ `irq_status.rx` → `irq_status_rx`, `IRQ_STATUS_RX_SHIFT` | Rust evet; C makroları AYNI değerdeyse sessiz |
| 3 | register ↔ alan sabiti | `ctrl_en` register'ının `CTRL_EN_MASK`'ı ↔ `ctrl.en`'in `CTRL_EN_MASK`'ı | C'de değer aynıysa sessiz |
| 4 | büyük/küçük harf katlaması | `ctrl` ↔ `Ctrl` → `CTRL_OFFSET` | Rust evet |
| 5 | üreticinin kendi adı | tek alanlı `new`/`read`/`write` register'ı ↔ Rust kurucusu/yardımcıları; `h`/`base` ↔ C `GPIO_H` koruması / `GPIO_BASE` | kısmen |
| 6 | C setter parametresi ↔ makro ya da `<stdint.h>` tipi | alan `uint32_t` → `uint32_t uint32_t` | C evet |
| 7 | iki modül aynı dosya köküne | `GpioRegs` ↔ `GPIORegs` → `build/sw/gpio_regs.*` | **hayır** — ikinci sürücü birincinin dosyasının üzerine yazar, ikisi de geçerli |
| 8 | setter parametresi ↔ gövde yereli | alan `word` → `set_r_word(word)` içinde `let word = self.read(..)` | **hayır (Rust)** — gölgeleme DERLENİR ve eski sözcüğü yazar; C'de yeniden bildirim hatası |

**Ağ bu sınıfı tek başına kapatamaz:** yalnız fixture'ların gösterdiği
adları derler; 7 ve 8 (Rust) hiçbir derleyicinin görmediği çarpışmalardır,
2/3 C'de aynı değerli makro yeniden tanımı olarak sessizdir.

**KARAR (ADR-0078 "sessizce ad değiştirme yok" ilkesiyle):**
- 1–7: **reddet — YENİ E1014.** Tanı ikinci adın konumuna bağlanır, ilk ad
  ikincil etiket olur; üretici adında hangi adın tutulduğu söylenir.
  Anahtar sözcük olan ad zaten E1013 alır (aynı ada ikinci tanı yok).
  Adlandırma şeması değiştirilmedi: sürücü API'si yazılımın çağırdığı
  şeydir ve mevcut sürücüler değişmemeli.
- 8: kullanıcının gördüğü hiçbir ad değişmeden üretici hijyeniyle
  düzeltildi: alan `word` ise okuma-değiştirme-yazma yereli `current`
  olur (yalnız bu durumda; diğer çıktılar bayt bayt aynı).
- Kural tek kaynaktır: `volt_ast::mmio_names` (üreticiler de oradan
  adlandırır). `volt-sw-emit/tests/names_tests.rs` üretilen Rust/C
  metninden tanımlayıcıları çıkarıp kuralın kümesiyle eşitler: üretici
  yeni bir ad yazarsa ya da kural kayarsa test düşer.

## 6. "Araç yoksa atla" (Bölüm 3) — `VOLT_REQUIRE_TOOLS`

`crates/volt-driver/tests/tools/mod.rs`: araç yoksa test atlanır;
`VOLT_REQUIRE_TOOLS` (virgüllü: `verilator,sby,yosys,cc,cxx,rustc` ya da
`all`) içindeki araç bulunamazsa test **düşer**. Bilinmeyen ad da düşürür
(yazım hatası hiçbir aracı sessizce zorunluluktan çıkarmasın). Arama
`volt`'un kendisiyle aynı sırada: `VOLT_VERILATOR`/`VOLT_SBY`/`CC`/`CXX`,
sonra PATH.

Tarama: araç bağımlı her test (`cli_tests` sby, `extern_source_tests`,
`reserved_name_tests`, `sim_bounds_tests`, `sim_contract_tests`,
`struct_tests`, `sw_emit_tests`, `output_net_tests`) bu kurala geçti;
`scripts/sta/*.py` zaten araç yoksa hata verir. Tarama iki gizli boşluk
buldu: `sim_bounds_tests` ve `struct_tests`'in gerçek Verilator yolları ile
`cli_tests`'in gerçek sby testleri yalnız araçsız test işinde koşuyordu —
yani CI'da **hiç** koşmuyordu. Artık araçlı işte zorunlu olarak koşarlar.

| İş | `VOLT_REQUIRE_TOOLS` |
|---|---|
| Format + Lint + Tests | — (atlama sürer) |
| Verilator lint | `verilator` (+ `cc`, `cxx`, `rustc` ilgili adımlarda) |
| Formal verification | `sby`, ağ adımında `yosys` |
| Timing | betikler araç yoksa zaten düşer |

Mutasyon kalıcıdır: `require_tools_tests` kendi sonda testini alt süreçte
PATH boşken koşturur — zorunlu değilse atlar (çıkış 0), zorunluysa ve yazım
hatasında düşer.

## Reddedilenler

- **Yalnız hata (uyarıları dışarıda tut):** üç derleyici kusurundan ikisi
  yalnız uyarıydı; örnekler zaten `-Wall` temiz.
- **Çarpışmada adlandırma şemasını değiştirmek** (`ctrl_raw` → `raw_ctrl`
  ya da önek): bütün mevcut sürücüler değişirdi ve yeni şema da çarpışabilir.
- **Çarpışan adı sessizce yeniden adlandırmak:** ADR-0078.
- **Ağı Python betiği olarak tek işte koşmak:** `VOLT_REQUIRE_TOOLS` ile
  aynı kurala uyması ve yerelde `cargo test` ile koşması için Rust testi;
  yalnız SDC (Yosys sentezi + OpenSTA imajı) betik kaldı.

## Sınırlar

- `SYNTH-SKIP`/`NET-SKIP` fixture'ları (62, 69) SDC taramasına girmez;
  69'un `-through` düzeltmesi birim testiyle (sdc_emit_tests) korunur.
- XDC için gerçek Vivado yok: kapı komut/seçenek/sayı/parantez düzeyindedir,
  nesne varlığını denetlemez (SDC'de OpenSTA denetler).
- Alt modül `.sdc`'leri üst modülün dosyasında hiyerarşik yolla denetlenir;
  alt modülün kendi dosyası yalnız komut içeriyorsa ayrıca okunur.
- Açık bulgu: ui/pass/93'ün `always_comb` + `assign` karışık sürücüsü
  (ADR-0073) bazı ticari araçlarda reddedilebilir; ayrı iş.
