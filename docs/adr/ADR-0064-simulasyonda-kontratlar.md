# ADR-0064: Simülasyonda Kontratlar — İzleyici Olarak `volt test`

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-21
> Etkilenen: volt-sv-emit (`sim_contract.rs` YENİ — `SvaMode::Simulation`,
> DPI izleyicileri, C++ prelude; `sva.rs` mod + `SvaProp::primitive`;
> `past.rs` yardımcı reg zinciri Simulation'da da; `builtin_prim.rs`
> primitif kontratlarının izleyici biçimi; `sim.rs`/`sim_script.rs`
> testbench), volt-driver (`sim/contracts.rs` YENİ; `test_cmd.rs`,
> `test_build.rs`, `run_cmd.rs`, `tb_output.rs`, `report.rs`, `main.rs`
> `--no-contracts`/`--contracts`), volt-diagnostics (W5001),
> cli-contract.md (§7, §8),
> `.github/workflows/ci.yml` (iki adım), `tests/fixtures/sim_contracts/`.
> DOKUNULMADI: volt-hir, volt-syntax, examples/, README.md;
> `volt verify` ve `volt build` çıktıları (bayt bayt aynı, bkz. §8).

## Sorun

`docs/research/rekabet-2026-09.md` §6: "Kontratlar simülasyonda koşmuyor
(ADR-0040). `volt test` kontrat ihlalini görmez." ADR-0040 bunu açık bir
sınır olarak yazmıştı. Sonuç: bir `invariant` simülasyonda bozulsa kimse
fark etmez; formal ise derinlik sınırına takılır (RV32IM keşfinde UART
cover'ı 40 adımda ulaşılamadı). Simülasyon binlerce çevrim koşuyor —
kontratlar orada izleyici olarak çalışırsa formal'in ulaşamadığı
derinlikte ihlaller yakalanır.

## Ölçüm — hangi biçim Verilator'da güvenle çalışır? (Verilator 5.050)

Fixture: `invariant: count < 5`, `invariant: !prev(en) || count != 0`,
`assume: en || !prev(en)`, `cover: count == 3`; testbench reset
boyunca `en = 1` tutar, sonra 7 çevrim `en = 1`, 2 çevrim `en = 0`.
Aynı modülün iki biçimi `--assert` ile (ve onsuz) derlendi:

| Gözlem | `--emit=sva --sva inline` (property + `$past`) | `volt verify` biçimi (immediate + yardımcı reg) |
|---|---|---|
| Derleme | temiz | temiz |
| `!prev(en) \|\| count != 0`, reset sonrası 1. çevrim | **YANLIŞ ALARM** — `$past(en)` reset içindeki `en = 1`'i örnekler | geçer — yardımcı reg reset'te 0 (ADR-0040 §5) |
| `count < 5` ihlali | çevrim 8'de | çevrim 8'de (aynı) |
| İlk ihlal | `%Error ... Verilog $stop` → `Aborting...` — süreç ölür, sonraki testler koşmaz | aynı |
| `assume` ihlali | `'assert' failed` — assert'ten ayırt edilemez | aynı |
| `cover` | hiçbir çıktı yok (yalnız `--coverage-user` veritabanı) | aynı |
| İleti | yalnız SV satırı + `TOP.Cnt` — Volt kaynağına eşleme yok | aynı |
| `--assert` olmadan | ölçülen 5.050'de assertion'lar YİNE etkin (7/6 hata satırı) | aynı |

`property`/`|->`/`disable iff`/`$past(x, N)` ayrıştırılır ve çalışır;
Verilator'un "sessizce yok saydığı" yapı çıkmadı — sorun anlamda:
`$past`'in reset içi örneği formal'in (ADR-0040) anlamıyla çelişir ve
Verilator'un assertion mekanizması test koşucusu olarak kullanışsızdır
($stop, sınıfsız ileti, sessiz cover).

## Karar

### 1. Biçim: immediate kalıbı + DPI geri çağrısı (`SvaMode::Simulation`)

`volt verify`'ın immediate kalıbı temel alınır — aynı saat kenarı, aynı
reset koruması, AYNI `prev()` yardımcı reg zinciri (reset değeri 0),
AYNI property adları (`inv_0`, `asm_0`, ...) — ama SV `assert`/`assume`/
`cover` yerine testbench'e DPI çağrısı üretilir:

```systemverilog
import "DPI-C" context function void volt_contract_fail(input string id);
import "DPI-C" context function void volt_cover_report(input string id, input longint hits);

// invariant from cnt.volt:7
always @(posedge clk)
    if (!(rst)) if (!(count < 8'd5)) volt_contract_fail("Cnt.inv_0");

// cover from cnt.volt:10
longint volt_hits_cov_0 = 0;
always @(posedge clk)
    if (!(rst)) if (count == 8'd3) volt_hits_cov_0 <= volt_hits_cov_0 + 1;
final volt_cover_report("Cnt.cov_0", volt_hits_cov_0);
```

- Kimlik `Modül.ad` — derleyicinin `SvaProp` kaydıyla (modül, ad,
  anahtar kelime, kaynak span'i) birebir eşlenir.
- Örnek yolu `svGetNameFromScope(svGetScope())` ile alınır (`context`
  import): `TOP.Top.u_sub` → raporda `dut.u_sub`.
- Çevrim sayacı testbench'tedir: `run_cycle` posedge'den ÖNCE artırır,
  her testin başlangıç reset'inden sonra 0'lanır → "cycle N" = testin
  N. saat kenarı.
- Cover sayacı SV'dedir (çevrim başına tek artırma), sayı `final`
  bloğunda bir kez raporlanır. İlk sürüm her tetiklenmede DPI çağırıyordu:
  VGA testlerinde (408 bin tetiklenme) yürütme +%24 idi; sayaç SV'ye
  taşınınca +%4.
- Yalnız kullanılan DPI bildirimleri modül gövdesinin başına konur.
  Kontratsız tasarımda SV ve testbench `--no-contracts` çıktısıyla bayt
  bayt aynıdır (DPI'sız tasarımda `svGetScope` bağlanmaz — testbench
  prelude'u yalnız SV izleyici içeriyorsa eklenir).
- Formal kurulum üretilmez: `initial assume (rst)`, primitiflerin
  "no mid-trace reset" kenar varsayımları ve `formal init` blokları
  yalnız Immediate'tedir (simülasyonda reset'i testbench sürer; testler
  `reset()` çağırabilir).
- RTL kısmı `None` moduyla aynıdır (çift yönlü portların `anyseq`
  formal modeli simülasyona girmez).

### 2. Kontrat türlerinin simülasyon anlamı

| Tür | Denetim | İhlal |
|---|---|---|
| `invariant`, `assert`, `ensures` | her saat kenarında, reset dışında | test DÜŞER: `contract violated` |
| `requires`, `assume` | aynı | test DÜŞER: `assumption violated by test stimulus` (ayrı başlık + not) |
| `cover` | aynı; doğru olduğu her çevrim sayılır | hata değil; `cover summary` bölümü, hiç tetiklenmeyen `NEVER HIT` |

İlk ihlalde test o çevrimde durur (`step(n)` döngüsü ihlal çevriminde
kırılır, `dut.final()` çağrılır); yürütülebilir sonraki testlere devam
eder. Aynı (kimlik, örnek) için yalnız ilk çevrim kaydedilir.

### 3. `assume` ihlali: test DÜŞER

Formal'de `requires`/`assume` girdi kısıtıdır; simülasyonda ihlali
tasarım hatası değil uyarıcı (stimulus) hatasıdır. Yine de test düşer:

1. Varsayım dışındaki girdiyle tasarımın hiçbir garantisi yoktur;
   sonraki her gözlem (geçen `assert_eq` dahil) anlamsızdır. Yeşil bir
   test "tasarım yasal girdide doğru" iddiası taşır — yasal girdi
   kullanmayan test bunu kanıtlayamaz.
2. Uyarı olarak bırakılsa CI'da görünmez; stimulus hatası sessizce
   birikirdi (bu ADR'nin kapattığı sorunun aynısı).
3. Sınıflandırma ayrıdır: başlık `assumption violated by test stimulus`,
   not "fix the stimulus (or the assumption), not the design". Kasıtlı
   yasa dışı girdi testi `--no-contracts` ile koşturulur.

Bu sınıf YALNIZ DUT'un kendi `requires`/`assume`'u içindir: testin
sürdüğü girdileri o kısıtlar. Alt örneğin varsayımını (ör. tüketici
tarafındaki `Handshake<T>` protokol kontratı — parser onu `assume`
üretir) test değil onu süren üst modül bozar; bu bir tasarım hatasıdır
ve `contract violated: assume (...)` + "its parent drives those inputs,
so the parent broke it" notuyla raporlanır (`volt run`'da da çıkış 5).

`volt run --contracts`'ta ise DUT'un varsayım ihlali çıkış kodunu ETKİLEMEZ:
duman uyarıcısı (tüm girişler 1) aracın kendisinindir, tasarımın
varsayımlarını gözetmez; ihlal yalnız raporlanır.

### 4. Reset ve `prev()`

- Reset süresince denetim kapalı (formal'deki reset koruması / `disable
  iff` ile aynı): izleyici `if (!(<reset koşulu>))` ile korunur; alanın
  kutbu (`!rst_n`) domain'den gelir. Test ortasındaki `reset()` de
  kapsanır.
- `prev(x)` yardımcı reg zinciri Immediate ile aynı koddan üretilir;
  reset sonrası ilk çevrimde `prev(x) == 0` — formal ile AYNI (ölçümdeki
  `$past` yanlış alarmı bu yüzden oluşmaz).

### 5. Otomatik kontratlar

- `Handshake<T>` protokol kontratları ve `@mmio` otomatik kontratları
  parser'da AST kontratına açılır — izleyiciler kendiliğinden üretilir
  (axi4lite, soc örneklerinde koştu).
- stdlib primitiflerinin kontratları (`builtin_prim.rs`) aynı DPI
  biçimine çevrilir; kimlik `Modül.<örnek>_<ad>` (`UartCtrl.fifo_cov_0`),
  raporda "stdlib SyncFifo contract 'fifo_inv_0'" olarak anlatılır
  (`SvaProp::primitive`). Bu metinler Yosys için yazılmıştır ve SV
  kuralıyla genişleyen karşılaştırmalar içerir (13 bit `addr <
  14'd8192`): Verilator bunu varsayılan olarak ölümcül WIDTHEXPAND ile
  reddetti (VGA örneği). Formal çıktı değişmesin diye primitif
  izleyicileri `// verilator lint_off WIDTH` ... `lint_on` ile sarılır.
- Alt modül örneklerindeki kontratlar kendi SV modüllerinde üretilir;
  her örnek kendi izleyicisini taşır, rapor örnek yolunu gösterir.

### 6. Rapor

```
---- counts_past_limit ----
  contract violated: invariant (cnt_test.volt:7)
    invariant: count < 5
  at cycle 6
  in instance: dut
  loop: i = 3                       (ihlal for içindeyse)

cover summary:
  Cnt.cov_0 (cnt_test.volt:10)  hit 12 times
  Cnt.cov_1 (cnt_test.volt:11)  NEVER HIT
```

Testbench protokolü (makine-okur): `VOLT-CONTRACT-FAIL <kimlik>
cycle=<n> [loop=i=3] inst=<kapsam>` (kapsam satır sonuna kadar),
`VOLT-COVER <kimlik> <sayı>`. Cover özeti tüm testler ve gruplar
boyunca kimliğe göre toplanır; çıkış kodunu etkilemez.

### 6a. İzlenemeyen kontrat — W5001

Kontrat ifadesi SV'ye inemiyorsa (emitter E0003 üretir, ör. `match`
ifadesi) formal akış onu reddeder; ama izleyiciler `volt test`'te
varsayılan açık olduğundan aynı hata eskiden geçen bir test dosyasını
derlenemez hâle getirirdi. Simulation modunda bu kontratın E0003'ü
**W5001** "contract cannot be monitored in simulation; skipped"
uyarısına indirgenir ve izleyicisi üretilmez; diğer kontratlar izlenir,
adları verify'daki gibi kalır (sayaç ilerler). `volt verify` davranışı
değişmez (E0003). `volt explain W5001` iki dilde.

### 7. Varsayılanlar ve bayraklar

- `volt test`: kontratlar VARSAYILAN AÇIK; `--no-contracts` kapatır
  (izleyici üretilmez, SV ve testbench eski çıktının aynısı).
- `volt run`: VARSAYILAN KAPALI; `--contracts` açar. Gerekçe: duman
  uyarıcısı yasal girdi üretmez, varsayılan açık olsa kullanıcıyı
  anlamsız ihlallere boğardı. Açıkken ihlaller koşu sonunda raporlanır
  (koşu durmaz); kontrat ihlali çıkış kodu 5.

### 8. Kısıtlar — değişmeyenler (golden)

`main` (57f5170) ve bu dal aynı betikle karşılaştırıldı: `examples/`,
`tests/ui/pass/`, `tests/fixtures/` altındaki her `.volt` için `volt
build` (RTL), `--emit=sva` (ayrı), `--emit=sva --sva inline` ve `volt
verify`'ın ürettiği formal SV + `.sby` dosyaları (54 verify birimi) —
**674 dosya bayt bayt aynı**, çıkış kodları aynı. Simulation kodu yalnız
yeni mod dalında çalışır; Immediate yolu (`contract_line`,
`sva_immediate`) değişmedi.

## Sonuçlar

### Performans (Docker, Verilator 5.050, ccache kapalı soğuk derleme)

| Ölçüm | kontratlı | `--no-contracts` | fark |
|---|---|---|---|
| riscv_core_test (59 test) — toplam, soğuk | 15,1 s | 13,6 s | +%11 (C++ `<map>/<string>`, DPI) |
| riscv_core_test — ccache sıcak | 1,35 s | 1,30 s | +%4 |
| riscv_core_test — yürütme (10 koşu) | 1,50 s | 1,49 s | gürültü içinde |
| vga_top_test (7 test, 408 bin cover tetiklenmesi) — yürütme (3 koşu) | 0,30 s | 0,29 s | +%4 |

### Mevcut testler kontratlar açık (ADIM 5)

14 dosya / 132 test (examples/ altındaki 9 `*_test.volt` + tests/ui/pass
altındaki 5 test dosyası) kontratlar açık koşturuldu:

- **A (gerçek tasarım hatası): 0.**
- **C (uyarıcı / assume ihlali): 0.**
- **B (derleyici hatası): 1** — `vga_top_test.volt` Verilator derlemesinde
  düştü: AsyncDualPortRam primitif kontratının WIDTHEXPAND uyarısı (§5).
  Derleyicide düzeltildi; 7/7 geçti.
- Yanlış alarm: 0. Kontratlı 132 testin tamamı geçiyor; cover özetleri
  gerçek boşlukları gösteriyor (ör. `Axi4LiteSlave.cov_2`,
  `I2cMaster.cov_5`, `FirFilter_4_16.cov_0/1`, `UartCtrl.fifo_cov_0` hiç
  tetiklenmedi — testlerin hiç ulaşmadığı durumlar).

### Formal derinliğinin ötesinde ihlal (ADIM 6, kalıcı test)

`tests/fixtures/sim_contracts/deep_violation.volt`: 0..99 sayması
gereken sayaç kasıtlı hatayla 100'de sarar; `invariant: count < 100`
ilk kez 101. çevrimde bozulur.

| Komut | Sonuç |
|---|---|
| `volt verify --mode bmc --depth 20` | çıkış 0 — 2 property doğrulandı (göremez) |
| `volt verify --mode bmc --depth 110` | çıkış 6 — "violated at cycle 102" (formal kendi reset adımını da sayar: formal = simülasyon + 1) |
| `volt test` (200 çevrim) | çıkış 5 — `contract violated: invariant (deep_violation.volt:14)`, `at cycle 101` |

CI: formal işi derinlik 20'nin geçtiğini, Verilator işi
`sim_contract_tests` ile testin düştüğünü denetler.

### Testler

`crates/volt-sv-emit/tests/sim_contract_emit_tests.rs` (25: izleyici
biçimi, DPI bildirimleri, prev zinciri, formal yapı yokluğu, property
adlarının verify ile aynılığı, negedge/aktif-düşük reset, stdlib/
Handshake/@mmio kontratları, alt modül, testbench C++'ı),
`crates/volt-driver/src/sim/contracts.rs` (8) + `tb_output.rs` (1),
`crates/volt-driver/tests/sim_contract_tests.rs` (18: üretilen dosyalar
her ortamda; invariant/ensures/assume ihlali, cover özeti, reset
süresince ihlal, prev ilk çevrim, alt modül örnek yolu, döngü bağlamı,
`--no-contracts`, testler arası devam, derin ihlal, `volt run
--contracts`, alt modül `requires`'ı üst modülün hatası, W5001 —
çalışma zamanı olanlar Verilator ister, yoksa atlanır);
volt-diagnostics `explain_tests` (W5001).

## Sınırlar / Ertelenen

- Saatsiz (kombinasyonel) modülün kontratları izlenmez (formal akışla
  aynı: saat yoksa property yok). Böyle bir alt modül saatli bir üst
  modülde örneklense de izleyicisi yoktur.
- Çok saatli modülde kontratlar ilk saat portunun kenarında denetlenir
  (mevcut SVA kuralı); primitifler kendi alanlarının saatini kullanır.
- `@mmio` otomatik kontratlarının konumu sentetik kaynağı gösterir
  (`<mmio:Gpio>:50`) — kullanıcının dosyası değil.
- İhlal, `step(n)` içinde ihlal çevriminde durdurur; `load` ve port
  yazımı saat ilerletmediğinden denetim gerektirmez.
- Cover sayısı "koşulun doğru olduğu çevrim sayısı"dır (kenar değil).
- `ensures`'ün `!a || b` deseni formal'de olduğu gibi dönüştürülmeden
  aynı çevrimde denetlenir.
- Test başarısız olduğunda (iddia ya da kontrat) cover sayıları o ana
  kadarki çevrimleri içerir.
- Reset'siz alanda (`reset = none`) izleyici korumasızdır: testbench'in
  başlangıç `apply_reset` çevrimlerinde cover sayacı da sayar (ihlaller
  o çevrimlerde `volt_contracts_begin` ile silinir, cover sayıları
  silinmez). Yalnız o çevrimlerde doğru olan bir cover `NEVER HIT`
  yerine tetiklenmiş görünebilir.
- Üretilen adlar (`volt_hits_<ad>`, `past_<sinyal>_<N>`) kullanıcı
  sinyaliyle çakışırsa SV çift bildirim hatası verir. `past_*` çakışması
  verify'da zaten vardı; `volt_` önekini sinyal adlarında kullanmayın.
- İnceleme ajanı kritik bulgu bulmadı; iki orta bulgu (E0003'ün
  `volt test`'i kırması → W5001; alt modül varsayımının uyarıcıya
  yüklenmesi → örnek yolu kuralı) bu ADR'de düzeltildi.
