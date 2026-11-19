# ADR-0125: Tanı İlkesi — Ne Oldu, Neden, Ne Yapabilirsin; Kaçış Son Çaredir

> Statü: Önerildi
> İlgili: ADR-0099 (§2 gidiş-dönüş sözleşmesi ve öneri sınırları, §3 okura dönük metin: atıf `volt explain` satırıdır), ADR-0100 (son hane; bu ADR tanı metni değişikliklerinin yerini yazar), ADR-0118 (K2 önce düzeltme sonra kaçış; K3 gerekçe; §2 eksik işaretin önerisi), ADR-0119 (İ4: niyet tahmini yalnız söyler)
> Issue: #341 (üretilmeyen kodlar), aşamalar #345–#351 (§8); ADR-0124 (#343, henüz main'de değil): bu ilkenin ilk uygulaması
> Tarih: 2026-11-19
> Etkilenen: volt-diagnostics (`Diagnostic::validate`, explain metinleri), tanı üreten bütün crate'ler (metin), tests (explain örnek derleme testi, help kodu testi), `just consistency` (yeni kontroller), book (bilinen sınırlar)

## Sorun

Volt'un sözü: "0 hata" güvenilir. Sözün öteki yarısı şudur: derleyici
bir tasarımı reddettiğinde kullanıcı nedenini bilir ve ne yapabileceğini
görür. Derleyici kullanıcıya hiçbir zaman yalnızca "yapamazsın" demez.

Tanı dökümü (2026-11-19, `main` db63cf54, 164 kod; tanımlı her kodun
gerçek çıktısı iki dilde alındı ve puanlandı) bugünkü durumu şöyle ölçtü:

