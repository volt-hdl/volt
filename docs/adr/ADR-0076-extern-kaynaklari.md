# ADR-0076: extern Modül Kaynakları — `@source`, Extern İçi CDC ve Flop'suz Modülün Reset Portu

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-24
> Etkilenen: volt-hir (`extern_source.rs` — YENİ: `@source` biçim, dosya
> ve kullanım denetimi, `resolve_in_project`, `FsSourceLocator`;
> `pipeline.rs` `pre_resolve_checks`), volt-syntax (`KNOWN_ATTRIBUTES`
> `source`), volt-driver (`extern_stage.rs` — YENİ; `main.rs`
> `compile_all` E1012; `sim/run_cmd.rs`, `sim/test_cmd.rs`,
> `sim/test_build.rs`, `verify.rs`), volt-sv-emit (`sby.rs`
> `sby_config_tasks` extern dosyaları; `lib.rs` kullanılmayan reset
> portu susturması), volt-lsp (`analysis.rs` E1012 paritesi),
> volt-diagnostics (E1012 YENİ), `docs/spec/cli-contract.md` (§7, §8,
> §8a notları), testler, `tests/fixtures/extern_source/`.
> DOKUNULMADI: README.md, docs/research/, examples/.
> Kapatır: ADR-0071 §"Sınırlar" (run/test/verify extern SV kaynağını
> bilmez; flop'suz modülde kullanılmayan `rst`), ADR-0072 §"Sınırlar"
> (hedefli SDC extern içi CDC'yi bilmez — karar §2).

## Sorun

ADR-0071 extern örneğini SV'ye indirdi: `volt build` bir extern'ü
`ExtFoo f (.a(x), ...)` olarak örnekler; gövde kullanıcınındır. Ama
`volt run`/`test` tasarımı Verilator'a, `volt verify` sby'ye yalnız
ÜRETİLEN SV ile veriyordu. Ölçüm (`main` ikilisi,
`tests/fixtures/extern_source/ext_top.volt`): `volt verify` → Yosys
"Module `\ExtDelay' referenced in module `\ExtTop' in cell `\r' is not
part of the design" → sby `ERROR` → "tool error", çıkış 3; `@source`
tanınmadığı için W0020. `volt run`/`test` aynı nedenle (Verilator'a
yalnız `ExtTop.sv` verilir) tanımsız modülle derlenemez. extern kullanan
bir tasarım simüle ya da doğrulanamıyordu.

## 1. Mekanizma: `@source("...")` niteliği (B)

```volt
@source("rtl/ext_ops.sv")
extern module ExtInvert {
    in  a : u8
    out y : u8
}
```

Seçenekler: (A) `Volt.toml [extern] sources = [...]`, (B) nitelik,
(C) ikisi. **Karar B.**

- Kaynak, extern'ün kendi özelliğidir: bildirimi okuyan gövdenin nerede
  olduğunu aynı yerde görür. A'da liste modüllerden kopuktur — hangi
  dosyanın hangi extern'ü tanımladığı bilinmez, "`Foo`'nun kaynağı yok"
  tanısı kesin konuma işaret edemez.
- Volt.toml gerektirmez: tek dosyalık tasarımlar, `tests/ui` fikstürleri
  ve `examples/` manifest'siz çalışır. Çok dosyalı birimde her dosya kendi
  göreli yolunu yazar.
- C aynı işi iki yolla yapmak olurdu (YAGNI). Üretici IP kütüphanesi gibi
  "dizin dolusu dosya" ihtiyacı çıkarsa manifest tarafı ayrı ADR'dir.

Kurallar:

- Argümanlar bir ya da daha çok string literaldir; bir SV dosyası birden
  çok extern tanımlayabilir (aynı dosya bir kez verilir), bir extern
  birden çok dosyaya yayılabilir.
- **Yol**, niteliği taşıyan `.volt` dosyasına göre göreli çözülür ve
  proje kökünün dışına çıkamaz. Kök `read_hex` ile aynıdır (ADR-0058):
  en yakın Volt.toml'un dizini (arama ADR-0061 tavanıyla), yoksa dosyanın
  dizini. Mutlak yol, kökü aşan `..`, sembolik bağla kaçış reddedilir.
  Sözcüksel kural (`normalize_data_path`) ve gerçek yol denetimi tek
  fonksiyondadır (`volt_hir::extern_source::resolve_in_project`); sürücünün
  `read_hex` yükleyicisi de aynı kuralı kullanır.
- **Tanılar** — sessizlik yok:
  - biçim (her ortam, editör dahil): nitelik extern dışında ya da
    argümanı string değil → E0009;
  - dosya (`check`, `build`, editör): yol projeden çıkıyor ya da dosya
    yok → **E1012** (YENİ, çözümleme ailesi — E1011 "modül bulunamadı"
    komşusu);
  - kullanım (`run`, `test`, `verify`): örneklenen extern'ün `@source`'u
    yok → **E1012**, çıkış 1. `build`/`check` bunu VERMEZ: yalnız
    örneklemeyi üretirler, gövdeye ihtiyaçları yoktur — `@source`'suz
    mevcut tasarımların çıktısı değişmez. Örneklenmeyen extern de sorun
    değildir.
- **Araca verme**: dosyalar aracın çalışma dizinine `extern_<ad>` olarak
  kopyalanır (Docker sarmalayıcıları yalnız çalışma dizinini bağlar;
  mutlak Windows yolu konteynerde yoktur) ve üretilen SV'den ÖNCE
  verilir: Verilator girdi listesi (`.vlt` → extern → tasarım), sby
  `[script]` `read -formal` sırası ve `[files]`. Aynı adlı iki farklı
  dosyaya sıra eklenir (`extern_1_fifo.sv`).

Uçtan uca (Docker, gerçek araçlar; `tests/fixtures/extern_source`):
`volt test` 1/1 geçti; `volt run` extern çıktıları doğru; `volt verify`
bmc ve prove (derinlik 4) geçti — invariant `(inv ^ d) == 255` gövde
olmadan kanıtlanamaz. Gövde bozulunca (`assign y = a`) verify E5001 (6),
test izleyicisi kontrat ihlali (5); `@source` silinince üç komut E1012 (1),
`build` 0.

## 2. Extern içi CDC ve SDC (3.2) — kısıt üretilmez

Extern'ün sembolik alanları (`@Src`/`@Dst`) bilinir ve örneklemede gerçek
alanlara bağlanır (ADR-0047); geçiş extern'ün İÇİNDEDİR. ADR-0065 hedefli
kısıtları (`set_max_delay`/`set_false_path`) üretilen senkronizörün
hücrelerini adlandırır: `-from`/`-to` uç noktaları bilinir. Extern'ün iç
register'larının adları Volt'a görünmez; üretilebilecek tek kısıt
`-through [get_pins f/*]` gibi bir tahmin olurdu — yanlış uç noktası
gerçek bir ihlali gizler ya da araçta sessizce eşleşmez. Üretici IP'leri
kendi kısıtlarıyla gelir (ör. FIFO çekirdeklerinin XDC'si).

**Karar: kısıt üretilmez; mevcut davranış korunur ve belgelenir.**
`--sdc-style=targeted` (varsayılan) saat grubu yazmaz: iki alan arası
yollar zamanlanır, extern içindeki geçiş zamanlama aracında RAPORLANIR,
GİZLENMEZ (SDC başlığı bunu zaten söyler: "a crossing without a
synchronizer is reported by the timing tool, not hidden").
`--sdc-style=clock-groups` alanları asenkron gruplar; o stilde bütün
alanlar arası yollar gibi bu yol da zamanlanmaz (stilin tanımı,
ADR-0065). SDC çıktısı bu ADR ile değişmedi.

## 3. Flop'suz modülün reset portu (3.3) — kalır, Verilator susturulur

Alanında reset olan (varsayılan alan dahil) ama register'ı olmayan modül
otomatik `rst` portunu taşır ve okumaz (ör. `ui/pass/62` Bridge);
Verilator `-Wall` `UNUSEDSIGNAL` verir.

Seçenekler: kaldır / bırak / uyar. **Karar: port KALIR; yalnız o port
satırı `// verilator lint_off UNUSEDSIGNAL` ile sarılır.**

- Arayüz bildirimden (saat portları + alanın reset yapılandırması)
  türemeli, gövdeden değil (ADR-0012 öngörülebilirlik/ECO). Kaldırmak
  modüle ilk register eklendiğinde portu geri getirir: Volt dışı SV
  entegratörünün örneklemesi kırılır. Volt içi üst modüller tutarlı
  kalabilirdi, dış dünya kalamaz.
- Uyarı reddedildi: port örtüktür, kullanıcının düzeltebileceği bir şey
  yoktur — eyleme dönüşmeyen gürültü.
- Susturma ADR-0072'nin "okunmayan örnek çıkışı" desenidir. Kullanım
  üretilen metinden ölçülür (gövde + gömülü/ayrı SVA'da `rst` adı);
  reset'i yalnız çocuğa geçiren ara modül (`.rst(rst)`) kullanıyor sayılır.

Verilator 5 `-Wall` (Bridge, `main` → bu dal): `UNUSEDSIGNAL 'rst'` → yok.

## Test ve doğrulama

- `volt-hir/src/extern_source.rs` birim testleri (biçim, port/deyimde
  E0009, kullanım yalnız örneklenen extern, proje dışı/yok/dizin yolu).
- `volt-driver/tests/extern_source_tests.rs`: ui/fail 99 (E1012 dosya
  yok), ui/fail 100 (E0009 modülde), proje dışı yol (Volt.toml kökü), build
  kaynak istemez ve çıktısı aynı, run/test/verify kaynaksız E1012, verify
  `.sby` sırası ve kopya, run/test Verilator argüman sırası (argüman döken
  sahte Verilator), gerçek Verilator/sby (PATH'te yoksa atlanır).
- `extern_stage.rs` birim testleri (aynı adlı dosyalar, kopyalanamayan
  dosya); `sby.rs` extern `[script]`/`[files]` sırası; `emit_tests.rs`
  flop'suz modül / register'lı modül / reset'i çocuğa geçiren modül.
- Mutasyon (`build/cleanup/mutate3.py`): 12/12 yakalandı.
- Golden (ADR-0075 dalının ikilisi → bu dal, 381 dosya): build farkı
  YALNIZ 3.3 susturma satırları — `tests/ui/pass/17`, `18`, `20`, `62`,
  `tests/ui/multifile/basic/{lib,main}`, `multifile/pubpriv/lib`,
  parite `d08c`, `d08f`, `d13b`, `d13c`, `d17` (hepsi flop'suz). `examples/`
  değişmedi. Check farkı: W0020 "known attributes" listesinde `@source`
  (p19) ve fikstürün önceki W0020'si.

## Sınırlar

- `volt build` extern kaynaklarını `build/rtl/`'e kopyalamaz (entegrasyon
  kullanıcının akışında); gerekirse ayrı karar.
- CI'ın Verilator/formal işleri yalnız belirli test ikililerini ve
  örnekleri koşar; `extern_source_tests`'in gerçek araç testleri CI'da
  ancak iş listesine eklenince koşar (`.github/` bu ADR'nin kapsamı
  dışında) — araçsız testler her ortamda koşar.
- Generic extern hâlâ E0003 (ADR-0071).
