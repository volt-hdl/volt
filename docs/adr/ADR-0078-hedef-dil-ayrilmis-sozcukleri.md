# ADR-0078: Hedef Dillerin Ayrılmış Sözcükleri — SV Anahtar Sözcüğü Ad Olamaz

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-25
> Etkilenen: volt-ast (`reserved.rs` — YENİ: SV/Rust/C/C++/Verilator
> sözcük tabloları), volt-sv-emit (`sv_names.rs` — YENİ: E1013 kesin
> denetimleri + güvenlik ağı; `lib.rs` port bildirimi SYMRSVDWORD
> susturması, `emit_unit` kancaları; `instance.rs` örnek çıkış teli;
> `sim.rs`/`sim_script.rs` `cpp_port`), volt-syntax (`parser/mmio.rs`
> `@mmio` adlarının Rust/C/C++ denetimi), volt-hir (`sim_top.rs` — YENİ:
> E8513; `sim.rs` `check_let_dut`), volt-driver (`sim/run_cmd.rs` E8513),
> volt-diagnostics (E1013, E8513), tests/ui (pass 108–109, fail 122–137;
> pass/10 ve fail/54, fail/115 adları), tests/fixtures/parity (kw01–kw05),
> CI (`integration` işine gerçek Verilator adımı)
> İlgili: ADR-0028 (Volt'un kendi ayrılmış sözcükleri), ADR-0053 (HW-SW
> köprüsü), ADR-0065 (SDC hücre adları), ADR-0070 (tanı paritesi),
> ADR-0077 (struct yaprak adları; yan bulgu buradan)

## Sorun

ADR-0077 Aşama 2'nin yan bulgusu: `out packed : u32` için `volt check` ve
`volt build` TEMİZ, üretilen SV Verilator'da **sözdizimi hatası**. "Volt
tamam diyor, çıktı geçersiz" sınıfı: ADR-0070 bu sınıfı tanılar için
kapattı (check = build = LSP), ama ÇIKTININ geçerliliği için kapatmadı.
ADR-0028 yalnız Volt'un kendi rezervasyonlarını (`fifo`, `ram`, ...) ve
bunların SV ile çakışmadığını tek tek denetledi; SystemVerilog'un 248
anahtar sözcüğü için genel bir denetim yoktu. Aynı sınıf üretilen diğer
dillerde de vardır: `@mmio` haritasının Rust/C sürücüsü ve Volt'un
Verilator testbench'i (C++).

## Tespit (ADIM 1)

### Ayrılmış sözcük kümeleri (ölçüldü)

