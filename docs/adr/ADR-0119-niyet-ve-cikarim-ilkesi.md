# ADR-0119: Niyet ve Çıkarım İlkesi — Sınırda Beyan, İçeride Çıkarım, Belirsizlikte Hata, Anlamda En Az Sürpriz

> Statü: Uygulandı
> İlgili: ADR-0002 (alan anotasyona bağlanır; tek saatli modülde çıkarılır, çok saatli modülde sınırda beyan zorunludur: domain-inference.md K3), ADR-0014 (hiçbir kontrat zorunlu değil: aynı ilkenin kontratlar için hâli), ADR-0099 (öneriler gidiş-dönüş testiyle sınanır), ADR-0100 (son hanede yeni tanı kodu yok; bu ADR bunu geçiş uyarısıyla genişletir, İ3), ADR-0112 ve ADR-0116 (W2015, W2016: geriye dönük olarak geçiş uyarısı), ADR-0114 (saatsiz modül özeti: İ2.1'in bilinen istisnası), ADR-0117 (metamorfik test: aynı niyetin iki yazımı aynı donanımı verir), ADR-0118 (K2: derleyici hiçbir şeyi kendiliğinden beyan saymaz), ADR-0120 ("İlke denetimi" bölümü taşıyan kural ADR'si), ADR-0122 ("İlke denetimi" bölümü taşır; ADR-0100'ü İ3'teki geçiş uyarısı gibi dar bir son hane istisnasıyla genişletir), ADR-0125 (İ4'ün tanılara uygulanışı: help'in üç durumu, kaçış son çare; "İlke denetimi" bölümü taşır), ADR-0127 ("İlke denetimi" bölümü taşır; İ1: `delay<K>` denetlenmeyen bir iddiadır)
> Issue: #266 (işlem genişliği, v0.2.0), #291 (modül düzeyi atama ile `comb` eşdeğerliği), #293 (kaçışların beyan biçimi)
> Tarih: 2026-11-16
> Etkilenen: yalnız süreç: bundan sonraki kural ADR'leri (§3). Kod değişikliği yok.

## Sorun

#233'teki ve ondan sonra bulunan sessiz yanlışların çoğu iki kökten
birine dayanıyor; ikisinde de fark hiçbir yerde söylenmiyordu.

**(a) Aynı kural birkaç yerde ayrı hesaplanıyor; aynı niyetin iki yazımı
farklı davranıyor.** #233'ün ortak kök tanımı budur. Şartname iki yazımı
zaten eşit sayıyordu; sorun uygulamadaydı.

- **#204:** alan başlatma kısaltması (`S { x }`) uzun biçimle aynı
  şeyi demesine rağmen güvenlik denetimlerine görünmüyordu.
- **#234:** saat, `clock` yerine bir tür takma adıyla yazılınca birkaç
  geçişte (sıfırlama alanı denetimi, SDC, test blokları, LSP) saat olarak
  tanınmıyor; aynı niyetin iki yazımı farklı denetleniyor.

**(b) Kural ile kullanıcının beklentisi farklı.** Derleyici kuralı doğru
uyguluyor, ama kural kullanıcının zihnindekinden farklı.

- **#266** (#233'ten sonra, #207 üzerinde çalışırken bulundu): `+ - *`
  işleminin genişliği bağlamdan çıkarılıyor; `a * b - a` yazan kullanıcı
  tam çarpımı bekliyor, çarpım onu okuyan çıkarmadan önce kesiliyor.

