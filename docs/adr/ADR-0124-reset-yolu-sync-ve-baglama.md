# ADR-0124: Reset'in Örneğe Yolu — `sync()` Reset Taşımaz, Reset Portuna Yalnız Reset Giriş Portu Bağlanır (#340)

> Statü: Önerildi
> İlgili: ADR-0065 (§1 tablosunun "Örneklemede ham port" kuralını uygular; "yapısal olarak imkânsız" tablosunun R3 ve "birleştiriciden türetilen reset" satırlarının bugün neden aşıldığını yazar), ADR-0100 (sessiz yanlış istisnası: önceden kabul edilen girdiyi reddeden düzeltme son hanede, v0.1.3), ADR-0103 (W3011 değişmez; senkronize kopyanın extern'e verilmesi #93'te kalır), ADR-0119 (İlke denetimi; İ2.4)
> Issue: #340 (ilgili: #93, #342)
> Tarih: 2026-11-19
> Etkilenen: `crates/volt-hir/src/typeck/reset_binding.rs` (YENİ: iki kural ve Volt alt modülü önerisi), `crates/volt-hir/src/typeck/synth.rs` (`synth_call`: kural 1'in çağrısı), `crates/volt-hir/src/typeck/instance.rs` (bağlama döngüsü: kural 2'nin çağrısı), `crates/volt-hir/src/domain/mod.rs` (`is_raw_reset` crate içine açılır), `crates/volt-diagnostics/src/explain/{en,tr}.rs` (E2003 ve W3011 açıklaması), `tests/ui/fail/291`–`296`, `tests/ui/pass/158`–`159`, `tests/suggestions/e2003_reset_through_sync.volt`, `tests/suggestions/e2003_reset_alias.volt`, `tests/real_bugs/340_reset_through_sync.volt`, `crates/volt-driver/tests/reset_binding_tests.rs` (YENİ), `crates/volt-driver/tests/seeded_bugs/kinds.rs` (`rdc.reset-through-sync`), `crates/volt-driver/tests/seeded_bug_tests.rs`, ui/pass sayımları (`ui_semantic_tests.rs`, `ui_pass_build_tests.rs`, `parser_tests.rs`), `.test-baseline`, `.github/badges.json`, `CHANGELOG.md`.
> DOKUNULMADI: `docs/spec/` (gereken değişiklik bu ADR'nin "Spec" bölümünde; bakımcı uygular), SV üretimi.

## Sorun

#340: ham bir reset portu `sync(rst, clk)`'ten geçip bir örneğin reset
portuna bağlanınca örnek reset'i hiç görmüyordu; `volt check`, `volt build`
ve `volt test` hiçbir şey söylemiyordu. Ölçüm (`main` db63cf54, Verilator
5.054, #340 yorumu):

- `sync()`'in iki flip-flop'u hedef saatin reset'iyle temizlenir
  (sv-mapping.md §8: aşamalar `<= 0`). Ham portlu modülde o reset, aynı
  porttan türetilen bırakma zinciridir (ADR-0065 §2). Kaynak hedefin
  reset'inin kendisi olunca flip-flop'lar tam reset assert olduğu sürece
  temizlenir.
- **active_high** (sync ve async): örneğin reset girişi her çevrimde 0.
  Extern hiç reset görmez; Volt alt modülünün register'ı iki reset boyunca
  güç açılışı değerinden sayar (`left: 7, right: 5`).
- **active_low** (sync ve async): aşamaların temiz değeri 0, active_low'da
  assert seviyesidir. Devre **tesadüfen** bir reset verir: reset süresince
  0, bırakma zincirden 2 kenar sonra (ham bırakmadan 4 kenar sonra).
  Örnek, yanındaki register'lardan 2 çevrim geç reset'ten çıkar.
- Veri için mekanizma olağandır: yalnız hedef reset'teyken var olan bir
  değer kaybolur, reset'ten sonra da süren değer 2 kenar sonra geçer.

Kök neden: ADR-0065 §1 tablosu "Örneklemede ham port" satırında bağlama
değerini "yalnız ebeveynin bir ham portu" diye yazar ve denetimi tipe
bırakır ("typeck: `Ty::Reset`, mevcut tip uyuşmazlığı tanısı"). Typeck
yalnız tipe bakar; `sync()`/`sync3()` argümanının tipini taşır (ADR-0090
§3). `Ty::Reset` taşıyan her ifade geçer. Aynı boşluktan dört kaçak daha
ölçüldü (çocukta ham `rst` portu, ebeveynde `clk` üzerinde register):

| Bağlama | Bugün |
|---|---|
| `rst: rst` | E3003 (aynı reset iki kez senkronlanıyor; doğru) |
| `let rs = sync(rst, clk)` → `rst: rs` | tanı yok (#340) |
| `let rs = sync3(rst, clk)` → `rst: rs` | tanı yok |
| `let rs = rst` → `rst: rs` (tipsiz let) | tanı yok: E3003 kaçar; extern'de W3011 de kaçar |
| `let rs : reset(sync, active_high) = rst` (tipli let) | `volt check` temiz, `volt build` E0003 ("values of type 'reset' outside an input port") |
| `let rs = if c { rst } else { rst }` → `rst: rs` | tanı yok; SV `wire rs = c ? rst : rst;` |

ADR-0065'in "yapısal olarak imkânsız" tablosu R3'ü (reset'in veri olarak
kullanımı) ve "birleştiriciden türetilen reset"i imkânsız sayıyordu:
`sync()` reset'i veri gibi örnekler (R3), `if`/`match` reset üzerinde bir
çoklayıcı kurar (birleştirici). Tipli ve tipsiz `let` aynı niyetin iki
yazımıdır ama biri reddediliyor, öteki derleniyordu (ADR-0119 İ2.4).

## Karar

İki kural, ikisi de E2003 (ADR-0065 §1'in adlandırdığı tip uyuşmazlığı
tanısı); ikisi de tek yerde denetlenir (`typeck/reset_binding.rs`):

1. **`sync()` ve `sync3()` reset taşımaz.** Kaynağın tipi `reset` ise
   E2003, çağrının kaynak argümanında. Kural kutba bakmaz: active_low'da
   devrenin tesadüfen çalışması kuralı değiştirmez (aşağıda "Yan etki").
2. **Reset tipli bir porta yalnız ebeveynin reset giriş portunun adı
   bağlanır** (`rst: rst` ya da kısayol `rst`; `domain::is_raw_reset`:
   giriş yönlü, `reset` tipli port). Başka her değer (tipsiz ya da tipli
   `let`, `if`, `match`, `sync()` sonucu) E2003, bağlamanın değerinde. Değer
   reset tipinde değilse olağan tip uyuşmazlığı (E2003) zaten çıkar; kural
   ikinci kez söylemez.

`sync(rst, clk)` bir `let`'te bağlanınca iki tanı çıkar: çağrıda kural 1,
bağlamada kural 2. İkisi ayrı yanlışı söyler (veri senkronizörü reset
taşımaz; reset yolu bir giriş portundan başlamaz). Volt alt modülü
önerisi ikisini birlikte kaldırır.

**Tanı metni** (iki dilde, explain dahil; ADR-0099 §3: ADR numarası ve
spec yolu yok):

- Ne oldu ve neden olmuyor: "Volt bir reset'i modüle girdiği yerde, reset
  giriş portunda senkronize eder ve oradan alt modüllere izler; let, if,
  match ya da sync() ile yapılan kopya sıradan bir sinyaldir: onu hiçbir
  senkronizör bırakmaz ve reset denetimleri onu göremez." Kural 1'in
  nedeni: `sync()`'in flip-flop'larını hedef saatin reset'i temizler; o
  reset aynıysa çıkış assert süresince temizdir, active_high hiç çıkmaz,
  active_low yalnız tesadüfen.
- **Volt alt modülü:** help "çocuktan reset portunu ve bu bağlamayı
  kaldırın; çocuk otomatik bir reset portu alır ve Volt onu bu modülün o
  saat için senkronize ettiği reset'e bağlar" (ADR-0065 §1 son satırı,
  §2 "Çocuk örneklemesi"). Makinece uygulanabilir öneri (ADR-0099): bağlama
  (ayırıcısıyla), çocuğun reset portu ve yalnız bu bağlamada okunan `let`
  silinir. Öneri yalnız şu koşullarda verilir, aksi hâlde yalnız metin
  kalır: bağlama ve çocuğun portu açılımdan (`for`, generic kopya)
  gelmez; çocuğun tek ham reset portu budur ve gövdesinde okunmaz; başka
  hiçbir örnek bu portu bağlamaz; silinen metnin okuduğu her ad (ham reset
  portları ve yerleşikler hariç) başka yerde de okunur. Böylece öneri
  uygulanınca yeni E1009, W1001 ya da E3010 doğmaz.
- **Extern:** makinece uygulanabilir öneri yok. Help: reset giriş portunun
  kendisini bağlayın (`rst: rst`) ve bırakmayı SystemVerilog modülü
  senkronlasın. Not: Volt bir extern'e senkronize reset'i henüz veremez,
  çünkü senkronizörün çıkışını yalnız bir Volt modülünün otomatik reset
  portuna bağlar ve extern'in böyle bir portu yoktur (#93). İki yol ve
  bedeli: ham reset'i bağlamak (modülün aynı saatteki register'ları
  senkronize kopyayla sıfırlanıyorsa W3011: extern reset'ten farklı
  çevrimde ve SV kodu senkronlamıyorsa saatle hizasız çıkar) ya da
  senkronlamayı extern'in içinde yapmak (iki flip-flop, hemen assert,
  saat kenarında bırakma; ham reset o zaman güvenlidir, W3011 yalnız farklı
  çevrimi hatırlatır).