| Küme | Kaynak | Sayı | Ölçüm |
|---|---|---|---|
| SystemVerilog | IEEE 1800-2017 Annex B | 248 | Her sözcük `module t(input logic <w>, ...)` portu olarak Verilator 5.050 `--lint-only`'de denendi: 247'si sözdizimi hatası; `global` Verilator'da bağlamsal kabul ediliyor ama standartta ayrılmış — listede kalır |
| Verilog-2005 | IEEE 1364-2005 | — | SV kümesinin alt kümesi (`cell`, `config`, `design`, `incdir`, `include`, `instance`, `liblist`, `library`, `use` dahil); 1800-2023 yeni sözcük eklemedi |
| Verilator C++ sözcükleri | `src/V3LanguageWords.h` (v5.050, GitHub'dan) | 122 | C++ anahtar sözcüğü, "C++ common word" (`interrupt`, `stack`, `queue`, `near`, `far`, `abort`, ...), SystemC; `std::` ve ön işlemci girdileri hariç |
| Verilator model üyeleri | Üretilen `V<Top>.h` + `VerilatedModel` | 20 | `eval`, `name`, `trace`, `rootp`, `contextp`, `final`, ... |
| Rust | 2021 katı + ayrılmış, 2024 `gen` | 52 | Her aday `pub fn <w>()` ve parametre adı olarak `rustc --edition 2021`'de denendi; bağlamsal `union`, `raw`, `default`, `macro_rules` geçer |
| C / C++ | C11 + C++20 (+ `NULL`, `bool/true/false` makroları) | 105 | `gcc -std=c11` ve `g++ -std=c++20` ile `void f(uint8_t <w>)` başlığı denendi (gcc:latest) |

### Ad yolları (her biri bir Volt tasarımıyla DOĞRULANDI)

Tarama: `build/reserved/survey.py` — her yol için anahtar sözcüklü bir
tasarım, `volt check` → `volt build` → Docker Verilator 5.050 `-Wall`.

| Yol | SV adı | Önce (check/build/Verilator) | Sonra |
|---|---|---|---|
| Port | aynen | temiz / temiz / **sözdizimi hatası** | E1013 |
| `reg` | aynen | temiz / temiz / **hata** | E1013 |
| `wire` | aynen | temiz / temiz / **hata** | E1013 |
| Modül seviyesi `let` | aynen (`wire`) | temiz / temiz / **hata** | E1013 |
| Modül adı | aynen (+ dosya adı, `.sby` `prep -top`) | temiz / temiz / **hata** | E1013 |
| Kullanıcı modülü örneği | aynen (`Sub instance (...)`) | temiz / temiz / **hata** | E1013 |
| Extern modül adı / portu | aynen (`.cell(a)`) | temiz / temiz / **hata** | E1013 |
| `const` (skaler) | satıra katlanır | temiz / temiz / temiz | E1013 (bkz. Karar 1 kural 3) |
| `const` dizi | `localparam <ad>` ya da `<ad>_at` işlevi (ADR-0041 biçimi) | temiz / temiz / `_at` biçiminde temiz, `localparam` biçiminde **hata** | E1013 |
| Struct yaprağı `<sinyal>_<alan>` | bileşik | `pulsestyle`+`ondetect` → **hata** | E1013 (birleşimi adlandırır) |
| Bundle alanı `<port>_<alan>` (ADR-0039) | bileşik | `pulsestyle`+`onevent` → **hata** | E1013 |
| Enum localparam `<Enum>_<Varyant>` | bileşik | `pulsestyle`+`ondetect` → **hata** | E1013 |
| Örnek çıkış teli `<örnek>_<port>` | bileşik | `pulsestyle`+`ondetect` → **hata** | E1013 |
| Yerleşik primitif örneği (ADR-0027) | yalnız önek (`buf_mem`) + yorum | `let buf = AsyncDualPortRam` temiz | temiz (reddedilmez) |
| Modül seviyesi `for` açılımı (ADR-0056) | `<ad>_<i>` | `let edge` → `edge_0` temiz | temiz |
| Generic monomorfizasyon | `<Ad>_<değer>` | rakamla biter, anahtar sözcük olamaz | temiz |
| `sync_*`, `rst_sync_*`, `past_*`, `volt_hits_*`, SVA etiketleri, `_ext`, `_at`, yerleşik primitif sonekleri | sabit önek/sonek | anahtar sözcük üretemez (güvenlik ağı yine denetler) | temiz |
| Struct tipi, struct alanı tek başına, enum tipi/varyantı tek başına, tip takma adı, domain, `fn` | SV'ye ADSIZ ya da yalnız bileşik iner | temiz | temiz (reddedilmez) |
| Blok içi `let`, `match` bağlaması | SV'ye inmez (E0003) | — | — |
| Test adı / test `let`'i | C++ dize literali / `v_<ad>` | temiz | temiz |
| `.vlt` `public_flat_rw -var "<ad>"` | dize | temiz | temiz |
| Üst modül portu, Verilator C++ sözcüğü (`char`, `interrupt`) | aynen; Verilator modeli `__SYM__<ad>` | temiz / temiz / **SYMRSVDWORD — varsayılan açık, `-Wall`'sız da çıkış 1**; `volt test` **düştü** | SV'de `lint_off SYMRSVDWORD` sarması; testbench `__SYM__<ad>` → `volt test` geçer |
| İç sinyal / alt modül portu, C++ sözcüğü (`reg delete`) | aynen | temiz (Verilator `__PVT__` önekler) | temiz |
| Üst modül portu, model üyesi (`eval`, `name`, `trace`, `rootp`, `contextp`) | aynen | SV geçerli; Verilator'ın ürettiği C++ **derlenmiyor** (`VT::eval()` ile çakışma; Verilator korumaz) | `volt test`/`volt run`'da E8513 |
| `@mmio` alan adı (Rust parametresi + metot) | `set_ctrl_mod(&mut self, mod: bool)` | `mod`, `yield` → **rustc hatası** | E1013 |
| `@mmio` alan adı (C parametresi) | `kw_set_ctrl_default(uint8_t default)` | **gcc hatası**; `class` → **g++ hatası** | E1013 |
| `@mmio` tek alanlı register adı (Rust getter) | `pub fn loop(&self)` | **rustc hatası** | E1013 |
| `@mmio` modül adı (Rust struct'ı) | `pub struct <Modül>` | yalnız küçük harfli adda | E1013 |
| SDC/XDC hücre adları (ADR-0065) | SV adlarından türer | — | değişmez (Karar 1 yeniden adlandırmaz) |

Büyük/küçük harf: SV duyarlıdır; `in Packed` temiz (ölçüldü).

## Karar

### Seçenekler

- **A — Reddet:** anahtar sözcük Volt'ta SV adı olamaz; `check`'te hata +
  öneri.
- **B — Kaçır:** emitter `\packed ` üretir. Geçerli SV, ama dış
  entegratörün bağlandığı port adı `\packed ` olur; SDC/XDC'de kaçış
  (`{\packed }`) gerekir, dalga biçimi araçları farklı gösterir, Verilator
  C++ modelinde ad yine değişir.
- **C — Yeniden adlandır:** emitter sonek ekler (`packed_v`). Dış port adı
  kullanıcının yazdığından SESSİZCE farklı; eşleme tablosu belgelenmeli,
  kullanıcı `packed_v` diye bir ad hiç yazmadığı için hata iletilerinde,
  SDC'de, testbench'te iki ad dolaşır.
- **D — Karma:** kullanıcı adında A, üretilen bileşik adda B/C.

### KARAR: D'nin ölçülerek daraltılmış biçimi

1. **SystemVerilog anahtar sözcüğü → A (E1013), bileşik adlar dahil.**
   Volt'un ilkesi yazdığınız adın çıktıda aynen durmasıdır (ADR-0024 dosya
   adı, ADR-0039 düz bundle portları, ADR-0065 SDC hücre adları, ADR-0077
   yaprak adları hep buna dayanır). B ve C bu ilkeyi yalnız kötü adlarda
   bozar ve bozukluk dış arayüzde görünür: port adı entegratörün
   sözleşmesidir. Bileşik adlarda da A seçildi, B/C değil: `pulsestyle` +
   `ondetect` birleşimi dış porttur (struct portu, bundle alanı) ya da
   dalga biçiminde görünen sinyaldir; sessiz değişiklik aynı sorunu taşır.
   Bileşik yolun gerçek kapsamı dardır (iki geçerli parçanın birleşiminden
   çıkan anahtar sözcük yalnız `pulsestyle_ondetect`/`pulsestyle_onevent`;
   `always_ff`, `s_until`, `sync_accept_on` gibi diğerlerinde bir parça
   zaten anahtar sözcüktür ya da Volt'un kendi anahtar sözcüğüdür, ör.
   `on`, `match`), yani A'nın kullanıcıya maliyeti ihmal edilebilir. Kurallar:
   - Adın SV'de AYNEN geçtiği her bildirim denetlenir: modül, extern modül ve
     portu, port, `reg`, `wire`, modül seviyesi `let`, kullanıcı/extern
     modülü örneği, `const`.
   - Bileşik adlar ÜRETİLDİĞİ yerde denetlenir: struct yaprağı ve bundle
     alanı (indirgenmiş AST'deki portlar), enum localparam'ı, örnek çıkış
     teli. İleti birleşimi adlandırır: `'pulsestyle' becomes the
     SystemVerilog name 'pulsestyle_ondetect', which is a keyword`.
   - `const`'un tamamı denetlenir (skaler de): skaler bugün satıra katlanır,
     ama dizi biçimi `localparam`'dır ve hangi biçimin seçileceği kullanıcıya
     görünmez; tek kural ("her sabit adı") öğrenilmesi kolay olandır. Ölçüm:
     depoda anahtar sözcük adlı sabit yok.
   - Enum'un BÜTÜN varyantları denetlenir, kullanılmayanlar da: localparam
     yalnız bir modülün kullandığı varyant için üretilir, ama ad bildirimde
     belirlenir ve düzeltilecek yer orasıdır (sabitlerle aynı tek kural).
   - SV'ye adsız inen adlar reddedilmez: struct alanı (`p_packed` geçerli),
     enum tipi ve varyantı tek başına (`release_force` geçerli), yerleşik
     primitif örneği (`buf_mem`), `for` açılımı (`edge_0`). Gereksiz
     yasak koymamak için.
2. **`@mmio` register/alan/modül adı, Rust ya da C/C++ anahtar sözcüğü →
   A (E1013).** Ad üretilen sürücüde metot, parametre ve (modül) tip adıdır;
   sürücü yazılımcının API'sidir, sessiz değişiklik (`r#mod`, `mod_`) API'yi
   Volt kaynağından koparır. Başlık `extern "C"` korumasıyla C++'ta da
   derlendiği için C ve C++ kümeleri birlikte uygulanır. Yalnız `@mmio`
   adları denetlenir; diğer Volt adları yazılım tarafına inmez.
3. **Verilator C++ sözcüğü (SV geçerli) → reddetme; aracın iç adını Volt
   karşılar.** `interrupt`, `stack`, `queue`, `abort`, `near`, `far` gerçek
   donanım adlarıdır ve SV'de geçerlidir; yalnız bir simülatörün iç C++
   modeli yüzünden reddetmek yanlış olur. Verilator bu adı üst modül
   portunda `__SYM__<ad>` yapar ve SYMRSVDWORD uyarısıyla (varsayılan açık,
   uyarılar varsayılan ölümcül) durur. Volt:
   - Port bildirimini `// verilator lint_off SYMRSVDWORD` ... `lint_on` ile
     sarar ve üstüne `// C++ word: as a Verilator top-level port this is
     __SYM__<ad>` yorumu koyar (kendi testbench'ini yazan kullanıcı için).
     Yalnız bu adları taşıyan tasarımların SV'si değişir.
   - Testbench (`volt run`, `volt test`) portlara `dut.__SYM__<ad>` diye
     erişir (`volt_ast::reserved::verilator_symbol`).
   - İç sinyal ve alt modül portu Verilator'da `__PVT__` önekini alır; uyarı
     yok (ölçüldü), dokunulmaz.
4. **Verilator model üyesiyle çakışan üst modül portu → E8513, yalnız
   simülasyonda.** `eval`, `name`, `trace`, `rootp`, `contextp`, ... portu
   olan modül üst modül yapılınca Verilator'ın ürettiği `V<Top>` sınıfı
   derlenmez (Verilator bunu yeniden adlandırmaz; ölçüldü). SV geçerli ve
   modül başka bir modülün alt modülü olarak sorunsuzdur, bu yüzden `check`/
   `build` reddetmez; `volt test` (test dosyasının `let dut = M { }` denetimi
   — `volt check` bir test dosyasında da görür) ve `volt run` (seçilen üst
   modül, Verilator'dan önce) E8513 verir. `V<Top>` adlı port (sınıf adıyla
   çakışma) da dahil.

### Değerlendirme ölçütleri

- **Dış SV entegrasyonu:** A'da port adı hiç değişmez; reddedilen ad zaten
  hiçbir araçta derlenemezdi. B/C'de entegratörün bağlantısı Volt kaynağına
  bakılarak yazılamazdı.
- **Kullanıcı deneyimi:** kullanıcı her zaman kendi yazdığı adı görür;
  bileşik adda ileti hangi iki parçanın birleştiğini söyler, öneri somut
  (`packed_`).
- **HW-SW köprüsü (ADR-0053):** Rust/C adları Volt adlarıyla birebir kalır;
  `check-regmap` (ADR-0063) eşlemesi değişmez.
- **SDC/XDC (ADR-0065):** A yeniden adlandırmadığı için hücre adları ve
  `scripts/sta` tutarlılık denetimi etkilenmez; Karar 3'ün sarması yalnız
  yorum satırıdır (SDC değişmedi, golden).

## Uygulama

- **`volt_ast::reserved`** tek tablo: `SV_KEYWORDS` (248),
  `RUST_KEYWORDS` (52), `C_CPP_KEYWORDS` (105), `VERILATOR_CPP_WORDS` (122),
  `VERILATOR_MODEL_MEMBERS` (20); sıralı, ikili arama; testi sıralamayı ve
  Annex B sayısını doğrular.
- **`volt-sv-emit/src/sv_names.rs`** (E1013). Denetim emitter'dadır, çünkü
  son SV adı (struct indirgemesi, bundle düzleştirmesi, `for` açılımı
  sonrası) yalnız burada bilinir; `volt check` ve LSP aynı emit'i çıktısız
  koşar (ADR-0070), yani parite yapısaldır. İki katman:
  1. Kesin denetimler, kaynağa işaret eden span'le:
     `audit_unit_names` (modül, extern, `const`, enum localparam'ı ve her
     modül için `audit_module_names`: portlar — bundle alanında gerçek span
     `BundleOrigin.port`'tan — ve gövde bildirimleri), `check_sv_name`
     (`instance.rs` örnek çıkış teli). Bütün modüllerin kesin denetimi emit
     döngüsünden ÖNCE koşar: alt modülün portu üst modülün örnek
     bağlantısında (`.table(a)`) önce görünür; üst modül kaynakta önce
     gelirse güvenlik ağı aynı adı ikinci ve yanlış satırlı bir tanıyla
     bildiriyordu (bağımsız inceleme bulgusu, regresyon testi
     `keyword_port_is_reported_once_whatever_the_module_order`).
  2. **Güvenlik ağı** `audit_emitted_text`: üretilen her modül metni
     belirteçlere ayrılır (yorum, dize, `` ` ``/`$` adları, tabanlı sayı
     harfleri atlanır); emitter'ın kendi yazdığı SV sözdizimi sözcükleri
     (`EMITTER_KEYWORDS`, 44 sözcük; bütün SVA kiplerinin çıktı külliyatından
     ölçüldü) dışında kalan her anahtar sözcük belirteci bir ad olarak
     yazılmıştır → E1013 (span: modül adı). Kesin denetimlerin bilmediği yeni
     bir ad yolu burada yakalanır; emitter yeni bir SV sözdizimi sözcüğü
     yazmaya başlarsa sözlüğe eklemek zorundadır (eklemezse bütün tasarımlar
     E1013 verir — mutasyon `net_vocab` bunu gösterir).
- **SYMRSVDWORD sarması** `lib.rs` `verilator_cpp_word_port` (port
  bildirimi, struct yaprağı dahil son SV adına göre); **`__SYM__`**
  `sim.rs` `cpp_port` — testbench'in port yazdığı/okuduğu 12 noktanın hepsi.
- **E8513** `volt-hir/src/sim_top.rs` `verilator_top_clashes` (struct
  yaprakları açılmış son port adları); `sim.rs` `check_let_dut` ve
  `volt-driver` `run_cmd.rs` (üst modül seçildikten sonra, Verilator'dan
  önce).
- **`@mmio`** `parser/mmio.rs` `check_sw_name`: modül, register, alan.
  Tanı doğrudan eklenir; ayrıştırıcının kaskad bastırma penceresi
  (`push_error`) desugar aşamasında ardışık adları yutuyordu (ölçüldü:
  altı adlı haritada tek tanı).

### Tanılar

| Kod | Ne | Nerede |
|---|---|---|
| **E1013** | Ad, üretilen bir dilin ayrılmış sözcüğü (SV; `@mmio`'da Rust/C/C++) | `check` = `build` = LSP |
| **E8513** | Üst modül portu Verilator model arayüzüyle çakışıyor | `volt test` (ve test dosyasında `volt check`), `volt run` |

İkisi de iki dilde (en/tr) ileti + `volt explain`; 5 parça (kod, konum,
açıklama, öneri, bu ADR).

## Doğrulama (ADIM 4)

- **Fixture'lar:** `tests/ui/fail/122–137` (port, reg, wire, let, const,
  modül adı, örnek, extern, struct yaprağı, bundle alanı, enum localparam'ı,
  örnek çıkış teli, `@mmio` Rust/C/register, model üyesi);
  `tests/ui/pass/108` (reddedilmeyen konumlar: `Packed`, `p_packed`,
  `b_table`, `release_force`, `edge_0`, `buf_rd_data`), `109` (C++ sözcüklü
  portlar); parite sondaları `kw01–kw05`. `volt-driver/tests/
  reserved_name_tests.rs` (24 test) kodu, satırı ve iletiyi denetler.
- **Üç hedef dil:** SV (Verilator lint + sözdizimi), C++ (Verilator modeli:
  `volt test` `char`/`interrupt`/`stack` portlarıyla uçtan uca; model üyesi
  E8513), Rust/C (anahtar sözcüğe komşu geçerli adlarla — `safe`, `mod_`,
  `default_`, `union_`, `interrupt` — üretilen sürücü `rustc` ve `gcc`/`g++`
  `-Werror` ile derlenir; anahtar sözcük adları E1013).
- **Gerçek Verilator (Docker, 5.050):** `reserved_name_tests` 24/24 (lint
  `-Wall` 0 bulgu, `volt test` geçti), `struct_tests` 29/29,
  `sim_contract_tests` 20/20, `extern_source_tests` 11/11; `-Wall` temiz:
  pass/108, 109, 10, 65, fail/54, kw05.
- **Mutasyon** (`build/reserved/mutate.py`, tek tek): **14/14 yakalandı** —
  port denetimi, birim adları, enum bileşimi, örnek çıkış teli, bundle
  span'i, ağ sözlüğü (`always_ff` çıkarılınca bütün tasarımlar düşer: ağ
  boru hattında etkin), ağ belirteç ayırıcısı, `@mmio` denetimi, C/C++
  listesi, SYMRSVDWORD sarması, `__SYM__` adı, E8513 (test), E8513 (run),
  modül denetimini emit döngüsüne geri taşımak (sıra regresyonu).
  İlk koşuda `bundle_span` kaçtı (fixture iletinin yalnız ikinci yarısını
  arıyordu); fixture iletileri kullanıcının yazdığı adı içerecek şekilde
  sıkılaştırıldı, sonra yakalandı. **Kasıtlı hata, gerçek araçla:**
  `__SYM__` kaldırılınca Docker'da testbench derlenmedi (`'class
  VIrqLatch' has no member named 'interrupt'`).
- **Golden** (`build/reserved/golden.py`; referans PR #36 sonrası `main`
  e4292c4; `volt check` + `volt build --emit=sva,rust,c,regmap,sdc`
  tanıları ve üretilen her `.sv/.sva/.sdc/.rs/.h/.json`): 447 dosyadan
  farklı olan 22'si yalnız yeni fixture'lar (fail/122–137, pass/109,
  kw01–kw05); kw05 ve pass/109'da fark yalnız C++ sözcüklü portun sarma
  satırları. Önceden var olan **bütün geçerli tasarımların çıktısı
  byte-aynı** (bütün örnekler dahil).

### Mevcut kod taraması (RAPOR)

Tarama: `build/reserved/scan_existing.py` (`.volt` + Rust testlerine gömülü
Volt metni) ve golden.

| Yer | Ad | Durum |
|---|---|---|
| `tests/ui/pass/10_explicit_cast.volt` | `in small`, `in large` | **Geçersiz SV üreten bir "pass" fixture'ı.** Portlar `x8`/`x16` oldu; eski hâli fail/122 |
| `tests/ui/fail/54_inout_unsynchronized.volt` | `out bit` | W3007 fixture'ı temiz derlenip **geçersiz SV** üretiyordu; `sample` oldu |
| `volt-driver/tests/struct_tests.rs` `bits_to_struct_slices_by_the_layout` | `out bit` | Test `assign bit = ...` satırını — geçersiz SV'yi — bekliyordu (ADR-0077 Aşama 2); port `f3_is5` oldu |
| `tests/ui/fail/115_struct_array.volt` | `reg table` | E0003 fixture'ı; artık E1013 de alırdı, `entries` oldu (fixture tek amaçlı kalsın) |
| `tests/ui/fail/02_width_mismatch.volt`, `fail/10_index_out_of_bounds.volt`, `volt-hir/tests/typeck_tests.rs` (`small`/`large`), `mono_tests.rs`/`mono_semantic_tests.rs` (`reg buf`) | SV sözcükleri | Birincil hata emit'i durdurur ya da yalnız ayrıştırıcı/HIR testi; SV'ye hiç ulaşmaz — değiştirilmedi |
| `tests/ui/pass/65`, `fail/51` (`let buf`), `pass/76` (`let cell` `for` içinde), `examples/riscv_core_test.volt` (`let program`, test dili) | SV sözcükleri | SV'de ad değiller (önek / `cell_0_1` / C++ `v_program`); geçerli, dokunulmadı |
| `examples/` | — | Hiçbir örnek SV anahtar sözcüğü, Verilator C++ sözcüğü ya da model üyesi port kullanmıyor; bütün örneklerin çıktısı byte-aynı |

## Reddedilenler

- **B (kaçış) ve C (sonek) SV adları için** — yukarıda: dış arayüzü sessizce
  değiştirir, SDC/dalga biçimi/testbench'te iki ad dolaşır.
- **Verilator C++ sözcüklerini reddetmek** — `interrupt`, `stack`, `abort`
  gerçek ve geçerli donanım adları; sorun bir aracın iç modelinde, çözümü de
  orada (Karar 3).
- **Kontrolü HIR çözümleyicisinde yapmak** — kullanıcı adları orada görünür,
  ama struct yaprağı, enum localparam'ı ve örnek çıkış teli emitter'da
  üretilir; iki yerde iki ad kuralı aynı sapma sınıfını doğururdu (ADR-0070
  gerekçesiyle aynı).
- **Yalnız güvenlik ağı** — her yolu yakalar ama span'i modül adıdır ve
  iletisi adın kaynağını söyleyemez; kesin denetimler kullanıcıya doğru
  satırı gösterir, ağ yalnız bilinmeyen yollar içindir.

## Sınırlar

- Verilator listeleri 5.050'den ölçüldü; Verilator yeni bir "common word" ya
  da model üyesi eklerse liste güncellenmelidir (belirti: `volt test`'te
  SYMRSVDWORD ya da C++ derleme hatası).
- Başka simülatörlerin (Icarus, ticari araçlar) kendi ek ayrılmış
  sözcükleri kapsanmadı; SV standardı dışındaki tek uyarlama Verilator'dur
  (Volt'un simülasyon aracı).
- Verilator `VL_*` makrolarıyla çakışan büyük harfli port adları
  (`VL_IN8`) ölçülmedi.
- Güvenlik ağının span'i modül adıdır; bugün hiçbir tasarım ona ulaşmaz
  (bütün bilinen yollar kesin denetimde). Ağ, kesin denetimin birimde zaten
  bildirdiği bir adı atlar: aynı ad başka bir modülde yalnız ağın göreceği
  bir yoldan da gelirse o ikinci yer ilk düzeltmeden sonra görünür (derleme
  yine düşer; yalnız tanı tamlığı).

## Yan bulgular (bu ADR'de düzeltilmedi)

- **`@mmio` alan adı `raw` Rust sürücüsünde register'ın ham erişimcisiyle
  çakışıyor**: `ctrl : { raw : bool, ... }` → `pub fn ctrl_raw(&self)` iki
  kez tanımlı (`{reg}_raw` ham sözcük + `{reg}_{alan}` alan getter'ı),
  `rustc` "duplicate definitions" hatası. Aynı sınıf ("Volt tamam diyor,
  çıktı derlenmiyor") ama anahtar sözcük değil ad çakışması; `set_`/`trigger_`/`clear_` önekleri de
  benzer çakışmalar üretebilir. Ayrı ADR.
- Verilator 5.050 `--lint-only`'de de "Verilation Report" özeti basıyor;
  "çıktı boş" ölçütüyle lint yapan betikler yanlış alarm verir, ölçüt `%`
  ile başlayan satır olmalı.
- Çift alt çizgili Volt adı (`a__b`) Verilator'da `___05F` kodlamasına
  uğrar; `volt test` (port erişimi) ölçüldü, sorunsuz. `load()`'un
  `rootp->...__DOT__<reg>` yolu çift alt çizgili register'la ölçülmedi.
