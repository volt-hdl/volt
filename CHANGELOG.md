# Değişiklik Günlüğü

Biçim [Keep a Changelog](https://keepachangelog.com/tr/) esaslıdır;
sürümleme [SemVer](https://semver.org/lang/tr/) izler.

## [Yayımlanmadı]

### Düzeltildi — Parser bellek taşması: özyineli bundle sonsuz açılım (2026-09-23, ADR-0067)

- Fuzz'ın bulduğu ilk hata (PR #19 gecelik/PR fuzz'ı, tohum `tests/ui/`):
  kendini içeren bir `struct port` (`struct port Req { in req : Req }`)
  sessizce 8 seviye açılıyor, k kendine dönen alan k⁹ port üretiyordu —
  2087 baytlık girdi 60 s'de bitmiyor, 10,5 GB bellek; CI'da libFuzzer
  OOM. Aynı sınıf `Handshake<T>` sade struct payload'ında da vardı (k⁸).
- **E4009** (yeni): kendini içeren `struct port` ya da Handshake payload'ı;
  döngüdeki her struct için bir tanı (birincil: struct adı, ikincil:
  döngüyü kapatan alan), döngüye ulaşan tanımlar düzleştirilmez. Eskiden
  bu girdi HATASIZ derleniyor ve `req_req_req_addr` gibi portlar üretiyordu.
- **E4010** (yeni): düzleştirme bütçesi — modül başına en çok 4096 düz
  port (`[Bundle; 256]` × 16 alan) ve en çok 8 seviye iç içelik; aşım
  açılım SIRASINDA durdurulur (sonradan saymak patlamayı önlemez).
  `MAX_NESTING` aşımı eskiden sessiz kesmeydi.
- `tests/fuzz_regressions/` (YENİ): her fuzz bulgusu ham haliyle; her
  dosya 5 s süre sınırıyla ayrıştırılır (`fuzz_regression_tests`). Dizin
  PR ve gecelik fuzz işlerinde salt okunur tohumdur.
- `volt explain E4009/E4010` iki dilde; `tests/ui/fail/72-73`.

### Eklendi — RDC denetimi, ham reset portu senkronizörü ve hedefli SDC (2026-09-22, ADR-0065)

- **E3003** artık üretilir: aynı `rst`/`rst_n` portunu paylaşan iki
  `reset = async` alanı, aynı ham reset'in bir saatte iki kez
  senkronlanması (yakınsama) ve beslediği alanla uyuşmayan ham port.
  **W3009**: kökteki otomatik asenkron reset portunun bırakılışı birimin
  dışında senkronlanmış sayılır (sözleşme görünür kılınır). **W3010**:
  senkron reset'in birden çok saat alanınca paylaşılması (uyarı; örnekler
  taşınınca hataya yükseltme yeniden değerlendirilecek). `volt explain`
  iki dilde; parser `reset = <ifade>` yazımını E0001 ile reddeder (eskiden
  sessizce yok sayılıyordu).
- Ham reset portu `in rst_n : reset(async, active_low)`: derleyici
  portun beslediği her saat için iki aşamalı bırakma senkronizörü
  (`rst_sync_<saat>_stage0/1`, asenkron assert / senkron bırakma) üretir
  ve alanı kendi zincirinden sıfırlar. Testbench ve `volt verify`
  zincir çıkışını kullanır; ham portsuz tasarımların çıktısı bayt bayt
  aynıdır.
- `--emit=sdc,xdc` varsayılan stili **`--sdc-style=targeted`**:
  `set_clock_groups -asynchronous` YAZILMAZ. Yalnız tanınan
  senkronizörlerin ilk aşamasına giden yol kısıtlanır (`sync()`,
  HandshakeSync, PulseSync, bellek verisi: `set_false_path`; AsyncFifo
  gray işaretçileri: `.sdc` `set_max_delay -ignore_clock_latency T_src`,
  `.xdc` `set_max_delay -datapath_only` + `set_bus_skew` + `ASYNC_REG`);
  ham reset'in yalnız assert yolu `set_false_path -from [get_ports …]`
  alır, zincir çıkışı → temizleme pinleri recovery/removal olarak
  zamanlanır. Derleyicinin kaçırdığı alanlar arası yol zamanlama aracında
  ihlal olarak görünür. `--sdc-style=clock-groups` ADR-0054 çıktısını
  korur. Üretilen her `get_cells` deseni aynı derlemenin SV'sindeki bir
  register'a eşleşir (test).
- CI'da kalıcı **`timing`** işi (`scripts/sta/run.py`, Yosys + OpenSTA
  3.1.0, özet sabitli imaj, önbellekli): `tests/ui/pass/87`'nin
  `build/` kopyasına elle eklenen senkronizörsüz geçiş ve yanlış saatin
  reset'i eski stilde `No paths found.`, hedefli stilde `VIOLATED` /
  zamanlanmış recovery ucu; temiz tasarımda tns 0,00; 22/22 desen Yosys
  netlistinde bir hücreye eşleşir. `examples/hybrid_accel` (AsyncFifo,
  400/200 MHz) hedefli stille alanlar arası ihlal vermez.
- README "No RDC checking" sınırı gerçek kapsama göre yeniden yazıldı.

### Eklendi — Kontratlar simülasyonda izleyici olarak koşar (2026-09-21, ADR-0064; PR #11, merge bekliyor)

- `volt test` kontratları (`invariant`, `assume`, `cover`, otomatik
  primitif kontratları) VARSAYILAN AÇIK izler: `volt verify`'ın immediate
  kalıbı + DPI geri çağrısı (`SvaMode::Simulation`); ihlal Volt kaynağına
  eşlenmiş konumla raporlanır, süreç `$stop` ile ölmez, `assume` ihlali
  testi düşürür. `--no-contracts` kapatır (SV ve testbench eski çıktının
  aynısı). `volt run` varsayılan kapalı, `--contracts` açar; kontrat
  ihlali çıkış kodu 5.
- Reset ve `prev()` anlamı formal ile aynı (yardımcı register zinciri;
  `$past`'in reset içi örneklemesi kullanılmaz).
- **W5001**: SV'ye inemeyen kontrat simülasyonda atlanır (formal'de E0003
  kalır). `volt explain W5001` iki dilde.
- Kalıcı test: formal derinliğinin ötesinde (binlerce çevrim) yakalanan
  ihlal. `volt verify` ve `volt build` çıktıları bayt bayt aynı.

