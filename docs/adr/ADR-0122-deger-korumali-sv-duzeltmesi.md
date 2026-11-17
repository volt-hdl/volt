# ADR-0122: Değer Korumalı SV Düzeltmesi Son Hanede Çıkabilir — Geçerli Tasarımda Duran Volt Komutu İçin ADR-0100 İstisnası (#303)

> Statü: Önerildi
> İlgili: ADR-0100 (§1'in son hane kuralını genişletir: sessiz yanlış istisnasının yanına ikinci bir istisna), ADR-0119 (İlke denetimi; İ3'ün geçiş uyarısı da ADR-0100'ü aynı biçimde genişletir)
> Issue: #303 (istisnanın ilk örneği; kanıtı PR #315)
> Tarih: 2026-11-17
> Etkilenen: yalnız süreç: sürüm numarasının seçimi (ADR-0100 §1) ve sürüm notunun "Behavior changes" bölümü (§3). Kod değişikliği yok.

## Sorun

ADR-0100 §1'e göre üretilen SV'nin değişmesi orta hane ister. Son hanede
tek istisna sessiz yanlıştır: başarılı görünüp yanlış sonuç veren bir
hatanın düzeltmesi.

#303 iki kuralın arasında kaldı. Aynı genişlikte ve aynı işaretli bir
`as` zinciri daha geniş bir bağlama girince üretici bileşik işleneni
sınırsız yazıyordu (`49'(((a | K1) & K2))`). SV'de değer doğruydu, ama
Verilator WIDTHEXPAND verdi; uyarı varsayılan olarak ölümcül olduğundan
geçerli bir Volt tasarımında `volt test` ve `volt run` durdu. Düzeltme
(PR #315) işleneni `$unsigned(…)`/`$signed(…)` ile yalıtır: üretilen SV
değişir, değer değişmez.

- Sessiz yanlış değildir: komut bir hatayla durdu, yanlış bir sonuç
  vermedi. ADR-0100'ün istisnası uygulanmaz.
- §1'in harfine göre üretilen SV değiştiği için orta hane ister. Oysa
  düzeltilen şey bir kullanıcının dün derlediği tasarımı bozmaz; tam
  tersine, dün koşmayan bir komutu koşturur. Kullanıcı bunun için bir
  sonraki orta hane sürümünü beklemek zorunda kalır, bir orta hane
  sürümü ise başka bozucu değişiklikler de getirebilir.

ADR-0100 §1'in orta hane kuralının amacı, son haneye yükselten
kullanıcının tasarımının aynı donanımı vermeye devam etmesidir. #303 gibi
bir düzeltme bu amacı bozmaz. Bu ADR, aynı soru her çıktığında yeniden
tartışılmasın diye istisnanın koşullarını yazar.

## Karar

### 1. İstisna

ADR-0100 §1'in son hane kuralı şu istisnayla genişler: **değer korumalı
SV düzeltmesi** son hanede (`0.Y.Z`, `Z > 0`) çıkabilir. Değer korumalı
SV düzeltmesi, üretilen SV'nin metnini değiştiren ve şu üç koşulu
birlikte sağlayan düzeltmedir:

**(a) Üretilen donanımın her değeri aynı kalır.** Düzeltmeden önceki ve
sonraki SV, aynı girdiye her çıkışta aynı değeri verir. Bu, en az iki
bağımsız benzeticiyle ya da önceki ve sonraki SV arasında bir
eşdeğerlik denetimiyle gösterilir.

- *Bağımsız*, ayrı kod tabanından gelen araç demektir (ör. Icarus ve
  Yosys `eval`). Durması giderilen araç bu ikisinden biri sayılmaz;
  düzeltmeden önce o araç zaten koşamıyordu.
- Benzetimle gösterimde iki araç aynı tasarım ve uyaran kümesini önce
  ve sonra koşturur, değerleri karşılaştırır. Küme, düzeltmenin metnini
  değiştirdiği biçimlerin her sınıfını kapsar; üreteci ve sayıları
  (tasarım, karşılaştırılan değer, fark) PR'da yazılıdır.
- Eşdeğerlik denetimi (ör. Yosys `equiv_*` ya da önceki ve sonraki
  modülün çıkışlarını karşılaştıran bir SBY kanıtı) değerlerin bütün
  girdilerde aynı olduğunu tek başına gösterir.

**(b) Düzeltme bir Volt komutunun geçerli bir tasarımda durmasını
giderir.** Geçerli tasarım, derleyicinin hatasız kabul ettiği
tasarımdır. Komut, kullanıcının çağırdığı bir Volt komutudur (`volt
test`, `volt run`, `volt build`, `volt verify`); durma, komutun bir
aracın (ör. Verilator'ın ölümcül uyarısı) ya da derleyicinin kendisinin
yüzünden hatayla bitmesidir. Yalnız görünüşü düzelten değişiklik
(okunabilirlik, ad, biçim, gereksiz parantez, uyarı vermeyen bir
yazımın daha temizi) bu istisnaya girmez; ADR-0100'e göre orta hane
ister.

**(c) Değişiklik sürüm notunun "Behavior changes" bölümüne girer.**
Madde hangi biçimin SV'sinin değiştiğini (önce ve sonra), değerin
değişmediğini ve hangi komutun artık durmadığını yazar; gündelik
`CHANGELOG.md` girdisi onu "**Davranış değişikliği.**" diye işaretler
(ADR-0100 §3).

### 2. Kapsamın dışında kalanlar

İstisna yalnız SV metnini kapsar. Yeni tanı kodu, yeni seçenek, bir
kuralın ya da bir değerin değişmesi, önceden kabul edilen girdinin
reddi bu istisnayla son hanede çıkmaz: ADR-0100'e göre orta hane ister
(değer değişiyorsa ve değişiklik bir sessiz yanlışı düzeltiyorsa ADR-0100'ün
kendi istisnası; anlam değişikliğinin geçiş uyarısı ADR-0119 İ3).

Üç koşuldan biri sağlanmazsa düzeltme orta hanededir. Koşulların
sağlandığına bakımcı karar verir; PR kanıtı (a) için sayılarıyla, (b) için
düzeltmeden önce duran komutun günlüğüyle sunar.

### 3. İlk örnek: #303

#303'ün düzeltmesi (PR #315) bu istisnayla son hanede çıkar:

- **(a)** #303 taraması (21 ifade × 15 dönüşüm zinciri × 6 bağlam, 2466
  derlenen tasarım): Icarus 14.0 ile 58 824 değer önce ve sonra
  şartnamenin modeliyle karşılaştırıldı, iki tarafta da 0 fark. Önce
  uyarı veren 180 tasarımda Yosys 0.69 `eval` 1416 değeri karşılaştırdı,
  iki tarafta da 0 fark. Fark testinin büyük koşusu (run 44112926993, dört
  parça, parça başına 1,3 milyonu aşkın değer) düzeltmeden sonra 0 fark
  verdi.
- **(b)** Düzeltmeden önce `volt test` ve `volt run` Verilator 5.054'ün
  WIDTHEXPAND uyarısıyla duruyordu: kanıt dalının kırmızı CI'ı (run
  44109218459, "Verilator lint" işi), PR #315'te alıntılı.
- **(c)** `CHANGELOG.md`'nin "Yayımlanmadı" bölümündeki #303 maddesi
  "**Davranış değişikliği.**" diye işaretli; sürüm kesilirken
  `### Behavior changes` altına girer.

## İlke denetimi

1. **Beyan:** uygulanmaz; istisna kullanıcıdan bir şey istemez.
2. **Çıkarım:** uygulanmaz; istisna derleyicinin bir şey çıkarmasıyla
   ilgili değildir.
3. **Anlam değişikliği:** yok. (a) koşulu değerin değişmesini dışarıda
   tutar; değişen yalnız SV metnidir. İ3'ün koşulları ve geçiş uyarısı
   gerekmez. ADR-0100'ün genişletilmesi İ3'teki geçiş uyarısıyla aynı
   biçimdedir: ADR-0100'ün amacını bozmayan dar bir durum için son hane.
4. **Niyet tahmini:** uygulanmaz.
5. **Metamorfik yeniden yazım:** uygulanmaz; istisna dilde iki yazımı
   eşdeğer saymaz. (a)'nın eşdeğerlik denetimi önceki ve sonraki SV
   arasındadır, kaynakta değil.

## Sınırlar

- **Benzetim bir örneklemdir.** İki benzetici yalnız denenen tasarım ve
  uyaranlarda aynı değeri gösterir; eşdeğerlik denetimi bütün girdileri
  kapsar. Benzetim kanıtının yeterli olup olmadığı (kümenin düzeltmenin
  değiştirdiği her biçimi kapsayıp kapsamadığı) bakımcının yargısıdır.
- **SV metnine dayanan kullanıcı.** SV'yi metin olarak karşılaştıran ya da
  satır numarasına bağlanan bir akış (ör. bir lint bekleme listesi)
  değişikliği görür. Bunu "Behavior changes" maddesi (c) söyler; değer
  aynı olduğundan sentez ve benzetim etkilenmez.
- **Başka bir araçta yeni uyarı.** Koşullar düzeltmenin başka bir araçta
  (sentez, Yosys, `-Wall`) yeni bir uyarı getirmediğini ayrıca istemez;
  #303'te `-Wall` uyarı sayısı önce ve sonra aynıydı. Böyle bir uyarı
  (b)'deki gibi bir komutu durduruyorsa düzeltme (b)'yi bozar.

## Reddedilenler

- **#303'ü sessiz yanlış istisnasıyla çıkarmak.** Komut bir hatayla durdu;
  sessiz değildi. Sessiz yanlış istisnasını genişletmek, iki farklı
  durumu (yanlış değer ve durma) tek tanıma sıkıştırırdı.
- **Orta haneyi beklemek** (ADR-0100'ün harfi). Geçerli tasarımda
  komutun durması bir sonraki orta hane sürümüne dek sürer; o sürüm
  başka bozucu değişiklikler de getirebilir.
- **Her SV değişikliğini son haneye almak.** Görünüş düzeltmeleri de
  girerdi; SV metnine dayanan kullanıcı için son hane güvenli yükseltme
  olmaktan çıkardı. (b) koşulu istisnayı bir komutun durmasının
  giderildiği durumla sınırlar.
- **Tek benzeticiyle yetinmek.** Bir benzeticinin hatası (#145, #302)
  iki biçimde aynı yanlış değeri verebilir; bağımsız ikinci araç ya da
  eşdeğerlik denetimi bunu ayırır.
