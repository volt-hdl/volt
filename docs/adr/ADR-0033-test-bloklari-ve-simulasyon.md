# ADR-0033: Test blokları ve Verilator simülasyon köprüsü

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-09
> Etkilenen: volt-syntax, volt-ast, volt-hir, volt-sv-emit, volt-driver,
> docs/spec/grammar-full.ebnf, docs/spec/cli-contract.md
> Uygulama aşaması: F4

## Sorun

`examples/uart_tx.volt` derleniyor, kontratları kanıtlanıyor ve Verilator
lint'i temiz geçiyor; ama tasarımın **davranışı** doğrulanamıyor. Formal
kontratlar durum uzayı özelliklerini kanıtlar, protokol dizilimini
(start biti 0 mı, 8 veri biti LSB önce mi, çerçeve 10 bit mi) kanıtlamaz.
Volt'un simülasyon yürütme yolu yok.

## Seçenekler

- **A — Yerleşik simülatör yazmak.** Tam kontrol; ama olay tabanlı bir
  simülatör başlı başına bir üründür, Verilator ile performans ve
  doğruluk rekabeti gerçekçi değildir. RED.
- **B — Verilator köprüsü.** Üretilen SV zaten Verilator lint'inden
  geçiyor; `--cc --exe --build` akışıyla C++ testbench derlenip
  koşulabilir. Endüstri standardı, VCD desteği bedava. KABUL.
- **C — cocotb/pyuvm köprüsü.** Python bağımlılığı ve iki dilli test
  yüzeyi; Volt'un "tek dosya, tek araç" UX hedefiyle çelişir. RED.

## Karar

### 1. `test` bloğu — bağlamsal anahtar kelime (ADR-0023 deseni)

`test`, lexer'da anahtar kelime DEĞİLDİR; yalnız öğe konumunda
`test <StringLit>` dizilimi test bloğu başlatır. `test` adlı port,
register ya da modül kırılmaz.

```volt
test "counter increments" {
    let dut = Counter { };
    dut.enable = true;
    step(1);
    assert_eq(dut.count, 1);
    step(3);
    assert_eq(dut.count, 4);
}
```

