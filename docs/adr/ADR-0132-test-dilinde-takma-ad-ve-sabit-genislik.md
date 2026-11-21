# ADR-0132: Test Dilinde Port Genişliği — Takma Ad ve Sabit İfadesi SV Üreticisiyle Aynı Tek Yerden Çözülür (#376)

> Statü: Önerildi
> İlgili: ADR-0059 (§1 tablosunun "takma ad → bilinmiyor" ve "`uint<N>`: N düz literal ya da düz literal `const`" sınırı kapanır), ADR-0100 (sessiz yanlış istisnası: önceden geçen testi düşüren düzeltme son hanede, v0.1.3)
> Issue: #376 (kapsam incelemesi #379; ortak olgu haritası #235)
> Tarih: 2026-11-21
> Etkilenen: `crates/volt-ast/src/alias.rs` (YENİ: `alias_chain_end`), `crates/volt-ast/src/const_eval.rs` (YENİ: `const_int`), `crates/volt-ast/src/enum_layout.rs`, `crates/volt-sv-emit/src/alias.rs`, `crates/volt-sv-emit/src/structs/{mod,value}.rs` (kopyalar kalktı, ortak fonksiyona devreder), `crates/volt-hir/src/sim_port.rs` (`scalar_width` takma adı izler; `const_value`/`const_u32` ortak değerlendiriciden; `literal_or_const` kalktı), `crates/volt-hir/src/sim_load.rs`, `tests/ui/fail/301`–`304`, `crates/volt-hir/tests/{ui_semantic_tests,test_port_width_tests}.rs`, `crates/volt-driver/tests/port_width_alias_tests.rs` (YENİ), `CHANGELOG.md`.
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

Kök neden aynı olgunun birden çok yerde hesaplanması (GOVERNANCE.md,
"One place per rule"; #235):

| Olgu | Kopyalar (önce) |
|---|---|
| Takma ad zinciri | `volt-sv-emit/src/alias.rs::resolve`, `volt-ast/src/enum_layout.rs::follow_aliases` (struct düzeni de bunu kullanır); test dilinde yok |
| Tip/düzen sabiti (genişlik, uzunluk, enum değeri) | `volt-sv-emit/src/structs/value.rs::const_int` (`%` katlar, derinlik 32), `volt-hir/src/sim_port.rs::const_value` (`%` katlamaz, derinlik 64), `volt-hir/src/sim_port.rs::literal_or_const` (yalnız düz literal; test dilinin port genişliği ve bellek uzunluğu) |

## Karar

1. **Takma ad zinciri tek yerde:** `volt_ast::alias::alias_chain_end`
   (`Ok(hedef)` zincir bitti, `Err(son)` derinlik sınırı). SV üreticisi
   (`Ok | Err` → varılan tip), enum/struct düzeni (`Ok` → hedef, sınırda
   `None`) ve test dili aynı fonksiyonu çağırır; her çağıranın sınır
   davranışı öncekiyle aynıdır.
2. **Tip/düzen sabiti tek yerde:** `volt_ast::const_eval::const_int`;
   anlamı SV üreticisinin kopyasınınkidir (literal, üst düzey `const`,
   tekli eksi, `+ - * / %`, kaydırma; derinlik 32). Test dili (`const_value`,
   genişlik ve uzunluk için `const_u32`) bunu kullanır. `literal_or_const`
   kalkar.
3. **Test dilinin port genişliği** (`scalar_width`): takma ad, hedef
   tipinin genişliğini taşır; `uint<W>`/`sint<W>`/`bits<W>` ve dizi
   uzunluğu `const_value` ile çözülür. Böylece E8512 (sabit), testbench'in
   `SetPortChecked` denetimi (hesaplanmış değer, `port_overflow`) ve
   `load`'un E8510 denetimi bu portlarda da çalışır. Generic parametre ve
   generic takma ad bilinmiyor kalır (ADR-0059 §1'deki C++ depolama tipi
   yedeği).

## Doğrulama

- Önce kırmızı, sonra yeşil: `tests/ui/fail/301`–`304` (takma adlı yazım,
  const genişlikli hiç eşleşmeyen karşılaştırma, `load` eleman genişliği ve
  uzunluğu) ve `port_width_alias_tests.rs` (gerçek Verilator: issue'nun
  örneği E8512; hesaplanmış 31 iki portta da `port_overflow` ile düşer,
  önce geçiyordu; sığan değerler geçer).
- **SV değişmez:** `tests/ui/pass`, `tests/real_bugs` ve `examples/`
  altındaki 201 tasarım `volt build --emit=sva,sdc,xdc` ile birleştirmeden
  önce (main cd40bcc9) ve sonra üretildi: 262 SV, 83 SVA, 192 SDC ve 192 XDC
  dosyası bayt bayt aynı; çıkış kodları ve tanılar (yol öneki dışında)
  aynı.
- `test_port_width_tests.rs`'teki iki test ADR-0059 §1'in kapanan sınırını
  ("takma ad çözülmez") sabitliyordu; takma adın hedef genişliğini
  beklemeleri için düzeltildi, çözülemeyen tip adı durumu ayrı sınanır.

## Sürüm (ADR-0100)

Önceden geçen bazı testler artık düşer: yalnız takma adlı ya da sabit
ifadeli genişlikteki porta sığmayan değer yazan, hiç eşleşemeyecek değerle
karşılaştıran ya da belleğe sığmayan veri yükleyen testler (sabitse
derlemede E8512/E8510, hesaplanmışsa koşuda `port_overflow`). Sessiz
yanlışın düzeltmesidir; son hanede (v0.1.3) çıkar. Yeni tanı kodu yok.

## Reddedilenler

- **Test dilinde ayrı bir takma ad çözücü.** Kopya sayısını üçe çıkarırdı;
  #376 tam da bu kopyaların ayrışmasından doğdu.
- **Ortak değerlendiricinin test dilinin anlamını taşıması** (`%`'siz,
  derinlik 64). SV üreticisinin sonucunu değiştirirdi (`%` ile yazılmış
  genişlikler); bu ADR SV'yi değiştirmez.
- **Genişliği HIR tip tablosundan almak.** Test denetimi isim
  çözümlemeden bağımsız çalışır (ADR-0033, kardeş dosya kuralı); bu
  yapıyı değiştirmek #235'in işi.
