> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Volt HDL — İçerik Pazarlaması Stratejisi

> Araştırma tabanlı. Kaynaklar: daily.dev Ads (2026), DEV Community,
> arXiv:2506.12643 (HN açık kaynak tanıtım analizi), markepear.dev
> Tarih: Ağustos 2026

---

## Bölüm 1 — Temel Gerçek: Dağıtım Tek Gerçek Hendek

```
2026 gerçeği:
  "AI inşa etme kısmını metalaştırdı.
   Herkes bir hafta sonunda çalışan ürün çıkarabiliyor.
   Zor kısım, insanların size güvenmesini sağlamak."

Volt için anlamı:
  Teknik üstünlük yeterli değil
  Arch, Spade, Veryl de teknik olarak iyi
  Fark: kim bulunur, kim güvenilir
```

**Açık kaynak bu güven problemini başka hiçbir pazarlama
kanalının çözemediği şekilde çözüyor.** GitHub repo'su aynı
anda landing page, güven sinyali ve dağıtım motoru.

---

## Bölüm 2 — Kanal Haritası (Volt'a Özgü)

Genel dev tool tavsiyeleri Volt için kısmen geçerli.
Donanım topluluğunun kendi kanalları var.

```
KANAL                      KİTLE              ÖNCELİK  ZAMAN
──────────────────────────────────────────────────────────────
Hacker News (Show HN)      Genel teknik       ★★★★★   F0 sonrası
r/FPGA                     FPGA mühendisi     ★★★★★   F0 sonrası
r/rust                     Rust geliştirici   ★★★★☆   F1 sonrası
LLVM Discourse (CIRCT)     Derleyici uzmanı   ★★★★★   F0 öncesi!
Zero to ASIC Discord       ASIC öğrencisi     ★★★★☆   F2 sonrası
FPGA Discord/Matrix        Hobi topluluğu     ★★★★☆   F1 sonrası
Twitter/X (#FPGA, #EDA)    Karışık            ★★★☆☆   sürekli
LinkedIn                   Kurumsal           ★★☆☆☆   V1 sonrası
DVCon / ORConf / FPL       Akademi+endüstri   ★★★★★   Yıl 2
Product Hunt               Genel              ★☆☆☆☆   ATLA
```

**Kritik fark:** Product Hunt 2026'da satüre oldu — görünürlük
24 saatlik oy penceresi ve mevcut sosyal sermaye tarafından
belirleniyor, ürün kalitesi değil. Donanım araçları için zaten
yanlış kitle.

---

## Bölüm 3 — Show HN: Rakamlar ve Taktikler

### 3.1 Beklenebilecek Etki

```
Başarılı bir Show HN:
  5,000-50,000 ziyaretçi (48 saat)
  Açık kaynak araçlar için upvote başına 1.4 GitHub yıldızı

  → 300 upvote ≈ 420 yıldız
  → 500 upvote ≈ 700 yıldız

Ön sayfa için gereken: ~30+ upvote (ilk saat kritik)
```

### 3.2 Zamanlama

```
En iyi: Salı-Perşembe, 09:00-12:00 ET
        (Türkiye saati: 16:00-19:00)

Niş projeler için alternatif: Pazar 19:00 ET

İlk 30-60 dakika mutlak kritik:
  Erken upvote momentumu
  Yorum kalitesi
  Hesap güvenilirliği
  → Bu üçü sıralamayı belirliyor
```

### 3.3 Başlık Formülü

```
Format: "Show HN: [Ad] – [ne yaptığı, teknik]"

❌ KÖTÜ:
  "Show HN: Volt – Revolutionizing Hardware Design with AI"
  → Pazarlama jargonu, HN nefret eder

  "Show HN: Volt – A new HDL"
  → Neden umursayayım?

✓ İYİ:
  "Show HN: Volt – An HDL that catches clock domain
   crossing bugs at compile time"
  → Somut sorun, somut çözüm, doğrulanabilir

  "Show HN: Volt – CDC errors won't compile"
  → Kısa, iddialı ama kanıtlanabilir
```

**Kural:** "5 yaşındaki çocuğa değil, 5 yıllık geliştiriciye
anlatır gibi yaz." Teknik derinlik güven yaratıyor.

### 3.4 Post İçeriği Şablonu

