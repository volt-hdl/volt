# ADR-0071: `extern module` Örneklemesinin SV Eşlemesi ve ui/pass Build Denetimi

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-24
> Etkilenen: volt-sv-emit (`instance.rs` — `InstTarget` (modül | extern),
> `inst_target_named`, `extern_conns`; `alias.rs` — generic extern E0003
> metni), `tests/fixtures/parity/p06_extern_instance.volt` (E0003 → ok),
> `crates/volt-sv-emit/tests/extern_emit_tests.rs` (YENİ),
> `crates/volt-driver/tests/ui_pass_build_tests.rs` (YENİ).
> DOKUNULMADI: examples/, README.md, docs/spec/, docs/research/.
> Genişletir: ADR-0047 (§"Sınırlar": sv-emit extern örneğini üretemiyordu).

## Sorun

`tests/ui/pass/62_extern_domains.volt` — ADR-0047'nin (extern modül
sınırında domain denetimi) tek "geçerli" fixture'ı — hiç derlenmiyordu:

```
error[E0003]: not supported yet: instances of extern module 'ExtAsyncFifo' (instance 'f'; ADR-0047)
error[E0003]: not supported yet: instances of extern module 'ExtDelay' (instance 'dly'; ADR-0047)
```

Kök neden: `instance.rs::module_decl_named` yalnız `ItemKind::Module`
arıyordu; extern hedefli örnek "hedef yok" koluna düşüp E0003 alıyordu.
ADR-0047 bunu "Sınırlar"da not etmişti. Sonuç: domain denetimi (E3001,
E3014) çalışıyordu ama denetlenen tasarım hiçbir zaman SV'ye inemiyordu —
güvenlik özelliği uçtan uca kullanılamazdı.

Fixture'ın bu hâlde "pass" dizininde durabilmesinin nedeni: ui/pass
harness'i (`volt-hir/tests/ui_semantic_tests.rs`) yalnız HIR analizini
koşar (`volt_hir::analyze`); emit hiç çağrılmaz. ADR-0070'ten sonra
`volt check` emit doğrulamasını da koştuğu için E0003 `check`'te de
görünüyordu, ama harness `check`'i de koşmuyordu.

## Karar

### 1. Extern örneği = adlandırılmış bağlantılı SV örneklemesi

`let f = Ext { a: x, b }` → `Ext f ( .a(x), .b(b), .q(f_q) );`

- **Portlar: bildirilenlerin kendisi, bildirim sırasıyla.** Dış SV
  modülünün arayüzü `extern module` bildirimidir; Volt modüllerindeki
  saat → reset → giriş → çıkış yeniden sıralaması ve **örtük alan reset
  portu (`rst`) eklenmez**. Extern bir reset istiyorsa onu port olarak
  bildirir (`in rst_n : reset` ya da `in rst : bool`) ve açıkça bağlanır.
  Gerekçe: örtük port, kullanıcının yazmadığı ve dış modülde olmayabilecek
  bir bağlantı üretirdi — Verilator/Yosys "port bulunamadı" ile düşerdi.
- Çıkışlar Volt modülleriyle aynı: `<örnek>_<port>` teli ön bildirilir,
  `f.q` okuması tele iner. Giriş/çıkış/çift yönlü bağlama kuralları
  (E2005 dahil) ortak fonksiyonlardan (`input_binding`, `output_binding`,
  `bidir_binding`) gelir.
- Extern bildirimi için SV modülü üretilmez; kaynağını kullanıcı verir
  (Verilator/Yosys komut satırına eklenen `.sv`).
- **Generic extern** (`extern module X<T>`) E0003 kalır, ama mesaj artık
  doğru: "instances of generic extern module 'X' (… extern generics are not
  monomorphized)". Önce generic extern'i örneklemeye izin verseydik port
  tipi `T` çözülemeyip extern'in port satırlarında anlamsız
  "user-defined type 'T'" E0003'ü çıkıyordu (ölçüldü).

### 2. ui/pass fixture'ları build'den de geçmeli