- UX Anayasası (`docs/design/Volt-UX-Anayasasi.md`, BÖLÜM IV ve X) her
  hatada `= neden:` satırını zorunlu tutuyor; gözlenen
  143 tanının 93'ünde bu satır yok. `Diagnostic::validate()` beş parçayı
  denetliyor, neden satırı bunlardan biri değil. 64 kodun tanısında kuralın
  ne için var olduğu hiç söylenmiyor (explain'de 145 kodda söyleniyor).
- 32 kodun help'i harfiyen uygulanınca çalışmıyor (6) ya da yalnız
  kısmen çalışıyor (26): hata sürüyor, yeni bir tanı çıkıyor ya da
  önerilen yollardan biri tutmuyor. Gidiş-dönüş testi 31 kodda var.
- Explain'inde tam modül örneği olan 34 kodun 21'inde örnek kendi başına
  derlenmiyor ya da kendi kodunu üretmiyor (12'si temiz; E3001'de yalnız
  ÖRNEK tam modül ve temiz); ÇÖZÜM blokları hiçbir testte derlenmiyor.
- E3009'da `declassify` tek ve ilk öneri, bedeli söylenmiyor. Çok bitli
  CDC'de (E3001, E3012, E3014) help `sync()` öneriyor; uygulanınca W3003.
- 19 kod hiç üretilmiyor, ama explain'leri onları çalışan denetimler gibi
  anlatıyor. E6003'ün explain'i "Volt yolun erişilemez olduğunu
  kanıtlayınca kabul eder" diyor; böyle bir kanıt yok, yanlış bir
  `@false_path` sessizce SDC'ye giriyor. E3004–E3008 olmayan mekanizmalar
  (`@isolate`, `@retain`) öneriyor.
- ADR-0099 §3'ü çiğneyen 4 yer (spec dosya yolu: 3'ü tanıda, 1'i W0024'ün
  explain'inde), 9 kodda EN/TR farkı.

Ortak desen #340'takiyle aynıdır, bkz. ADR-0124 (#343): kural yazılıydı,
denetimi yoktu. Bu ADR ilkeyi yazar ve her parçasının CI'da nasıl denetleneceğini
söyler.

## Karar

### 1. Her tanı üç şey söyler

1. **Ne oldu:** kodun yazıldığı hâliyle ne anlama geldiği (ana ileti ve
   etiket).
2. **Neden olmuyor:** hangi kural ve kuralın ne için var olduğu, sade
   dille (`= neden:` / `= reason:` satırı). Tanıda ADR numarası ve spec
   dosya yolu yazılmaz (ADR-0099 §3); kaynağa giden yol
   `= for more: volt explain <KOD>` satırıdır.
3. **Ne yapabilirsin:** `help` satırı, §2'deki üç durumdan biri.

Kapsam: hatalar ve uyarılar. Kapalı istisna: ayrıştırıcının "beklenen X,
bulunan Y" hataları (E0001, E0002); orada "beklenen" ifadesi kuralın
kendisidir ve neden satırı istenmez. Listeyi genişletmek yeni bir ADR
ister.

**Tanı bağlayıcıdır.** Explain tanıyı genişletir (NEDEN SORUN, ÖRNEK,
ÇÖZÜM), onunla çelişmez. Tanı ile explain farklı yol gösteriyorsa (bugün
W1004: tanı "`_` ile sustur", explain "sil ya da oku") bu bir hatadır.

### 2. "Ne yapabilirsin": üç durum

**(a) Volt'ta bir yol var.** Help yolu gösterir ve "bunu yaparsan olur"
der; yol kısa bir kodla yazılabiliyorsa kodu verir. Birden çok yol varsa
güvenli olan önce gelir.

**(b) Volt'ta yol yok, resmi bir kaçış var.** Help bunu açıkça söyler,
nedenini yazar, kaçışı bedeliyle anar (§3).

**(c) Ne yol ne kaçış var.** Help bunu açıkça söyler ve nedenini yazar.
Explain, bunun bir dil değişikliği önerisi olarak bildirilebileceğini
söyler.

Derleyici bir yolun olup olmadığını bilmiyorsa "yol yok" demez. Sabit
metinler:

| | Türkçe | İngilizce |
|---|---|---|
| Yol yok (b, c) | "Volt'ta bunun bugün bir yolu yok: <neden>." | "Volt has no way to do this today: <reason>." |
| Bilinmiyor | "Volt bunun için bilinen bir yol göstermiyor." | "Volt does not know a way to do this." |

Durum yalnız metinde görünür; JSON'a bir durum alanı eklenmez (ADR-0100).
Gerekirse v0.2.0'da ayrı karar.

### 3. Kaçış son çaredir ve isteğe bağlıdır

- Güvenli bir yol varsa önce o gösterilir. (a) durumunda kaçış ancak
  güvenli yoldan sonra, "bunu bilerek istiyorsan" koşuluyla anılır.
  Örnek E3009: önce akışı kaldırmak, sonra `declassify`.
- Kaçış her anıldığında bedeli yazılır: Volt bundan sonra neyi
  denetlemez, hangi iz uyarısı çıkar, kaçış raporunda görünür.
- **Kaçış hiçbir zaman gerekçesiyle birlikte tek adımda uygulanmaz.**
  Makine önerisi kaçışın yalnız iskeletini ekler; gerekçe alanı boş
  kalır. Gerekçesiz kaçış derleme hatası olduğundan (ADR-0118 K3)
  kullanıcı gerekçeyi kendisi yazmadan kod derlenmez. ADR-0118 §2'deki
  eksik extern işaretinin önerisi bu biçimde uygulanır.
- Kaçış iskeleti makine önerisi olarak yalnız (b) durumunda verilir.
  (a) durumunda kaçış metinde anılır, makine önerisi olmaz (E3009'da
  `declassify` önerisi metindir).
- Susturma kaçış değildir (`_` öneki, `@allow(unenforced)`, ADR-0118),
  ama aynı sıraya uyar: düzeltmeden sonra anılır.

### 4. Öneriler ve örnekler sınanır

- **Makine önerisi:** ADR-0099'un gidiş-dönüş testi (var olan kural).
  Makine önerisi taşıyan her kodun bir fikstürü olduğu CI'da denetlenir.
  Tek istisna kaçış iskeletidir (§3): uygulanınca özgün tanı gider ve tek
  yeni tanı, iskeletin boş gerekçesindeki gerekçesiz kaçış hatasıdır
  (ADR-0118 K3); test aynı dosyayı gerekçe doldurulmuş hâliyle de derler
  ve orada yeni hata ya da uyarı çıkmaz. Bu, ADR-0118 §2 ve §3'teki tek
  adımda uygulanan öneriyi daraltır: işaret tek adımda eklenir, gerekçeyi
  kullanıcı yazar.
- **Help'teki kod:** Help bir kod parçası veriyorsa (yalnız bir ad
  değil), bir test o kodu tetikleyiciye uygulanmış hâliyle derler: hata
  kaybolur, yeni bir hata ya da güvenlik uyarısı çıkmaz.
