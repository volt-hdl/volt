# ADR-0107: Araç İsteyen Testler CI'da Atlanamaz — Araç Defteri, Tek Araçlı İş, Sıfır SKIP

> Statü: Uygulandı
> Önceki karar: ADR-0079 — §6'daki iş tablosu: her araçlı CI adımının çalıştıracağı test ikililerini ve `VOLT_REQUIRE_TOOLS` değerini elle seçmesi
> İlgili: ADR-0094 (Docker köprüsü: `docker_e2e_tests` kendi işinde, sıfır SKIP ile), ADR-0099 (testlerde araç arka ucu açık), ADR-0108 (fark testi araçlı işte, defterle), ADR-0109 (mutasyon testinin her parçası araçlı işin araçlarıyla, sıfır SKIP)
> Tarih: 2026-11-04
> Etkilenen: `crates/volt-driver/tests/tools/mod.rs` (defter, `opt_in`),
> `require_tools_tests.rs` (defter testleri, sondalar yalnız alt süreçte),
> `docker_e2e_tests.rs` (`tools::opt_in`), `scripts/ci/tool_ledger.py`
> (YENİ), `scripts/sta/sweep.py` (istisnalar gerekçeli ve görünür,
> `--expect-excluded`), `.github/workflows/ci.yml`, `AGENTS.md`,
> `CHANGELOG.md`

## Sorun

Verilator, sby, Yosys ya da C derleyicisi isteyen bir test, araç yoksa
`SKIP:` basıp `ok` ile geçer (ADR-0079 §6). Araçlar CI'da araçlı işlerde
kuruludur, ama bu işler testleri elle tutulan bir listeden çalıştırıyordu
(`cargo test --test a --test b ...`, `-- verilator cc_ ...` süzgeçleri).
Listeye girmeyen bir test CI'da hiç koşmaz, araçsız işte `ok` görünür:
#111'in iki testi (`compare_width_tests`) böyle birleşti ve #113'e kadar
hiçbir işte koşmadı. ADR-0079 §6'nın kendi taraması da aynı sınıftan üç
boşluk bulmuştu; liste her yeni testte yeniden unutulabilir.

## Karar

### 1. Araç defteri

Testlerdeki her araç sorgusu `tests/tools/mod.rs`'ten geçer ve
`VOLT_TOOL_LEDGER` bir dosya yolu ise oraya bir satır yazar:

```
<SKIP|RAN>  <test ikilisi>  <test adı>  <araç ya da değişken>  <zorunlu 0|1>
```

`RAN` aracın bulunduğunu ve `VOLT_REQUIRE_TOOLS` ile zorunlu olup
olmadığını söyler. Test adı libtest'in iş parçacığı adıdır; ikili adı
cargo'nun hash eki olmadan.

### 2. Değişkenle açılan ağır testler

Bir ortam değişkeniyle açılan test (`VOLT_DOCKER_E2E=1`) aynı yoldan
geçer: `tools::opt_in("VOLT_DOCKER_E2E")` açık değilse `SKIP`, açıksa
`RAN` (zorunlu) yazar. Testte araç başka yoldan aranmaz.

### 3. Tek araçlı iş, liste yok

