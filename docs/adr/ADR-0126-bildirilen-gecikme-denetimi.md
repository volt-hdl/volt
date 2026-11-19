# ADR-0126: Bildirilen Gecikme Denetlenir — `let`, `wire` ve Çıkış Portundaki `Delayed<T, N>` (#354)

> Statü: Önerildi
> İlgili: ADR-0037 (§4 yeniden zamanlama iddiası: bu ADR iddiayı karışımla sınırlar; gecikmesi tam bilinen değerin bildirimi denetlenir), ADR-0100 (sessiz yanlış istisnası: önceden kabul edilen girdiyi reddeden düzeltme son hanede, v0.1.3), ADR-0118 (karışım üzerindeki bildirim işaretsiz bir kaçıştır, v0.2.0: #358), ADR-0119 (İlke denetimi; İ1 denetlenmeyen beyan, İ2.1 çıkış portu sınırdaki beyandır), ADR-0125 (tanı ilkesi: ne oldu, neden, ne yapılabilir; makine önerisi yok, `delay<K>` önerilmez)
> Issue: #354 (ilgili: #232, #358)
> Tarih: 2026-11-19
> Etkilenen: `crates/volt-hir/src/timing.rs` (`check_declared`: bildirim kuralının tek yeri; çıkış portunun bildirimi çıkarımla ezilmez), `crates/volt-diagnostics/src/explain/{en,tr}.rs` (E5010 açıklaması), `crates/volt-hir/tests/timing_tests.rs`, `crates/volt-hir/tests/ui_semantic_tests.rs`, `tests/ui/fail/297`–`299`, `tests/ui/pass/160_delayed_declared_counts.volt`, `tests/real_bugs/354_delayed_let_wrong_count.volt`, `crates/volt-driver/tests/delayed_declaration_tests.rs` (YENİ), `crates/volt-driver/tests/seeded_bugs/kinds.rs` (`timing.declared-delay-shift`), `crates/volt-driver/tests/seeded_bug_tests.rs`, ui/pass sayımları (`ui_pass_build_tests.rs`, `parser_tests.rs`), `.test-baseline`, `.github/badges.json`, `CHANGELOG.md`.
> DOKUNULMADI: `docs/spec/` (gereken değişiklik bu ADR'nin "Spec" bölümünde; bakımcı uygular), SV üretimi.

## Sorun

#354: `@strict_timing` modülünde `Delayed<T, N>` bildirimi yalnız
register'da gerçek gecikmeyle karşılaştırılıyordu (`timing.rs`: kaynak
+ 1 = N, değilse E5010). `let`, `wire` ve çıkış portunda N hiç
denetlenmiyordu:

- `let` ve `wire`: bildirim varsa başlatıcı ve atamalar tümüyle
  atlanıyordu; ne içlerindeki karışım ne bildirilen N denetleniyordu.
- Çıkış portu: bildirilen N gecikme tablosuna giriyor, ama sabit nokta
  onu çıkarılan gecikmeyle eziyordu. `out y : Delayed<u32, 5>` ile
  `y = a` (1 çevrimlik register) hatasız derleniyordu.

Karışım olmayan bir tasarımda bile yanlış bildirim E5010'u bir satır
aşağıda susturuyordu:

```volt
reg a : Delayed<u32, 1> = 0
reg b : Delayed<u32, 2> = 0
on clk { a <= x  b <= a }
let s : Delayed<u32, 1> = b     // b tam 2 çevrim; bildirim 1 diyor
y = s + a                       // 1 ile 1: hizalı görünüyor
```

Simülasyonda (Verilator 5.054) `y`, `x(t−1) + x(t−2)` idi: 10, 20, 30
girişlerinden sonra 40 yerine 50. `@strict_timing`'in sözü tam bu
karışımın reddedilmesidir; `volt check` başarılı görünüp başka bir devre
üretiyordu.

ADR-0037 §4 anotasyonlu `let`'i, bilinçli ileri besleme için bir
*yeniden zamanlama iddiası* olarak tasarladı: başlatıcısının içindeki
karışım denetlenmez. Sonuçlar bölümü sınırı yazar: iddialı bir `let`
içindeki yanlış aşama başvurusu denetlenmez. Yazılan sınır karışımla
ilgilidir; gecikmesi tam bilinen bir başlatıcının üstündeki yanlış N'in
denetlenmediğini söylemez. #354'ün tasarımı bu durumdur.

## Karar

1. **`let`, `wire` ve çıkış portundaki `Delayed<T, N>`, değerin
   gecikmesi tam biliniyorsa N ile karşılaştırılır; uyuşmazlık E5010,
   bildirimde.** Değerin gecikmesi, anotasyonsuz bir tanımın gecikmesini
   veren çıkarımın kendisidir (`compute_def`: kaynaklarının birleşimi).
   Kural `check_declared`'da, register'ın var olan denetimiyle aynı
   fonksiyondadır (GOVERNANCE.md, "One place per rule"); ikinci bir
   gecikme hesabı yazılmaz.
2. **Çıkış portunun bildirimi çıkarımla ezilmez.** Bildirim modülün
   sınırındaki sözdür (ADR-0119 İ2.1); sürücüsüyle denetlenir. Yeniden
   zamanlama iddiası değildir: sürücüsündeki karışım bildirimden önce
   olduğu gibi E5010 alır (ADR-0037 iddiayı yalnız `let` için tanımlar). Giriş
   portunun bildirimi değişmez: sürücüsü modülün dışındadır, içeride bir
   varsayımdır (ADR-0037).
3. **Gecikmesi tam bilinmeyen durumlar bu kararla değişmez:**
   - Değer aşamaları karıştırıyor (doğrudan, iç içe ya da farklı
     aşamalardan sürülen bir `wire`): bildirim bugünkü gibi yeniden
     zamanlama iddiasıdır, N denetlenmez, içteki karışım bastırılır.
     ADR-0118'e göre bu işaretsiz bir kaçıştır; açık işaret, gerekçe ve
     kaçış raporu v0.2.0'da (#358).
   - Değer zamanlama taşımıyor (literal, sabit) ya da geri besleme
     döngüsünde sabit bir gecikmeye oturmuyor: karşılaştırılacak bir
     gecikme yoktur.
   - Pipeline desugar'ının aşamasına sabitlediği `let`'ler (ADR-0038 §3)
     kullanıcının yazdığı bir bildirim değildir; denetlenmez.
4. **Register değişmez:** her yazımı ayrı denetlenir, metni aynıdır.

**Tanı metni** (iki dilde; ADR-0125, ADR-0099 §3: ADR numarası ve spec
yolu yok):

- Ne oldu: "declared delay does not match the value"; birincil konum
  bildirim ("declared as 1 cycle here"), ikincil konum değer ("this value
  is 2 cycles old").
- Neden (`= reason:`): değeri okuyan her yer bildirilen sayıyı onun yaşı
  sayar; yanlış sayı, okunduğu yerde farklı çevrimlerin birleşmesini
  gizler.
- Ne yapılabilir (help): sayıyı değerin gerçek gecikmesine düzeltmek; ya
  da değer gerçekten N çevrim yaşında olmalıysa onu eksik sayıda register
  aşamasından geçirmek (N fazlaysa) ya da daha önceki bir aşamadan almak
  (N azsa).
- **`delay<K>` önerilmez:** bugün register eklemez (#232); öneri E5010'u
  susturmanın bir yolu daha olurdu.
- **Makine önerisi yok:** N'i değerin gecikmesine çevirmek, bildirimin
  gizlediği karışımı okunduğu yerde geri getirebilir (#354'ün tasarımında
  `s + a` E5010 alır). ADR-0099 §2'nin gidiş-dönüş sözleşmesi (öneri
  uygulanınca yeni tanı yok) genel olarak sağlanmaz; yalnız metin.

`volt explain E5010` bir cümle kazanır: register, `let`, `wire` ya da
çıkış portundaki bildirim değerin yaşı hakkında bir sözdür; derleyici
gecikmeyi tam biliyorsa N onunla aynı olmalıdır.

## ADR-0100 gerekçesi

- Var olan bir tanı kodu (E5010) kullanılır; yeni kod, yeni seçenek yok.
- Bir sessiz yanlışın düzeltmesidir (#354: `volt check` başarılı görünüp
  farklı çevrimlerin değerlerini toplayan devre üretiyordu) ve önceden
  kabul edilen girdiyi reddeder: ADR-0100 §1'in son hane istisnası.
  **v0.1.3'e girer**, "Behavior changes" bölümüne yazılır.
- Yalnız bildirimi kanıtlanabilir biçimde yanlış olan girdi reddedilir:
  değerin gecikmesi tam bilinir ve N'den farklıdır. Karışım üzerindeki
  bildirim (ileri besleme) bugünkü gibi derlenir.
- Kabul edilmeye devam eden hiçbir tasarımın SV'si değişmez: `Delayed`
  tip düzeyinde silinir (ADR-0037), değişen yalnız tanıdır.

## İlke denetimi

1. **Beyan (İ1):** Yeni beyan istenmez. `Delayed<T, N>` zaten bir
   beyandır ve #354'e kadar `let`, `wire` ve çıkış portunda
   denetlenmiyordu: İ1'in "denetlenmeyen beyan, beyan olmamasından
   kötüdür" dediği durum. Bu karar İ1.2'yi sağlar: derleyici kodun beyana
   uyup uymadığını kanıtlar. Kanıtlayamadığı tek durum (karışım) kaçıştır
   ve ADR-0118'e göre v0.2.0'da görünür kılınır (#358); o zamana kadar
   sınır ADR-0037'nin Sonuçlar bölümünde yazılıdır.
2. **Çıkarım (İ2):** Kural yeni bir şey çıkarmaz; var olan gecikme
   çıkarımını bildirimle karşılaştırır. **İ2.1 (sınırda beyan, içeride
   çıkarım):** çıkış portunun `Delayed` bildirimi sınırdaki beyandır;
   bugüne kadar içerideki çıkarım onu sessizce eziyordu. Artık sürücüsüyle
   denetlenir ve ezilmez. Giriş portunun bildirimi sınırdaki varsayım
   olarak kalır. **İ2.3 (tek cevaplı):** gecikme iki cevaplıysa (karışım)
   derleyici seçmez, bildirimi denetlemez; hata vermez çünkü bu bugün
   bilinçli bir kaçış yazımıdır.
3. **Anlam değişikliği (İ3):** Hiçbir tasarımın anlamı ve SV'si değişmez.
   Değişen, önceden kabul edilen girdinin reddidir; bu İ3'ün değil
   ADR-0100'ün sessiz yanlış istisnasının konusudur.
4. **Niyet tahmini (İ4):** Help iki niyetin ikisini de söyler (sayı
   yanlış; ya da değer yanlış aşamadan) ve seçmez. Makine önerisi yok.
5. **Metamorfik yeniden yazım (ADR-0117):** Kural iki yazımı eşdeğer
   saymaz. Tohumlanmış hata derlemine (ADR-0113) `timing.declared-delay-shift`
   türü eklenir: `let`, `wire` ya da çıkış portundaki `Delayed<T, N>`'in
   N'ini bir artırır, beklenen kod E5010. Karışım üzerindeki iki bildirimin
   mutantı (`tests/ui/pass/44`, `160`) `KNOWN_MISSES`'te #358 ile durur.

## Spec

`docs/spec/` bu PR'da düzenlenmez (AGENTS.md). Bakımcının uygulayacağı
yama PR açıklamasında anılır: type-inference.md §12. Kurallar tablosuna
bildirimin satırı ("`x : Delayed<T,N>` ve değeri `Delayed<T,M>`, N ≠ M
→ E5010"); muafiyetler paragrafında yeniden zamanlama iddiasının yalnız
aşamaları karıştıran değer için geçerli olduğu; çıkış portunun
bildiriminin sürücüsüyle denetlendiği, giriş portununkinin varsayım
olduğu.

## Testler

- Önce düşen (değişiklikten önce kırmızı, sonra yeşil):
  `timing_tests` içinde `let_declared_with_a_wrong_count_is_e5010`
  (karışım yokken yanlış bildirim, #354'ün tasarımı),
  `wire_declared_with_a_wrong_count_is_e5010`,
  `output_port_declared_with_a_wrong_count_is_e5010`;
  `ui_fail_297`–`299`; `delayed_declaration_tests` içinde
  `issue_354_design_is_e5010_at_the_declaration` (iki dilde: tek E5010,
  birincil konum bildirim, gerekçe notu, help, öneri yok, metinde
  `delay<`, ADR ve `.md` yok).
- Korunanlar: doğru N'li `let`, `wire`, çıkış portu ve `delay<K>` ile
  hizalanan başlatıcı geçer; register denetimi aynı
  (`register_declaration_is_unchanged`); bildirimli çıkış portunun
  sürücüsündeki karışım yine E5010
  (`output_port_declaration_is_not_a_retiming_assertion`; bu düzeltmenin
  ilk hâlinde kırmızıydı, ana dal E5010 veriyordu); karışım üzerindeki bildirim
  N ne olursa olsun geçer, iç içe karışım ve farklı aşamalardan sürülen
  `wire` dahil (`a_declaration_over_a_mix_stays_a_retiming_assertion`,
  `a_mix_under_a_declaration_still_compiles`); literal başlatıcı
  denetlenmez; pipeline'ın sabitlenmiş `let`'leri denetlenmez
  (`tests/ui/pass/46`).
- Simülasyon: #354'ün kontrol tasarımı (gerçek aşama register'ı `c`)
  hâlâ 40 verir
  (`aligned_stage_registers_still_add_values_of_the_same_age`, Verilator
  gerekir; araç işinde koşar).
- `tests/real_bugs/354_delayed_let_wrong_count.volt` (ADR-0117); bilinen
  hatalar listesinde #354 girdisi yoktu.

## Sınırlar

- Karışım üzerindeki bildirim denetlenmez (yukarıda, #358).
- `delay<K>` bugün yalnız sayıya K ekler, register eklemez (#232). Bu
  kural onun sayısına güvenir: `let s : Delayed<u32, 2> = delay<1>(a)`
  geçer, ama donanımda `s` 1 çevrimliktir. #354'ün düzeltmesi #232'ye
  bağlı değildir; #232'nin açığı bu karardan önce de vardı ve aynı kalır.
- Sınırın ötesi denetlenmez: ebeveyn bir örneğin çıkış portunu okurken
  çocuğun bildirimini görmez (örnek port bağlamaları denetlenmez,
  ADR-0037 §6).

## Reddedilenler

| Seçenek | Neden |
|---|---|
| Bildirimi her durumda denetlemek, karışımı da E5010 yapmak | ADR-0037'nin ileri besleme kaçışını kaldırır; bugün derlenen bilinçli bypass tasarımları (`tests/ui/pass/44`) yol göstermeden reddedilir. ADR-0118'e göre kaçış işaretle ve gerekçeyle kalır; işaret v0.2.0'da gelir (yeni sözdizimi ve tanı davranışı, ADR-0100'e göre orta hane). |
| Ayrı bir denetim fonksiyonuyla `let`/`wire`/port gecikmesini yeniden hesaplamak | Aynı kural iki yerde hesaplanır; GOVERNANCE.md "One place per rule": sessiz yanlışların çoğu kopyaların ayrışmasından çıktı. Karar çıkarımın kendisini (`compute_def`) kullanır. |
| Makine önerisiyle N'i değerin gecikmesine çevirmek | Gidiş-dönüş sözleşmesini (ADR-0099 §2) genel olarak sağlamaz: düzeltilen bildirim, gizlediği karışımı okunduğu yerde E5010 olarak geri getirir. Hangisinin yanlış olduğunu (sayı mı, değer mi) yalnız kullanıcı bilir (ADR-0119 İ4). |
| Help'te `delay<K>(...)` önermek | Bugün register eklemez (#232): E5010'u susturur, donanım değişmez. |
| Çıkış portunun bildirimini yalnız uyarıyla bildirmek | Sessiz yanlış çıktı aynen kalır; yanlış bildirim `@strict_timing`'in sözünü bozar. Var olan bir kodun (E5010) tetikleyicisinin genişlemesi ADR-0100'ün sessiz yanlış istisnasıdır; yeni bir uyarı kodu ise son hanede çıkamaz. |