### Eklendi — Register haritası tutarlılık denetimi (2026-09-21, ADR-0063)

- `volt check-regmap <tasarım> --against <dosya>...`: üretilmiş `.h`
  (`--emit=c`), `.rs` (`--emit=rust`) ve `.json` (`--emit=regmap`)
  dosyaları RTL'deki `@mmio` haritasıyla karşılaştırılır (taban adres;
  register başına offset, erişim, bit maskesi, reset değeri; alan başına
  lsb, maske, `W1C`/self-clearing davranışı — yorum ve sıra farkı ayrışma
  değildir). Ayrışma **E9003** (`= note:`
  satırlarında dosya/RTL değerleri, `= help: regenerate with volt build
  --emit=c gpio.volt`); Volt'un üretmediği ya da imzasız dosya **E9004**.
- `volt build --check-regmap`: her `@mmio` modülü için sürücü çıktıları
  bellekte yeniden üretilip SV ile karşılaştırılır (derleyici iç
  tutarlılığı).
- Üretilen sürücü dosyaları imza taşır (`regmap-hash`); hash eşleşince
  hızlı yol, `--format json` zarfına `regmap_check` nesnesi eklenir.

### Düzeltildi — Test bloğunda porta maskesiz yazım: SESSİZ YANLIŞ SİMÜLASYON (2026-09-20, ADR-0059)

- Verilator giriş portlarını maskelemez: `addr : u3` portuna 8 yazınca
  model donanımda imkânsız bir durumu yürütüyordu (`out echo : u3` 8
  okuyor, `tbl[addr]` yanlış eleman veriyor, `addr > 5` doğru çıkıyordu)
  ve hiçbir uyarı yoktu.
- Sabit değer artık derlenmez: **E8512** `value does not fit in port
  width` (değer + port tipi + geçerli aralık). Sabit ifadeler katlanır
  (`4 + 4`, `1 << 3`).
- Hesaplanmış değer (`for i in 0..16 { dut.addr = i }`) testbench'te
  denetlenir; sığmayan değer porta ULAŞMAZ, test düşer:
  `port 'addr' (u3) cannot hold value 8 at t_test.volt:14` +
  `range: 0..7` + `loop:  i = 8`. Maskeleme bilerek yapılmaz.
- İşaretli port bit desenini (`i8`: 0..255) ve `0 - n` ile yazılmış
  negatif sayıyı (−128'e kadar) kabul eder; negatif sayı porta kayıpsız
  deseniyle yazılır (eskiden `sint<12>` porta `0 - 1` kirli bit bırakırdı).
- `assert_eq`/`assert_ne(dut.out, <sabit>)`: sabit portun okuyabileceği
  aralığı aşıyorsa E8512 — `assert_ne` sessizce hep geçiyordu.
- Genişlik AST port tipinden çözülür (`load` ile ortak yol); çözülemeyen
  (takma ad, generic) portta denetim C++ depolama tipine düşer.
- Mevcut 143 simülasyon testi (59 RISC-V dahil) değişmeden geçti.

### Eklendi — Test dili: yerel değişken, dizi, `for`, `read_hex`, `load` (2026-09-20, ADR-0058)

- `let n = 4;`, `let expected = [0x63, 0x7c];`, `expected[i]`, `len(x)` ve
  tam ifade kümesi (`* / % + - << >> & ^ |`, karşılaştırma, `&& || !`);
  öncelik donanım ifadeleriyle aynı. Değerler 64 bit sayı ya da sayı dizisi.
- `for i in 0..16 { ... }` — üretilen C++'ta ÇALIŞMA ZAMANI döngüsü
  (açılım değil): gövde bir kez üretilir, sınır port/`len` olabilir,
  düşen assert sayaçları raporlar (`loop:  i = 2, j = 1`).
- `read_hex("dosya.hex")` — `$readmemh` metni derleme zamanında okunur;
  yol test dosyasına göre görelidir ve projeden çıkamaz (E8507), biçim
  hatası satır numarasıyla E8508.
- `load(dut.mem, veri)` / `load(dut.cpu.mem, veri)` — tasarımın dizi
  yazmacına doğrudan yazar. Üretilen SV değişmez: hedefler bir `.vlt`
  dosyasıyla tek tek `public_flat_rw` yapılır. Yüklenen bellek
  `reset()`'ten sağ çıkar. E8509 (hedef bellek değil), E8510 (sığmıyor).
- İndeks taşması, sıfıra bölme ve sığmayan `load` testi çökertmez,
  konumuyla düşürür. Yeni kodlar E8507–E8511, `volt explain` iki dilde.
- ADR-0058 özelliği kullanmayan testlerin testbench'i bayt bayt aynıdır.
- `examples/riscv_sw/`: `hex2volt.py` ve `hello_rom.volt` silindi;
  `HelloSoc` programı `read_hex` + `load` ile alır, `hello.hex` 32 bit
  kelime dökümü oldu. 59/59 test, 913 çevrim / 847 komut aynı.

### Düzeltildi — SV üretiminde operatör önceliği: SESSİZ YANLIŞ DERLEME (2026-09-20, ADR-0057)

- **Belirti**: `(a & b) == 0` → `a & b == 8'd0` üretiliyordu; SV bunu
  `a & (b == 0)` okur. Parantez kararı Volt öncelik tablosuyla
  veriliyordu, oysa `&` `^` `|` Volt'ta karşılaştırmadan SIKI (ADR-0013
  §2.2), SystemVerilog'da GEVŞEK bağlanır. Tanı ya da lint uyarısı yoktu.
- **Kapsam**: 3 bit düzeyi × 6 karşılaştırma = 18 operatör çifti, bit
  düzeyi işlem karşılaştırmanın operandı olduğunda (sol ya da sağ). RTL,
  `if` koşulları ve **SVA kontratları** aynı üreticiyi kullandığından hepsi
  etkileniyordu; kontrat içinde bu kalıp boş-doğru kanıta yol açabilir.
- **Çözüm**: `sv_prec` IEEE 1800-2017 Tablo 11-2'ye geçti; ayrışan
  düzeyde parantez iki yönde basılır (`a & (b == 0)` de parantezli kalır).
  Diğer çıktılar değişmez.
- **Depo etkisi**: 156 üretilmiş SV dosyası ve 476 property metni
  karşılaştırıldı — yalnız `tests/ui/pass/04_operator_precedence.volt`
  çıktısı değişti; `examples/` ve `counter.expected.sv` aynı, boş-doğru
  property bulunmadı.