```markdown
Show HN: Volt – An HDL that catches CDC bugs at compile time

Verilog'da bu sessizce derlenir ve genellikle silisyumda
ortaya çıkar:

    always_ff @(posedge fast_clk)
        slow_reg <= fast_signal;   // CDC ihlali

Volt'ta derlenmez:

    error[E3001]: iki farklı saat alanı doğrudan bağlanamaz
      ┌─ design.volt:12:14
      │
    12│     slow_out = fast_data
      │                ^^^^^^^^^ 'fast_data' → fast_clk alanında
      │     ^^^^^^^^ 'slow_out' → slow_clk alanında
      │
      = neden: sinyal kararsız bir anda yakalanabilir
      = çözüm: slow_out = sync(fast_data, slow_clk)

Standart SystemVerilog üretiyor — mevcut araç zincirinizle
(Vivado, Yosys, DC) çalışıyor.

Neden yaptım: [1-2 cümle kişisel motivasyon]

Durum: erken alpha. Counter → SV → Verilator zinciri çalışıyor.
Tip sistemi geliştiriliyor.

Repo: github.com/volthdl/volt
Playground: play.volthdl.org
```

**Kritik unsurlar:**
- Kod örneği ilk 10 satırda
- Karşılaştırma (öncesi/sonrası)
- Dürüst durum beyanı ("erken alpha")
- Kayıt/signup gerektirmiyor
- Doğrulanabilir (repo linki)

### 3.5 Yorum Yönetimi

```
İlk 60 dakika:
  Her yoruma yanıt ver
  Eleştiriye savunmacı olma
  "Haklısınız, bu eksik" demek güven yaratıyor

Beklenecek yorumlar (hazırlıklı ol):
  "Chisel/Amaranth zaten var" → hazır yanıt
  "Clash bunu 16 yıldır yapıyor" → hazır yanıt
  "Verilog yeterli" → sayılarla yanıtla (%14 ilk silisyum)
  "Bu ne için?" → somut senaryo ver

Eleştiriye düşünceli yanıtlar güven inşa ediyor
ve momentumu sürdürüyor.
```

---

## Bölüm 4 — İçerik Türleri ve Etkileri

### 4.1 "Yıkım Hikayeleri" (En Yüksek Etki)

Donanım topluluğunda en çok okunan içerik türü: gerçek hatalar.

```
Başlık örnekleri:
  "The $475M Bug: How CDC Errors Reach Silicon"
  "Why 86% of ASIC Projects Fail First Silicon"
  "I Analyzed 50 Chip Re-spins. Here's What Broke."

Yapı:
  1. Gerçek olay (Pentium FDIV, Ariane 5, gerçek re-spin)
  2. Kök neden analizi
  3. Neden mevcut araçlar yakalayamadı
  4. Volt'un yapısal yanıtı (son %20)

Neden işe yarıyor:
  Sorunu yaşayan herkes okuyor
  Volt satılmıyor, sorun anlatılıyor
  Çözüm doğal sonuç olarak beliriyor
```

### 4.2 Teknik Derin Dalış

```
Konular (sırayla yayımla):
  "Why Clock Domain Crossing Should Be a Type Error"
  "Building an HDL Compiler in Rust: Lessons from
   rust-analyzer's Architecture"
  "CIRCT: What MLIR Brings to Hardware Design"
  "Linear Types for Hardware Ports"
  "Why LLMs Write Bad RTL (and What Type Systems Can Do)"

Bu son başlık özellikle güçlü:
  AssertEval verisi (%63 semantik hata) somut
  AI + donanım kesişimi trend
  Volt'un tip sistemi doğal çözüm olarak çıkıyor
```

### 4.3 Karşılaştırma İçeriği

```
"Volt vs Arch vs Spade vs Veryl: An Honest Comparison"

Kural: rakipleri dürüstçe övmek
  Spade'in lineer tipleri gerçekten iyi → söyle
  Veryl'in araç ekosistemi olgun → söyle
  Arch'ın todo! mekanizması akıllı → söyle
  Clash CDC'yi 16 yıldır yapıyor → söyle

Sonra: "Volt bunları neden birleştiriyor"

Neden işe yarıyor:
  Dürüstlük güven yaratıyor
  Rakip topluluklardan trafik geliyor
  "Bu adamlar alanı biliyor" algısı
```