Gramer eki (grammar-full.ebnf — bu ADR'nin verdiği izinle):

```ebnf
ItemKind  = ... | TestDecl ;
TestDecl  = "test" StringLit "{" { TestStmt } "}" ;
TestStmt  = "let" Ident "=" Ident "{" "}" ";"
          | Ident "." Ident "=" TestExpr ";"
          | Ident "(" [ TestExpr { "," TestExpr } ] ")" ";" ;
TestExpr  = IntLit | "true" | "false" | Ident "." Ident ;
```

Test gövdesi donanım değil doğrusal bir betiktir; bu yüzden modül gövdesi
deyimlerinden ayrı, KÜÇÜK bir dil kullanır ve `;` ile sonlanır (modül
gövdesinde `;` yoktur — betik/donanım ayrımı görünür kalsın diye bilinçli).

Yerleşikler (yalnız test gövdesinde):

| Yerleşik | İmza | Anlam |
|---|---|---|
| `step(n)` | n: IntLit ≥ 1 | n saat çevrimi ilerlet (posedge+eval) |
| `reset()` | — | `rst`'yi 2 çevrim yüksek tut, bırak |
| `assert_eq(a, b)` | TestExpr, TestExpr | eşitlik denetimi |
| `assert_ne(a, b)` | TestExpr, TestExpr | eşitsizlik denetimi |
| `assert_true(a)` | TestExpr | a != 0 denetimi |
| `assert_false(a)` | TestExpr | a == 0 denetimi |

Simülasyon anlambilimi: koşu `rst=1` ile 2 çevrim başlar, sonra `rst=0`
(üretilen SV'deki örtük `rst` girişi, register'ları başlangıç değerine
alır). `dut.port = v` bir sonraki `step`ten önce uygulanır; okuma ve
assert'ler son `eval` sonrası değerleri görür. Çok saatli modüllerde tüm
saat portları birlikte sürülür (F4 sınırı; ileride saat başına periyot).

Port yön kuralları: yalnız `in` portlarına yazılır, yalnız `out`
portlarından okunur (`clock` tipli porta yazmak da hatadır).

### 2. Kardeş dosya kuralı

`X_test.volt` derlenirken aynı dizindeki `X.volt` (varsa) otomatik
birlikte ayrıştırılır; testler oradaki modülleri örnekleyebilir. Modül
testle aynı dosyada da olabilir.

### 3. CLI — `volt run` ve `volt test` (cli-contract.md'de zaten [F4] olarak ayrılmıştı)

```
volt run  [--cycles N] [--vcd <dosya>] [--top <modül>] <dosya>
volt test [filtre] [--nocapture] [--target-dir <dizin>]
```

`volt run`: derle → SV üret → C++ testbench üret → `verilator --cc
--exe --build` → koştur. Testbench: 2 çevrim reset; `cycle 0` satırı
reset sonrası durumu basar; ardından saat dışı tüm girişler 1'e sürülür
ve her çevrimde saat dışı tüm portlar tablo satırı olarak yazılır
(deterministik duman stimulusu — gerçek doğrulama `volt test`indir).
Birden çok modül varsa `--top` zorunludur (yoksa kullanım hatası, çıkış 2).

`volt test`: `filtre` bir `.volt` yolu ise o dosyanın testleri; değilse
çalışma dizinindeki `*_test.volt` dosyaları taranır ve test adı alt dizesi
olarak süzer. Çıktı cargo biçimindedir (`running N tests`, `test ad ...
ok/FAILED`, `failures:` bloğu, özet satırı). Görünen test adı, string
addaki boşlukların `_` yapılmış halidir. `--nocapture` testbench ham
çıktısını da akıtır.

Çıkış kodları (cli-contract.md §2 ile uyumlu): 0 başarı, 1 derleme
hatası, 2 kullanım hatası, **3 Verilator yok / araç hatası**, **5 test
koştu ve en az biri kaldı**.

Verilator `VOLT_VERILATOR` > `PATH` sırasıyla aranır (VOLT_SBY deseni).
Bulunamazsa UX Anayasası biçiminde kurulum yardımı basılır ve ayrıntı
`volt explain simulation-setup` konusuna taşınır.

### 4. VCD

`--vcd <dosya>` Verilator `--trace` ile VCD üretir. Üretilen SV sinyal
adları Volt kaynağındaki adlarla birebir olduğundan (ADR-0024) dalga
görüntüleyicide Volt adları görünür.

### 5. Yeni tanı kodları (E85xx — simülasyon testleri)

Bu ADR, tutarlılık taramasının kod tanım kaynağıdır:

| Kod | Anlam |
|---|---|
| E8501 | test bloğunda bilinmeyen modül örnekleniyor |
| E8502 | test bloğunda bilinmeyen port |
| E8503 | `in` olmayan porta yazma (clock dahil) |
| E8504 | `out` olmayan porttan okuma |
| E8505 | geçersiz yerleşik çağrı (bilinmeyen ad, argüman sayısı/tipi, `step(0)`) |
| E8506 | tanımsız ya da yinelenen `dut` adı |

Test HATALARI (assert kaldı) tanı değildir; cargo biçimli rapora ve
çıkış kodu 5'e gider.

## Sonuçlar

- Dört UART sorusu artık `examples/uart_tx_test.volt` ile yürütülerek
  yanıtlanır; formal + lint + simülasyon üçlüsü tamamlanır.
- Testbench üretimi `volt-sv-emit`te yaşar (AST'yi bilen codegen crate),
  koşturma/raporlama `volt-driver`da (verify.rs deseni).
- `volt build`/`check`/`verify` Verilator GEREKTİRMEZ; yalnız
  `run`/`test` çağırır.
- Gelecek: test gövdesinde aritmetik ifadeler, `expect_seq` gibi dizilim
  yardımcıları, saat başına periyot — ayrı ADR ister.

## Emsal

- Rust `cargo test` çıktı biçimi ve `--nocapture` bayrağı.
- Verilog-mode/cocotb akışlarında Verilator `--cc --exe --build` standardı.
- ADR-0023 (bağlamsal anahtar kelime) ve F4b `verify` araç-köprüsü deseni
  (VOLT_SBY → VOLT_VERILATOR).
