# ADR-0040: Ardışık Kontratlar — `prev()` Yerleşiği

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-13
> Etkilenen: sv-mapping.md §15, volt-hir (resolve.rs prelude `prev` +
> E5017, typeck.rs tip taşıma, domain.rs alan kuralı), volt-sv-emit
> (past.rs, sva.rs Immediate yardımcı reg, expr.rs `$past`),
> volt-diagnostics (E5017), examples/axi4lite_slave.volt

## Sorun

`examples/axi4lite_slave.volt` (ADR-0039) keşfinde AXI protokolünün
ASIL kuralları yazılamadı: "valid, ready gelene kadar sabit kalmalı",
"bvalid, bready'ye kadar korunmalı", "el sıkışmadan sonra yanıt
gelmeli". Hepsi bir ÖNCEKİ döngüdeki değere referans gerektirir.
Volt'ta `prev(x)` ya da `$past(x)` yoktu; kontratlar yalnız aynı
döngünün kombinasyonel ilişkilerini söyleyebiliyordu — yani AXI'nin
AXI olduğu doğrulanamıyordu.

## Karar

### 1. Yerleşik

```volt
prev(x)      // bir önceki döngüdeki değer
prev(x, N)   // N döngü önceki değer, N >= 1 tamsayı literali
```

Tip: `prev(x: T) -> T`. `prev` prelude adıdır (`sync`, `zext` gibi),
sözdizimi değişmez. Yalnız KONTRAT bağlamında geçerlidir
(requires/ensures/invariant/cover/assert/assume — modül ve fn
kontratları). İç içe `prev(prev(x))` `prev(x, 2)` ile eşdeğerdir.

### 2. Tanı — E5017

RTL bağlamında (`let`, atama, `on`/`comb` bloğu, fn gövdesi) kullanım
E5017: "prev() yalnızca kontratlarda kullanılabilir"; öneri "RTL'de
geçmiş değer için reg kullanın: `reg x_r : T = 0; on clk { x_r <= x }`".
Gerekçe: donanımın örtük geçmişi yoktur; RTL'deki geçmiş değerin saati,
reset'i ve genişliği açık olmalıdır. Aynı kod geçersiz argümanları da
kapsar: argümansız, ikiden fazla argüman, `N < 1` ya da literal olmayan
derinlik. Denetim isim çözümlemede (`Resolver::in_contract` bayrağı),
`volt explain E5017` iki dilde.

### 3. Domain kuralı

`prev(x)` x'in saat alanında değerlendirilir (domain.rs: çağrının
alanı = ilk argümanın alanı). Kontrat başka alandan sinyal karıştırırsa
mevcut join kuralı E3001 üretir (`tests/ui/fail/38_prev_wrong_domain.volt`).
Yeni kod yok — prev bir sinyalin zamanda kaydırılmış halidir, alanı
değişmez.

### 4. SV üretimi — iki mod (sva.rs deseni)

| Volt | `--emit=sva` (Inline/Separate) | `volt verify` (Immediate) |
|---|---|---|
| `prev(x)` | `$past(x)` | `past_x_1` |
| `prev(x, 3)` | `$past(x, 3)` | `past_x_1 → past_x_2 → past_x_3` |

Yosys'in Verilog ön ucu `$past`'i desteklemez (Yosys `read_verilog -sv`
sınırı; ADR-0033'te immediate assertion'a geçilmesiyle aynı sınıf) —
bu yüzden verify akışında `$past` hiç üretilmez. Immediate modda `past.rs`
kontratlardaki her `prev()` çağrısını toplar, argüman başına tek
yardımcı register zinciri üretir ve assertion'da halkayı kullanır:

```systemverilog
logic past_b_valid_1;
always_ff @(posedge clk) begin
    if (rst) begin past_b_valid_1 <= '0; end
    else begin past_b_valid_1 <= b_valid; end
end
always @(posedge clk)
    if (!(rst)) assert (!(past_b_valid_1 && !past_b_ready_1) || b_valid); // volt:inv_5
```