### 4.4 Yapım Günlüğü (Build in Public)

```
Haftalık kısa güncelleme:
  Ne yapıldı, ne kırıldı, ne öğrenildi

Platform: GitHub Discussions + Twitter/X thread

Örnek:
  "Hafta 12: Tip çıkarımı çalışıyor ama CDC kontrolü
   yanlış pozitif veriyor. Sorun: domain inference
   fonksiyon sınırlarında kayboluyor. Filament'in
   yaklaşımına bakıyorum."

Neden işe yarıyor:
  Süreklilik → algoritma seni hatırlıyor
  Şeffaflık → güven
  Sorunlar → topluluk yardım ediyor
```

### 4.5 Video ve Görsel

```
Asciinema (terminal kaydı):
  30 saniyede: volt build → hata → düzelt → çalışıyor
  → README'ye gömülebilir, hafif

Screencast (5-10 dk):
  "Building a UART in Volt"
  → YouTube + repo linki

Görsel önemli çünkü:
  HDL öğrenmek soyut
  Çalışan bir şey görmek ikna edici
```

---

## Bölüm 5 — Zamanlama: Ne Zaman Ne Yayımlanmalı

```
F0 ÖNCESİ (şimdi):
  ✗ Show HN YAPMA — kod yok
  ✓ LLVM Discourse'da teknik soru sor
    "CIRCT'te reg semantiği için seq mi FIRRTL mi?"
    → Sessiz tanıtım, uzman ağı kuruluyor
  ✓ Domain al, GitHub org aç, boş repo hazırla

F0 SONRASI (counter → SV → Verilator çalışıyor):
  ✓ İlk blog yazısı: "Why CDC Should Be a Type Error"
  ✓ r/FPGA'da paylaş (Show HN'den ÖNCE)
    → Daha küçük kitle, feedback al, düzelt
  ✓ 2-3 hafta sonra Show HN

F2 SONRASI (tip sistemi çalışıyor):
  ✓ Show HN (asıl büyük lansman)
  ✓ Playground canlı olmalı
  ✓ 5+ örnek tasarım hazır

F5 SONRASI (beta):
  ✓ Konferans bildirisi (DVCon, ORConf, FPL)
  ✓ Karşılaştırma yazısı
  ✓ Tiny Tapeout submission → "Volt ile üretildi"

V1 SONRASI:
  ✓ Vaka çalışması (gerçek kullanıcı)
  ✓ Kurumsal içerik (LinkedIn)
  ✓ Akademik makale
```

**En kritik hata:** Kod olmadan Show HN yapmak.
Bir kez yaptın, ikinci şans yok.

---

## Bölüm 6 — Donanım Topluluğuna Özgü Taktikler

Genel dev tool tavsiyelerinde olmayan, bu alana özgü:

### 6.1 Tiny Tapeout Etkisi

```
Volt ile tasarlanan bir çip Tiny Tapeout'tan çıkarsa:
  → "Gerçek silikon" kanıtı
  → $300 maliyet
  → Topluluk çok ilgi gösteriyor
  → Fotoğraf + hikaye = viral içerik

Bu tek eylem, on blog yazısından etkili.
```

### 6.2 Referans Tasarım Kütüphanesi

```
Volt ile yazılmış, açık kaynak, çalışan tasarımlar:
  UART, SPI, I2C
  FIFO (senkron + asenkron)
  RISC-V ALU
  Ternary sistolik dizi

Neden pazarlama:
  Kopyala-yapıştır ile başlanabiliyor
  Google'da "verilog uart" arayanlar buluyor
  Kod okunuyor → dil öğreniliyor
  SEO değeri yüksek
```

### 6.3 Akademik Kanal

```
Donanım topluluğunda akademi ağırlıklı:
  arXiv preprint → görünürlük
  DVCon, ORConf, FPL, ICCAD → bildiri
  Üniversite dersi → uzun vadeli benimseme

Veryl bunu yaptı: DVCon Japan 2024, ISCA 2025
Spade bunu yaptı: FPL 2022, ACM TRETS 2026
→ Akademik meşruiyet bu alanda önemli
```

### 6.4 Karşı Örnek: Neyi Yapmamalı

