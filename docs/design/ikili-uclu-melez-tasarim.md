# İkili–Üçlü Melez Tasarım Notu

> Statü: Tasarım notu — bağlayıcı değil. Buradaki her fikir uygulanmadan önce kendi ADR'sini ve "nasıl sınanır" cevabını ister (ADR-0139).
> İlgili: ADR-0003 (Trit), ADR-0062 (çarpansız ternary MAC), ADR-0077 Karar 3 (struct `as uN`), ADR-0136 §3 (IR kararı), ADR-0139 (ikilinin ötesi), ADR-0141 (`is_signed`, `CheckedTypes`)
> Issue: #430, #434, #436
> Tarih: 2026-11-26

## Amaç

Bir donanım tasarım aracının ikili ve üçlü sistemi birlikte, verimli ve
güvenli kullanabilmesi için ne gerektiğini tek yerde toplamak. Not, yol
haritasındaki Aşama 3'ün (melez sınır kuralları) başlangıç noktasıdır;
ne zaman başlanacağı aşağıda, §6'da.

Ana fikir: verim her şeyi üçlü yapmaktan gelmez. Üçlü, verinin kendisi
üç değerli olduğu yerde kullanılır; ikili geri kalan her yerde kalır;
aradaki sınır açık, denetlenen ve ucuz olur. Bu, Volt'un saat alanları
için yaptığının (`domain`, `sync()`, E3001) değer kümelerine
genellemesidir.

## 1. Üçlü nerede kazandırır, ikili nerede kalır

Üçlünün kazandırdığı yerler:

- **Üçlü ağırlıklı sinir ağları.** Ağırlık −1/0/+1 olunca çarpma
  "çıkar / atla / ekle" olur; çarpan gerekmez (ADR-0062). Sıfır ağırlık
  seyrekliktir: o işlem hiç yapılmayabilir.
- **İşaretli basamaklı sayı sistemleri.** {−1, 0, +1} basamaklarıyla
  toplamada elde uzun zincir boyunca yayılmaz; hızlı toplayıcı ve
  çarpıcılarda (Booth kodlaması) bugün ikili silikonda kullanılır.
- **Simetrik değerler.** Dengeli üçlüde negatif sayı için ayrı işaret
  mantığı yoktur; yuvarlama kesmeyle aynıdır.

İkili kalanlar: denetim mantığı ve durum makineleri, adresler, bellek
arayüzleri, dış arayüzler (AXI, UART, I2C) ve **toplayıcılar** (üçlü
girdilerin toplamı artık üç değerli değildir).

Doğal melez yapı: **girdiler ve ağırlıklar üçlü; toplama ve denetim
ikili.**

## 2. Değer kümesi, kodlama, depolama

ADR-0139'un üç bilgisi (değer kümesi, kodlama, zaman modeli) burada
verim kaynağıdır. Aynı üç değerli veri kullanıldığı yere göre farklı
kodlanabilir:

| Kodlama | Nerede iyi | Bedeli |
|---|---|---|
| 2 bit işaretli (bugünkü Volt: `logic signed [1:0]`) | Aritmetik; doğrudan toplanır | Bir kod (`10`) geçersiz |
| (sıfır değil, işaret) | "Sıfırsa atla" tek bite bakarak; güç | Toplamadan önce dönüşüm |
| (artı, eksi) iki tel | Toplama/çıkarma seçimi doğrudan | `11` geçersiz |
| 5 trit = 8 bit (3⁵ = 243 ≤ 256) | Bellekte depolama: trit başına 1,6 bit, 2 bitlik koda göre %20 az yer | Okuma/yazmada çözücü/kodlayıcı |
| Yerel üçlü tel | Gelecekteki üçlü cihazlar | Bugün yok (araştırma aşaması) |

İlke: **depolama kodlaması ile hesaplama kodlaması farklı olabilir;
dönüşüm sınırda yapılır.** Kodlama bilgisi tek yerde durmalıdır
(ADR-0139); bugün o yer `TypeArena::bit_width` ve `TypeArena::is_signed`'ın
`Trit` kollarıdır (ADR-0141). Kodlama koda dağılırsa değiştirilemez.

## 3. Sınır kuralları (Aşama 3 adayı)

Saat alanları arasında `sync()` nasıl zorunluysa, değer kümeleri
arasında da açık dönüşüm zorunlu olmalıdır:

- **İkili → üçlü kayıplıdır (nicemleme).** Hangi eşik, hangi yuvarlama
  açıkça yazılır. Örtük nicemleme reddedilir (Volt "tahmin etmez").
- **Üçlü → ikili kayıpsızdır** (işaretli tamsayıya genişletme). Basit
  kalabilir; yine de görünür olmalıdır.
