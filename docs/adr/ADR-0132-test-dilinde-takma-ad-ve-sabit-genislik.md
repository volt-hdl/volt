# ADR-0132: Test Dilinde Port Genişliği — Takma Ad ve Sabit İfadesi SV Üreticisiyle Aynı Tek Yerden Çözülür (#376)

> Statü: Uygulandı
> İlgili: ADR-0059 (§1 tablosunun "takma ad → bilinmiyor" ve "`uint<N>`: N düz literal ya da düz literal `const`" sınırı kapanır), ADR-0100 (sessiz yanlış istisnası: önceden geçen testi düşüren düzeltme son hanede, v0.1.3)
> Issue: #376 (kapsam incelemesi #379; ortak olgu haritası #235; değerlendirme süresi #390)
> Tarih: 2026-11-21
> Etkilenen: `crates/volt-ast/src/alias.rs` (YENİ: `alias_chain_end`), `crates/volt-ast/src/const_eval.rs` (YENİ: `eval`, `const_int`, `width`, `array_element(s)`, `Scope`), `crates/volt-ast/src/enum_layout.rs`, `crates/volt-sv-emit/src/alias.rs`, `crates/volt-sv-emit/src/expr.rs` (`Emitter::eval_const` ortak değerlendiriciye devreder, kopya kalktı), `crates/volt-sv-emit/src/{scope,const_array,lib,sim_width}.rs`, `crates/volt-sv-emit/src/structs/{mod,value}.rs` (kopya kalktı), `crates/volt-hir/src/sim_port.rs` (`scalar_width` takma adı izler, genişliği `const_eval::width` ile hesaplar; `literal_or_const` kalktı), `crates/volt-hir/src/sim_load.rs`, `crates/volt-lsp/src/hover.rs` (kopya kalktı), `tests/ui/fail/305`–`308`, `crates/volt-hir/tests/{ui_semantic_tests,test_port_width_tests,shared_width_facts_tests}.rs`, `crates/volt-driver/tests/{port_width_alias_tests,const_chain_width_tests}.rs` (YENİ), `CHANGELOG.md`.
> DOKUNULMADI: `docs/spec/` (E8512/E8510 yalnız ADR'lerde tanımlı), üretilen SV, SVA, SDC ve XDC (birleştirmeden önce ve sonra bayt bayt aynı; "Doğrulama").

## Sorun

#376: Test dili port genişliğini (`sim_port::scalar_width`) yalnız düz
tipten ve düz literal değerli `const`'tan çözüyordu. `type Nib = u4` ve
`const W : u32 = 2 + 2` ile `uint<W>` "bilinmiyor" sayılıyordu (ADR-0059
§1). SV üreticisi aynı portu 4 bit olarak biliyor ve `[3:0]` üretiyordu.
Sonuç:

- Sabit değer E8512'den kaçıyordu: `dut.a = 31` ve `assert_ne(dut.q, 300)`
  kabul ediliyordu; ikincisi her donanımda geçer.
- Hesaplanmış değer yalnız C++ depolama tipine (8 bit) göre denetleniyordu:
  31 4 bitlik porta ulaşıyor, Verilator girişi maskelemediğinden 4 bitlik
  `q` 31 okuyordu ve `assert_eq(dut.q, 31)` geçiyordu (Verilator 5.054).
- `load` eleman genişliği ve bellek uzunluğu (`[uint<W>; N]`) denetimi
  atlanıyordu (E8510).

Aynı ayrışma uzun bir sabit zincirinde de vardı. Test dili ve struct düzeni
32 düzeyde duruyor, SV üreticisinin port genişliği 64 düzeye iniyordu.
`const K0 : u32 = K1`, …, `const K32 : u32 = 4` (33 sabit) ile `uint<K0>`
SV'de `[3:0]` idi, test dilinde bilinmiyordu; 31 yazan test geçiyordu.

Kök neden aynı olgunun birden çok yerde hesaplanması (GOVERNANCE.md,
"One place per rule"; #235):

| Olgu | Kopyalar (önce) |
|---|---|
| Takma ad zinciri | `volt-sv-emit/src/alias.rs::resolve`, `volt-ast/src/enum_layout.rs::follow_aliases` (struct düzeni de bunu kullanır); test dilinde yok |
| Derleme zamanı sabiti: SV'nin port, reg ve tel genişliği, dizi uzunluğu, sabit indeks, döngü sınırı | `volt-sv-emit/src/expr.rs::Emitter::eval_const` ve `const_array.rs::const_array_element(s)`: literal, `const`, döngü değişkeni, `+ - * / %`, bit işlemleri, kaydırma, bool/karşılaştırma/mantıksal, `if`, `match`, sabit dizi indeksi; adlar kapsam zinciriyle; derinlik 64 |
| Tip/düzen sabiti: struct ve enum düzeni, testbench port genişliği (`sim_width`) | `volt-sv-emit/src/structs/value.rs::const_int`: literal, `const`, tekli eksi, `+ - * / %`, kaydırma; derinlik 32 |
| Test dilinin port genişliği, bellek uzunluğu ve düzeni | `volt-hir/src/sim_port.rs::const_value` (`%` katlamaz, derinlik 64; düzen) ve `literal_or_const` (yalnız düz literal; genişlik ve uzunluk) |
| LSP hover'daki struct düzeni | `volt-lsp/src/hover.rs::const_int` (yalnız literal ya da düz literal `const`) |

## Karar

1. **Takma ad zinciri tek yerde:** `volt_ast::alias::alias_chain_end`
   (`Ok(hedef)` zincir bitti, `Err(son)` derinlik sınırı). SV üreticisi
   (`Ok | Err` → varılan tip), enum/struct düzeni (`Ok` → hedef, sınırda
   `None`) ve test dili aynı fonksiyonu çağırır; her çağıranın sınır
   davranışı öncekiyle aynıdır.
2. **Derleme zamanı sabiti tek yerde:** `volt_ast::const_eval`. Anlam ve
   sınır, SV üreticisinin port genişliğini hesaplayan
   `Emitter::eval_const`'ınkidir: literal, `const`, döngü değişkeni, tekli
   eksi, `+ - * / %` (taşma ve sıfıra bölme değer vermez), negatif
   olmayan değerde bit işlemleri ve kaydırma, 0/1 değerli
   bool/karşılaştırma/mantıksal, `if`, `match` (enum anahtarı dahil), sabit
   dizi indeksi; derinlik 64 (`MAX_CONST_DEPTH`). Adın anlamını çağıran
   verir (`Scope`): SV üreticisi kapsam zinciriyle (döngü değişkeni,
   gölgeleme), düzenler, testbench, test dili ve LSP üst düzey
   `const`'larla (`TopLevel`). `Emitter::eval_const` ve dizi sabiti
   yardımcıları, struct değer hesabı (`structs::const_int`), `sim_width`,
   test dilinin `const_value`'su ve LSP hover bunu çağırır; kopyalar kalkar.
3. **Tip genişliği tek fonksiyon:** `const_eval::width` (en az 1 olan ve
   `u32`'ye sığan sabit). SV üreticisi (`sig_of_typeref`), testbench
   (`sim_width`) ve test dili (`scalar_width`) port genişliğini bununla
   hesaplar.
4. **Sınır aşılırsa tahmin yok:** değer `Unknown::TooDeep` olur. SV
   üreticisi E2005 verir ve help sınırı söyler (ADR-0125): "N is nested
   more than 64 levels deep …". Test dili de genişliği bilmez; derleme o
   noktada zaten durmuştur. `volt check`, `volt build` ve `volt test` aynı
   tanıları verir; test koşmaz, SV yazılmaz.
5. **Test dilinin port genişliği** (`scalar_width`): takma ad, hedef
   tipinin genişliğini taşır; `uint<W>`/`sint<W>`/`bits<W>` `width` ile,
   dizi uzunluğu `const_value` ile çözülür. Böylece E8512 (sabit),
   testbench'in `SetPortChecked` denetimi (hesaplanmış değer,
   `port_overflow`) ve `load`'un E8510 denetimi bu portlarda da çalışır.

## Doğrulama

- Önce kırmızı, sonra yeşil: `tests/ui/fail/305`–`308` (takma adlı yazım,
  const genişlikli hiç eşleşmeyen karşılaştırma, `load` eleman genişliği ve
  uzunluğu) ve `port_width_alias_tests.rs` (gerçek Verilator: issue'nun
  örneği E8512; hesaplanmış 31 iki portta da `port_overflow` ile düşer,
  önce geçiyordu; sığan değerler geçer).
- Uzun zincir (`const_chain_width_tests.rs`, gerçek Verilator): 33 ve 64
  sabitte SV `[3:0]`, sabit 31 iki E8512 alır, hesaplanmış 31
  `port_overflow` ile düşer (önce `check` 0 ile çıkıyor, test geçiyordu);
  65 sabitte `check`, `build` ve `test` aynı E2005 tanılarıyla 1 döner,
  help sınırı söyler, test koşmaz.
- `shared_width_facts_tests.rs`: ortak değerlendiricinin anlamı (her
  işleç, `if`, `match`, dizi, enum anahtarı, kapsam), tek sınırın 64
  olduğu ve tam sınırı, `Unknown`'ın iki nedeni, `width`'in kuralı.
- **SV değişmez:** `tests/ui/pass`, `tests/real_bugs` ve `examples/`
  altındaki tasarımlar `volt build --emit=sva,sdc,xdc` ile birleştirmeden
  önce ve sonra üretildi; üç kez, her birleştirme adımından sonra (main
  cd40bcc9 ve 46d98df0'a karşı takma ad ve tip/düzen sabiti; main
  ba93e8f9'a karşı `Emitter::eval_const` dahil tek değerlendirici). Son
  karşılaştırma: 202 tasarım, 262 SV, 83 SVA, 192 SDC ve 192 XDC dosyası
  bayt bayt aynı; çıkış kodları aynı (185 / 17); stderr yalnız çıktı yolu
  önekinde ayrışıyor.
- `test_port_width_tests.rs`'teki iki test ADR-0059 §1'in kapanan sınırını
  ("takma ad çözülmez") sabitliyordu; takma adın hedef genişliğini
  beklemeleri için düzeltildi, çözülemeyen tip adı durumu ayrı sınanır.

## İlke denetimi

1. **Beyan (İ1):** Yeni beyan istenmez. Port tipi (`uint<K0>`, `Nib`)
   zaten bir beyandır; test dili onu SV üreticisinin okuduğundan farklı
   okuyordu. Artık aynı fonksiyonla okunur ve denetlenir.
2. **Çıkarım (İ2):** Kural yeni bir şey çıkarmaz; genişlik bildirilen
   tipten okunur. Değer bulunamazsa (sabit değil ya da sınırı aşıyor)
   seçilmez: SV üreticisi durur (İ2.3).
3. **Anlam değişikliği (İ3):** Derleyen hiçbir tasarımın SV'si değişmez
   (derlem bayt bayt aynı). Değişenler: önceden kabul edilen test girdisinin
   reddi ya da düşmesi (ADR-0100'ün sessiz yanlış istisnası); struct
   düzeninde önceden çözülmeyen alan genişliği (`if`, `match`, bit işlemi,
   33–64 düzey) artık çözülür. Bu tasarımlar önce E0003 ile reddediliyordu;
   artık kabul ediliyorlar. Bu bir anlam değişikliği değildir.
4. **Niyet tahmini (İ4):** Uygulanmaz.
5. **Metamorfik yeniden yazım (ADR-0117):** Kural aynı genişliğin dört
   yazımını eşdeğer sayar: `u4`, `type Nib = u4`, `uint<2 + 2>` ve bir
   sabit zinciri (`uint<K0>`). Port tipini eşdeğer bir takma ada ya da
   sabite çeviren yeniden yazım eklenebilir. Katalogdaki yeniden yazımlar
   test bloğunun sonucunu karşılaştırmaz; bu ADR'de eklenmez.

## Sınırlar

- Derinlik sınırı tektir ve 64'tür: SV üreticisinin port genişliğini
  hesaplayan kopyasınınki. Struct düzeninin, testbench'in ve test dilinin
  kopyası 32'deydi ve 33–64 düzeylik zincirde SV ile ayrışıyordu.
- İki değerlendirici ayrı kalır:
  - HIR'ın tip denetimindeki değerlendiricisi
    (`volt-hir/src/consteval.rs`): önbellekli, derinlik sınırı yok, tanı
    üretir.
  - Ayrıştırıcının generic/döngü açma değerlendiricisi
    (`volt-syntax/src/parser/mono/unroll.rs`): sınırı 64.

  İkisinin de ayrı işi var (tanı, açma); birleştirmeleri #235'te. 65
  düzeylik zinciri HIR kabul eder, SV üreticisi E2005 ile durur.
- Değerlendirme önbelleksizdir: bir sabite iki kez başvuran zincirde
  (`A1 = A0 + A0`, …) süre her halkada ikiye katlanır (#390, v0.1.4).
- Generic: generic modül doğrudan DUT olamaz (E2021), generic takma ad
  sinyal tipi olamaz (E0003). Generic parametreden genişlik alan bir port
  test bloğuna yalnız generic olmayan bir sarmalayıcının somut portu olarak
  ulaşır ve o portta denetim çalışır (31 → `port_overflow`, Verilator
  5.054). ADR-0059 §1'deki "generic parametre bilinmiyor" yedeği bugün bir
  test bloğundan erişilemez.

## Sürüm (ADR-0100)

Önceden geçen bazı testler artık düşer: takma adla, sabit ifadesiyle ya da
uzun bir sabit zinciriyle verilen genişlikteki porta sığmayan değer yazan,
hiç eşleşemeyecek değerle karşılaştıran ya da belleğe sığmayan veri
yükleyen testler. Sabitse derlemede E8512/E8510 alır, hesaplanmışsa koşuda
`port_overflow` ile düşer. Bu, sessiz yanlışın düzeltmesidir ve son hanede
(v0.1.3) çıkar. Yeni tanı kodu yok; E2005'in help'i derinlik sınırında
sınırı söyler (ADR-0125). Alan genişliği `if`, `match` ya da bit işlemiyle
yazılmış struct önceden SV üreticisinde E0003 alıyordu; artık derlenir.

## Reddedilenler

- **Test dilinde ayrı bir takma ad çözücü.** Kopya sayısını üçe çıkarırdı;
  #376 tam da bu kopyaların ayrışmasından doğdu.
- **İki değerlendiriciyi ayrı tutup yalnız sınırı eşitlemek.** Anlamları da
  ayrışıyordu (`if`, `match`, bit işlemleri, kaydırma); kural tek yerde
  durmalı (GOVERNANCE.md).
- **Sınırı 32'de birleştirmek.** 33–64 düzeylik zincirle genişliği verilen,
  bugün derlenen tasarımlar E2005 alırdı; SV üreticisinin sonucu değişirdi.
- **Ortak değerlendiricinin test dilinin eski anlamını taşıması** (`%`'siz).
  SV üreticisinin sonucunu değiştirirdi (`%` ile yazılmış genişlikler).
- **Genişliği HIR tip tablosundan almak.** Test denetimi isim
  çözümlemeden bağımsız çalışır (ADR-0033, kardeş dosya kuralı); bu
  yapıyı değiştirmek #235'in işi.
