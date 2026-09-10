# ADR-0035: Değişken Dizi İndeksi ve Indexed Part-Select

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-10
> Etkilenen: grammar-full.ebnf §11/§13 (PostfixSuffix, LValueSuffix),
> type-inference.md §3.5, sv-mapping.md §2/§5, volt-ast
> (ExprKind::PartSelect, LValueSuffix::PartSelect), volt-syntax
> (`+:`/`-:` tokenları), volt-hir (typeck/domain/resolve),
> volt-sv-emit (dizi reg bildirimi + part-select üretimi)
> Uygulama aşaması: F4 sonrası — riscv_core keşif raporu bulgusu

## Sorun

`examples/riscv_core.volt` (RV32I tek çevrimli çekirdek) yazımı iki
temel engel ortaya çıkardı:

1. **Değişken dizi indeksi yok.** `reg regs : [u32; 32]` bildirimi
   tip sisteminde vardı ama (a) `= [0; 32]` başlangıç literali eleman
   tipine daraltılmadığından E2003 veriyordu, (b) SV üretimi dizi
   tiplerini "gelecek özellik" sayıp sinyali hiç bildirmiyordu.
   Sonuç: 32 ayrı reg + okuma/yazma için iki 32-kollu match
   (~500 satır). Register dosyası olan HİÇBİR tasarım yazılamıyordu.

2. **Değişken bit aralığı yok.** `data[hi:lo]` sınırları derleme
   zamanı sabiti olmak zorunda (E2008); E2008'in yardım metni
   `x[i +: WIDTH]` öneriyordu ama o sözdizimi dilde yoktu. Kaydırma
   temelli dolambaçlı yollar gerekiyordu.

## Seçenekler

**Seçenek A — Çalışma zamanı sınır denetimli indeksleme**
Her değişken indekse donanım karşılaştırıcısı + tuzak sinyali üretmek.
Donanımda "panik" yoktur; alan maliyeti gizli ve öngörülemez olur.
UX Anayasası'nın "üretilen SV'de sürpriz yok" ilkesine aykırı.
Elenmiştir.

**Seçenek B — Yalnız `match` ile açılım**
Derleyicinin değişken indeksi otomatik 2^N kollu mux'a açması.
BRAM çıkarımını imkânsız kılar (sentez aracı mux ağacını RAM olarak
tanıyamaz); büyük dizilerde patlar. Elenmiştir.

**Seçenek C — SV semantiğine birebir eşleme (SEÇİLDİ)**
`arr[idx]` ve `data[i +: W]` doğrudan SV karşılığına iner. Değişken
indekste derleme zamanı denetimi YOKTUR; `idx < N` garantisi kontratla
(`invariant: idx < N`) sağlanır. SV'de sınır dışı okuma x döndürür,
yazma etkisizdir — davranış SV standardıyla aynıdır.

## Karar

### 1. Değişken dizi indeksi

- **Okuma:** `let v = arr[idx]` — `arr : [T; N]`, `idx` herhangi bir
  sayısal tip → sonuç `T`.
- **Yazma:** `arr[idx] <= value` — indeksli hedef kısmi sürücüdür
  (drivers.rs `partial`), mevcut §11.2 kuralları geçerlidir.
- **Sınır denetimi:** Sabit indekste derleme zamanı (mevcut E2006).
  Değişken indekste denetim YOK; öneri: `invariant: idx < N`.
- **Başlangıç literali:** `= [0; 32]` tekrar literali ve `= [a, b]`
  liste literali çift yönlü denetimde hedef eleman tipine daraltılır
  (check-mode; `check_int_lit` ile aynı §4 kuralı).
- **SV üretimi:** bildirim `logic [W-1:0] regs [0:N-1];` (unpacked
  boyut isimden sonra — sentez araçları BRAM/dağıtık RAM'e eşler),
  okuma `assign v = regs[idx];`, yazma `always_ff` içinde
  `regs[idx] <= value;`, reset `regs <= '{default: W'd0};`.
- Dizi tipli PORT ve `let` bu ADR'nin kapsamı dışındadır (gelecek
  özellik uyarısı sürer); yalnız `reg` bildirimleri dizilenebilir.
  İç içe dizi (`[[T; N]; M]`) desteklenmez.