"Verilator lint" işi OSS CAD Suite'i (Verilator, Yosys, sby, çözücüler)
kurar ve çalışma alanının bütün testlerini
`VOLT_REQUIRE_TOOLS=all cargo test --workspace` ile koşturur. Araç isteyen
yeni bir test hiçbir listeye eklenmeden bu işte, aracı zorunluyken koşar.
Tek istisna `docker_e2e_tests`'tir (`--skip
new_test_verify_and_run_through_docker`): imajları indirir ve yerel
araçları gizler; kendi işinde ("Docker backend") `VOLT_DOCKER_E2E=1` ile
koşar. "Formal verification" işi yalnız `volt verify` komut satırı
adımlarını tutar; sby testleri araçlı işe taşındı.

### 4. Her işin sonunda SKIP sayısı

Test koşan her iş defteri `scripts/ci/tool_ledger.py report` ile özetler:
`Tool ledger [<iş>]: N test(s) asked for a tool; ... SKIP: S` ve atlayan
her test adıyla. Araçların kurulu olduğu işlerde (`--require-zero-skips`)
S > 0 işi düşürür; `--min-ran` defterin testlere hiç ulaşmadığı (ya da
araçlı testlerin sessizce azaldığı) koşuyu düşürür.

### 5. Envanter denetimi ("Tool coverage" işi)

Araçsız iş araç isteyen her testi görür (orada atlar ya da koşucunun
derleyicisini bulur). `tool_ledger.py cover`, herhangi bir defterde görülen
her (ikili, test, araç) üçlüsünün bir araçlı işte aracı zorunluyken `RAN`
olduğunu denetler; hiç koşmamış ya da yalnız aracın zorunlu olmadığı bir
işte koşmuş test adıyla ve gerekçesiyle düşer. Bu, ileride araçlı işe bir
süzgeç ya da liste geri gelirse onu yakalayan kalıcı denetimdir.

### 6. SDC taramasının istisnaları

`scripts/sta/sweep.py` `//~ NET-SKIP` / `//~ SYNTH-SKIP` işaretli
fixture'ları OpenSTA taramasının dışında bırakır. İşaret gerekçesiz olamaz
(betik düşer); her istisna günlüğe gerekçesiyle ve GitHub notu olarak
basılır; CI `--expect-excluded 2` ile sayıyı sabitler, yeni bir işaret aynı
PR'de `ci.yml`'i değiştirmeden geçemez. Bugünkü iki istisna, gerekçeleri
fixture'daki işarette:

| Fixture | İşaret | Gerekçe |
|---|---|---|
| `tests/ui/pass/62_extern_domains.volt` | NET-SKIP | extern modüllerin `@source` gövdesi bilerek yok (fixture alan bağlamayı gösterir); tasarım dış SV olmadan eksik |
| `tests/ui/pass/69_opendrain_basic.volt` | SYNTH-SKIP | paylaşılan hat `tri1` çekme ağı (açık-akaç hattın kart üstü çekmesi); Yosys `tri1` ağ tipini ayrıştıramaz |

İstisna öbür yönde de sessiz değildir: `output_net_tests` işaretli
fixture'ları Verilator'a ve Yosys'e yine verir, temiz geçerlerse işaret
bayattır ve test düşer.

## Kanıt

PR #114, üç adım:

1. **Denetim, bugünkü main'in listeleriyle** (run 41733768024): araçsız
   iş `Tool ledger [test]: 65 test(s) asked for a tool; 7 lookup(s) found
   the tool (0 required), SKIP: 60`; araçlı işler `SKIP: 0`; "Tool
   coverage" `67 (test, tool) pair(s) in the ledgers, 67 ran with the tool
   required, 0 did not`. #113 listeyi tamamladığından bugünkü main'de eksik
   test yok; envanter 67 çifttir (ikili başına tablo PR'de).
2. **Denetim, #111 birleştiği andaki listeyle** (geçici commit, e7628a5a
   listesi; run 41733884962): "Verilator lint" işi YEŞİL kaldı (eski
   sorun: listede olmayan test hiçbir şey söylemez), "Tool coverage" düştü:
   `64 ran with the tool required, 3 did not` ve
   `compare_width_tests::shift_and_not_keep_their_own_width_in_simulation
   (verilator): never ran in a tool job`, aynısı
   `compare_width_tests::wrapping_and_keeping_the_carry_differ_for_b_7_a_0`
   ve `part_select_value_tests::part_select_of_a_sum_reads_the_carry_in_simulation`
   için.
3. **Yapısal düzeltme** (liste kalktı, `VOLT_REQUIRE_TOOLS=all cargo test
   --workspace`; run 41735791595): araçlı iş 3529 geçti, 0 düştü, 1 süzüldü
   (`docker_e2e_tests`), `Tool ledger [integration]: 64 test(s) asked for
   a tool; 66 lookup(s) found the tool (66 required), SKIP: 0`; Docker
   işi `1 lookup(s) found the tool (1 required), SKIP: 0`; "Tool coverage"
   `67 ran with the tool required, 0 did not`. Eski düzende 4 çift
   (`extern_source_tests` ve `template_tests`'in sby ve Verilator testleri)
   bir işte aracı zorunlu olmadan da koşuyordu; artık her çift tek işte
   zorunlu. SDC taraması `excluded (NET-SKIP / SYNTH-SKIP): 2` ve iki
   fixture'ı gerekçesiyle bastı.

Yerelde (Windows, araçsız) defter `require_tools_tests`'in sondalarının
normal koşuda da sorgu yaptığını gösterdi: araçlı işte `probe_opt_in`
SKIP sayılırdı. Sondalar artık yalnız alt süreçte (`VOLT_PROBE=1`) iş
yapar.

## Reddedilenler

- **Listeyi bir betikle üretmek** (`mod tools` içeren dosyaları bulup
  `--test` listesi kurmak): süzgeç ve zorunluluk boşluğu kalır; bir testin
  koştuğunu değil, adının listede olduğunu kanıtlar.
- **`--nocapture` çıktısında `SKIP:` aramak:** paralel testlerin çıktısı
  karışır ve alt süreçlerin basabileceği metinle çakışır; defter satırı
  yapısaldır.
- **Docker uçtan uca testini araçlı işte de koşturmak:** ~650 MB imaj ve
  "Docker backend" işinin yaptığı işin tekrarı.

## Sınırlar

- "Tool coverage" işi kuralda zorunlu durum denetimi değildir; dal
  kuralına eklenmesi depo sahibinin işidir.
- Bir testin aracı modül dışında araması (ör. `Command::new("verilator")`
  doğrudan) deftere girmez; AGENTS.md bunu yasaklar, mekanik denetim yok.

## Ek: Hakem sürümü (#145, 2026-11-06)

Araçlı testler doğru değeri Verilator simülasyonundan alır: davranış
testleri, fark testi (ADR-0108), mutasyon testi (ADR-0109), kitabın test
blokları. Verilator'ın kendisi yanlış derlerse ya sahte bulgu çıkar ya da
gerçek bir hata gizlenir. CI, Verilator'ı OSS CAD Suite 2026-09-21'den
alıyordu; paketteki `5.053 devel rev v5.052-159-g46774fded` doğru bir
SV'de `assign o151a = 4'(p247) - 4'(p247);`'yi `4'hf` bastı (#145).
Yerel Docker arka ucu ise `verilator/verilator:v5.052` koşturuyordu: CI
ile kullanıcı farklı hakemle ölçülüyordu.

### Kural

1. **Hakem sürümü sabittir.** Simülasyon yapan her CI işi ("Verilator
   lint", kitabın "Book code examples" işi, `difftest.yml`, `mutants.yml`)
   Verilator'ı tek bir tanımdan alır: `scripts/ci/verilator.env` (sürüm,
   imaj, özet). Değer bir sürüm etiketidir; geliştirme derlemesi ya da
   gecelik paket kullanılmaz. Yosys, sby ve çözücüler OSS CAD Suite'ten
   gelmeye devam eder; kural yalnız Verilator içindir.
2. **CI ile Docker arka ucu aynı ikiliyi kullanır.** `.github/actions/verilator`
   eylemi Verilator'ı Docker arka ucunun özetle sabitlenmiş imajından
   (`docker.rs`, `SIMULATION_IMAGE`) kopyalar ve OSS CAD Suite'in
   Verilator'ını siler. `crates/volt-tools/tests/verilator_pin_tests.rs`
   sürüm dosyasıyla `SIMULATION_IMAGE`'ın adını, özetini ve `volt doctor`
   metnini karşılaştırır.