Aynı argümanın farklı derinlikleri tek zinciri paylaşır (en büyük
derinlik kadar halka); bileşik argüman (`prev(a && b)`) `past_e<k>_N`
adını alır; iç içe `prev(prev(x))` içteki halkanın adından beslenir.
Emitter isim çözümlemesine erişmediğinden `prev` adıyla tanınır
(gölgeleme E1xxx'te yakalanır).

### 5. Başlangıç durumu

Reset sonrası ilk döngüde `prev(x)` tanımsızdır. SVA'da `disable iff
(rst)` zaten var; `$past`'in reset sırasındaki örneği araç
tanımlıdır. Yardımcı reg'de reset değeri 0'dır: **ilk döngüde
`prev(x) == 0` kabul edilir.** Bu, "prev(valid) && ... -> ..."
biçimindeki bütün protokol kurallarını ilk döngüde boş-doğru yapar ve
k-induction'ın tutarsız geçmiş durumlarından kaçınır (yardımcı reg
modelin parçasıdır; `initial assume (rst)` onu da sıfırlar).

## Sonuçlar

`examples/axi4lite_slave.volt` — yedi ardışık kural yazıldı:

| Kural | Kontrat türü | Neden |
|---|---|---|
| `prev(aw.valid) && !prev(aw.ready) -> aw.valid` | assume | master yükümlülüğü (giriş) |
| `prev(w.valid) && !prev(w.ready) -> w.valid` | assume | master yükümlülüğü |
| `prev(ar.valid) && !prev(ar.ready) -> ar.valid` | assume | master yükümlülüğü |
| `prev(b.valid) && !prev(b.ready) -> b.valid` | invariant | yanıt kabul edilene kadar korunur |
| `prev(r.valid) && !prev(r.ready) -> r.valid` | invariant | okuma yanıtı korunur |
| `prev(aw.valid) && prev(aw.ready) -> b.valid` | invariant | el sıkışma → yanıt bir sonraki döngüde |
| `prev(ar.valid) && prev(ar.ready) -> r.valid` | invariant | okuma el sıkışması → yanıt |

Görevdeki üçüncü kural (`prev(aw.valid) && prev(aw.ready) -> !aw.valid`,
"el sıkışma sonrası valid düşer") YAZILMADI: (a) `aw.valid` slave'in
girişidir, bir invariant olarak kanıtlanamaz (serbest giriş); (b) AXI
bunu ZORUNLU KILMAZ — master ardışık transfer için valid'i yüksek
tutabilir. Slave tarafındaki karşılığı ("el sıkışma → yanıt") yazıldı
ve kanıtlandı. İlk iki görev kuralı slave için `assume`, çünkü slave
master'ını zorlayamaz; slave'in kendi yükümlülüğü olan b/r kanalı
eşdeğerleri `invariant` olarak yazıldı.

Doğrulama: 16 kontrat (9 invariant + 3 assume + 4 cover) SymbiYosys/
boolector prove (derinlik 3), bmc (12) ve cover (12) modlarında geçti;
5 sim testi geçiyor; Verilator `-Wall` lint temiz (yardımcı reg'ler
yalnız Immediate modda üretilir, RTL çıktısında yoktur).

Testler: `tests/ui/pass/49_prev_contract.volt`,
`tests/ui/fail/37_prev_in_rtl.volt` (E5017),
`tests/ui/fail/38_prev_wrong_domain.volt` (E3001);
volt-hir/tests/prev_tests.rs (20), volt-sv-emit/tests/prev_emit_tests.rs (12).

## Sınırlar / Ertelenen

- `prev` derinliği literal olmalı (const isim V1).
- Yardımcı reg zinciri kontratın SAAT alanının reset'ine bağlanır;
  çok saatli modülde ilk saat portu esas alınır (mevcut SVA kuralı).
- `$past` (Inline/Separate) Verilator `--lint-only` ile denetlenmez;
  ticari araç hedefidir.
- Simülasyonda (`volt test`) kontratlar çalıştırılmaz; `prev` yalnız
  formal akışta etkilidir.