### 2. Değişken tek bit seçimi

`data[i]` — `i` değişken olabilir; sonuç `bool`, SV çıktısı `data[i]`.
(Tip kuralı zaten böyleydi; bu ADR davranışı bağlayıcı kılar.)

### 3. Indexed part-select `+:` / `-:`

- **Gramer:** `PostfixSuffix`/`LValueSuffix`'e
  `"[" Expr "+:" Expr "]"` ve `"[" Expr "-:" Expr "]"` eklenir.
  `+:`/`-:` ayrı tokenlardır (`PlusColon`/`MinusColon`).
- **Semantik:** `data[i +: W]` = i'den başlayan W bit (i, i+1, ...,
  i+W-1); `data[i -: W]` = i'de biten W bit (i-W+1, ..., i).
  IEEE 1800 §11.5.1 ile birebir.
- **Tip kuralı:** taban sayısal/bits olmalı; `W` derleme zamanı
  sabiti olmalı (değilse E2008 "part-select width must be a
  compile-time constant"), `1 <= W <= taban genişliği` (değilse
  E2006). `i` değişken olabilir; `i` sabitse tüm aralık derleme
  zamanında denetlenir (E2006). Sonuç tipi `bits<W>`.
- **SV üretimi:** `data[i +: W]` / `data[i -: W]` — birebir.

### 4. Tanılar

Yeni kod YOKTUR. E2006 (sınır dışı) ve E2008 (değişken genişlik)
yeni bağlamlarda yeniden kullanılır; E2008'in aralık yardım metni
artık gerçekten var olan `x[i +: WIDTH]` sözdizimini önerir.

## Sonuçlar

- `examples/riscv_core.volt` register dosyası tek `reg regs :
  [u32; 32] = [0; 32]` bildirimi + `regs[rs1]`/`regs[rd] <= ...`
  erişimleriyle yazılır; iki 32-kollu match kalkar (~617 → ~350
  satır). `x0 == 0` kuralı `invariant: regs[0] == 0` ile doğrudan
  ifade edilir.
- Üretilen bildirim standart unpacked dizidir; Yosys ölçümü
  (synth_xilinx, 256×u32 sonda): dizinin KENDİSİ BRAM-uyumludur —
  reset döngüsü elle kaldırılınca tek RAMB18E1'e eşlenir (86 hücre).
  Volt `reg` semantiği reset değerini garanti ettiğinden (kontrat
  kanıtları buna dayanır, ör. `regs[0] == 0`) otomatik reset döngüsü
  üretilir ve bu, diziyi FF'lere açar (32×u32 register dosyası için
  doğru ve beklenen sonuç). Resetlenmeyen BÜYÜK bellekler için doğru
  araç ADR-0029'daki `Ram`/`DualPortRam` stdlib primitifleridir;
  dizi reg'ler register dosyası ölçeğindeki durum içindir.
- Dizi reset'i `'{default: v}` deseniyle DEĞİL `for` döngüsüyle
  üretilir: Yosys'in Verilog önyüzü assignment pattern'i tanımaz
  (formal akış kırılırdı); for döngüsünü iki araç da kabul eder.
- `tests/ui/pass/41_dynamic_array_index.volt`,
  `tests/ui/pass/42_indexed_part_select.volt` uçtan uca kullanımı;
  `tests/ui/fail/30_part_select_variable_width.volt` E2008'i sabitler.

## Emsal

SystemVerilog `arr[idx]` / `data[i +: W]` (IEEE 1800 §11.5),
VHDL dizileri, Chisel `Vec` dinamik indeksi, Amaranth `Array` —
tüm HDL'lerde değişken indeks çalışma zamanı semantiğiyle, sınır
garantisi doğrulama katmanıyla sağlanır.
