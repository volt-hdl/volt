# ADR-0139: Değer Kümesi, Kodlama ve Zaman Modeli Ayrı Bilgilerdir — İkili Varsayımların Envanteri, Kodlamanın Tek Yeri, Ara Temsile Gereksinim

> Statü: Uygulandı — 2026-11-25; Aşama 0, envanter
> İlgili: ADR-0003 (Trit tipi: değer kümesi {-1, 0, +1}; Aşama 1'in turnusolu), ADR-0028 (`Spike`, `PTrit`, `spike` rezerve: bu ADR mekanizma eklemez), ADR-0062 (Trit'in SV eşlemesi: bugünkü tek ikili olmayan değer kümesinin kodlaması), ADR-0136 (§3'ün ara temsil kararına bir gereksinim koyar, kararı vermez), ADR-0137 (kodlama bilgisi anlamsal bilgidir ve tek kaynaktan okunur), ADR-0142 (envanterin "Açık bulgular" 1'i kapanır; `~` Trit'te tanımlı değil, ilkenin ilk uygulaması); tasarım notu: `docs/design/ikili-uclu-melez-tasarim.md` (ikili–üçlü melez sınır kuralları ve aşamaların koşulları; bağlayıcı değil), ADR-0144 (fark testi Trit'i değer kümesiyle modeller; ret ölçütünün Trit satırları §1'in ayrımını sınar)
> Tarih: 2026-11-25
> Etkilenen: `docs/design/ikili-varsayimlar-envanteri.md` (YENİ, Aşama 0). Kod ve davranış değişmez.

## Sorun

Bakımcının uzun vadeli hedefi Volt ile ikili olmayan donanım da
tasarlanabilmesidir: ternary, ikili–üçlü melez, nöromorfik, belki
fotonik. Bugün bunların hiçbirinin arka ucu yazılmıyor ve bu ADR de
yazmıyor. Ama bugünkü kod "her şey ikilidir" varsayımını üç ayrı bilgiyi
tek kavramda birleştirerek yapar:

- **Değer kümesi:** bir tipin alabileceği değerler. `bits<N>` N tane
  {0, 1} bitidir, `uN` 0..2^N−1'dir, `Trit` {−1, 0, +1}'dir (ADR-0003).
- **Kodlama:** bir değerin fiziksel temsili: kaç tel, hangi desen.
  `Trit` iki bitlik işaretli i2'dir, +1 = `01`, 0 = `00`, −1 = `11`
  (ADR-0062); bir enum ikili koddur; bir struct paketli bir bit
  vektörüdür.
- **Zaman modeli:** değerin ne zaman değiştiği. Bir `reg` alanının saat
  kenarında güncellenir; kombinasyonel mantık zamansızdır; `clock = none`
  kenarsız bir alandır (E3016). Olay güdümlü bir model yoktur; gramer
  yalnız `on spike(input)` biçimini V2 için ayırır (grammar-full.ebnf
  §11, §17; ADR-0028).

Tohumlar zaten var: `Trit` (ADR-0003, "ternary/nöromorfik AI donanımı
için"), Trit'in SV eşlemesi ve çarpansız ternary MAC (ADR-0062), rezerve
`Spike`, `PTrit` ve `spike` (ADR-0028), ikili FPGA üzerinde Trit
taklidiyle yapılan sentez ölçümü (`docs/design/ternary-olcum.md`).

#419 bunun neden önemli olduğunu gösterdi: "Trit 2 bittir" bilgisi beş
yerde yazılmıştı ve biri yanlıştı (`ShiftRegister<Trit, 4>`'ün `taps`'ı
tip denetiminde `bits<4>`, SV'de 8 bit). Düzeltme bilgiyi tek yere
topladı: `volt_ast::bit_width::Scalar` ve `TypeArena::bit_width`
(`crates/volt-hir/src/ty.rs`). Bu, kodlama bilgisinin toplanacağı yerin
ilk örneğidir.

Aşama 0'ın envanteri (`docs/design/ikili-varsayimlar-envanteri.md`) bu üç
bilgiyi koddaki yerleriyle sayar: 289 satır (değer kümesi 88, kodlama
87, zaman modeli 114). Bunların 166'sı dilin gereğidir ve kalır, 100'ü
yalnız bugünkü ikili arka ucun gereğidir, 23'ü belirsizdir (sınıfı
bakımcının kararı). 137 satır bilgiyi kendisi hesaplar ve aynı bilgi
başka yerlerde de hesaplanır; bunlar 44 dağınık bilgide toplanır.

Envanterin gösterdiği üç şey bu ADR'nin gerekçesidir:

- **Kodlama dilden görünür.** ADR-0077 Karar 3 (normatif) struct'ın bit
  düzenini `p as uN` ile gözlenebilir kılar ve Trit yaprağını "ADR-0003'ün
  2 bitlik kodu" olarak yazar. Trit'in `01`/`11` kodu bugün yalnız SV'nin
  değil dilin bilgisidir.
- **#419 yalnız genişliği topladı.** `Scalar` bit sayısını tutar; Trit'in
  işaretli olduğu, `01`/`11` kodları ve kullanılmayan `10` kodu 16 yerde
  ayrı ayrı yazılıdır. `bool`, `clock` ve `reset`'in 1 bit olduğu 15
  yerde sabittir.
- **Bir bit işlemi değer kümesini aşabilir.** Spec `~x`'in tipini her
  tipte korur (type-inference.md §3.4); `~t` (t: Trit) Trit tiplidir, ama
  kodlamada `~01 = 10`, yani −2: ADR-0003'ün değer kümesi dışında. Bu,
  bir işlemin değer kümesi yerine kodlama üzerinde tanımlandığında olan
  şeydir (envanterin "Açık bulgular" 1; koşturulmadı, karar bakımcının).

## Karar (öneri)

### 1. Üç ayrı bilgi

Bir sinyalin değer kümesi, kodlaması ve zaman modeli ayrı bilgilerdir.
Değer kümesi tipin anlamıdır ve tip denetimi onu bilir. Kodlama, değer
kümesinin bir hedefte nasıl temsil edildiğidir. Zaman modeli sinyalin
alanının bir özelliğidir ve alan çıkarımı onu bilir.

Değer kümesi gereği ikili olan şey ikili kalır: `bits<N>` üzerindeki
`& | ^ ~`, kaydırma, birleştirme, parça seçimi ve `uN`'nin 2^N aralığı
dilin tanımıdır; bunlar kodlama katmanının arkasına taşınmaz. Envanterin
her satırı bu ayrımı yapar: "dilin gereği" (kalır) ya da "arka ucun
gereği" (kodlama katmanının arkasına taşınabilir).

### 2. Kodlama bilgisi tek yerde durur

Bir tipin kodlaması (genişliği bit cinsinden, değerlerinin bit deseni,
paketleme düzeni) tek yerde durur: bugün genişlik için
`volt_ast::bit_width::Scalar` ve `TypeArena::bit_width`, düzen için
`volt_ast::enum_layout` ve `volt_ast::struct_layout`. Bu, ADR-0137'nin
kuralının kodlamaya uygulanmasıdır: genişlik ADR-0136 §1'in anlamsal
bilgi listesindedir.

Yeni kod ikili kodlama varsayımını o katmanın dışında çoğaltmaz: "Trit
2 bittir", Trit'in `01`/`11` kodları, "varsayılan enum kodu `max(1, clog2(n))`
bittir" (ADR-0074), "bool 1 bittir" gibi bir bilgi yeni bir yerde yazılmaz, katmandan okunur.
Bugün katmanın dışında kalan yerler envanterde "dağınık" diye
işaretlidir; her biri ADR-0136 Aşama 2'nin yöntemiyle, üretilen çıktı
bayt bayt aynı kalarak toplanır. Bu ADR o taşımaların sırasını
belirlemez.

Kodlamanın tek yeri bir kodlamanın dilden görünmesini engellemez: bir
ADR'nin dile açtığı kodlama (ADR-0074'ün enum kodu, ADR-0077 Karar 3'ün
struct düzeni ve onun içinden Trit'in kodu) dilin anlamıdır ve kalır.
Kural yalnız onun bir yerde yazılmasını ister. Bir kodlamanın dilden
görünmez olması (ör. Trit içeren struct'ın `as uN`'sinin kodu değil
değeri vermesi) bir anlam değişikliğidir ve kendi ADR'sini ister.

### 3. Ara temsil kararına gereksinim (ADR-0136 §3)

ADR-0136 §3 ara temsilin yerini (A `volt-lower`, B yeni crate, C
`volt-hir` içinde modül) bakımcıya bırakır. Bu ADR o seçimi yapmaz; ona
bir gereksinim ekler:

**Ara temsil bir sinyalin değer kümesini, kodlamasını ve zaman modelini
ayrı taşıyabilmelidir.** Arka uç bugün yalnız SV olsa bile:

- değer kümesi: ifadenin ve sinyalin tipi (bugün `Ty`);
- kodlama: tipten türetilen ama ayrı sorgulanan bilgi; arka uç bir
  sinyalin bit genişliğini tipten kendisi hesaplamaz, kodlamadan okur;
- zaman modeli: sinyalin alanı ve alanın türü (bugün kenarlı saat ya da
  kenarsız); arka uç "saatli" olmayı saat portunun varlığından
  çıkarmaz, alandan okur.

"Taşıyabilmeli", veri yapısında bu üç bilginin ayrı yeri olması
demektir; bugün her biri tek bir değer alır (ikili kodlama, kenarlı ya
da kenarsız alan). Seçeneklerden hiçbiri bu gereksinimle elenmez; her
biri onu karşılayabilir. Gereksinim seçimin değerlendirmesinde bir satır
olur.

### 4. Gelecek yön (karar değil)

Aşağıdakiler bir yön tarifidir; hiçbiri bu ADR ile karar olmaz, her biri
kendi ADR'sini ister.

- **Saat alanı fikrinin genişletilmesi.** Volt'un ayırt edici fikri,
  saat alanı sınırının açık bir eşzamanlayıcı olmadan geçilememesidir
  (ADR-0002; E3001). Aynı fikir iki yeni sınıra uygulanabilir: değer
  kümesi sınırı (ikili ↔ üçlü: bir Trit değeri ikili bir sinyale ve
  tersi yalnız açık bir dönüşümle geçer) ve zaman modeli sınırı (saatli
  ↔ olay güdümlü: bir olay alanından saatli alana yalnız açık bir
  köprüyle geçilir). Bugünkü dilde ilk sınırın tohumu vardır: sayısaldan
  Trit'e dönüşüm yasaktır (E2009) ve Trit literali {−1, 0, +1}
  dışındaysa E2011'dir.
- **İkili–üçlü sınır kuralları bugünkü SV arka ucunda sınanabilir.**
  Trit ikili donanımda iki bitle taklit edilir (ADR-0062) ve bu taklit
  sentezde ölçülmüştür (`docs/design/ternary-olcum.md`). Bir sınır kuralı
  tip denetiminde tanı verir; kuralın kabul ettiği tasarımın SV'si
  Verilator'da koşturulur. Üçlü donanım olmadan da kuralın hem reddi hem
  kabulü sınanır.

Bu ADR nöromorfik ara gösterimler, ternary devre araştırmaları ya da
fotonik araçlar hakkında bir iddiada bulunmaz; Aşama 3 ve 4'ün ADR'leri
dış kaynakları kaynağıyla yazar.

### 5. Aşamalar

Her aşamanın "nasıl sınanır" sorusunun cevabı vardır; cevabı olmayan
aşama yazılmaz.

0. **Envanter** (bu ADR). `docs/design/ikili-varsayimlar-envanteri.md`:
   değer kümesi, kodlama ve zaman modeli eksenlerinde her yer, dilin mi
   arka ucun mu gereği, bilgi tek yerde mi dağınık mı.
   *Nasıl sınanır:* her satır `dosya:satır` ve fonksiyon adı taşır;
   satırlar o günün `main`'inde okunarak doğrulandı; kesin olmayan satır
   "belirsiz" diye işaretlidir. Belge davranış iddiası taşımaz.
1. **Trit'in uçtan uca tamamlanması, turnusol olarak.** Bugünkü tek
   ikili olmayan değer kümesi Trit'tir; o, kodlamanın tek yerde durup
   durmadığının sınavıdır. Trit tip denetiminden, SV'den, test dilinden
   (#430: test dilinde Trit portunu −1, 0, 1 değeriyle sürmek ve
   beklemek), dalga biçiminden, formal doğrulamadan ve LSP'den geçer; her
   biri Trit'in kodlamasını katmandan okur. Bunun için katman bit
   sayısının yanında Trit'in işaretini ve kodlarını da taşır (bugün
   `Scalar` yalnız bit sayısını taşır; envanterin "Trit'in temsilini bilen
   yerler" tablosu). `~t`'nin değer kümesi dışına çıkması (envanterin
   "Açık bulgular" 1) bu aşamadan önce bakımcının kararını ister.
   *Nasıl sınanır:* her araç zinciri adımı için `tests/ui/` geçen ve
   kalan testleri ve Verilator'da değer sınayan testler (#430'un testi
   dahil); Trit'in kodlamasını bilen yerlerin bir izin listesi testi
   (ADR-0137 §4'ün biçimi): katmanın dışındaki yeni bir yer testi
   düşürür, liste aşama ilerledikçe küçülür.
2. **Ara temsil kararı** (ADR-0136 §3). Bakımcı yeri seçer; seçilen ara
   temsil §3'ün gereksinimini karşılar.
   *Nasıl sınanır:* ara temsilin üç bilgiyi ayrı sunduğunu bir test her
   sinyal için okur; üretilen çıktı ADR-0136'nın yöntemiyle önceki
   hâliyle bayt bayt aynıdır.
3. **Melez sınır kuralları, ikili–üçlü.** Değer kümesi sınırı için tip
   kuralları ve tanı kodları, kendi ADR'siyle.
   *Nasıl sınanır:* Trit taklidiyle bugünkü SV arka ucunda: her kural
   için `tests/ui/fail/`'de tanı, `tests/ui/pass/`'te kabul; kabul edilen
   tasarımın değerleri Verilator'da elle hesaplanmış değerlerle; kural
   bir güvenlik denetimi olduğu için tohumlu hata derlemine en az bir
   mutasyon türü (ADR-0113).
   Saatli ↔ olay güdümlü sınır bu aşamada yoktur: bugün bir olay güdümlü
   modeli koşturacak ve sınayacak bir yol yoktur; o yol bulunana kadar
   §4'te yön olarak kalır.
4. **Bayrak arkasında deneysel ikinci arka uç.** Hedefi bu ADR seçmez.
   *Nasıl sınanır:* aynı tasarım için ikinci arka ucun çıktısı bir
   benzeticide ya da bağımsız bir başvuru modelinde koşturulur ve SV arka
   ucunun Verilator sonucuyla aynı değerleri verir (ADR-0108'in fark
   testi biçimi). Bu iki taraftan birini CI'da koşturmanın yolu yoksa
   aşama başlamaz.

### 6. Kapsam dışı ve açıkça reddedilenler

- **Bugün ternary, nöromorfik ya da fotonik bir arka uç yazmak.** Hedef
  yok, benzetici yok, kullanıcı yok; sınanamaz.
- **Her şeyi şimdiden genelleştirmek:** genişliği "bit ya da trit
  sayısı" yapmak, `bits<N>`'i bir taban parametresiyle soyutlamak, bit
  işlemlerini değer kümesinden bağımsız yazmak, bütün `u32` genişlik
  alanlarını birim taşıyan bir tipe çevirmek. Tek tüketici SV iken
  bunların hiçbiri sınanamaz ve her biri bugünkü kodu karmaşıklaştırır.
- **Dilin gereği olan ikiliyi taşımak.** `bits<N>` üzerindeki bit
  işlemleri, `uN` aralığı, ikili ve onaltılı literaller değer kümesinin
  tanımıdır; kodlama katmanına girmez.
- **`Spike`, `PTrit`, `spike` için mekanizma.** ADR-0028'in rezervasyonu
  olduğu gibi kalır (E0003).
- **Yeni tanı kodu, yeni dil yapısı, yeni seçenek.** Yoktur.

## Alternatifler

- **Hiçbir şey yapmamak, ihtiyaç doğunca bakmak.** #419'un dersi buna
  karşıdır: dağınık bir kodlama bilgisi sessiz yanlış üretir ve ihtiyaç
  doğduğunda yerlerin sayısı bugünkünden büyük olur. Envanter ucuzdur ve
  kodu değiştirmez.
- **Şimdiden genel bir değer kümesi soyutlaması** (tip başına taban
  parametresi, genel "basamak" birimi). Reddedildi (§6): sınanamaz, tek
  tüketicisi SV.
- **Kodlamayı SV üreticisinde toplamak.** Kodlama yalnız SV'nin bilgisi
  değildir: test dili, dalga biçimi, formal ve sürücüler de okur
  (envanterin kodlama ekseni). ADR-0136 ve ADR-0137 anlamsal bilginin
  arka uçta yeniden hesaplanmamasını ister.
- **Ternary'yi Volt'un dışında, ayrı bir dil ya da araç olarak tutmak.**
  Bakımcının hedefiyle çelişir; ayrıca Trit bugün dilin içindedir
  (ADR-0003, ADR-0062) ve onun tamamlanması ayrı bir dil gerektirmez.

## Sonuçlar

- Kod, üretilen çıktı ve tanılar değişmez. Bu ADR yalnız bir belge ve
  bir kural önerisi getirir.
- Bakımcı onaylarsa kodlama bilgisini yeni bir yerde çoğaltan değişiklik
  gözden geçirmede bu ADR'ye dayanılarak geri çevrilir; mekanik denetim
  (Trit kodlamasının izin listesi testi) Aşama 1'in işidir.
- ADR-0136 §3'ün seçiminde bir değerlendirme satırı daha vardır (§3).
- Envanter bir gün yaşlanır: satır numaraları 2026-11-25'in koduna
  aittir. Aşama 1'in izin listesi testi, envanterin kodlama ekseninin
  Trit satırlarını mekanik olarak güncel tutar.

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; kullanıcıdan yeni bir yazım istenmez.
   Aşama 3'ün sınır kuralları (gelecek ADR) açık dönüşüm isteyecekse
   onu İ1'e göre gerekçelendirir: derleyici değer kümesi sınırının hangi
   yönde geçileceğini çıkaramaz.
2. **Çıkarım (İ2):** çıkarımın kendisi değişmez. Kural, kodlama
   bilgisinin tek cevaplı tek bir çıkarımı olmasını ister (ADR-0137 ile
   aynı gerekçe: #419'da iki cevap vardı).
3. **Anlam değişikliği (İ3):** yok. Bu ADR'yi getiren PR derleyicinin
   kaynağını değiştirmez; sonraki her taşıma çıktının bayt bayt aynı
   kalmasıyla kabul edilir. Aşama 1'in #430'u reddedilen bir girdiyi
   kabul eder (davranış değişikliği, CHANGELOG'a girer).
4. **Niyet tahmini (İ4):** uygulanmaz.
5. **Metamorfik yeniden yazım:** uygulanmaz.

## Doğrulama

- Derleyici değişmez: PR `crates/` altında dosya değiştirmez; yalnız
  `docs/adr/` ve `docs/design/`.
- `just consistency` (ADR başlığı, bağlantılar, dizin) geçer.
- Envanterin satır numaraları 2026-11-25'te `main`'in (2503284d) koduna
  karşı okunarak doğrulandı: her `dosya:satır` başvurusunun (573) o
  satırda adlandırılan fonksiyonu, tipi ya da sabiti taşıdığı bir betikle
  denetlendi. Sınıfı kesin olmayan satır "belirsiz" işaretlidir.

## Sınırlar

- Envanter fonksiyon düzeyindedir ve bir metin taramasıdır: bir
  varsayımın kodun hangi davranışına yol açtığını koşturarak ölçmez.
- "Dilin gereği" ile "arka ucun gereği" ayrımı bugünkü spec'e ve
  ADR'lere göre yapıldı. Spec'in sustuğu yerde satır "belirsiz"dir; bir
  belirsiz satırın sınıfı bakımcının kararıdır.
- Zaman modeli ekseninde "dilin gereği" satırları bugünkü dilin
  saatli eşzamanlı anlamıdır; olay güdümlü bir modelin bunlardan
  hangisini değiştireceği bu ADR'nin sorusu değildir.