- **İşlem değer kümesine aittir.** Bir değer kümesinin işlemi ötekinin
  koduna sızmaz. #434 bunun örneğiydi: ikili `~`, Trit'in 2 bitlik koduna
  uygulanınca sonuç üç değerin dışına çıkıyordu; ADR-0142 ile `~` Trit'te
  reddedildi (E2003, yerine `-t`). Struct `as uN` ile kodun
  dilden görünmesi (ADR-0077 Karar 3) aynı türden bir sızıntıdır.

Bugün sınanabilir: Trit taklidiyle SV'de; ui testlerinde reddedilmesi
gereken melez yazımlar (fail) ve kabul edilen açık dönüşümler (pass).

## 4. Derleyicinin verim için yapabilecekleri

Volt arkadan iş çevirmez; aşağıdakiler gizli optimizasyon değil, kesin
kural, kontrat ve rapordur. Her biri "nasıl sınanır" cevabıyla:

| Fikir | Ne | Nasıl sınanır |
|---|---|---|
| Kesin toplayıcı genişliği | N tritin toplamı −N…+N; ⌈log₂(2N+1)⌉ bit, işaretli (ör. 256 trit → 10 bit) | Formal: en kötü durumda taşma yok; genişlik gölgesinde eşitlik |
| Geçersiz kod kontratı | 2 bitlik kodda `10` hiç oluşmaz; derleyici her üçlü register için değişmezi kendisi yazar (Handshake'in otomatik kontratları gibi, ADR-0050) | Formal kanıt; tohumlanmış hata ile `10` üretilip yakalandığının gösterilmesi |
| Açık sıfır atlama | "Ağırlık sıfırsa işlem atlanır" tasarımcıca yazılır; derleyici maliyetini raporlar | Verilator toggle sayısı ya da Yosys hücre sayısıyla karşılaştırma |
| Paketli depolama | 5 trit ↔ 8 bit kodlayıcı/çözücü | 243 değerin tamamında gidiş-dönüş (kapsamlı test) |
| Maliyet raporu | Aynı tasarımın ikili ve üçlü sürümünün alan/güç karşılaştırması | `docs/design/ternary-olcum.md`'nin yöntemi; Yosys sentezi |

İlk ikisi küçüktür ve Aşama 1 (#436) sırasında bile değerlendirilebilir.

## 5. Bugün ve yarın

Bugünkü FPGA ve ASIC'ler ikilidir. Üçlünün getirisi şu an yerel üçlü
tellerden değil, aritmetiğin sadeleşmesinden (çarpansız MAC, seyreklik)
ve bellekte paketlemeden gelir. Doğru mimari: **aynı tasarım kaynağı,
kodlama değiştirilerek bugün ikili silikona, yarın yerel üçlüye
gidebilmeli.** Ön koşulu, IR'nin değer kümesini, kodlamayı ve zaman
modelini ayrı taşımasıdır (ADR-0136 §3, ADR-0139). ADR-0141'in
`CheckedTypes` tablosu bu IR değildir; taşıdığı genişlik ve işaret
kodlama bilgisidir.

## 6. Ne zaman

Tarih yok; sıra ve koşul var (bakımcı, 2026-11-26; `docs/roadmap.md`):

1. **Şimdi (v0.1.x):** yalnız sigorta. Yeni kod ikili varsayımı
   çoğaltmaz; Trit'in sessiz yanlışları (#434) doğruluk işi olarak
   düzeltilir (0.1.4'te düzeltildi, ADR-0142).
2. **v0.2'ye doğru:** Aşama 1 (#430, #434, #436) ve Aşama 2 (IR kararı).
3. **Aşama 3 ve 4:** aşağıdakilerden en az ikisi oluşunca:
   - bilinen sessiz yanlış listesi büyük ölçüde boşalmış ve 1.0
     kararlılık politikası yazılmış;
   - ikinci düzenli katkıcı var;
   - gerçek talep: üçlü ağırlıklı ağ hızlandırıcısı ya da nöromorfik
     tasarım isteyen bir kullanıcı, araştırma grubu ya da ortak;
   - sınanabilir hedef: açık bir simülatör ya da FPGA'da ölçülebilir
     melez tasarım.

## Açık sorular

- Kodlama seçimi nasıl yazılır: tipin parametresi mi, bildirim
  anotasyonu mu, modül düzeyinde bir varsayılan mı? (Sözdizimi
  önerilmedi; ADR ister.)
- Kodlama dilden tamamen gizlenmeli mi? Bugün struct `as uN` ile
  görünüyor (ADR-0077 Karar 3); gizlemek ayrı bir dil kararıdır.
- Nicemleme kuralları dilde mi, kütüphanede mi? (#403'teki "sayı
  kuralları ailesi" sorusuyla aynı eksen.)
- Ayrılmış `PTrit`, `Spike` ve `on spike` (ADR-0028) bu sınır
  modeline nasıl oturur? Zaman modeli farklı olan değer kümeleri için
  sınır, saat alanı geçişiyle değer kümesi geçişinin birleşimidir.