```
✗ "AI-native HDL" konumlandırması
  → Arch aldı + terim tüketildi + yanlış kitle

✗ Ternary'yi öne çıkarmak
  → "Ezoterik araştırma" algısı

✗ "Golden spec çözdük" iddiası
  → 30 yıllık şüphe

✗ Kurumsal ton
  → HN ve r/FPGA bunu reddediyor

✗ Kayıt gerektiren playground
  → Ürününüz karmaşık signup olmadan erişilebilir olmalı
```

---

## Bölüm 7 — Ölçüm

```
METRİK                     HEDEF (Yıl 1)   NASIL ÖLÇÜLÜR
─────────────────────────────────────────────────────────────
GitHub yıldızı             500-1,500       GitHub
Benzersiz klon             200+/ay         GitHub Insights
Discord üye                100-300         Discord
Playground kullanımı       1,000+/ay       analytics
Blog okuma                 5,000+/yazı     analytics
Katkıcı sayısı             5-15            GitHub
Issue/PR aktivitesi        haftalık        GitHub

Gerçek başarı göstergeleri (yıldızdan önemli):
  Birinin Volt ile bir şey yapıp paylaşması
  Birinin bug report açması (kullanıyor demek)
  Birinin PR göndermesi (yatırım yapıyor demek)
  Rakip projelerin Volt'tan bahsetmesi
```

---

## Bölüm 8 — 12 Aylık İçerik Takvimi

```
AY   İÇERİK                                  KANAL
─────────────────────────────────────────────────────────────
0    CIRCT teknik sorusu                     LLVM Discourse
1    (F0 çalışıyor) "Why CDC Type Error"     Blog + r/FPGA
2    Show HN lansmanı                        HN
2    Asciinema demo                          README
3    "Building HDL Compiler in Rust"         Blog + r/rust
4    Referans tasarım: UART                  GitHub + r/FPGA
5    "Why LLMs Write Bad RTL"                Blog + HN
6    Playground lansmanı                     HN + Twitter
7    "Honest HDL Comparison"                 Blog
8    Referans tasarım: RISC-V ALU            GitHub
9    Tiny Tapeout submission                 Tüm kanallar
10   "Linear Types for Hardware"             Blog + r/rust
11   Konferans bildirisi hazırlığı           arXiv
12   Beta duyurusu + yıl özeti               Tüm kanallar

Haftalık (sürekli):
  Build-in-public güncellemesi (Twitter/GitHub Discussions)
```

---

## Bölüm 9 — Tek Kişilik Ekip Uyarısı

```
Araştırmadan kritik uyarı:

"Açık kaynak proje bir taahhüttür. Issue'lara yanıt
 veremiyorsanız, PR'ları inceleyemiyorsanız ve düzenli
 güncelleme yayımlayamıyorsanız, proje durgunlaşır ve
 markanıza yardımdan çok zarar verir."

Volt için:
  Show HN başarılı olursa → 50+ issue gelebilir
  Yanıtlanmazsa → "terk edilmiş proje" algısı
  → Lansman öncesi bant genişliği planla

Öneri:
  Show HN sonrası 2 hafta yoğun destek için zaman ayır
  Otomatik yanıt şablonu hazırla
  "good first issue" etiketleri önceden hazır
```

---

## Özet: En Kritik Beş Karar

```
1. Show HN'i F2'ye kadar bekle
   Tek şansın var. Tip sistemi çalışmadan yapma.

2. Başlık teknik olsun, pazarlama olmasın
   "CDC bugs won't compile" > "Revolutionary HDL"

3. Kod örneği ilk 10 satırda
   Öncesi/sonrası karşılaştırması en ikna edici

4. Rakipleri dürüstçe öv
   Spade, Veryl, Clash, Arch'ın gerçek katkılarını söyle
   → Güven + rakip topluluklardan trafik

5. Tiny Tapeout = en güçlü tek kanıt
   $300, gerçek silikon, viral hikaye
   On blog yazısından etkili

Ve en önemlisi:
  İçerik pazarlaması sorunu anlatmaktır, ürünü değil.
  %14 ilk silisyum başarı oranı → herkesin sorunu
  Volt bu sorunun bir dilimine yanıt → doğal sonuç
```
