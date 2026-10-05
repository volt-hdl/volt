# ADR-0103: Senkronize Reset'li Modülde Ham Reset'in Extern'e Gidişi — W3011

> Statü: Uygulandı
> İlgili: ADR-0065 (ham reset portu ve bırakma senkronizörü), ADR-0076 (extern `@source`).
> Tarih: 2026-10-06
> Etkilenen: volt-hir (`domain/rdc/extern_reset.rs` — YENİ; `domain/rdc/mod.rs`,
> `domain/rdc/facts.rs` `is_extern_instance` görünürlüğü), volt-diagnostics
> (W3011 YENİ: kod, ileti, en/tr açıklama), `tests/ui/fail/219`,
> `crates/volt-hir/tests/rdc_tests.rs`, `crates/volt-hir/tests/ui_semantic_tests.rs`,
> `crates/volt-driver/tests/extern_raw_reset_tests.rs` (YENİ),
> `book/src/limitations.md`, `CHANGELOG.md`.
> DOKUNULMADI: SV üretimi (çıktı değişmez).

## Sorun (#93)

Ham reset portu (`in rst : reset(sync, active_high)`) için Volt, portun
beslediği ve register süren her saate bir bırakma senkronizörü üretir
(ADR-0065 §2): modülün register'ları `rst_sync_<saat>_stage1` ile
sıfırlanır. Aynı ham port bir extern örneğine bağlanınca (`rst: rst`)
extern portun kendisini alır. Ölçüm (`main` 754ab305, Verilator 5.052):
`PressCounter.sv` senkronizörü üretiyor, `RisePulse det (.rst(rst), ...)`;
`verilator --lint-only -Wall` → `SYNCASYNCNET: Signal flopped as both
synchronous and async: 'rst'`. Extern'ün flop'ları yanındaki
register'lardan farklı çevrimde ve SV modülü kendisi senkronlamıyorsa
saate hizasız reset'ten çıkar. Volt hiçbir şey söylemiyordu.

## Karar

**W3011** (saat/reset ailesi, W3009/W3010'un komşusu), RDC geçidinde:
modülün bir ham reset portu, register süren (`ClockFact::used`) bir saati
besliyorsa (`feeds_of`) ve aynı port bir extern örneğinin bağlamasında
okunuyorsa (`rst: rst`, `rst` kısayolu ya da portu okuyan bir ifade),
bağlama başına bir uyarı. Birincil etiket bağlamada, ikincil etiket ham
port bildiriminde ("Volt releases 'rst' to the registers through a
synchronizer on 'clk'").

**Davranış değişmez.** Çıktı aynıdır; extern ham portu almaya devam eder.
Senkronize kopyanın extern'e nasıl verileceği (otomatik mi, yeni bir
sözdizimiyle mi, yoksa ham reset'in belgelenip bırakılması mı) v0.2'de
ayrı bir ADR ile kararlaştırılacak; #93 o karara kadar açık kalır.

Uyarı çıkmaz:
- modülün o saatte register'ı yoksa (senkronize kopya üretilmez, iki taraf
  aynı ham reset'i görür);
- extern ham portu okumuyorsa;
- ham port belirsizse (E3010; kaskad üretilmez, `check_shared_ports` ile
  aynı kural).

## Doğrulama

- Önce düşen: `rdc_tests::raw_reset_to_extern_beside_synchronized_registers_is_w3011`,
  `shorthand_raw_reset_binding_is_w3011_too`,
  `ui_semantic_tests::ui_fail_219_extern_raw_reset_w3011`,
  `extern_raw_reset_tests::build_warns_once_and_keeps_the_output`.
- Negatif: `raw_reset_to_extern_without_registers_is_not_w3011`,
  `extern_without_the_raw_reset_is_not_w3011`.
- Çıktı aynı: sürücü testi `volt build`'in çıkış 0 verdiğini, tek tanının
  W3011 olduğunu, SV'nin `rst_sync_clk_stage1`'i ve extern'e `.rst  (rst)`
  bağlamasını taşıdığını denetler.
- Depodaki izlenen `.volt` dosyalarında W3011 yalnız yeni fikstürde çıkar.