- **Explain ÖRNEK ve ÇÖZÜM:** Derleme testi: ÖRNEK kendi kodunu üretir,
  ÇÖZÜM o kodu ve başka bir hatayı üretmez. Parça bloklar bir test
  sarmalayıcısıyla derlenir (sarmalama kuralı uygulamada kesinleşir).
  Sarmalanamayan blok açıkça işaretlenir; işaretli blokların listesi
  yalnız küçülür.
- **Çok bitli CDC:** Help metni ADR-0099 §2'deki tek bit sınırına
  bağlıdır. Çok bitli veride `sync()` değil, stdlib yapısı (`AsyncFifo`,
  `HandshakeSync`) gösterilir.

### 5. Var olmayan denetim anlatılmaz

- Hiç üretilmeyen bir kodun explain'i ilk satırında "Volt bu denetimi
  bugün yapmıyor" der; denetim planlıysa hangi sürümde geleceğini ve
  kullanıcının bugün neye dikkat etmesi gerektiğini yazar (E6003:
  yanlış bir `@false_path` sessizce SDC'ye girer). Bu, ADR-0118'in bilinen
  sınırlar şartıyla aynı bilgidir; explain ondan muaf değildir.
- Var olmayan bir mekanizma (nitelik, primitif, seçenek) hiçbir tanıda ve
  explain'de önerilmez.
- Bir kodun kaldırılması ya da `Rezerve` statüsü #341'de karara bağlanır
  (ADR-0100'e göre v0.2.0).
- Kullanıcı yolunda her zaman başka bir tanının arkasında kalan kod
  (bugün E3011) üretilmeyen kod gibi ele alınır.
- İkincil tanılar da bu ilkeye uyar. Bir hatanın yanında anlamsız bir
  ikincil tanı (bugün E2029'un yanında "expected 'integer literal', found
  'integer literal'") bir hatadır.

### 6. Denetim: kural yazılı kalmaz

CI'da beş kontrol:

1. `Diagnostic::validate()` neden satırını da denetler (§1'deki istisna
   hariç).
2. Tanı, explain, `--list` ve doctor metninde ADR numarası ya da spec
   dosya yolu yoktur (ADR-0099 §3).
3. Explain'in bölüm yapısı (NEDEN SORUN, ÖRNEK, ÇÖZÜM) iki dilde de
   vardır.
4. Explain örnek derleme testi ve help kodu testi (§4).
5. Makine önerisi taşıyan her kodun gidiş-dönüş fikstürü vardır.

Her kontrol bugün uymayan kodların bir izin listesiyle açılır. Liste
yalnız küçülür; yeni bir kod listeye giremez. Böylece kontroller ilk
günden yeni tanıları korur, eskiler aşamalı düzelir.

### 7. Tanı metni ve yama sürümü (ADR-0100)

ADR-0100 tanı kodunu ve JSON alanını düzenler; tanının metni için bir şey
söylemez. Bu ADR boşluğu şöyle doldurur:

- İleti, neden, not ve help metni, explain metni ve makine önerilerinin
  içeriği son hanede değişebilir. Koşul: yeni tanı kodu, yeni seçenek,
  yeni JSON alanı yoktur; tanının hangi girdide çıktığı değişmez; üretilen
  SV, SVA, SDC ve sürücü çıktısı değişmez.
- CHANGELOG: makine önerisi eklenen, değişen ya da kaldırılan kodlar
  "Behavior changes" bölümünde listelenir, çünkü editörler ve araçlar
  bunları uygular. Salt metin düzeltmeleri tek satırda özetlenir ("N
  kodun tanı metni düzeltildi").
- Bir tanının tetikleyicisinin değişmesi (bir uyarının artık çıkması ya
  da çıkmaması) bu maddenin kapsamında değildir; ADR-0100'e göre
  değerlendirilir.

### 8. Uygulama sırası

Her aşama ayrı bir issue ve PR'dır.

1. **Yanıltıcı metin** (#345): üretilmeyen kodların explain'leri (önce E6003),
   E3004–E3008'deki olmayan mekanizmalar, ADR-0099 §3'ü çiğneyen 4 yer.
2. **Güvenlik tanılarında sıra ve bedel** (#346): E3009, E3014; E3001, E3012,
   E3014'te çok bitli veri.
3. **Çalışmayan öneriler** (#347): dökümün listesi, en sık görülenlerden başlayarak
   (E2003, E2009, E2010, E1010, E4011, E8506, W0022, W2010, W2015); her
   birine bir test.
4. **Explain örnek derleme testi** (#348): önce tam modül bloklar (34 kodda 63
   blok), sonra parçalar.
5. **Neden satırı** (#349): önce güvenlik ve doğruluk kodları (E0014, E0017,
   E3015, E4011, E8510, E8512, E9004, W2015, W2016, W3005, W3006), sonra
   en sık görülenler, sonra kalanı; `validate()` kontrolü izin listesiyle.
6. **EN/TR farkları** (#350): 9 kod.
7. **v0.2.0** (#351): üretilmeyen ve gözlenemeyen kodların kaldırılması
   ya da `Rezerve` statüsü (karar #341'de).

1–6 son hanede çıkabilir (§7).

## İlke denetimi

Bu ADR dilin bir kuralını değiştirmez; tanıların biçimini yazar.

1. Beyan (İ1): uygulanmaz.
2. Çıkarım (İ2): uygulanmaz.
3. Anlam değişikliği (İ3): hiçbir tasarımın anlamı ve üretilen çıktı
   değişmez (§7).
4. Niyet tahmini (İ4): §2 ve §3, İ4'ün tanılara uygulanışıdır. Öneri
   kodu kendiliğinden değiştirmez; kullanıcı uygular, öneri sınanır.
5. Metamorfik yeniden yazım (ADR-0117): uygulanmaz.

## Sınırlar

- Kullanıcıların hangi tanıyı ne sıklıkla gördüğü ölçülmedi; döküm test
  derlemini ve kitabı vekil olarak kullandı.
- CI neden satırının varlığını denetler, kalitesini denetlemez; kalite
  incelemede kalır.
- Durum (a, b, c) makinece okunur bir alan değildir (§2).

## Reddedilenler

| Seçenek | Neden |
|---|---|
| Kaçışı hiç anmamak | (b) durumunda kullanıcı yolsuz kalır ve Volt'un dışına çıkar. Resmi kaçış görünür ve denetlenebilir olduğu için anılmalıdır. |
| Kaçışı gerekçesiyle tek adımda uygulamak | Kaçış bilinçsizleşir; "susturma düğmesi"ne döner (ADR-0118 K2). |
| Tanıda ADR numarası ya da spec yolu | Okur bunları bilmez (ADR-0099 §3); kaynağa `volt explain` götürür. |
| Nedeni yalnız explain'de tutmak | Kullanıcı çoğu zaman explain'i açmaz; UX Anayasası nedeni tanıda ister. |
| Bütün kodları tek PR'da düzeltmek | 164 kod tek PR'da incelenemez; izin listeli kontroller ve aşamalar. |
