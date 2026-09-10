# ADR-0036: İşaretli Kaydırma (`>>>`) ve `$signed`/`$unsigned` Üretimi

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-10
> Etkilenen: sv-mapping.md §6 (operatör tablosu), volt-sv-emit
> (emit_cast, Binary/Shr emisyonu, width_of kaydırma kuralı)
> Uygulama aşaması: ADR-0035 riscv_core raporu bulgusu

## Sorun

`examples/riscv_core.volt` yazımında iki emit kısıtı dolambaçlı koda
zorladı:

1. `>>` her zaman mantıksal üretiliyordu (`>>>` yok) → SRA elle
   işaret maskesiyle (`(a >> s) | ~(ones >> s)`) yazıldı.
2. Aynı genişlikteki işaret-değiştiren cast no-op'tu (`$signed`
   üretilmiyordu) → işaretli karşılaştırmalar bias hilesiyle
   (`(a ^ 0x80000000) < (b ^ 0x80000000)`) yazıldı.

Tip sistemi her iki işlemi de doğru tipliyordu; kayıp yalnız SV
üretimindeydi — tip olarak işaretli bir değer SV'de `logic`
(işaretsiz) ifadeye düşüyor, semantik sessizce değişiyordu.

## Karar

### 1. Aritmetik kaydırma

- Tip kuralı (zaten mevcut, bağlayıcı kılındı): kaydırma sonucu SOL
  operandın tipidir — `SInt >> N → SInt` (aritmetik), `UInt/bits >>
  N` (mantıksal). Sabit değerlendirme Rust i128 `>>` ile zaten
  aritmetiktir; kural tutarlıdır.
- SV üretimi: sol operandın imzası işaretliyse `a >>> n`, değilse
  `a >> n`. `<<` işaretliden bağımsız `<<` kalır (`<<<` eşdeğerdir).
- `width_of` kaydırma sonucu için genel `max(l,r)` + `l.signed &&
  r.signed` kuralını DEĞİL sol operandın imzasını döndürür — miktar
  operandı sonucu etkilemez. Bu olmadan `((a as i32) >> n) as u32`
  cast'i kaynağı işaretsiz sanıp `$unsigned` sınırını atlıyordu.

### 2. İşaret yeniden yorumlama cast'i

- Aynı genişlikte işaret DEĞİŞİYORSA no-op değildir:
  `a as i32` → `$signed(a)`, `a as u32` → `$unsigned(a)`.
- Genişlik değişen cast'ler aynen kalır: zero-extend
  `{{N{1'b0}}, a}`, sign-extend `{{N{a[msb]}}, a}`, daraltma dilim.
- `$signed()`/`$unsigned()` çağrıları IEEE 1800 §11.8.1 gereği
  öz-belirlenimlidir (self-determined): argüman, dış ifadenin
  işaret bağlamından ETKİLENMEZ. Bu, `x ? ($signed(a) >>> n) : b`
  gibi karışık bağlamlarda `>>>`'ın mantıksala düşmesini engelleyen
  bilinçli bir sınırdır — sarmalayıcılar yalnız kozmetik değildir.

### 3. İşaretli karşılaştırma

SV'de karşılaştırma yalnız İKİ operand da işaretliyse işaretlidir.
Volt tarafında `i*` tipli operandlar ya işaretli bildirilmiş
sinyaldir (`logic signed`, doğal işaretli karşılaştırma) ya da (2)
sayesinde `$signed(...)` sarmalıdır; typeck karışık işaret
karşılaştırmasına zaten izin vermez (E2002). Ek mekanizma gerekmez:
`(a as i32) < (b as i32)` → `$signed(a) < $signed(b)`.

## Sonuçlar

- riscv_core.volt dolambaçları kalktı: SRA `((rs1_v as i32) >>
  shamt) as u32`, SLT/BLT `(a as i32) < (b as i32)`, immediate'ler
  `((instr as i32) >> n) as u32` oldu; `sa/sb/sbr/ones/sra_fill`
  yardımcı wire'ları silindi (169 → 161 satır). 26 test, Verilator
  -Wall ve formal (prove/cover/bmc, 5 property) doğrulandı.
- sv-mapping.md §6 tablosu güncellendi: `>>` satırı işarete göre
  `>>`/`>>>`; işaret yeniden yorumlama cast satırları eklendi.
- `tests/ui/pass/43_signed_ops.volt` uçtan uca kullanım; emit
  testleri `>>>`/`$signed`/`$unsigned` çıktısını ve genişleyen
  cast'lerin değişmediğini sabitler.

## Emsal

Verilog-2001'den beri `>>>` + `$signed()` bu işin standart aracıdır;
VHDL `shift_right(signed)`, Chisel `SInt >> `, Amaranth `.as_signed()`
aynı ayrımı tip düzeyinde yapar ve üretimde işaretli operatöre iner.
