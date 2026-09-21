# ADR-0062: Trit SV Eşlemesi — İşaretli 2 Bit ve Çarpansız Ternary MAC

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-21
> Etkilenen: volt-sv-emit (`trit.rs` YENİ — `is_trit`, `try_emit_trit_mul`,
> `trit_sum_sig`; `lib.rs` `sig_of_typeref`, `trits` kümesi, `error`
> tekilleştirmesi; `expr.rs` `arith_ctx`), examples/hybrid_accel/,
> tests/ui/pass/82_trit_emit.volt.
> DOKUNULMADI: docs/spec/ (salt okunur), volt-hir (tip kuralları aynı).
> İlgili: ADR-0003 (Trit tipi, 2 bit işaretli depolama), ADR-0041
> (aynı-işaret genişleme, `W'(x)`), ADR-0057 (SV önceliği).

## Sorun

`Trit` ayrıştırılır, tip denetiminden (type-inference.md §3.3) ve saat
alanı denetiminden geçer; `volt check` 0 hata verir. SV üreticisi ise
her `Trit` tip başvurusunda E0003 veriyordu ("SV mapping of the 'Trit'
type is not supported"). Sonuç: `examples/hybrid_accel` ağırlığı `i2`
olarak taşımak ve seçiciyi elle yazmak zorunda kaldı; dilin vitrindeki
ternary MAC kuralı üretilen donanıma ulaşmıyordu.

Yan hata: aynı port için E0003 İKİ kez basılıyordu.

## Karar

1. **Depolama:** `Trit` → `logic signed [1:0]`, ikiye tümleyen i2
   (ADR-0003 "2 bit işaretli depolama"): +1 = `2'sb01`, 0 = `2'sb00`,
   -1 = `2'sb11`. `2'sb10` (-2) Volt kaynağından üretilemez (literal
   E2011, sayısal → Trit E2009); yalnız serbest giriş (formal araç,
   dış sürücü) taşıyabilir.
2. **Çarpma:** `Trit * x` (x: `iN` ya da `Trit`, her iki sırada) çarpan
   ÜRETMEZ:
   ```
   t == 2'sb01 ? x : (t == 2'sb11 ? -(x) : <W>'sd0)
   ```
   x ve sıfır, çarpımın etkin bağlamında (ADR-0041: max(kendi, hedef))
   basılır; `-(…)` parantezi `-9'(x)` biçiminin Yosys'te boyut −9 cast
   olarak okunmasını önler. Kullanılmayan `2'sb10` kodu 0'a gider.
3. **Diğer işlemler** kodlama sayesinde sıradan işaretli aritmetiktir:
   `-t` (2 bit), `t as iN` (işaret genişletme), `Trit ± Trit` → i3
   (`width_of` 3 bit döndürür; +1 + +1 = +2 taşmaz).
4. **Tanıma:** Trit-lik AST'den çıkarılır (üretici HIR tiplerini
   görmez): Trit tipli port/wire/reg/let adı, `[Trit; N]` elemanı,
   Trit tipli üst düzey `const`, `-t`, `Trit * Trit`, iki kolu Trit olan
   `if`. Tanınmayan biçim (ör. örnek portu `inst.t`) jenerik `*` üretir:
   işaretli i2 kodlamasında aritmetik olarak yine doğrudur, yalnız
   çarpan sentezlenir.
5. **Tanı tekilleştirme:** `Emitter::error` birebir aynı tanıyı (kod,
   ileti, konum, çözüm) ikinci kez eklemez. Kök neden: port tipi hem
   sembol tablosu geçişinde hem port bildiriminde `sig_of_typeref`'ten
   geçer ve her çağrı tanı basar. `sim/test_build.rs` `unreported()`
   (ADR-0060 PR'ı) yalnız `volt test` yolunu kapsıyordu; bu yol
   `volt build`'dir (açık `reset` portu, kullanıcı tipi portlar da
   etkileniyordu).

## Doğrulama

- 2304 girişlik kapsamlı eşdeğerlik (Verilator 5.050): w, a ∈ {-1,0,1},
  x ∈ [-128, 127]; `x * w` (i8), `w * x` → i16, `w * a`, `w - a` (i3)
  SV `*`/`-` referansıyla birebir, 0 hata.
- Yosys 0.36 `proc; opt; stat` (TernaryPe): `$mul` yok — 2 `$eq`,
  2 `$mux`, 1 `$neg`.
- `examples/hybrid_accel` gerçek `Trit` ile: Verilator `-Wall` 7 modülde
  temiz, `volt test` 6/6, `volt verify --mode bmc --depth 16` TernaryPe
  6 + TernaryArray 3 özellik.

## Alternatifler

- **Jenerik `*` bırakmak** (yalnız tip eşlemesi): doğru ama 2×N bitlik
  çarpan sentezler; ternary'nin tek donanım gerekçesi çarpansızlıktır.
- **`case` bloğu:** `always_comb` + geçici sinyal gerektirir; ifade
  içinde (let/assign/`<=`) kullanılamaz. Üçlü operatör aynı `$mux`
  ağacını verir.
- **Bit seçimiyle kod çözme** (`t[0] ? (t[1] ? -x : x) : 0`): bileşik
  Trit ifadesinde (`(a * b) * x`) SV bit seçimi yazılamaz; karşılaştırma
  her ifadede çalışır.
- **HIR tiplerini üreticiye taşımak:** doğru ama kapsamı büyük (üretici
  bugün yalnız AST görür); AST çıkarımı tüm yerleşik biçimleri kapsıyor,
  kaçan biçim doğru (yalnız çarpanlı) sonuç verir.

## Bilinen sınırlar

- Örnek portundan okunan Trit (`inst.t * x`) jenerik `*` üretir.
- `volt test` Trit portunu sürmek için negatif literal gerektirir;
  test dili bunu desteklemediğinden Tb sarmalayıcıları bool girişlerden
  Trit kurar (hybrid_test.volt).
- Planlanan FPGA verimsizlik uyarısı (Dil-Spesifikasyonu-v3 §3.5) için
  hâlâ kod tanımlı değil (ADR-0003 "Sonuçlar").
