# ADR-0025: Aritmetik Sonuçlarda Esnek Genişlik Aralığı ve W2013

> Statü: Uygulandı — tipsiz `let` telinin SV genişliği ekte (2026-10-01)
> Tarih: 2026-09-03
> Etkilenen: type-inference.md §3.3/§5 yorumu, volt-hir (typeck), W2013
> Uygulama aşaması: F2b; ek (2026-10-01): volt-sv-emit (`let_width.rs`, `expr.rs`,
> `match_expr.rs`), examples/riscv_core.volt

## Sorun

type-inference.md §3.3 aritmetik sonuçları genişletir (`u8 + u8 → u9`,
`u8 * u8 → u16`) ve §5 örtük daraltmayı DA genişlemeyi DE yasaklar. Bu iki
kural birebir uygulandığında, belge öncelik sırasında en üstte duran
Volt-UX-Anayasası'nın Seviye 0 örneği derlenemez:

```volt
reg count = 0
on clk { count <= count + 1 }   // u9 → u8: E2001?
```

Aynı gerilim bağlayıcı `tests/ui/pass` fixture'larında da görülür:

- `02_register_basic.volt` — sayaç deseni (`r <= r + 1`, hedef u8).
- `04_operator_precedence.volt` — `a as u16 + (b as u16) * (c as u16)`
  hedef u16: çarpım doğal olarak u32'dir, toplama operandı u16'dır.
- `06_let_binding.volt` — `let scaled = widened * (c as u16)` (doğal u32)
  sonra `y = scaled` (hedef u16).
- `05/07/16/18/21` — akümülatör deseni (`acc <= acc + x`, hedef operand
  genişliği).

Spec'in kendisi SALT OKUNUR olduğundan çelişki bir uygulama kararıyla
çözülmelidir.

## Seçenekler

**Seçenek A — Katı harfiyen uygulama**
`u9 → u8` her yerde E2001. Anayasa'nın Seviye 0 örneği ve 8 pass fixture'ı
derlenmez; belge öncelik sırası (Anayasa > spec) ihlal edilir. Elenmiştir.

**Seçenek B — Örtük daraltmayı serbest bırakmak**
Verilog'un sessiz kesmesine geri dönülür; `u16` portu `u8`'e atamak da
geçerli olur. "Sessiz veri kaybı yok" ilkesi (Anayasa) çöker. Elenmiştir.

**Seçenek C — Esnek genişlik aralığı (SEÇİLDİ)**
Aritmetik sonuç, iç gösterimde bir genişlik ARALIĞI taşır:
`[işlem genişliği, doğal genişlik]`. `u8 + u8` sonucu `[8, 9]`'dur;
kullanıcıya her zaman doğal genişlik (`u9`) gösterilir. Atanabilirlik,
hedef genişliği aralığın içindeyse geçerlidir: `u9` hedefe tam genişlik,
`u8` hedefe taşma biti atılarak (sayaç sarması) uyar. `u10` gibi aralık
dışı hedefler E2001 olarak kalır. Bildirilen tipler (port, wire, reg,
açık `let`) her zaman somut tek genişliktir; aralık yalnız anonim
aritmetik sonuçlarında ve tipsiz `let` bağlamalarında yaşar.

## Karar

Seçenek C uygulanır (`Ty::UIntFlex { lo, hi }` / `Ty::SIntFlex`):

- Operand uyumu aralık KESİŞİMİYLE kurulur: `u16 + (u16*u16 → [16,32])`
  işlem genişliği 16'da buluşur; kesişim boşsa E2001 (fail/02 korunur).
- Toplama/çıkarma doğal genişliği 1 bit, çarpma iki kat büyütür;
  bölme/mod genişletmez; sonuç `MAX_WIDTH` ile sınırlıdır (§3.3 aynen).
- İşaret karışımı E2002, `bits<N>` aritmetiği E2004 olarak kalır.
- Bit düzeyi operatörler GENİŞLEMEZ: kesişim aralığı aynen korunur.
- Açık `as` dönüşümü ve register çıkarımı aralığı doğal genişliğe sabitler.
- Taşma bitinin atılması Verilog'un sabit sayaç pratiğiyle uyumludur ve
  KULLANICININ SEÇTİĞİ hedef genişliğe bağlıdır — sessiz değil, bildirimle
  görünürdür (`reg r : u8` yazan taşmayı bilerek atar).

## W2013 — Kaydırma Miktarı Genişliği Aşıyor