- **YAPMANIZ GEREKEN**: kendi tasarımınızda `&` `|` `^` ile `==` `!=` `<`
  `>` `<=` `>=` operatörlerini aynı ifadede kullandıysanız yeniden
  derleyin ve `volt verify` sonuçlarını yeniden üretin.
- Yeni: `crates/volt-sv-emit/tests/precedence_tests.rs` (24 test) —
  19 × 19 operatör çifti × 2 ağaç biçimi, bağımsız bir IEEE öncelik
  ayrıştırıcısıyla gidiş-dönüş sınanır.

### Eklendi — SDC/XDC üretimi: `@timing` uygulanıyor, zamanlama kısıtları domain bilgisinden (2026-09-16, ADR-0054)

- **`volt build --emit=sdc,xdc`**: saat portu olan her modül için
  `build/constraints/<Modül>.sdc` (Synopsys/Quartus/OpenSTA) ve `.xdc`
  (Vivado). Hiçbir nitelik gerekmeden: domain `frequency`'den
  `create_clock` (100 MHz → `-period 10.000`, 25_175.khz → `39.722`),
  farklı alanlar arasında `set_clock_groups -asynchronous`, üretilen her
  CDC köprüsü (`sync()`/`sync3()`, `AsyncFifo`, `HandshakeSync`,
  `PulseSync`, `AsyncDualPortRam`) için `set_false_path`; XDC'de ek
  olarak senkronizatör zincirlerine `ASYNC_REG`. Alt modül örnekleri
  `örnek/` önekiyle üst dosyaya düzleştirilir.
- **Durum tespiti**: `DomainKey::Frequency` gramerde ve ayrışıyordu ama
  hiçbir geçit okumuyordu; `@timing` argümanları (`clk >= 100.mhz`,
  `max_delay(a, b) <= 5.ns`, `from = a, to = b`, `cycles = 3`) bugünkü
  ifade ayrıştırıcısıyla zaten hatasız ayrışıyor — yeni sözdizimi yok.
- **`@timing` / `@false_path` / `@multicycle` UYGULANIYOR**: yeni
  `volt-hir/src/constraints/` (model + biçimler) ve `volt-sdc-emit`
  crate'i (metin). Desteklenen biçimler: `@timing(clk = F)`, `@timing(clk
  >= F)` (alan frekansıyla tutarlılık denetimi), `max_delay(a, b) <= T`,
  `min_delay(a, b) >= T`, `@false_path(from, to)` (modül/port/`reg`),
  `@multicycle(from, to, cycles = N)` ve `reg` üstünde `@multicycle(N)`.
  Frekans `Hz`/`.khz`/`.mhz`/`.ghz`, süre `.ps`/`.ns`/`.us` (birim
  zorunlu). Ondalık literal yok: 25.175 MHz `25_175.khz` yazılır.
- **E0017** (yeni): desteklenmeyen ya da tutarsız kısıt — bilinmeyen
  sinyal, birimsiz süre, `let` uç noktası, `clk <= F`, alan frekansıyla
  çelişen `@timing`; `volt check`, LSP ve `analyze`'de her zaman.
- **W0022** (yeni): alanın `frequency`'si yok, `create_clock` üretilmedi —
  alan başına bir kez ve YALNIZ `--emit=sdc,xdc` istendiğinde.
- **W0021 güncellendi**: `@timing`, `@false_path`, `@multicycle` listeden
  çıktı; uyarı artık hâlâ uygulanmayanları (`@domain`, `@budget`,
  `@version`, `@abi_version`, `@dft`, `@debug_visible`, `@debug_trace`,
  `@synthesis_target`) ikinci bir notta listeler. `tests/ui/pass/63-64`
  örneği `@budget`'a geçti; `volt explain W0021` yenilendi.
- `examples/vga`: `SysDomain frequency = 100.mhz`, `PixDomain frequency =
  25_175.khz`, `VgaTiming` `@timing(pix_clk >= 25_175.khz)`; `volt check`
  artık `0 error(s), 1 warning(s)` (yalnız W3006). Üretilen `VgaTop.sdc`
  iki `create_clock` + `set_clock_groups` + dört CDC `set_false_path`.
- Doğrulama: `volt_sdc_emit::syntax_check` (araçsız sözdizimi kapısı,
  CI); OpenSTA 3.1 (Docker `openroad/opensta`) `read_sdc` ile yerel
  doğrulama — Yosys sentezi + `rename -wire -suffix _reg` ile hücre
  adları eşleşir (ayrıntı ADR-0054 §7).
- `docs/spec/cli-contract.md` §4/§5 `--emit=sdc,xdc` (ADR kaynaklı);
  `tests/ui/pass/73-74`, `fail/57`; `constraints_tests.rs` (39),
  `render_tests.rs` (15), `sdc_emit_tests.rs` (10); 120 kod.

### Eklendi — HW-SW köprüsü: `@mmio`'dan sürücü, başlık, regmap.json ve belge (2026-09-15, ADR-0053)

- **`volt build --emit=rust,c,regmap,regmap-md`**: birimdeki her `@mmio`
  modülü için `build/sw/<modül>.rs` (`no_std` Rust sürücüsü),
  `build/sw/<modül>.h` (C başlığı), `build/sw/<modül>.json`
  (`volt-regmap/1`) ve `build/docs/<modül>.md` (register haritası
  tabloları). Register haritası artık tek yerde yaşar: RTL, sürücü ve
  belge aynı `RegInfo` listesinden üretilir.
- **Durum tespiti**: `@mmio` bilgisi HIR'da DURMUYORDU — ADR-0044 silme
  ilkesiyle parser'da tüketiliyordu. Yeni `volt_ast::mmio::RegMap`
  (saf veri: base, bus, register offset/access/volatile, alan lsb/
  genişlik/tip/`@reserved`/`@self_clearing`/`@w1c`, üç seviyede doc)
  parser'da RTL ile aynı anda kurulur, `ParseResult.regmaps` ile
  sürücüye döner. Alan doc yorumu (`/// açıklama` bir alanın üstünde)
  eklendi (`MmioFieldDecl.doc`).
- **Erişim hakkı derleme zamanında**: ReadOnly register'a setter,
  WriteOnly register'a getter üretilmez (Rust'ta "method not found",
  C'de tanımsız fonksiyon). `@reserved` bitler `*_MASK` ile maskelenir;
  `@self_clearing` → `trigger_*`, `@w1c` → `clear_*`; okuma-değiştirme-
  yazma darbe bitlerini korumaz (yeniden tetikleme yok).