`crates/volt-driver/tests/ui_pass_build_tests.rs`: her
`tests/ui/pass/*.volt` `volt build` ile (geçici hedef dizinine) derlenir,
`success == true` beklenir. İstisna yalnız gerekçeli işaretle:

```
//~ CHECK-ONLY: <neden>
```

Gerekçesiz işaret (`//~ CHECK-ONLY`, `//~ CHECK-ONLY:`) ve **build'den
geçen** işaretli dosya (bayat işaret) testi düşürür. Sayım assert'i
(`(built, check_only) == (81, 0)`) boş dizin okumasını yakalar.

Harness volt-driver'da, çünkü volt-hir emitter'a bağımlı değil (ve
olmamalı); gerçek CLI yolu (`load_unit` + ortak boru hattı + emit) aynı
anda denenir.

## Ölçüm

- 81 ui/pass dosyası `volt build`: **1 başarısız** (62). Sınıf dağılımı:

  | Sınıf | Sayı | Dosya | Çözüm |
  |---|---|---|---|
  | A — gerçek derleyici eksiği | 1 | 62_extern_domains | sv-emit extern eşlemesi (bu ADR) |
  | B — aslında fail olmalı | 0 | — | — |
  | C — bilinçli yalnız-check | 0 | — | — |

  Sonuç: `CHECK-ONLY` işaretli dosya yok.
- Depoda extern kullanan dosyalar: `ui/pass/62`, `ui/fail/49` (E3001),
  `ui/fail/50` (E3014), `fixtures/parity/p06`. `examples/` içinde yok.
- Uçtan uca (62 + boş stub'lar `ExtAsyncFifo.sv`, `ExtDelay.sv`):
  `volt check` temiz; `volt build` `Bridge.sv` üretir; `verilator
  --lint-only -Wall` bağlantı uyarısı yok (PINMISSING/PINNOTFOUND/WIDTH
  yok). Tek uyarı `UNUSEDSIGNAL rst`: Bridge'in kendi flop'u yok ama
  alanların reset'i olduğundan `rst` portu üretilir — extern'den bağımsız,
  mevcut modül davranışı (aşağıda "Sınırlar").
- Kasıtlı ihlal (`ui/fail/49`: `wr_data` PixDomain'den) → E3001 hâlâ.
- `--emit=sva` ve `--emit=sdc` 62'de hatasız.
- Mutasyon: harness'te build sonucu yok sayılınca geçici bir
  derlenemeyen pass fixture'ı (generic extern örneği; HIR kabul eder,
  emit reddeder) **yakalanmadı**; geri konunca yakalandı.

## Sınırlar

- Hedefli SDC (ADR-0065) extern'in içindeki CDC'yi bilmez: 62'de
  SysDomain → PixDomain yolu extern FIFO'dan geçer ama `set_max_delay`
  üretilmez; zamanlama aracı yolu raporlar (gizlemez). Extern CDC
  primitifinin kısıtı kullanıcının SDC'sindedir.
- `volt run` / `volt test` / `volt verify` extern SV kaynağını
  bilmez: Verilator/Yosys komutuna dış dosya eklenmediğinden extern
  içeren tasarım bu akışlarda araç hatasıyla düşer. Dış dosya
  referansı (ör. `Volt.toml [extern] sources`) ayrı karar ister.
- Flop'suz ama reset'li alanlı modül kullanılmayan `rst` portu taşır
  (Verilator `UNUSEDSIGNAL`). Portu kaldırmak modül arayüzünü ve
  geçerli tasarımların çıktısını değiştirir; bu ADR'nin kapsamı dışında.

## Test

- `crates/volt-sv-emit/tests/extern_emit_tests.rs` — 8 test: bildirim
  sırası, örtük reset yok, çıkış ön bildirimi, extern için SV modülü yok,
  inout/ham reset/dizi port, bağlanmamış giriş E2005, generic extern
  E0003 (tek tanı, örnekte), ui/pass/62.
- `crates/volt-driver/tests/ui_pass_build_tests.rs` — 2 test.
- `tests/fixtures/parity/p06_extern_instance.volt` başlığı `ok`
  (parite testi artık check + build temizliğini ister).
