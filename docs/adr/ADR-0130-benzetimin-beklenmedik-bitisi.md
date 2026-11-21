# ADR-0130: Benzetimin Beklenmedik Bitişi — `volt test` Çıkış Kodunu ve Eksik Test Sonucunu Uzlaştırır, Başlamayan Test "not run" Olur (#377)

> Statü: Uygulandı
> İlgili: ADR-0033 (test blokları ve Verilator köprüsü; `VOLT-TEST-BEGIN/END` protokolü bu ADR'de testlerle uzlaştırılır), ADR-0100 (sessiz yanlış istisnası: önceden geçen koşuyu düşüren düzeltme son hanede, v0.1.3), ADR-0117 (gerçek hata derlemi; bu hata derlemin biçimine uymaz, "Doğrulama"da), ADR-0125 (tanı ilkesi: ne oldu, neden, ne yapabilirsin)
> Issue: #377 (kapsam incelemesi #379; `$finish` ayrı: #382)
> Tarih: 2026-11-21
> Etkilenen: `crates/volt-driver/src/sim/tb_output.rs` (`parse_tb_output` yarıda kalan testi tanır; `reconcile`, `SimStop`, `GroupStop`, `GroupRun`), `crates/volt-driver/src/sim/report.rs` (`not run` satırları, duruş ayrıntısı, özet), `crates/volt-driver/src/sim/test_cmd.rs` (grup sonucu uzlaştırılır; `--nocapture` stderr'i de basar), `crates/volt-driver/src/sim/test_waves.rs` (test yardımcısı), `crates/volt-driver/tests/sim_stop_tests.rs` (YENİ), `CHANGELOG.md`, `docs/adr/README.md`.
> DOKUNULMADI: `docs/spec/` (gereken değişiklik "Spec" bölümünde; bakımcı uygular), testbench üretimi (`volt-sv-emit`), `volt run`.

## Sorun

#377: `volt test` benzetim sürecinin çıkış kodunu hiç okumuyordu
(`run_group`, yalnız Docker konteyner hatası için bakılıyordu) ve sonucu
yalnız `VOLT-TEST-END` satırlarından kuruyordu; özet bu sayıyı "running N
tests" ile karşılaştırmıyordu. Testbench bir testin ortasında kesilince
o test ve ondan sonrakiler rapordan sessizce düşüyor, koşu "ok" diyip 0
ile çıkıyordu.

Gerçek tetikleyici: `extern` modülün `@source` SystemVerilog gövdesindeki
bir denetleyici (ADR-0076). Verilator 5.054 ile ölçüldü (#377'nin
örneği: 3 test, 2.'si denetleyiciyi tetikler, 3.'sü `q == 99` bekler ve
düşmeliydi):

| Gövdedeki çağrı | Testbench | `volt test` (önce) |
|---|---|---|
| `$fatal(1, "...")` | `%Fatal: ...`, `Verilog $stop`, `Aborting...`; çıkış 1 | "1 passed; 0 failed", çıkış 0 |
| `$error("...")` | `%Error: ...`, `Verilog $stop`; çıkış 1 | aynı |
| `$stop` | `%Error: ... Verilog $stop`; çıkış 1 | aynı |
| `assert (...) else $error(...)` | `%Error: ... Assertion failed`; çıkış 1 | aynı |
| `$finish` | `Verilog $finish` basılır, süreç sürer (her test kendi `VerilatedContext`'iyle koşar) | test geçer — bu ADR'nin kapsamı dışında, #382 |

İki gözlem kuralı belirler:

1. Verilator koşuyu keserken **1** ile çıkar; testbench de bir test
   kalınca 1 ile çıkar (`return failed == 0 ? 0 : 1`). Çıkış kodu tek
   başına normal bir düşüşü kesilmiş koşudan ayıramaz.
2. Simülatörün iletileri stdout'a yazılır (stderr boştur); sürücü onları
   hiç göstermiyordu.

## Karar

### 1. Olağan bitiş ve geri kalan her şey

Bir grubun (bir modülün testleri, tek testbench) koşusu **olağan**
biter, ancak ve ancak:

- gruptaki her test `VOLT-TEST-END` satırını yazdı, ve
- süreç, testbench'in sözleşmesindeki kodla çıktı: hiçbir test
  kalmadıysa 0, en az biri kaldıysa 1.

Olağan bitişte sonuç, rapor ve çıkış kodu bu ADR'den önceki gibidir
(bayt bayt aynı rapor). Başka her bitiş benzetimin koşuyu kestiğini
gösterir ve şöyle raporlanır:

- **Yarıda kalan test** (`VOLT-TEST-BEGIN` var, `VOLT-TEST-END` yok)
  **kalır** (`FAILED`). Ayrıntısında çıkış kodu (ya da "ended by a
  signal") ve simülatörün iletileri vardır.
- **Başlamayan testler** geçti sayılmaz: `test <ad> ... not run`
  satırıyla ve özetten önce ayrı bir "not run" listesiyle raporlanır.
- **Hiçbir teste bağlanamayan bitiş** (her test sonucunu yazdı ama kod
  sözleşmeye uymuyor; iki test arasında ya da ilk testten önce bitti)
  grubun duruşudur: "failures" altında `---- simulation of <modül> ----`
  başlığıyla raporlanır.

### 2. Rapor biçimi

Yarıda kalan testin ayrıntısı (ADR-0125 düzeni: ne oldu, neden, ne
yapabilirsin; rapor bilerek İngilizcedir, makine okur):

```
---- trips_checker ----
  the simulation stopped during this test (exit code 1)
    simulator: [4] %Fatal: extern_chk.sv:2: Assertion failed in TOP.M.c: value out of range
    simulator: %Error: extern_chk.sv:2: Verilog $stop
    simulator: Aborting...
  reason: the simulator ended the run before every test finished ($fatal, $error, $stop, a failed assertion or a crash), so this run has no trustworthy result
  help: fix what the simulator reports above; `volt test --nocapture` shows its full output

not run (the simulation stopped before these tests started):
    never_runs

test result: FAILED. 1 passed; 1 failed; 1 not run
```

- Simülatör satırları: son `VOLT-TEST-BEGIN`'den sonraki protokol dışı
  stdout satırları, ardından stderr; en çok 20 satır.
- Özet satırına `; N not run` yalnız N > 0 iken eklenir: olağan koşunun
  özeti değişmez ("test result: ok. N passed; 0 failed", "test result:
  FAILED. N passed; M failed").
- `--nocapture` ham stdout'un yanında stderr'i de basar.
- Yarıda kalan test düşen testtir: dalga formu (ADR-0095) onun için de
  izle yeniden koşar; yeniden koşunun ayrıştırması da yarıda kalmayı
  tanır, "yeniden koşunca geçti" yanlış uyarısı çıkmaz.

### 3. Çıkış kodu

Yarıda kalan, başlamayan test ya da grubun duruşu olan koşu **5** ile
çıkar (cli-contract.md §2: "Testler çalıştı, bazıları kaldı"). 3 (G/Ç ya
da araç hatası) değil: araç çalıştı; koşuyu tasarım (ya da extern
gövdesi) kesti, sonuç bir test sonucudur.

### 4. Kapsam dışı

- **`volt run`** zaten sıfırdan farklı çıkışta "simulation exited with
  code N" der ve 3 ile çıkar (sessiz değil). Simülatörün stdout'taki
  iletilerini tablo satırlarıyla basar. Değişmez.
- **`$finish`** süreci bitirmez (her test kendi `VerilatedContext`'i
  ile koşar, testbench `gotFinish()`'e bakmaz); `$finish` çağrılan test
  koşmaya devam eder ve geçebilir. Bunun ne anlama geldiği (testin
  sonu mu, hatası mı) testbench üretiminde bir karardır: #382.

### 5. Sürüm (ADR-0100)

Önceden "ok" ile 0 dönen bazı `volt test` koşuları artık 5 ile düşer:
yalnız benzetimin koşuyu kestiği koşular. Bu bir sessiz yanlışın
düzeltmesidir (düşmesi gereken bir test hiç koşmadan koşu geçiyordu);
ADR-0100'ün istisnasıyla son hanede (v0.1.3) çıkar ve "Behavior changes"
bölümünde yazılır. Yeni tanı kodu, seçenek ya da JSON alanı yoktur.
Olağan koşuların raporu ve çıkış kodu değişmez.

## Spec

`docs/spec/cli-contract.md` §8'e (çıkış kodu paragrafından sonra) şu
alt bölüm eklenir; bakımcı uygular:

> **Beklenmedik bitiş (ADR-0130).** Testbench her testin sonucunu yazar
> ve hiçbir test kalmadıysa 0, biri kaldıysa 1 ile çıkar. Başka her
> bitiş (`$fatal`, `$error`, `$stop`, başarısız `assert`, çökme)
> benzetimin koşuyu kestiğini gösterir: sonucunu yazmadan kesilen test
> `FAILED` olur ve ayrıntısında çıkış kodu ile simülatörün iletileri
> yazılır; hiç başlamayan testler `test <ad> ... not run` satırıyla ve
> özetten önce ayrı bir listeyle raporlanır, geçti sayılmaz; özet
> `test result: FAILED. P passed; F failed; N not run` olur (`; N not
> run` yalnız N > 0 iken). Çıkış kodu 5'tir. Olağan koşunun raporu bu
> kuraldan etkilenmez. `$finish` süreci bitirmez (#382).

§2'deki tablo değişmez (5'in anlamı aynı). `docs/spec/cli-contract.md`
henüz yalnız Türkçedir (İngilizce çevirisi bekliyor); ekleme o dosyaya
yapılır.

## Doğrulama

- `crates/volt-driver/tests/sim_stop_tests.rs` (gerçek Verilator; araçlı
  işte zorunlu): #377'nin örneği (`$fatal`: 1 passed, 1 failed, 1 not
  run, çıkış 5, simülatör iletisi raporda); son testte `$error` (başlamamış
  test yok, süreç normal düşüş gibi 1 ile çıkar, test yine kalır);
  olağan geçen ve düşen testler (özet satırı ve kod eskisi gibi). İlk
  ikisi düzeltmeden önce kırmızıydı (çıkış 0, "1 passed; 0 failed" /
  "2 passed; 0 failed"), üçüncüsü önce de sonra da yeşil.
- `tb_output.rs` ve `report.rs` birim testleri: uzlaştırmanın her kolu
  (olağan koşu, yarıda kalan test, iki test arası ve ilk testten önce
  bitiş, son testten sonra beklenmeyen kod ya da sinyal, kalan test
  varken 0) ve rapor satırları.
- `$stop` ve başarısız `assert` aynı mekanizmadan geçer (Verilator'da
  aynı çıktı biçimi); Verilator ile elle doğrulandı, ayrı entegrasyon
  testi yok (Verilator derlemesi pahalı).

**Gerçek hata derlemi (ADR-0117).** `tests/real_bugs/` bir tasarımın
doğru sonucunu `volt check` kodları ve elle hesaplanmış değerlerle
*geçen* test blokları olarak yazar; koşucu `volt test`'in başarılı
olmasını ve "N passed; 0 failed" basmasını şart koşar
(`metamorphic_tests.rs`). #377'nin doğru sonucu düşen bir koşudur ve bir
extern SV gövdesindeki `$fatal`'a dayanır; derlem dosyası olarak
metamorfik ve diferansiyel testlerin girdisine de girer ve orada her
benzetimi keserdi. Bu yüzden derleme girmez; çoğaltması
`sim_stop_tests.rs`'tedir.

## Reddedilenler

- **Yalnız çıkış koduna bakmak.** Verilator koşuyu keserken 1 ile çıkar,
  normal düşen koşu da 1 ile: son testte kesilen koşu ayırt edilemezdi.
- **Kesilen koşuya 3 (araç hatası) vermek.** Araç çalıştı; kesilme
  tasarımın sonucudur. `volt test`'i CI'da koşan kullanıcı 5'i "testler
  kaldı" diye okur, doğru okuma budur.
- **Başlamayan testleri "failed" saymak.** Hiç koşmamış bir testi
  "kaldı" saymak yanıltır (iddiası sınanmadı); ayrı sayı neyin
  sınanmadığını gösterir. Koşu yine düşer.
- **Testbench'i yeniden yapılandırmak (her test ayrı süreçte).** Kesilen
  testten sonrakileri de koştururdu, ama her test için ayrı süreç ve
  üretilen testbench değişikliği ister; bu düzeltme üretilen hiçbir
  çıktıyı değiştirmez.
