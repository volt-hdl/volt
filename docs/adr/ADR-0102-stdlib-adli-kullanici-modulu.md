# ADR-0102: Standart Kütüphane Adını Taşıyan Kullanıcı Modülü — E1016

> Statü: Uygulandı
> İlgili: ADR-0027 (yerleşik primitifler ada göre tanınır), ADR-0076 (extern `@source`; bu ADR stdlib adlı extern'ü reddeder).
> Tarih: 2026-10-06
> Etkilenen: volt-hir (`resolve/stdlib_name.rs` — YENİ; `resolve/collect.rs`
> extern bildirimi, `resolve/stmt.rs` örnekleme), volt-diagnostics (E1016
> YENİ: kod, ileti, en/tr açıklama), `tests/ui/fail/215-218`,
> `crates/volt-driver/tests/stdlib_name_tests.rs` (YENİ), kitap
> (`cookbook/existing-project.md`), `README.md`, `CHANGELOG.md`.
> Kapatır: #92.

## Sorun

Yerleşik primitifler (ADR-0027) SV üretiminde ve birkaç denetimde
örneklemenin ADINA göre tanınır (`BuiltinPrim::from_name`). Çözümleyici
"kullanıcı aynı adla modül tanımlarsa o kazanır" der, ama emit ve
denetimler kullanıcı tanımına bakmaz. Ölçüm (`main` 855b73a1):

| Girdi | Sonuç |
|---|---|
| `extern module EdgeDetect { in clk, in signal, out rise, out fall }` (yerleşikle aynı portlar), örneklenmiş | `volt build` çıkış 0, uyarı yok; SV yerleşik kenar algılayıcıyı açar ve tanımsız `e_rise`/`e_fall` tellerini okur; `@source` dosyası hiç kullanılmaz |
| aynı ad, farklı portlar (`level`/`rise`) | E4011 "port 'signal' ... is not bound" — kullanıcının yazmadığı port |
| `extern module SyncFifo` / `extern module Counter` | E0003 "'SyncFifo' without a <T> type argument" / "'Counter' without its generic arguments" |
| Volt `module EdgeDetect`, başka modülde örneklenmiş | `volt build` çıkış 0; yerleşik açılır, kullanıcı modülü örneklenmez |
| Volt `module PulseSync { clk, a, b }`, örneklenmiş | E4011 "port 'src_clk' of 'PulseSync' instance 'u' is not bound" |
| Volt `module SyncFifo { clk, a, b }`, örneklenmiş | E0003 "'SyncFifo' without a <T> type argument" |

Birinci ve dördüncü satır sessiz yanlış sonuçtur.

## Karar

**E1016** (isim çözümleme ailesi, E1011/E1012'nin komşusu):

1. `extern module <yerleşik ad>` bildirildiği yerde E1016. Extern'ün tek
   işi örneklenmektir; örneklenmese de ad yanlıştır. E1016 bu adda W1003'ün
   yerine geçer (tek bulgu tek kod, ADR-0075).
2. Volt `module <yerleşik ad>` yalnız başka bir modülde ÖRNEKLENDİĞİ
   yerde E1016, ikincil etiket modül bildiriminde. Bu adla üst modül
   geçerli kalır: `volt new` şablonunun `Counter`'ı, kitap ve
   `tests/fixtures/counter.volt` hiçbir modülde örneklenmez. Test
   dilindeki `let dut = Counter { }` modül örneklemesi değildir (ayrı yol,
   `sim::check_tests`) ve etkilenmez.

Yerleşik adlar `BuiltinPrim::from_name`'in tanıdıklarıdır (`volt explain
stdlib` listeler). HIR hatası emit'ten önce derlemeyi durdurduğu için
yerleşiğin E0003 ve E4011 zinciri artık çıkmaz.

**Öneri metni.** Extern'ün adı SV modülünün adıdır: `Counter` adlı mevcut
bir SV modülü doğrudan bildirilemez. Yardım satırı ve `volt explain E1016`
başka adlı bir SV sarmalayıcı modül yazmayı ve onu bildirmeyi söyler.
Makine önerisi verilmez: extern'ü yeniden adlandırmak SV modülünü
yeniden adlandırmadan doğru olmaz.

### Reddedilen seçenekler

- **Kullanıcı tanımı her katmanda kazansın.** Çözümleyicinin yorumu bunu
  öngörür, ama ad eşlemesi sekiz yerde (HIR kısıtları, RDC olguları, sv-emit
  primitif, örnek, flop denetimi, ast reset zinciri) ayrı ayrı yapılır; biri
  atlanırsa sessiz yanlış geri gelir. v0.1.0 öncesi açık hata güvenlidir;
  gölgelemeye izin vermek ileride ayrı bir ADR'dir.
- **Her bildirimde E1016 (Volt modülü dahil).** Şablonun, kitabın ve
  fikstürlerin `module Counter`'ı kırılırdı; sorun yalnız örneklemededir.

## Doğrulama

- Önce düşen: `stdlib_name_tests::every_fixture_reports_e1016_alone_on_the_marked_line`
  (215: çıkış 0, hata yok; 216/217: E0003; 218: çıkış 0) ve
  `the_explanation_names_the_rule_and_the_workaround` (E1016 yok, çıkış 2).
  Sonra: dört fikstür `check` ve `build`'de yalnız E1016, işaretli satırda.
- `a_top_module_named_like_the_stdlib_stays_valid`: örneklenmeyen
  `module Counter` temiz.
- Depodaki 681 izlenen `.volt` dosyasında E1016 çıkmaz.
- İki HIR testi "kullanıcı kazanır" kuralını yalnız HIR'da sınıyordu; aynı
  girdiler tam boru hattında düşüyordu (tablonun son iki satırı):
  `builtin_tests::user_module_named_like_builtin_wins` →
  `user_module_named_like_builtin_is_e1016_where_instantiated`,
  `stdlib_tests::user_module_named_sync_fifo_wins_over_builtin` →
  `user_module_named_sync_fifo_is_e1016_where_instantiated`.
  `extern_domain_tests::extern_port_named_like_root_item_has_no_shadow_warning`
  extern'e tesadüfen yerleşik ad `Ram` vermişti; sınadığı kural (extern
  portunda gölgeleme uyarısı yok) değişmeden ad `ExtRam` oldu.
