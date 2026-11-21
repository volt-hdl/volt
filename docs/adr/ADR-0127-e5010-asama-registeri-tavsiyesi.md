# ADR-0127: E5010'un Tavsiyesi Gerçek Aşama Register'ıdır — `delay<K>` Bir İddiadır, Hizalama Değildir (#232)

> Statü: Uygulandı
> Önceki karar: ADR-0037 — §5'te help'in içeriği: `delay<K>(...)` hizalama olarak önerilmez; help genç değer için aşama register'ını gösterir, `delay<K>`'yı en son ve bir iddia olarak anar
> İlgili: ADR-0100 (son hane: tanı metni değişir, SV ve tetikleyici değişmez), ADR-0126 (E5010'un bildirim kuralı: `let`, `wire` ve çıkış portundaki `Delayed<T, N>` denetlenir; help'i `delay<K>` önermez), ADR-0118 (`delay<K>` işaretsiz bir kaçıştır; işaret, gerekçe ve rapor v0.2.0'da: #358), ADR-0119 (İlke denetimi; İ1 denetlenmeyen beyan, İ4 tahmin yalnız söyler), ADR-0125 (tanı ilkesi: §1 üç parça, §2 (a) yol, §3 kaçış en son ve bedeliyle, §4 help'teki kod sınanır, §7 tanı metni son hanede)
> Issue: #232 (ilgili: #353, #354, #358, #122, #362)
> Tarih: 2026-11-19
> Etkilenen: `crates/volt-hir/src/timing.rs` (E5010'un iki help'i, etiketleri ve neden satırı; `stage_register_help`, `register_write_help`), `crates/volt-hir/src/{lib,pipeline}.rs` (zamanlama geçidi tip sonucunu alır), `crates/volt-diagnostics/src/explain/{en,tr}.rs` (E5010), `tests/ui/pass/44`, `tests/ui/pass/161` (YENİ), `tests/ui/fail/32` (yorum), `tests/real_bugs/232_e5010_advice_delay_adds_no_register.volt` (YENİ), `crates/volt-hir/tests/e5010_advice_tests.rs` (YENİ), `crates/volt-driver/tests/e5010_advice_tests.rs` (YENİ), `crates/volt-driver/tests/seeded_bugs/specials.rs`, ui/pass sayımları, `README.md`, `book/src/limitations.md`, `CHANGELOG.md`
> DOKUNULMADI: `docs/spec/` (type-inference.md §12 E5010'un önerisini anlatmaz; `delay<K>`'nın silindiğini zaten yazar), SV üretimi.

## Sorun

`@strict_timing` modülünde E5010'un help'i "align the younger value with
delay<K>(...)" diyordu; register kuralının help'i "align the source with
delay<K>(...)". `volt explain E5010`'un ÇÖZÜM bloğu da aynısını
öneriyordu:

```volt
let sum = delay<1>(a) + b   // ✓ both sides are 2 cycles old
```

`delay<K>` register eklemez. Parser onu soyar (yalnız iç ifade AST'de
kalır, K yan tabloya yazılır); yan tabloyu yalnız zamanlama geçidi okur.
ADR-0037 §2 ve type-inference.md §12 bunu açıkça söyler: "`Delayed` and
`delay<K>` erase at the type level … generated RTL is unchanged".

#232'nin ölçümü (main 62373205, Verilator 5.054): modül düzeyi `let`,
ifade içi, `on clk` bloğu, register bildirimi, `comb` bloğu, `wire`
ataması, çıkış portu, `pipeline` aşaması ve `fn` gövdesinin hiçbirinde
`delay<1>` register üretmez. İlk beşinde `delay<1>`'li dosyanın SV'si,
`delay<1>` ve `@strict_timing` olmadan yazılmış modülünkiyle bayt bayt
aynıdır (`// Source:` satırı dışında). Simülasyonda (`x` = 10, 20, 30, 40)
`delay<1>(a) + b` help'in "iki taraf da 2 çevrim yaşında" iddiasına göre
60 vermeliydi; 70 (= x4 + x3) verdi. Gerçek aşama register'ıyla kurulan
kontrol tasarımı 60 verdi.

Derleyici şartnameye uyar; yanlış olan tanının söylediğidir. Kullanıcı
derleyicinin kendi tavsiyesini uygular, E5010 gider, modül uyarısız
derlenir ve devre `@strict_timing`'in reddetmeyi vaat ettiği karışımı
taşır: sessiz yanlış. ÇÖZÜM bloğu ayrıca derlenmiyordu (`in clk` yok,
E1001).

## Karar

