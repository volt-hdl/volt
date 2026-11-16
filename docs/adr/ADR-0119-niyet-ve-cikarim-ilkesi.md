# ADR-0119: Niyet ve Çıkarım İlkesi — Sınırda Beyan, İçeride Çıkarım, Belirsizlikte Hata, Anlamda En Az Sürpriz

> Statü: Önerildi
> İlgili: ADR-0002 (alan anotasyonu: port sınırda beyan eder), ADR-0014 (hiçbir kontrat zorunlu değil: her şey için beyan istenmez), ADR-0099 (öneriler gidiş-dönüş testiyle sınanır), ADR-0100 (yama sürümü kural değiştirmez), ADR-0112 ve ADR-0116 (W2015, W2016: anlam değişmeden önce uyarı), ADR-0117 (metamorfik test: aynı niyetin iki yazımı aynı donanımı verir), ADR-0118 (K2: derleyici hiçbir şeyi kendiliğinden beyan saymaz)
> Issue: #266 (işlem genişliği, v0.2.0), #291 (modül düzeyi atama ile `comb` eşdeğerliği), #293 (kaçışların beyan biçimi)
> Tarih: 2026-11-16
> Etkilenen: yalnız süreç: bundan sonraki kural ADR'leri (§3). Kod değişikliği yok.

## Sorun

#233'teki sessiz yanlışların çoğunun ortak bir kökü var: derleyicinin
bir şeyi çıkarma ya da yorumlama kuralı, kullanıcının zihnindeki kuraldan
farklıydı ve fark hiçbir yerde söylenmiyordu.

- **#266:** `+ - *` işleminin genişliği bağlamdan çıkarılıyor; `a * b`
  yazan kullanıcı tam çarpımı bekliyor, sonuç işlemden önce kesiliyor.
- **#234:** saat, `clock` yerine bir tür takma adıyla yazılınca
  tanınmıyordu; aynı niyetin iki yazımı farklı denetlendi.
- **#204:** alan başlatma kısaltması (`S { x }`) uzun biçimle aynı
  şeyi demesine rağmen güvenlik denetimlerine görünmüyordu.

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
Beyan istemek varsayılan değildir: her şey için beyan istenmez (ADR-0014).

### İ2 — Çıkarım: içeride, yerel ve tek cevaplıysa

Derleyici şu durumda bir şeyi kendisi çıkarır:

1. **Sınırda beyan, içeride çıkarım.** Modülün sınırı (portlar) beyan
   eder; modülün içi çıkarımla çalışır. Bir modülün anlamı başka bir
   modülün gövdesindeki bir değişiklikle sessizce değişmez.
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
4. **Önce uyarı, sonra ana sürüm.** Yama sürümünde değişecek her yeri
   gösteren bir uyarı çıkar (W2015, W2016 gibi); anlam ana sürümde
   değişir (ADR-0100). Dün derlenen bir tasarım, aynı kodla ve hiçbir
   uyarı görmeden başka bir devre üretmez.

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
  sınanır: öneri uygulanınca kod derlenir ve tanı kaybolur.
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

- **Her şeyi beyan ettirmek.** Güvenli ama kod törensel yazıyla dolar,
  yeni başlayan boğulur (ADR-0014'ün gerekçesi).
- **Her şeyi çıkarmak.** Yazması kolay ama çıkarım kuralı ile kullanıcının
  zihni ayrıştığında sessiz yanlış doğar; #266 ve #234 bunun örneği.
- **Niyeti tahmin edip kodu sessizce düzeltmek.** Tasarımcının yazdığı
  ile çipe giden ayrışır; "0 hata" sözüyle bağdaşmaz.
- **İlkeyi yalnız AGENTS.md'ye yazmak.** Bu bir dil tasarımı ilkesidir,
  kural ADR'lerinin ölçütüdür; ADR'ler arasında durmalıdır.