W3011'in açıklamasına bir cümle eklenir: reset'i `sync()`'ten ya da bir
`let`'ten geçirmek de yol değildir (E2003). W3011'in tetikleyicisi ve
iletisi değişmez.

### Yan etki: active_low

active_low bir reset'in `sync()`'ten geçirilmesi bugün devre olarak
çalışıyordu (temiz değer 0 = assert seviyesi; bırakma ham bırakmadan 4
kenar sonra, yanındaki register'lardan 2 çevrim geç). Bu ADR'den sonra
o yazım da E2003'tür. Davranış değişikliği olarak CHANGELOG'a ve sürüm
notuna yazılır. Depoda, kitapta ve örneklerde bu yazım yoktur (#340
yorumundaki tarama). Doğru yol active_high'daki ile aynıdır: alt modüle
reset portu yazılmaz.

## ADR-0100 gerekçesi

- Var olan bir tanı kodu (E2003) kullanılır; yeni kod, yeni seçenek yok.
- Bir sessiz yanlışın düzeltmesidir (#340: başarılı görünüp reset'siz
  devre üretiyordu) ve önceden kabul edilen girdiyi reddeder: ADR-0100
  §1'in son hane istisnası. **v0.1.3'e girer**, "Behavior changes"
  bölümüne yazılır.
- Kabul edilmeye devam eden hiçbir tasarımın SV'si değişmez (yalnız ret).
- Yeni bir kural da değildir: ADR-0065 §1'in yazılı kuralı ("değer yalnız
  ebeveynin bir ham portu olabilir") uygulanır; kural 1 o kuralın `sync()`
  için söylediğidir (R3: reset veri değildir).

## İlke denetimi

1. **Beyan (İ1):** Yeni beyan istenmez. Reset giriş portu zaten bir
   beyandır; kural onun denetlenmesini sağlar (denetlenmeyen beyan sahte
   güven verir: #340).
2. **Çıkarım (İ2):** Kural bir şey çıkarmaz; sözdizimsel ve yereldir
   ("bağlama değeri bu modülün bir reset giriş portunun adı mı"). Belirsiz
   durum yok. **İ2.4 (yazımdan bağımsız):** tipli ve tipsiz `let` artık
   aynı sonucu verir (bağlanınca ikisi de E2003). Bağlanmayan tipli `let`
   `volt build`'de E0003 olarak kalır (`tests/fixtures/parity/p33`);
   bağlanmayan tipsiz `let` W1001 alır ve SV'ye bir tel olarak iner. Bu
   kalan fark bir reset'i hiçbir örneğe ulaştırmaz; ayrı bir karar konusudur
   (Sınırlar).
3. **Anlam değişikliği (İ3):** Kabul edilmeye devam eden hiçbir tasarımın
   anlamı değişmez. Değişen, önceden kabul edilen girdinin reddidir; bu
   İ3'ün değil ADR-0100'ün sessiz yanlış istisnasının konusudur.
4. **Niyet tahmini (İ4):** Help, kullanıcının çocuğa senkronize reset'i
   vermek istediğini varsayar ("reset portunu yazmayın"). Tahmin yalnız
   help'te ve öneridedir, kodun anlamını değiştirmez; öneri gidiş-dönüş
   testinden geçer (`tests/suggestions/e2003_reset_through_sync.volt`,
   `e2003_reset_alias.volt`).
5. **Metamorfik yeniden yazım (ADR-0117):** Kural iki yazımı eşdeğer
   saymaz; tersine, reset'i bir `let`'e çıkarmayı yasaklar. Yeniden yazım
   eklenmez. Tohumlanmış hata derlemine (ADR-0113) `rdc.reset-through-sync`
   türü eklenir: bir reset bağlamasını `let <r>_sync = sync(<r>, <saat>)`
   üzerinden geçirir, beklenen kod E2003 (bu ADR).

## Spec

`docs/spec/` bu PR'da düzenlenmez (AGENTS.md). Bakımcının uygulayacağı
yama PR açıklamasında anılır: sv-mapping.md §8 (`sync()` kaynağı reset
olamaz; aşamaların hedef reset'le temizlenmesinin sonucu), domain-inference.md
K9 (aynı kural, CDC köprüsünün tanımında) ve sv-mapping.md §9 (reset tipli
porta yalnız reset giriş portunun adı bağlanır; doğru yol otomatik port).

## Testler

- Önce düşen (değişiklikten önce kırmızı, sonra yeşil):
  `typeck::reset_binding::tests` içinde `sync` (active_high, active_low),
  `sync3`, bağlanmamış `sync`, tipsiz ve tipli `let`, `if`/`match`,
  extern'e `let`, öneri biçimi ve öneri koşulları;
  `ui_fail_291_296_reset_path_e2003`; `reset_binding_tests` içinde
  `issue_340_design_is_e2003_with_a_fix_for_the_volt_child` ve
  `a_reset_through_a_let_to_an_extern_has_no_fix_and_names_both_ways`.
- Korunanlar: ham portun kendisi (uzun ve kısayol) kabul; veri `sync()`'i
  değişmez; tip uyuşmazlığı bir kez; extern'e ham `rst` yalnız W3011
  (`a_raw_reset_to_an_extern_is_still_only_w3011`).
- Simülasyon: reset portu yazılmayan çocuk `reset()`'ten sonra 5, üç
  saatten sonra 8, ikinci `reset()`'ten sonra yine 5
  (`a_child_without_a_reset_port_starts_from_its_reset_value_in_simulation`,
  Verilator gerekir; araç işinde koşar).
- `tests/real_bugs/340_reset_through_sync.volt` (ADR-0117); bilinen
  hatalar listesinde #340 girdisi yoktu.

## Sınırlar

- Extern'e senkronize kopyanın verilmesi bu ADR'nin konusu değildir
  (#93, v0.2). Bugün extern'e yalnız ham reset bağlanabilir.
- Bağlanmayan reset tipli `let` (tipsiz) hâlâ derlenir; bir reset'i hiçbir
  yere ulaştırmadığı için güvenlik açığı değildir, ama tipli biçimle
  farkı (E0003) sürer.
- `in r : reset` (spec'siz) porta ham reset bağlamak bugün yanlışlıkla
  E2003 ("expected 'reset', found 'reset'"): ADR-0065 §1'e göre spec'siz
  port beslediği alanın spec'ini alır. Bu ADR'den bağımsız, önceden var
  olan bir hata: #342 (tohumlanmış hata testinde `KNOWN_FALSE_ALARMS`).

## Reddedilenler

| Seçenek | Neden |
|---|---|
| **(b)** `sync()` reset alınca doğru reset senkronizörünü (hemen assert, saat kenarında bırakma) üretsin | `sync()` tipe göre iki ayrı devre üretir olur; bugün çalışan active_low yazımının bırakma gecikmesi değişir (kabul edilen tasarımın devresi değişir: ADR-0100'e göre orta hane, İ3'ün dört koşulu; İ3.1'in yaygın yanlış okuma kanıtı yok, derlemde 0 kullanım). Reset bir değer olarak dolaşır: ADR-0065 §R1'in R3 ve R6 gerekçesiyle reddettiği biçimdir. SDC kısıtları (ADR-0065 §4.2) ve RDC (E3003 yakınsaması) yeniden tasarlanmak zorunda kalır. |
| **(d)** W3011 `sync()` ile türetilen reset'i de görsün | Yalnız uyarı: sessiz yanlış çıktı aynen kalır (extern hiç reset görmez). W3011 extern'e özgüdür, Volt alt modülü durumunu kapsamaz; iletisi ("raw reset goes to an extern") yanlış teşhis olur: değer ham değil, sabit 0. Var olan kodun tetikleyicisinin genişlemesi ADR-0100'e göre tanı davranışı değişikliğidir ve son hane istisnalarına girmez. Bu ADR'den sonra tetikleyecek girdi derlenmez. |

**Ertelenen (c):** ayrı bir reset senkronlama primitifi bugün yok
(ADR-0065 §R1 `reset_sync()`'i reddetti). Eklemek §R1'in yerini kısmen
alan bir ADR, typeck'te reset değeri kuralları, sv-emit, SDC/XDC, RDC
yakınsaması ve belge ister; yeni özellik olduğu için v0.2. Asıl ihtiyaç
(extern'e senkronize kopya) #93'ün v0.2 kararının konusudur; (c) orada
bir seçenek olarak tartılır (ör. extern'in `reset` tipli portunun, Volt
alt modülünün otomatik portu gibi zincirin çıkışına bağlanması).