1. **E5010'un help'i önce yolu gösterir: genç değer için gerçek bir aşama
   register'ı, kodu ve bedeliyle** (ADR-0125 §2 (a)). Genç değer tek adlı
   bir sinyalse ve tipi `0` ya da `false` ile başlatılabiliyorsa kod adla
   yazılır:

   ```
   = help: add a stage register so that the 1-cycle value is 2 cycles old
     too: `reg a_d2 : Delayed<u32, 2> = 0`, write `a_d2 <= a` in the `on clk`
     block, and use `a_d2` here (cost: 1 register of 32 bits); …
   ```

   - Fark K çevrimse K register'lık bir zincir önerilir (`a_d2`, `a_d3`,
     …; üçten uzunsa ilk ve son register yazılır). Bedel "K register of
     W bits" biçimindedir.
   - Modülde tek saatin `on` bloğu varsa blok adıyla anılır; değilse
     "the `on` block of its clock".
   - Genç değer bileşik bir ifadeyse, tipi `0`/`false` ile
     başlatılamıyorsa ya da önerilen ad kullanılıyorsa help genel biçimi
     verir (`reg … : Delayed<T, N> = 0`, bedel yine yazılır).
2. **`delay<K>` en son ve bir iddia olarak anılır** (ADR-0125 §3): "is
   not such a register: it is a claim that adds no hardware and that Volt
   does not check, right only if the value really is N cycles old already
   (it does not change during …)". Hizalama olarak önerilmez.
3. **Ne oldu ve neden** (ADR-0125 §1): etiketler her değerin kaç çevrim
   yaşında olduğunu söyler ("this value is 1 cycle old"); neden satırı
   farklı çevrimlerin değerlerinin birleştiğini, genç değerin kaç çevrim
   sonra giren bir öğeye ait olduğunu.
4. **Register kuralı** (kaynak `n`, bildirim `N`, register `n + 1` tutar):
   kaynak gençse (`n + 1 < N`) önce aşama register'ı, sonra bildirimin
   düzeltilmesi, en son `delay<K>`'nın iddia olduğu; kaynak yaşlıysa
   bildirimin düzeltilmesi ya da daha önceki aşama; `N = 0` ise
   register'ın en az 1 çevrim yaşında olduğu.
5. **`volt explain E5010`:** ÖRNEK ve ÇÖZÜM tam, derlenen modüllerdir;
   ÇÖZÜM aşama register'ıyla hizalar. Metin yolu (aşama register'ı ve
   bedeli), bilinçli karışım için anotasyonlu `let` kaçışını (Volt içteki
   karışımı denetlemez) ve en son `delay<K>`'nın iddia olduğunu anlatır.