3. **Her iş sürümü denetler.** Eylemin son adımı `verilator --version`'ı
   PATH'ten okur; çıktı `Verilator <sürüm> <tarih> rev v<sürüm>` değilse ya
   da PATH'te birden çok `verilator` varsa iş açık bir hatayla düşer.
4. **Güncelleme ayrı bir PR'dır ve kanaryadan geçer.** `verilator.env` ile
   `docker.rs` birlikte değişir; kanarya testi
   (`crates/volt-driver/tests/verilator_canary_tests.rs`) yeni sürümle
   geçer; fark testinin büyük koşusu yeni hakemle koşar ve sonucu PR'da
   raporlanır.

### Kanarya

#145'in toplu koşusundan (tohum 20261107, 9. toplu koşu, a577d7a9)
küçültülmüş, tek vektörlü (bütün girişler 0) 256 satırlık doğru bir SV:
`verilator_canary/issue145.sv`. Beklenen değerler alan alan SV anlamından
elle hesaplandı ve testte yazılı. Komut satırı fark testininkiyle aynıdır
(DFG açık, `-O0`). Aynı cargo testi Linux'ta (Docker, Ubuntu 24.04):

- 5.053 devel: `o151k: expected 0x0, got 0x1`, `o151e/o151d/o151b/o151a:
  expected 0x0, got 0xf` — düşer;
- 5.052: `2 passed` — geçer.

Mekanizma (üretilen C++): DFG `o150a..o150e` karşılaştırmalarını geniş
`$display` birleştirmesine `((a >= b) << 0x1f) | …` olarak gömer; C++'ta
`bool << 31` işaretli `int`tir, kelime bölmenin `>>` kaydırması aritmetik
olduğundan işaret biti komşu alanlara yayılır. Küçültme sırasında hata
alanların bit konumlarına bağlı çıktı: sabit çıkışları birleştirmek ya da
bir alanı silmek hatayı iki sürümde de gizler.

### Kanıt

PR #147:

- "Verilator lint" (run 42112543425): `verilator --version: Verilator
  5.052 2026-09-05 rev v5.052`, `Verilator version check: OK`, OSS CAD
  Suite'in yedi `verilator*` dosyası silindi; 178 test özetinde 3675
  geçti, 0 düştü; fark testi (tohum 20261104, 2000) `diffs=0`; `Tool
  ledger [integration]: 117 test(s) asked for a tool; 119 lookup(s)
  found the tool (119 required), SKIP: 0`; "Tool coverage" `120 ran with
  the tool required, 0 did not`.
- Fark testinin büyük koşusu (difftest.yml, run 42112584394, tohum
  20261105-20261108, parça başına 12500): dört parça `diffs=0`, toplam
  50.293 kabul edilmiş ifade, her parçada `SKIP: 0`. 20261107 #145'in
  tohumudur.
- İlk koşuda (run 42111275983) imajın `verilated.mk`'si her derlemenin
  önüne `ccache` koyduğundan `volt test` "make: ccache: No such file or
  directory" ile düştü; eylem kopyadaki `OBJCACHE ?= ccache` satırını
  boşaltır (yalnız derleme önbelleği; üretilen kod değişmez).

### Sınırlar