- **Tutarlılık testi**: `regmap.json`'daki her adres üretilen SV'nin
  `mmio_whit`/`mmio_rhit` kümesi, `case (ar_addr)`/`case (aw_addr)`
  kolları ve okuma maskeleriyle karşılaştırılır; denetimin kendisi de
  kaydırılmış bir adresle sınanır. Üretilen Rust `cargo check`
  (`#![no_std]`, `-D warnings`, clippy pedantic temiz), C başlığı `gcc
  -std=c99 -Wall -Wextra -Werror -pedantic -fsyntax-only` + C11 + `g++
  -std=c++17` ile doğrulandı.
- Yeni crate `volt-sw-emit` (rust/c/json/markdown üreticileri, saf
  `RegMap → String`); `cli-contract.md` §4/§5 (ADR kaynaklı);
  `tests/ui/pass/72_mmio_driver_generation.volt`; testler +50.

### Eklendi — güven seviyeleri: `trust_level`, E3009, `declassify` (2026-09-15, ADR-0052)

- **Domain'in dördüncü boyutu uygulandı**: `domain D { trust_level =
  secret | confidential | public }` ayrıştırılır (önce W0020 + E0003
  veriyordu); üç kelime ayrılmış listeden çıkıp bağlamsal oldu
  (ADR-0023 kalıbı). Kafes `public < confidential < secret`; bilgi yalnız
  eşit ya da yüksek seviyeye akar, yüksekten düşüğe akış **E3009**
  (ilk kez üretiliyor — kod 2024'ten beri tabloda "[V1]" olarak
  bekliyordu).
- **Akış analizi saat çıkarımının üstünde** (`volt-hir/trust.rs`):
  sinyalin seviyesi alanının seviyesi (K1/K2/K4), ifade en yükseği taşır
  (K5), atama + `if`/`match` koşulu örtük akış (K6/K7), örnekleme (K8),
  `sync()` etiketi korur (K9). Alanı trust_level'sız sinyaller
  sınıflandırılmamıştır ve yazılan en yüksek seviyeyi alır (sabit nokta):
  anotasyonsuz register / alt modül sır aklayamaz. trust_level yoksa geçit
  hiç koşmaz — mevcut tasarımlar değişmedi.
- **K11 — trust'lı anotasyon saat alanı açmaz**: `@Debug` gibi bir
  anotasyon, modülde clock portu taşımıyorsa sinyali modülün saatinde
  bırakır, yalnız sınıflandırır (tek saat + iki güven bölgesi artık
  E3001 değil). Çoklu saatte E3010; clock portu taşıyorsa eski davranış.
- **`declassify(expr, "gerekçe")`**: tek meşru düşürme, sonuç public;
  gerekçe zorunlu (**E0016**, yeni), her çağrı **W3008** (yeni) iz
  kaydı — güvenlik incelemesi derleyici çıktısını okumaya iner. Parser'da
  soyulur (`delay<K>` gibi), SV üretimi değişmez. Otomatik "sızıntı yok"
  kontratı ÜRETİLMEZ (iki-izli özellik, ADR-0052 §5).
- **E3009 beş parça**: iki trust_level satırı, hedef `@Debug (public)`,
  kaynak tanığı `@SecureCore (secret)`, neden, `declassify` önerisi.
  `volt explain` E0016/E3009/W3008 iki dilde; 118 kod.
- `examples/crypto/key_store.volt` (YENİ): AES anahtar kaydı + yükleme
  FSM'i, iki domain tek saat, üç `declassify`; kasıtlı `debug_out =
  key_r[7:0]` E3009. 65 satır SV, Verilator `-Wall` temiz, `bmc 12` /
  `prove 3` / `cover 12` 6/6.
- `tests/ui/pass/70_trust_levels`, `71_declassify`, `fail/55_trust_leak`
  (E3009), `fail/56_declassify_no_reason` (E0016); +57 test.

### Eklendi — çift yönlü portlar: `inout` yazma + `opendrain` tipi (2026-09-15, ADR-0051)

- **`inout` artık yazılabiliyor, `opendrain` yeni port yönü**: sürücü
  niyeti modülün kendi register'larıdır — `p.drive(value)` (inout),
  `p.drive_low()` (opendrain), `p.release()` yalnız `on` bloğunda;
  `p.read()` hattın seviyesi; `p.released` / `p.driving` kontrat ve
  ifadelerde. Parser `<p>_oe`/`<p>_out` (`<p>_drive_low`) register'larını
  sentezler; aşağı akış aşamaları yalnız sıradan port + register görür
  (Seçenek B+: yöntem çağrıları — koşullu `released` literaline (C) ve
  elle `oe` sinyaline (A) tercih edildi, ADR'de karşılaştırma tablosu).
- **SV**: `inout wire` port (IEEE 1800 23.2.2.3), tek üç durumlu tampon
  `assign p = enable ? value : 'z` — sv-mapping §11 "z üretilmez" kuralına
  tek istisna (§17). Örnek bağlaması adla; üst `wire` `inout` için `wire`,
  `opendrain` için `tri1` (pull-up + kablolu-VE). Formal çıktıda dış aygıt
  `(* anyseq *)` ile modellenir (Yosys serbest `'z`yi 0 okur).
- **E4008**: çift yönlü porta doğrudan atama, `on` dışında sürme,
  bilinmeyen üye, tip kuralı. **W3007**: `inout`/`opendrain` okuması
  harici sayılır — `sync()` kaynağı ya da kontrat değilse uyarı
  (clock stretching için gerekli). `volt explain` iki dilde.
- `examples/i2c/`: 69 satırlık elle yazılmış `i2c_top.sv` sarmalayıcı
  KALKTI; master `opendrain sda/scl` + `sync()`, köle modeli aynı,
  tezgâh iki `wire` (`tri1`). 12/12 sim (Docker Verilator), 13 özellik
  `bmc 12` / `prove 3` / `cover 190` (7/7), Verilator `-Wall` temiz.
  Sistem saati 3.2 MHz (32/8 clk-bit): senkronizatör gecikmesi hızlı
  modda bit başına +1 çevrim.
- `tests/ui/pass/68_inout_bidirectional`, `69_opendrain_basic`,
  `fail/53_opendrain_direct_assign` (E4008), `fail/54_inout_unsynchronized`
  (W3007); +53 test. Yan düzeltme: `0 as bits<8>` literal cast'i emitter'da
  E2005 vermiyor artık (hedef genişliğinde literal).