§3.3 kaydırma kuralına eşlik eden yeni uyarı: sağ operand derleme zamanı
sabiti ve sol operandın genişliğine eşit ya da ondan büyükse sonuç her
zaman 0'dır — donanımda neredeyse kesin bir hatadır ama yasal SV ürettiği
için hata değil UYARI olarak raporlanır:

```
W2013  Kaydırma miktarı genişliği aşıyor (sonuç hep 0)
```

Spec §7 tablosu salt okunur olduğundan kod bu ADR ile tanımlanır;
`scripts/check-consistency.*` kod-tanım taramasına `docs/adr/` da dahil
edilmiştir (kontrol 1/2 her iki dizini okur).

## Sonuçlar

- `volt-hir/src/ty.rs`: `UIntFlex`/`SIntFlex` varyantları; gösterim doğal
  genişliktir (`u9`), §10 test vektörleri birebir doğrulanır.
- `volt-hir/src/typeck.rs`: ikili operatör sentezinde aralık kesişimi;
  atanabilirlik, literal sığdırma, cast, bit seçimi aralık bilir.
- `tests/ui/pass` 21/21 temiz; `tests/ui/fail` 02→E2001, 08→E2002,
  09→E2004 üretir.
- Sonraki fazlar (lowering/sv-emit) esnek tipi her zaman somutlaştırılmış
  görür: atama noktasında hedef genişlik, ifade bağlamında doğal genişlik.

## Emsal

FIRRTL genişlik çıkarımı aynı "sonradan daralt" esnekliğini çözücüyle
sağlar; Verilog bağlam-genişliği kuralı atama hedefini işlem genişliğine
dahil eder. Esnek aralık, iki dünyanın kesişimini Volt'un "açık dönüşüm"
ilkesini bozmadan (bildirilen tipler somuttur) alır.

## Ek — Tipsiz `let` telinin SV genişliği (2026-10-01, `fix/book-findings-correctness`)

Bu ek kararı değiştirmez; "Sonuçlar" bölümündeki "ifade bağlamında doğal
genişlik" cümlesinin SV üretiminde eksik kalan yarısını anlatır.

### Belirti

Kitap yazılırken (PR #68) bulundu: `a`, `b` u8 iken `let s = a + b`
tip denetiminde `u8..u9` aralığındaydı ve `sum : u9 = s` kabul
ediliyordu, ama emitter teli `wire [7:0] s = a + b;` bastı. `255 + 1`
gerçek Verilator'da `sum = 0` verdi: taşma biti sessizce kayboldu. Aynı
kayıp `-`, `*` ve işaretli `-x` için, blok içi `let`'te ve tel kipinde
açılan `fn` gövdesindeki tipsiz `let`'te de vardı.

### Kök neden

Emitter tip denetimi sonuçlarını kullanmaz (ADR-0041) ve tipsiz `let`'in
genişliğini kaba çıkarımla (`max(sol, sağ)`) yeniden hesaplıyordu; bu
çıkarım aralığın ALT ucunu verir.

### Uygulama

- Tipsiz `let` (modül, blok içi, `fn` gövdesi) aralığın ÜST ucunda,
  yani doğal genişlikte bildirilir: `wire [8:0] s = 9'(a) + 9'(b);`.
  Aralık emitter'da tip denetimiyle aynı kurallarla hesaplanır
  (`let_width.rs`): ortak aralık kesişimle, `+ -` 1 bit, `*` iki kat,
  `/ % & | ^` ve kaydırma genişlemez, `-x` somut `i(N+1)`, `if` dalların
  kesişimi.
- Aralığın alt ucu saklanır; zincirli kullanımda işlem tip denetimiyle
  aynı ortak genişlikte yapılır (`let t = s ^ a` → `8'(s) ^ a`).
- Dar hedefe atama (aralık içinde, kararın izin verdiği) kesmeyi açık
  yazar: `assign sum8 = 8'(s);`.
- Taşma biti modülde hiç okunmuyorsa bildirim Verilator -Wall
  UNUSEDSIGNAL susturmasıyla sarılır (struct yaprağıyla aynı biçim).
- `examples/riscv_core.volt` adres toplamlarında 2^32'de sarmaya
  dayanıyordu (`addr >> 28 == 2`); `let addr : u32`, `let jump_tgt : u32`
  yazıldı, üretilen SV öncekiyle birebir aynı.

Örnek ve `tests/ui/pass` külliyatında (159 dosya) SV'si değişen dosyalar
yalnız taşma bitini taşıyan beş fixture'dır (06, 44, 118, 124, 125);
hepsi Verilator 5.052 `-Wall` ile uyarısız.