- **5.052 de bu sınıftan bir yanlış derleme yapıyor** (#148): kanarya
  tasarımından `o150a`'yı okuyan sözleşme çıkarılınca 5.052 de `4'hf`
  basar, `-fno-dfg` ile doğrudur. Sabitleme CI ile kullanıcıyı aynı
  hakeme bağlar ve #145'in tetikleyicisini kapatır; hakemi hatasız
  yapmaz. `-fno-dfg` ya da başka bir sürüm bakımcının kararıdır (#148).
- Kanarya hatanın bir tetikleyicisini yakalar, sınıfın tamamını değil;
  tetik bit konumlarına duyarlıdır, tasarım yeniden biçimlendirilmez.
- "Formal verification" ve "Timing constraints" işleri Verilator
  kullanmaz; OSS CAD Suite'in Verilator'ı orada PATH'te kalır ve
  denetlenmez.
- Eylem Verilator'ı imajdan koşucuya kopyalar (ikisi de Ubuntu 24.04;
  `verilator_bin` yalnız libc ve libm'e bağlı). Üretilen C++'ı koşucunun
  g++'ı derler; imajdaki g++ ile aynı sürüm olması denetlenmez.

## Ek: DFG kapalı (#148, 2026-11-06)

### Gerekçe

Sabitlenen hakem de hatasız değil (yukarıda, "Sınırlar"): #145'in
kanarya tasarımından `o150a`'yı okuyan sözleşme çıkarılınca Verilator
5.052 de `assign o151a = 4'(p247) - 4'(p247);`'yi `4'hf` basar (#148).
Mekanizma #145'inkidir: DFG 1 bitlik karşılaştırmaları geniş `$display`
birleştirmesine `((a >= b) << 0x1f) | …` olarak gömer, C++'ta `bool << 31`
işaretli `int`tir, kelime bölmenin `>>` kaydırması işaret bitini komşu
alanlara yayar. `-fno-dfg` ile iki sürüm de doğru basar. Bu yalnız CI'ın
hakemini değil, kullanıcının `volt test`'ini de (yerel Verilator ya da
Docker arka ucunun 5.052'si) etkiler: Volt doğru SV üretse de simülasyon
sessizce yanlış değer verebilir.

Bakımcı kararı (#148'in (a), (c) ve (d) seçenekleri): DFG, Volt'un
Verilator'ı derleme ya da simülasyon için çağırdığı her yerde kapalıdır;
hata Verilator'a bildirilir; #148'in tasarımı ikinci kanaryadır.

### Kural

1. **Tek tanım.** Volt'un Verilator'a her derlemede verdiği seçenekler
   `volt_tools::VERILATOR_BUILD_FLAGS`'tır (bugün `["-fno-dfg"]`).
2. **Her derleme yolu onu kullanır:**
   - `volt test`, `volt run`, `volt test --waves`, yerel ve Docker arka
     ucu: `volt-driver` `sim/verilator.rs` (`VerilateJob::args`; Docker
     kolu aynı argümanları konteynere verir);
   - fark testinin koşucusu: `volt-difftest` `run.rs`
     (`Tools::verilator_flags`; crate `volt-*`'a bağlanmaz, ADR-0108 §1,
     listeyi `difftest_tests` verir);
   - hakem kanaryası: `verilator_canary_tests`.

   Davranış testleri, kitabın test blokları (`check_book.py`) ve
   mutasyon testi Verilator'ı `volt test` üzerinden çağırır, bayrağı
   oradan alır. `--lint-only` çağrıları (CI'ın "Verilator lint" adımı,
   `output_net_tests`, `reserved_name_tests`) eniyileme yapmaz, bayrak
   almaz.
3. **Mekanik denetim.** Her yolun komut satırı kendi testinde denetlenir
   (`sim/verilator.rs` `every_verilate_path_passes_volts_build_flags`,
   `docker_backend_tests`, `volt-difftest`
   `verilator_command_carries_the_callers_flags`);
   `crates/volt-tools/tests/verilator_flags_tests.rs` depoyu tarar:
   Verilator'ı derleme kipinde çağıran (`"--binary"`, `"--cc"`,
   `"--build"`; betik ve iş akışlarında `verilator --binary|--cc|--build`)
   ve envanterde olmayan ya da bayrağı tek tanımdan almayan her yer testi
   düşürür.
4. **İki kanarya.** `verilator_canary/issue145.sv` (#145) ve
   `verilator_canary/issue148.sv` (#148, aynı tasarım, sözleşmesiz; aynı
   alanlar, aynı girişler, ortak elle hesaplanmış beklenenler). İkisi de
   `VERILATOR_BUILD_FLAGS` ile derlenir.

### Ölçülen maliyet

`volt test`'in duvar saati süresi, `main`'in `volt`'u (f3a78bde,
bayraksız) ile bu değişikliğin `volt`'u (`-fno-dfg`), aynı Linux
konteynerinde (Verilator 5.052, g++ 13.3, 4 çekirdek; ccache kapalı,
`OBJCACHE=`), her koşu boş bir `--target-dir` ile (Verilator ve C++
derlemesi baştan). `examples/` altındaki dokuz test dosyası üç tur (2. tur
ters sırada), kitap iki tur. Hücre: medyan (turlar). Kitap satırında 7
`volt verify` koşusu konteynerde sby olmadığından iki ikilide de aynı
biçimde düşer (çıkış 3); sekiz test dosyası iki ikilide de geçer.

| Tasarım | Test | Bayraksız (main), s | `-fno-dfg` (dal), s | Fark |
|---|---|---:|---:|---:|
| `axi4lite_slave_test.volt` | ok. 5 passed; 0 failed | 6.43 (7.8, 6.4, 6.2) | 6.51 (7.6, 6.4, 6.5) | +1.2 % |
| `fir_filter_test.volt` | ok. 10 passed; 0 failed | 19.16 (22.4, 19.2, 18.2) | 19.11 (22.2, 19.1, 18.1) | -0.3 % |
| `hybrid_accel/hybrid_test.volt` | ok. 6 passed; 0 failed | 27.36 (34.5, 27.4, 26.9) | 28.01 (33.6, 28.0, 27.9) | +2.4 % |
| `i2c/i2c_test.volt` | ok. 12 passed; 0 failed | 6.33 (6.6, 6.3, 6.3) | 6.53 (6.8, 6.5, 6.4) | +3.2 % |
| `riscv_core_test.volt` | ok. 59 passed; 0 failed | 13.61 (14.6, 13.6, 13.6) | 13.52 (14.7, 13.5, 13.4) | -0.7 % |
| `riscv_pipeline_test.volt` | ok. 15 passed; 0 failed | 6.85 (6.8, 6.4, 7.1) | 6.43 (7.1, 6.3, 6.4) | -6.1 % |
| `soc/top_test.volt` | ok. 5 passed; 0 failed | 6.82 (7.2, 6.8, 6.8) | 7.13 (7.1, 6.8, 7.2) | +4.5 % |
| `uart_tx_test.volt` | ok. 4 passed; 0 failed | 6.07 (6.5, 6.1, 6.1) | 6.10 (6.7, 6.1, 6.1) | +0.5 % |
| `vga/vga_top_test.volt` | ok. 7 passed; 0 failed | 19.21 (19.7, 18.1, 19.2) | 18.54 (19.2, 18.5, 18.5) | -3.5 % |
| kitap (`check_book.py --run-tests`) | 8 test dosyası, 30 blok | 50.15 (49.9, 50.4) | 49.27 (49.5, 49.1) | -1.8 % |
| **Toplam (medyanlar)** | | 161.99 | 161.15 | -0.5 % |

Ölçülebilir bir maliyet yok: tek tasarımlarda fark −%6 ile +%5 arasında,
işareti karışık; toplamda %−0,5. Aynı ikilinin ilk turu ile sonraki
turları arasındaki fark (ısınma, %20'ye kadar) bayrağın etkisinden
büyüktür. Ölçülmeyen: uzun bir simülasyonda DFG'nin eniyilediği modelin
koşu süresi; buradaki testler kısadır ve süreyi derleme belirler.

### Bayrağın kaldırılma ölçütü

`-fno-dfg` ancak şunların hepsi sağlanınca kaldırılır:

1. Hata Verilator'da düzeltilmiştir (bildirilen issue kapanmış, düzeltme
   bir sürümdedir);
2. `scripts/ci/verilator.env` ile `docker.rs` o sürüme geçmiştir (ayrı PR,
   yukarıdaki kural 4);
3. o sürümle, `VERILATOR_BUILD_FLAGS` boşken iki kanarya da geçer ve fark
   testinin büyük koşusu fark vermez.

Kaldırma kendi PR'ıdır; CHANGELOG'da davranış değişikliği olarak yazılır.
Bu ölçüt sağlanmadan bayrağı kaldıran bir değişiklik `verilator_flags_tests`'i
(`the_flags_turn_dfg_off`) düşürür.

### Kanıt

Kanaryalar, Verilator'ın kendisiyle (`verilator --binary --timing
--assert -Wall -Wno-fatal -Wno-DECLFILENAME --top-module tb -Mdir obj -o
vtb -j 2 -CFLAGS -O0 [-fno-dfg] issueNNN.sv`), `R 0 17` satırının ilk 11
hanesi (doğrusu `fc0040003f8`, `o151a..o151k` yanlışken `fc00ffffff8`):

| Verilator | Seçenek | `issue145.sv` | `issue148.sv` |
|---|---|---|---|
| 5.052 (`verilator/verilator:v5.052`) | — | doğru | **yanlış** |
| 5.052 | `-fno-dfg` | doğru | doğru |
| 5.053 devel (OSS CAD Suite 2026-09-21) | — | **yanlış** | **yanlış** |
| 5.053 devel | `-fno-dfg` | doğru | doğru |

Aynı cargo testi, `VERILATOR_BUILD_FLAGS` boşaltılmış bir kopyada (5.052,
Linux): `issue145` geçer, `issue148` düşer (`o151k: expected 0x0, got
0x1`, `o151e/o151d/o151b/o151a: expected 0x0, got 0xf`); bayrakla iki
kanarya da geçer.

PR #149:

- "Verilator lint" (CI run 42229639709): `Verilator version check: OK
  (Verilator 5.052 2026-09-05 rev v5.052)`; 180 test özetinde 3683
  geçti, 0 düştü; `verilator_canary_tests` 3 geçti (iki kanarya dahil),
  `name_scope_sim_tests` 23, `parser_scope_sim_tests` 29 geçti; fark
  testi (tohum 20261104, 2000) `diffs=0`, `regressions=27 failed=0`;
  `Tool ledger [integration]: 119 test(s) asked for a tool; 121
  lookup(s) found the tool (121 required), SKIP: 0`; "Tool coverage"
  `122 ran with the tool required, 0 did not`.
- Fark testinin büyük koşusu (difftest.yml, run 42229656156, tohum
  20261105-20261108, parça başına 12500, `-fno-dfg` ile): dört parça
  `diffs=0`, toplam 50.293 kabul edilmiş ifade, her parçada `SKIP: 0`.
- Kitabın "Book code examples" işi (run 42229639719): `30 volt block(s)
  checked, 8 test file(s) run, 7 verify run(s), 0 failure(s)`.
- CI süresi: "Verilator lint" 782 s (`main`'in son koşusu 42161602424:
  817 s), içinde `difftest_tests` 105,5 s (108,8 s); büyük koşunun dört
  parçası toplam 2111 s (bayraksız önceki koşu 42112584394: 1962 s;
  parça başına −%11 ile +%32 arası, koşucu gürültüsü).

Verilator deposunda (verilator/verilator) aynı mekanizmayı anlatan bir
issue bulunamadı (2026-11-06, salt okuma araması); hata raporu bakımcı
tarafından gönderilir.

### Sınırlar

- `-fno-dfg` Verilator 5.002'den (DFG'nin geldiği sürüm, 2022-10) beri
  vardır; `volt doctor`'ın alt sınırı 5.0'dır, desteklenen her sürüm
  seçeneği tanır.
- Bayrak bilinen tetikleyiciyi kapatır, Verilator'ın başka
  eniyileyicilerindeki hataları değil.
- Kullanıcının üretilen SV'yi Verilator'da kendisi derlemesi Volt'un
  dışındadır; kitap (`limitations.md`) `-fno-dfg` eklemesini önerir.