6. **Örnekler:** `tests/ui/pass/44` (README'nin `Delayed` örneği) gerçek
   aşama register'ıyla hizalar. `delay<K>`'nın kendisi
   `tests/ui/pass/161`'de kalır; yorumu bunun bir iddia olduğunu yazar.

Makine önerisi eklenmez; aşama register'ını ekleyen makine önerisi #353'tür
(v0.1.4).

## ADR-0100 ve ADR-0125 §7 gerekçesi

- Değişen yalnız tanı metnidir: help, etiketler, neden satırı ve explain.
  Yeni tanı kodu, yeni seçenek, yeni JSON alanı yoktur.
- E5010'un hangi girdide çıktığı değişmez: aynı tasarımlar reddedilir,
  aynıları kabul edilir.
- Üretilen SV, SVA, SDC ve sürücü çıktısı değişmez; `delay<K>`'nın anlamı
  da değişmez (ADR-0037 §2).
- E5010'un makine önerisi yoktur; CHANGELOG'da "Diagnostics" altında tek
  madde.
- Bu, ADR-0125 §7'nin son hane koşullarıdır: **v0.1.3'e girer.** Sessiz
  yanlışın kaynağı tanının metni olduğundan düzeltme de metindir.

## İlke denetimi

1. **Beyan (İ1):** `delay<K>` derleyicinin denetlemediği bir iddiadır:
   İ1'in "denetlenmeyen beyan, beyan olmamasından kötüdür" dediği durum.
   Bu ADR iddiayı tavsiye olmaktan çıkarır ve bedelini (Volt denetlemez)
   her anıldığı yerde yazar. İddiayı görünür bir kaçış yapmak (işaret,
   gerekçe, rapor) ADR-0118'in v0.2.0 işidir (#358); aşağıdaki açık soru.
2. **Çıkarım (İ2):** Uygulanmaz; kural yeni bir şey çıkarmaz. Help'in
   saat adı yereldir ve tek cevaplıdır: tek saatli modülde o saat, değilse
   ad verilmez.
3. **Anlam değişikliği (İ3):** Yok. `delay<K>`'nın anlamı ve hiçbir
   tasarımın SV'si değişmez.
4. **Niyet tahmini (İ4):** Help "genç değer bir aşama eksik" okumasını
   önerir ama kodu değiştirmez; makine önerisi yoktur. Bilinçli karışım
   için anotasyonlu `let` explain'de, kaçış olarak, bedeliyle anılır.
5. **Metamorfik yeniden yazım (ADR-0117):** Kural iki yazımı eşdeğer
   saymaz. `delay<1>(a)` ile `a_d2` eşdeğer değildir: biri iddia, öteki
   register; bu yüzden bir yeniden yazım eklenmez.

## v0.2.0 için açık soru

`delay<K>`'nın anlamı #122'de karara bağlanır:

- **İddia + kaçış:** `delay<K>` iddia olarak kalır ve ADR-0118'in kaçışı
  olur (açık işaret, gerekçe, kaçış raporu; #358 ile). Adı register gibi
  okunduğundan yeniden adlandırılabilir; o zaman v0.1.x'te bir geçiş
  uyarısı (ADR-0119 İ3), v0.2.0'da eski ada hata.
- **Gerçek register:** `delay<K>` K register üretir. Bu anlam
  değişikliğidir: saat (değerin alanı), reset değeri, `pipeline`
  aşamasında stall, `fn` gövdesi ve kontratlar için kurallar, ADR-0040
  §2'nin "RTL'de örtük geçmiş yoktur" gerekçesine bir cevap, v0.1.x'te
  geçiş uyarısı ve v0.2.0 gerekir. Kabul edilen tasarımların SV'si
  değişir.

#232'nin analizi ilk seçeneği önerir. Bu ADR iki seçenekle de uyumludur:
tavsiye her durumda gerçek aşama register'ıdır.

## Testler

- Önce düşen (değişiklikten önce kırmızı, sonra yeşil):
  - `crates/volt-hir/tests/e5010_advice_tests.rs` (14 test): etiketler
    ve neden; tam help metni; genç değer sağda ya da `if` kolunda; iki,
    üç ve dört çevrimlik zincir; `i8`, `bits<4>`, `u16`, `bool`; bileşik
    genç değer, struct ve kullanılan ad genel biçimi alır; iki saatli
    modül; register kuralının üç dalı; hiçbir help "align" demez.
  - `crates/volt-driver/tests/e5010_advice_tests.rs`: explain ÖRNEK'i
    yalnız E5010, ÇÖZÜM tanısız (iki dilde); help'teki kod ÖRNEK'e
    uygulanınca E5010 gider, yeni tanı çıkmaz (iki dilde); uygulanmış
    tasarım ve ÇÖZÜM Verilator'da 60 verir (araç işinde zorunlu).
- `tests/real_bugs/232_e5010_advice_delay_adds_no_register.volt`
  (ADR-0117): eski tavsiye uygulanmış tasarım 70 (şartnameye göre
  `delay<1>` register eklemez), aşama register'lı tasarım 60; ikisi de
  tanısız.
- `tests/ui/pass/161_delay_is_a_claim.volt`: `delay<K>` E5010'u giderir
  ve derlenir. `tests/ui/pass/44` gerçek register'la hizalar; tohumlanmış
  hata derleminin pass/44'e bağlı üç özel mutantı aşama register'ına
  göre yeniden yazıldı (`timing.drop-delay` türü artık pass/161'in
  `delay<1>`'ini kaldırır, beklenen E5010).

## Sınırlar

- `delay<K>` bugünkü gibi derlenir ve E5010'u giderir; değişen yalnız
  derleyicinin bunu önermemesi ve iddia olarak anmasıdır.
- `fn` gövdesindeki `delay<K>`'yı zamanlama geçidi görmez; çağrı yeri yine
  E5010 alır (#362, v0.1.4).
- Help'in kodu yalnız tek adlı genç değer için adla yazılır; bileşik
  değerde genel biçim verilir.

## Reddedilenler

| Seçenek | Neden |
|---|---|
| `delay<K>`'yı help'ten tümüyle çıkarmak | Kullanıcı onu kitapta ya da eski kodda görür; help onu anmazsa iddianın bedeli (Volt denetlemez) yazılmamış olur (ADR-0125 §3). |
| `delay<K>`'yı v0.1.3'te register yapmak | SV'yi ve değerleri değiştiren anlam değişikliği: ADR-0100'e göre orta hane, ADR-0119 İ3'e göre önce geçiş uyarısı. v0.2.0'ın açık sorusu. |
| Aşama register'ını makine önerisi yapmak | Yeni makine önerisi ayrı iştir (#353, v0.1.4); bu ADR yalnız metni düzeltir. |
| `delay<K>`'yı reddetmek | Önceden kabul edilen girdiyi reddeder ve bilinçli kullanımlar (değişmeyen bir değer için iddia) yanlış değildir: sessiz yanlış istisnasının dışında, orta hane. |
