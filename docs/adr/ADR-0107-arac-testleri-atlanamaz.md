# ADR-0107: Araç İsteyen Testler CI'da Atlanamaz — Araç Defteri, Tek Araçlı İş, Sıfır SKIP

> Statü: Uygulandı
> Önceki karar: ADR-0079 — §6'daki iş tablosu: her araçlı CI adımının çalıştıracağı test ikililerini ve `VOLT_REQUIRE_TOOLS` değerini elle seçmesi
> İlgili: ADR-0094 (Docker köprüsü: `docker_e2e_tests` kendi işinde, sıfır SKIP ile), ADR-0099 (testlerde araç arka ucu açık)
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

(CI koşu kimlikleri ve günlük satırları PR'de doldurulur.)

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