### Eklendi — `Handshake<T>` yerleşik el sıkışma bundle'ı (2026-09-14, ADR-0050)

- **Tek saatli valid/ready artık bir port tipi**: `out tx : Handshake<u8>`
  (üretici) / `in rx : Handshake<u8>` (tüketici, yönler terslenir);
  ADR-0039 düzleştirmesiyle `tx_data`/`tx_valid`/`tx_ready` düz
  portları, sade struct payload alan alan (`aw_data_addr`). Sanal
  alanlar `tx.fired` (`valid && ready`) ve `tx.stalled`
  (`valid && !ready`) ifadeye yazılır — Seçenek B (bundle üzerinde alan),
  yerleşik fonksiyona (Seçenek A) tercih edildi: sıfır yeni kavram.
- **Protokol kontratları otomatik**: her Handshake portu için "valid,
  ready gelene dek düşmez" ve "veri el sıkışma tamamlanana dek sabit"
  (düz veri alanı başına) — üreticide `invariant`, tüketicide `assume`.
  `@no_protocol_check` (port ya da modül) kapatır.
- **E4007**: üretici tarafta `valid`, `ready`'ye kombinasyonel bağımlı
  olamaz (kilitlenme); sürekli atama/`let`/`comb` yolu izlenir, `on`
  bloğu keser. `volt explain E4007` iki dilde.
- `examples/axi4lite_slave.volt` 163 → 131 satır: beş `struct port` (34
  satır) → dört payload struct (4 satır), beş elle yazılmış tutma
  kuralı → 14 otomatik kural; 25 özellik `prove 3 --engine boolector` /
  `bmc 12` / `cover 12` ile kanıtlı, 5/5 sim testi, Verilator `-Wall`
  temiz.
- SoC: AXI kanalları Handshake'e geçti; `SocTop`'ın otomatik `b`/`r`
  kontratları `BusDecoder`'da gerçek bir hata buldu (yanıt teslim
  edilirken başka periferiğe istek kabul edilip yanıt muxu sahibini
  değiştiriyordu) — `wr_pending`/`rd_pending` kapısıyla düzeltildi.
  `AxiToReg`'in 7 protokol satırı 3'e indi. Gpio (`@mmio`) düz AXI
  adlarını korur (ayrı ADR).
- `tests/ui/pass/66_handshake_basic`, `67_handshake_contracts`,
  `fail/52_handshake_protocol_violation` (E4007); docs/stdlib.md
  "Handshake" bölümü + "Handshake mı, HandshakeSync mı?" tablosu.

### Eklendi — domain-aware bellek: `AsyncDualPortRam<T, DEPTH>` (2026-09-14, ADR-0049)

- **"Bir alanda yaz, ötekinde oku" belleği artık tek primitif**: yazma
  portu `wr_clk`/`wr_addr`/`wr_data`/`wr_en` (@Src), okuma portu
  `rd_clk`/`rd_addr`/`rd_data` (@Dst). ADR-0047 sembolik alanları K8
  yerleşik yolundan geçer: yanlış alandan bağlanan port **E3001**,
  `rd_data` okuma saatinin alanını taşır. Mevcut `DualPortRam`
  değişmedi.
- **Senkronizatör yok, bellek dizisi CDC sınırı**: üretilen SV resetsiz
  yazma `always_ff` + okuma register'ı; Yosys `synth_xilinx` tek
  RAMB18E1 (FDRE yok). Adresler kendi alanlarında kalır — gray kod/FIFO
  kararı kullanıcıdan alındı.
- **W3006 çift saatli biçim**: diğer saatten yazılmakta olan adresin
  okunması TANIMSIZ; her örneklemede uyarılır, başlık ve `volt explain
  W3006` iki biçimi anlatır. **W3003** artık dört alternatif önerir
  (`AsyncDualPortRam<T, N> — random-access data`).
- Kontratlar: `wr_addr < DEPTH` (`wr_clk`), `rd_addr < DEPTH` (`rd_clk`),
  `cover: wr_en`.
- `examples/vga/frame_buffer.volt` 99 → 52 satır (elle yazılan
  AsyncFifo + pop + çözme yolu kalktı), üretilen SV 135 → 49 satır,
  `bmc 24` 212 s → 6 s, 7/7 simülasyon testi, Verilator `-Wall` temiz.
- `tests/ui/pass/65_async_dual_port_ram`, `fail/51_async_ram_domain_violation`
  (E3001); docs/stdlib.md "DualPortRam mı, AsyncDualPortRam mı?" tablosu;
  +33 test.

### Eklendi — uygulanmayan nitelikler artık uyarıyor: W0021 (2026-09-14, ADR-0048)

- **"Sessizce yok sayma" ihlali kapandı**: `@timing`, `@budget`,
  `@false_path`, `@multicycle`, `@version`, `@abi_version`, `@dft`,
  `@debug_visible`, `@debug_trace`, `@synthesis_target` ve `@domain`
  gramerde olduğu için W0020 üretmiyor ama hiçbir geçit okumuyordu
  (SDC yok, bütçe denetimi yok). Kullanıcı kısıt yazdığını sanıyordu.
  Artık her kullanım **W0021** üretir: `= reason:` nitelik ailesine
  göre neyin eksik olduğunu, `= help:` bu arada ne yapılacağını söyler.
- **Susturma**: aynı öğede `@allow(unenforced)` (öğenin portları ve
  gövdesi dâhil) ya da Volt.toml `[lint] unenforced_attributes =
  "allow"` (paket geneli). Hatalı `@allow` argümanı **E0009** — yanlış
  yazılmış susturma sessiz kalmaz.
- W0021 kodu daha önce hiç üretilmeyen "kullanılmayan doc yorumu"
  rezervasyonuydu; ADR-0048 ile yeniden tanımlandı (spec §18 satırı
  ADR tarafından geçersiz kılınır). `volt explain W0021` iki dilde.
- Yeni geçit `volt-hir/src/attrs.rs`; sürücü (Volt.toml politikası),
  `volt_hir::analyze` ve LSP aynı denetimi koşar.
- `examples/vga/vga_timing.volt` `@timing`i korur ve bilerek W0021
  üretir (şeffaflık); `tests/ui/pass/63_unenforced_attribute_warns`,
  `64_allow_unenforced_silences`; +35 test.
- Uzun vade (yalnız belge): ADR-0048 `@timing → create_clock /
  set_max_delay`, `@false_path → set_false_path`, `@multicycle →
  set_multicycle_path` eşlemelerini ve `build/constraints/<Top>.sdc`
  üretimini V1 planı olarak yazar.