İ2.4 (yazımdan bağımsız çıkarım, ADR-0117'nin metamorfik testi) (a)'yı,
İ3 (anlam değişikliğinin koşulları ve geçiş uyarısı) (b)'yi karşılar.

Önümüzdeki kararlar aynı soruyu tekrar tekrar soracak: v0.2.0'daki
doğal genişlik (#266, #150), modül düzeyi atamanın `comb` ile eşdeğerliği
(#291), kaçışların beyan biçimi (#293) ve yol haritasındaki niyet
farkındalığı. Her kararda ölçütün yeniden tartışılmaması için ilke
yazılı olmalı.

## Karar

Volt, kullanıcının niyetini dört yoldan ele alır. Her birinin ne zaman
kullanılacağı aşağıdadır.

### İ1 — Beyan: derleyicinin çıkaramadığı ve denetleyebildiği şey için

Kullanıcıdan açık bir beyan (`sync()`, `declassify`, `Handshake` port,
port alanı, reset kutbu) ancak şu iki koşul birlikte sağlanınca istenir:

1. Derleyici bunu kendi başına çıkaramıyor ya da çıkarması riskli
   (yanlış çıkarımın bedeli büyük: saat alanı, gizli bilgi, protokol).
2. Beyan denetlenebiliyor: derleyici kodun beyana uyup uymadığını
   kanıtlar ya da beyanı kaçış kurallarına (ADR-0118) göre görünür kılar.

Denetlenmeyen bir beyan, beyan olmamasından kötüdür: sahte güven verir.
Beyan istemek varsayılan değildir: her şey için beyan istenmez; kontratlar
için aynı ilke ADR-0014'te yazılıdır (hiçbir kontrat zorunlu değil).

### İ2 — Çıkarım: içeride, yerel ve tek cevaplıysa

Derleyici şu durumda bir şeyi kendisi çıkarır:

1. **Sınırda beyan, içeride çıkarım.** Modülün sınırı (portlar) beyan
   eder; modülün içi çıkarımla çalışır. Bir modülün anlamı başka bir
   modülün gövdesindeki bir değişiklikle sessizce değişmez. Bilinen
   istisna: saatsiz modülün anotasyonsuz çıkışının alanı, örnek sitesinde
   çocuğun gövdesinden çıkarılan özetten gelir (ADR-0114); çocuğun gövdesi
   değişince ebeveyndeki denetim, yani tanı, değişebilir. Bu değişiklik
   sessiz değildir ve ebeveynin ürettiği donanımı değiştirmez.
2. **Yerel.** Çıkarımın sonucu, kullanıcının bakabileceği yakın bir
   kaynaktan gelir (bir `let`'in alanı, kaynaklarının alanından).
3. **Tek cevaplı.** İki cevap da mümkünse derleyici seçmez, hata verir
   ve iki yolu da gösterir. Sessiz seçim yoktur (cli-contract.md §4a'daki
   üst modül kuralı bunun örneğidir).
4. **Yazımdan bağımsız.** Aynı niyetin şartnamenin eşdeğer dediği iki
   yazımı aynı çıkarımı verir; ADR-0117'nin metamorfik testi bunu sınar.

### İ3 — Anlam değişikliği: dört koşulun dördüyle

Var olan bir kuralın anlamı, kullanıcının niyetine yaklaştırmak için
ancak dört koşul birlikte sağlanınca değişir:

1. **Yaygın yanlış okuma kanıtlanmış.** Bir issue, bir örnek ya da bir
   ölçüm (ör. W2016'nın derlemde kaç yerde çıktığı) yanlış okumanın tek
   tük değil yaygın olduğunu gösterir.
2. **Güvenli yönde.** Yeni anlam daha az bilgi kaybeder ya da daha çok
   hatayı görünür kılar.
3. **Maliyet görünür.** Değişiklik donanımı büyütüyorsa (daha geniş
   çarpıcı, daha çok yazmaç) bu kullanıcıya söylenir; donanım maliyeti
   sessizce artmaz.
4. **Önce geçiş uyarısı, sonra bir sonraki orta hane sürüm.** Anlamı
   değişecek her yeri gösteren bir geçiş uyarısı (aşağıda) önce çıkar;
   anlam bir sonraki orta hane sürümde (0.x'te `0.Y.0`, ADR-0100)
   değişir. Dün derlenen bir tasarım, aynı kodla ve hiçbir uyarı
   görmeden başka bir devre üretmez.

**Geçiş uyarısı (ADR-0100'ün genişletilmesi).** ADR-0100'e göre son hane
sürümü yeni tanı kodu getirmez. Bu ADR bunu tek bir durumda genişletir:
tek amacı bir sonraki orta hane sürümde anlamı değişecek yerleri
göstermek olan bir uyarı, hiçbir kuralı değiştirmeden ve hiçbir girdiyi
reddetmeden son hanede çıkabilir. W2015 (v0.1.1, ADR-0112) ve W2016
(v0.1.2, ADR-0116) geriye dönük olarak bu istisnadadır. İstisna yalnız
böyle bir uyarıyı kapsar; yeni seçenek ve yeni hata ADR-0100'e göre yine
orta hane ister.

Ayrıca anlam, SystemVerilog'daki karşılığından ancak bir gerekçeyle
uzaklaşır: Volt'la öğrenen kişinin sonra SystemVerilog okuyabilmesi bir
hedeftir (yol haritası, "Learning hardware design with Volt").

### İ4 — Niyet tahmini: yalnız söyle, asla kendiliğinden düzeltme

Derleyici kullanıcının olası niyetini tahmin edebilir: elle yazılmış bir
kalıbı tanımak, adı ile kullanımı uyuşmayan bir sinyali fark etmek, bir
tanıda "şunu mu demek istedin?" demek (yol haritası, "Intent
awareness"). Tahmin yalnız tanı metnine, uyarıya ya da öneriye girer:

- Kodun anlamını değiştirmez; üretilen donanım tahminden etkilenmez.
- Tahmine dayanan tanı hata değil uyarı ya da nottur (ad bir bilgi
  değildir, bir ipucudur).
- Öneri uygulanabilir biçimdeyse ADR-0099'daki gidiş-dönüş testiyle
  sınanır: öneri uygulanınca özgün tanı gider, yeni hata ya da uyarı
  çıkmaz ve hatasız kalan dosya `volt build` ile derlenir.
- Tanı önce kodun yazıldığı hâliyle ne anlama geldiğini, sonra olası
  niyetin yazımını gösterir.

### 3. Uygulama: her kural ADR'sinde "İlke denetimi"

Bu ADR'den sonra dilin bir kuralını ekleyen ya da değiştiren her ADR,
"İlke denetimi" başlıklı kısa bir bölüm taşır ve şu soruları cevaplar:

1. Kural kullanıcıdan bir beyan istiyor mu? İstiyorsa İ1'in iki koşulu
   nasıl sağlanıyor?
2. Kural bir şey çıkarıyor mu? Çıkarım sınırda mı içeride mi, yerel mi,
   belirsizlikte ne oluyor (İ2)?
3. Kural var olan bir anlamı değiştiriyor mu? Değiştiriyorsa İ3'ün dört
   koşulu ve hangi uyarının önce çıktığı.
4. Kural bir niyet tahmini içeriyor mu? İçeriyorsa İ4'e uygun mu?
5. Kural iki yazımı eşdeğer sayıyorsa, bunun için metamorfik yeniden
   yazım eklenebilir mi (ADR-0117)?

Bölüm bir sayfayı geçmez; cevabı "uygulanmaz" olan soru tek satırdır.

## Sınırlar

- İlke geriye dönük değildir: var olan ADR'ler yeniden yazılmaz. İlkeye
  uymayan bugünkü kurallar issue'larla izlenir (ör. saatsiz extern'in
  denetlenmeyen beyanı: #292, #293; işlem genişliği: #266).
- İ3'ün "yaygın yanlış okuma" ölçütü bir yargı içerir; son karar
  bakımcınındır. Bu ADR yalnız hangi kanıtın sunulacağını söyler.
- Kaçışlar (kullanıcının bilerek denetimi atlaması) ayrı ADR'dedir
  (ADR-0118); bu ADR kaçışın kendisini değil, beyanın ve çıkarımın genel
  ilkesini yazar.

## Reddedilenler

- **Her şeyi beyan ettirmek.** Güvenli ama kod zorunlu kalıp yazıyla
  dolar, yeni başlayan boğulur (UX Anayasası İlke 2: "10 satır boilerplate
  zorunluluğu" yasak kalıptır; kontratlar için aynı karar ADR-0014).
- **Her şeyi çıkarmak.** Yazması kolay ama çıkarım kuralı ile kullanıcının
  zihni ayrıştığında sessiz yanlış doğar; #266 bunun örneği.
- **Niyeti tahmin edip kodu sessizce düzeltmek.** Tasarımcının yazdığı
  ile çipe giden ayrışır; "0 hata" sözüyle bağdaşmaz.
- **İlkeyi yalnız AGENTS.md'ye yazmak.** Bu bir dil tasarımı ilkesidir,
  kural ADR'lerinin ölçütüdür; ADR'ler arasında durmalıdır.