### Eklendi — extern modül sınırında domain anotasyonu (2026-09-14, ADR-0047)

- **CDC güvenlik açığı kapandı**: `extern module` portları isim
  çözümlemede bildirilmiyor, tiplenmiyordu; K8 haritası boş kalıyor,
  yanlış alandan bağlanan port SESSİZ geçiyordu. Artık extern portları
  tanım/tip alır, K8 sınırda uygulanır → yanlış bağlama **E3001**.
- **Sembolik saat alanı** (örtük, seçenek A): extern içinde tanımsız
  `@Src`/`@Dst` extern'e özel parametredir (`DefKind::DomainParam`);
  örneklemede saat bağlantısı gerçek alana bağlar, çıkış okumaları
  (`f.rd_data`) bağlanan alanı taşır. Gramer değişmedi.
- Yeni kod **E3014**: aynı sembolik/açık alana iki farklı alandan saat
  (sıradan modüller için de geçerli); `volt explain E3014` iki dilde.
  Extern biçimli **E3002** (sembolik alanın clock portu yok) ve
  **E3010** (çok saatli extern'de anotasyonsuz port).
- Extern portları W1001 üretmez; LSP hover "symbolic clock domain".
- `tests/ui/pass/62_extern_domains.volt`, `fail/49_extern_domain_violation`
  (E3001), `fail/50_extern_clock_conflict` (E3014); +36 test.
- Bilinen sınır (dokunulmadı): volt-sv-emit extern örneğini
  üretemiyor (E0003 "target module is not in this file").

### Eklendi — F5: çoklu dosya derleme ve import sistemi (2026-09-13, ADR-0042)

- **Dosya keşfi**: `volt build <dosya>` bağımlılıkları `use`
  bildirimlerinden bulur — `./a/b.volt`, sonra `<Volt.toml kökü>/<src>/a/b.volt`,
  `std::` yerleşik. `Volt.toml [package] name/src`.
- **`package a::b;`** dosya başına bir kez; `pub` olmayan öğe dışarıya
  kapalı (E1004 ilk kez üretiliyor). `use a::b::X`, `use a::{b::X, c::Y}`,
  `use a::b::*`, `use a::b::X as Z` çözülür. Yeni kod **E1011** (modül
  bulunamadı, aranan yollar listelenir); döngüsel import E1006; belirsiz
  import E1010. `volt explain E1011` iki dilde.
- **Derleme birimi**: tüm dosyalar tek arena'da (`parse_unit`), iki
  geçiş; bundle düzleştirme ve monomorfizasyon birim üzerinde bir kez.
- **ADR-0024 uygulandı**: modül başına `build/rtl/<Modül>.sv`
  (`// Module:` başlığı, modülün kendi kaynak adı); `--single-file` eski
  düzen. Verilator DECLFILENAME kalktı.
- **`examples/soc/`** altı dosyaya bölündü (`top/bus/axi/gpio/timer/uart`);
  `soc.volt` ve `flatten.sh` silindi, `UartTx`/`Axi4LiteSlave` referansla
  yeniden kullanılıyor. 8 kaynak → 8 SV, 0,02 s; Verilator `-Wall` temiz;
  5/5 simülasyon testi; 74 property `bmc 12`.
- `tests/ui/multifile/{basic,pubpriv,notfound,cyclic}`; UI harness
  çoklu dosya birimi analiz eder.

### Eklendi — F4b: SymbiYosys entegrasyonu ve `volt verify` (2026-09-07)

- **`volt verify` komutu** (`volt-driver/src/verify.rs`): derle →
  `build/formal/<modul>.sv` + `.sby` üret → `sby -f` koştur → sonucu
  yorumla. Bayraklar: `--depth N` (varsayılan 20), `--engine
  z3|boolector|yices`, `--mode bmc|prove|cover`, `--target-dir`,
  `--format`. Çıkış kodları (cli-contract.md §2): 0 doğrulandı,
  1 derleme hatası, 3 sby yok/araç hatası, 6 karşı örnek.
- **Yosys-uyumlu üretim** (`SvaMode::Immediate`): Yosys'in Verilog ön
  ucu adlandırılmış `property/endproperty` bloklarını ayrıştıramıyor
  (TOK_PROPERTY — hdlc/formal imajıyla doğrulandı); verify akışı bu
  yüzden `always @(edge) if (!rst) assert (ifade); // volt:<ad>`
  immediate kalıbını gömer. `--emit=sva` çıktısı (property blokları)
  ticari araçlar için değişmedi. BMC'nin kısıtsız başlangıç durumuna
  karşı `initial assume (rst);` varsayımı eklendi.
- **E5001 karşı örnek tanısı**: sby FAIL logundaki `dosya.sv:satır`
  konumu `// volt:<ad>` işaretiyle Volt kontratına geri eşlenir; tanı
  ihlal döngüsünü (`violated at cycle N`), kopyalanan
  `<modul>_cex.vcd` yolunu (`= counterexample:`) ve `gtkwave`/`surfer`
  önerisini taşır. Yeni `NoteKind::Counterexample` satırı iki dilde.
- **Kurulum yardımı**: sby bulunamayınca UX Anayasası biçiminde
  Linux/Docker/Windows kurulum seçenekleri basılır (çıkış 3);
  `VOLT_SBY` ortam değişkeni özel sby konumunu gösterebilir.
- **`volt explain verify-setup`**: konu bazlı açıklama altyapısı
  (`explain/topics.rs`) — kod olmayan girdiler önce konu tablosunda
  aranır; `verify-setup` iki dilde kurulum sayfası döndürür.
- **Fixture'lar**: `tests/ui/pass/23_provable_invariant.volt`
  (BoundedCounter, gerçek sby'de PASS) ve
  `tests/ui/fail/24_violated_invariant.volt` (LeakyCounter, gerçek
  sby'de 7. döngüde FAIL). Docker'da (hdlc/formal) uçtan uca
  doğrulandı: çıkış 0 / çıkış 6.
- **CI**: opsiyonel `verify` job'u (`continue-on-error: true`,
  YosysHQ/setup-oss-cad-suite) iki fixture'ı gerçek sby ile koşar;
  lokal testlerde sby yoksa gerçek-araç testleri SKIP eder, sahte sby
  betiğiyle FAIL/PASS yolları sby'siz de test edilir.
- Test: +36 (829 baseline) — 15+ sby'siz verify/explain CLI testi,
  sby log yorumlama birim testleri, `.sby` üretimi ve
  `SvaMode::Immediate` testleri.

### Eklendi — F2c-CLI: CDC kontrolü komut satırında (2026-09-03)

- **Aşamalı boru hattı** (`volt-driver/src/main.rs`): `check` ve `build`
  artık tam anlamsal analiz koşuyor — parse (E0xxx) → resolve (E1xxx) →
  const+typeck (E2xxx/E4xxx) → domain (E3xxx) → emit. Bir aşamada hata
  varsa sonrakine geçilmez (kaskad tanı önlemi). CDC ihlali komut
  satırında E3001 + `= çözüm: sync(...)` satırıyla görünüyor.
- **Hatalı tasarım SV üretmez**: `build` herhangi bir aşama hata
  verdiğinde `build/rtl/` altına dosya yazmaz, çıkış kodu 1.
- **`--format=human|json|short`** (cli-contract.md §3/§5): JSON zarfı
  (`version/command/success/diagnostics/summary/artifacts/duration_ms`)
  stdout'a; human/short tanıları stderr'e. JSON'da hatalar önce
  sıralanır — CI `diagnostics[0]`'da engelleyiciyi görür.
- **`check` emit koşmaz** (§6): sv-emit'in F0 sınırları (örn. `sync()`
  çağrısı E0003) anlamsal doğrulamayı engellemiyor —
  `13_cdc_correct_bridge` `check` ile temiz geçiyor.
- **Fixture düzeltmesi**: `04_undriven_output.volt` `//~^` anotasyonu
  kapanış parantezinden port bildirimine (satır 7) taşındı — tanının
  gösterdiği doğru konum.
- **Demo yenilendi** (`scripts/demo.ps1|.sh`): sayaç derlemesi → CDC
  ihlali E3001 ile reddediliyor → sync() köprüsü temiz geçiyor.
- Test: +10 CLI testi (15 toplam, 697 baseline); `volt check
  tests/ui/fail/01_cdc_violation.volt` terminalde E3001 gösteriyor.

### Eklendi — F2c: Domain çıkarımı ve CDC kontrolü (2026-09-03)

- **Domain gösterimi** (`volt-hir/src/domain.rs`, domain-inference.md §1):
  `DomainId` (Explicit/Timeless/Unresolved/Error), `DomainInfo`
  (isim + ClockSpec + ResetSpec + tanım span'i); açık `domain`
  bildirimleri ve anotasyonsuz clock portlarının örtük alanları tek
  tabloda. `Timeless` sabitler ve saf kombinasyonel için her alanla
  birleşir.
- **Tek saat kuralı** (K2, UX Anayasası): tek clock'lu modülde
  anotasyonsuz her sinyal o alana atanır — kullanıcı 'domain' kelimesini
  hiç görmez; sıfır clock → Timeless; çoklu clock → anotasyon zorunlu.
- **Çıkarım kuralları**: K1 açık anotasyon kazanır; K3 çoklu saatte
  anotasyonsuz sinyal E3010 (aday saatlere ikincil span'ler); K4
  register domain'i 'on' yazıcılarından (0 yazıcı → W3001+Timeless,
  çok alan → E3011); K5 ifade ağacında aşağıdan yukarı `join_domains`
  — kombinasyonel karışım E3001 (glitch); K6 atama uyumu E3001
  (Timeless muaf); K7 'on' bloğunda yabancı okuma E3012; K8 örnekleme
  saat bağlantısı haritasıyla port denetimi; K9 `sync()/sync3()`
  çıkışı hedef alanda — aynı alan W3002, çok bitli W3003.
- **E3002 iyileştirmesi** (`resolve.rs`): domain konumunda çözülemeyen
  isim artık E1001 değil "tanımsız saat alanı" E3002 üretir
  (Levenshtein önerili); clock tipinde olmayan anotasyon hedefi de
  E3002.
- **E3001/E3010/E3011 mesajları** 5 parçalı: iki span (kaynak + hedef)
  ve domain tanım satırlarına ikincil etiketler, `= neden:`
  (glitch/metastabilite), `= çözüm:` (sync() örneği).
- **Determinizm düzeltmesi** (`consteval.rs`): const değerlendirme
  sırası DefId'ye göre sabitlendi — E2020 döngü tanısının konumu
  koşudan koşuya değişmiyor.
- **UI harness sıkılaştırması** (`ui_semantic_tests.rs`,
  error-recovery.md §8.1): fixture'ların yalnız hata kodu değil
  `//~^ ERROR` satır numarası da doğrulanıyor.
- Test: +78 (687 baseline; domain testleri 71 + 7 ui); ui/fail 01→E3001,
  07→E3002, 13→E3010, 14→E3001, 15→E3011 doğru kod VE satırda; ui/pass
  13 sync() köprüsüyle, 14 domain sözcüğü geçmeden temiz; 21/21 korunuyor;
  kapsam %84,8 (domain %85,1).

### Eklendi — F2b: İkili operatörler ve genişleme kuralları (2026-09-03)

- **Aritmetik taşma genişlemesi** (`volt-hir/src/typeck.rs`,
  type-inference.md §3.3): `+`/`-` bir bit, `*` genişlik kadar genişler;
  `/`/`%` genişlemez; sonuç `MAX_WIDTH` ile sınırlı. Operand genişliği
  uyuşmazsa E2001, işaret karışırsa E2002, `bits<N>` aritmetiğinde E2004.
  Literal operand somut tarafa uyarlanır (taşmada E2010); literal+literal
  literal kalır; `Ty::Error` sessiz yayılır.
- **Esnek genişlik aralığı** (ADR-0025, `Ty::UIntFlex`/`SIntFlex`):
  aritmetik sonuç `[işlem, doğal]` genişlik aralığı taşır — `u8 + u8`
  kullanıcıya `u9` görünür ama sayaç deseni (`count <= count + 1`) taşma
  bitini atarak operand genişliğine uyar; aralık dışı hedef E2001.
  Operand uyumu aralık kesişimiyle kurulur; `as` ve `reg` çıkarımı doğal
  genişliğe sabitler.
- **Trit kuralları**: `Trit * Trit → Trit` (kapalı küme),
  `Trit ± Trit → i3` (taşma), `Trit * iN → iN` (ternary MAC, iki yönde);
  kalan kombinasyonlar E2003.
- **Bit düzeyi** (`&`, `|`, `^`): genişlemez; `bool&bool → bool`, aynı
  genişlik `uN/iN/bits<N>` korunur; genişlik farkı E2001, işaret karışımı
  E2002, uyumsuz tipler E2003.
- **Kaydırma** (`<<`, `>>`): sonuç sol operandın tipi; sağ operand
  sayısal değilse E2003; sabit miktar sol genişliği aşarsa yeni W2013
  uyarısı (kod ADR-0025 ile tanımlı, tutarlılık taraması artık
  `docs/adr/` da okuyor).
- **Karşılaştırma**: her zaman `bool`; operandlar `unify_for_comparison`
  ile aynı tipe birleştirilir, uyumsuzluk iki tipi de gösteren E2003.
- **Mantıksal** (`&&`, `||`): iki operand da `bool`, sonuç `bool`.
- **Koşullu ifade** (§3.7): sentez konumunda dallar birleştirilir;
  uyumsuz dallar iki tipi de gösteren E2003; esnek aralıklar kesişimle
  birleşir; literal dal somut dala uyarlanır.
- Test: +93 (566 toplam; ikili operatör testleri 85); ui/fail 02→E2001,
  08→E2002, 09→E2004 artık doğru kodu üretiyor; ui/pass 21/21 temiz.

### Eklendi — F2a: Tip sistemi temeli (2026-09-03)

- **Tip gösterimi** (`volt-hir/src/ty.rs`): `Ty` enum'u (Bool, UInt,
  SInt, Bits, Trit, Clock, Reset, Array, Tuple, Struct, Enum, Instance,
  IntLit, Error) + interning'li `TypeArena` (aynı tip → aynı `TypeId`).
- **Çift yönlü tip kontrolü** (`volt-hir/src/typeck.rs`,
  type-inference.md §2-§6): `synth`/`check` akışı; literal çözümleme
  (soneksiz → bağlamdan, taşmada E2010, Trit dışı E2011); tekli
  operatörler (`!` → bool, `~` genişliği korur, `-` → i(w+1), işaretsiz
  negasyon E2002); bit seçimi (E2006 sınır denetimi), aralık seçimi
  (E2007 ters aralık, E2008 değişken sınır); cast tablosu (daraltmada
  W2010, sayısal→Trit E2009); atanabilirlik (örtük daraltma VE genişleme
  E2001, işaret uyumsuzluğu E2002); `reg` tip belirsizliği E2012, `let`
  varsayılanı W2012.
- **Sürücü analizi** (`volt-hir/src/drivers.rs`, §11): `DriverTable`;
  E4001 çift sürücü (aynı on/comb bloğu içi koşullu atamalar muaf),
  E4002 sürücüsüz çıkış portu, W4001/W4002 sürülen-ama-okunmayan sinyal.
- **İsim çözümleme köprüleri** (`resolve.rs`): bildirim/kullanım span →
  DefId haritaları, tip konumu Path çözümleri, okuma kümesi dışa açıldı.
- F2a kapsam sınırı: ikili operatörler (aritmetik/bit/karşılaştırma/
  mantıksal) F2b'de — şimdilik hatasız `Ty::Error` döner.
- Test: +81 (516 toplam); ui/fail 03, 04, 10, 11, 12, 17, 20 artık
  doğru kodu üretiyor; kapsam %83,9 (typeck %82,9).

## [0.1.0] - 2026-09-03 — F0 tamamlandı

### Eklendi

- **Lexer** (`volt-syntax`): logos tabanlı; 53 aktif anahtar kelime,
  36 ayrılmış kelime (E0003), tüm operatörler, radix'li/sonekli sayısal
  literaller, iç içe blok yorum desteği, mutlak span'ler.
- **Parser** (`volt-syntax`): LL(2) iniş + Pratt ifade parser'ı
  (operator-precedence.md §3 binding power tablosu birebir); hata
  kurtarma (senkronizasyon kümeleri, ilerleme garantisi, 2-token kaskad
  bastırma); E0002/E0004/E0006/E0007/E0008/E0010 özel tanıları;
  libFuzzer hedefi (525k koşu, 0 panik).
- **AST** (`volt-ast`): arena tabanlı (`Arena<T>`/`Idx<T>`), her düğümde
  span, her enum'da hata kurtarma için `Error` varyantı.
- **Diagnostics** (`volt-diagnostics`): 86 hata/uyarı kodu (Display'li,
  `volt explain` temeli); 5 parça kuralı (`kod + konum + açıklama +
  çözüm + spec referansı`) yapısal olarak zorunlu; codespan-reporting
  ile insan çıktısı, cli-contract.md §5 şemasıyla birebir JSON çıktısı.
- **Span** (`volt-span`): `SourceMap`, bayt ve UTF-8 karakter sütunu
  (`line_col` / `line_col_utf8`).
- **SV emisyonu** (`volt-sv-emit`): sv-mapping.md uyumlu string template;
  otomatik reset portu ve reset bloğu (§7'nin 4 varyantı + none),
  always_ff/assign/wire üretimi, literal boyutlandırma (belirsizlikte
  E2005), `counter.volt → counter.expected.sv` birebir eşleşme.
- **CLI** (`volt-driver`): `volt build` / `volt check`; çıkış kodları
  0/1/2/3 (cli-contract.md §2); tanılar stderr'de, insan formatında.
- **Test altyapısı**: 215 test (63 lexer, 80 parser, 38 emisyon,
  21 tanı, 8 span, 5 CLI); cargo-fuzz hedefi + yerleşik fuzzer.

### Bilinen Sınırlar

- F0 kapsamı gramerin ~%30'u: `fn`, `struct`, `enum`, `match`, `for`,
  `comb`, `wire`, generics, kontratlar ve modül örnekleme F1+ (E0003).
- Tip çıkarımı kaba (ifade genişliği = en geniş operand); gerçek
  çıkarım ve taşma genişlemesi F2'de HIR ile gelecek.
- **CDC kontrolü henüz YOK** — domain anotasyonları ayrıştırılıyor ama
  doğrulanmıyor (F2).
- `sync()` CDC çağrısı `sync` anahtar kelimesiyle çakışıyor; bağlamsal
  anahtar kelime çözümü ADR-0023'te kararlaştırıldı, F1'de uygulanacak.
- `u9`/`u17` gibi ara genişlikler tip sözdiziminde yok (bits<N>
  kullanılmalı); `tests/ui/pass` altındaki bazı F1+ fixture'ları bu
  nedenle F0'da tanıyla reddediliyor.
