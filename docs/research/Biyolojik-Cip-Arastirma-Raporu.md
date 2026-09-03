> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Biyolojik Çip (Biological Computing) — Araştırma Raporu

> Kaynaklar: Nature Electronics, Neuron, Frontiers in Science,
> arXiv, Cortical Labs, Johns Hopkins OI programı
> Tarih: Ağustos 2026

---

## Bölüm 1 — Tanım ve Kapsam

### Biyolojik Çip Nedir

```
Biyolojik çip = Canlı nöronlar + Silikon elektrot dizisi

Klasik çip:              Biyolojik çip:
  Transistör               Canlı nöron (iPSC türevi)
  Metal bağlantı           Sinaptik bağlantı (kendiliğinden oluşur)
  Programlama              Eğitim (uyarı-tepki döngüsü)
  Deterministik            Adaptif, olasılıksal
  Sabit topoloji           Öğrenirken yeniden bağlanıyor
```

**Kritik ayrım:** Nöromorfik çip beyni *taklit* eder (silikon).
Biyolojik çip gerçek nöron *kullanır* (canlı hücre).

### Terminoloji

```
OI  — Organoid Intelligence: beyin organoidleri ile hesaplama
SBI — Synthetic Biological Intelligence: Cortical Labs terimi
Wetware — donanım/yazılım'a karşılık "ıslak ürün"
DishBrain — kültür ortamında nöron ağı (ilk büyük gösterim)
```

---

## Bölüm 2 — Nasıl Çalışıyor

### 2.1 Nöron Kaynağı

```
1. Kan veya deri hücresi alınır (insan donörden)
2. iPSC'ye dönüştürülür (indüklenmiş pluripotent kök hücre)
3. Nöron farklılaşması yapılır
4. Elektrot dizisi üzerine ekilir
5. Nöronlar kendiliğinden ağ kurar (sinaptogenez)

Süre: 4-8 hafta kültür
Yaşam süresi: CL1'de 6 aya kadar
```

### 2.2 Arayüz Mekanizması

Cortical Labs'in CSO'su Brett Kagan'ın vurguladığı temel nokta:
sinir hücreleri ile silikon arasındaki **ortak dil elektriktir.**

```
Elektrot dizisi (59 elektrot CL1'de):

  OKUMA:  Nöron ateşler → elektrik sinyali → ADC → dijital
  YAZMA:  Dijital → DAC → elektrik uyarımı → nöron aktive

  Çift yönlü, gerçek zamanlı, kapalı döngü
```

### 2.3 Öğrenme Mekanizması — Free Energy Principle

DishBrain'in en şaşırtıcı bulgusu: nöronlar **ödül olmadan** öğreniyor.

```
Karl Friston'ın Free Energy Principle'ına dayanıyor:

  Nöron ağları "öngörülemezliği azaltmaya" çalışıyor

  Doğru davranış  → öngörülebilir sinyal (düzenli uyarım)
  Yanlış davranış → kaotik sinyal (rastgele gürültü)

  Nöronlar kaostan kaçınmak için doğru davranışı öğreniyor
  → Klasik ödül/ceza yok, sadece öngörülebilirlik
```

Bu, geleneksel pekiştirmeli öğrenmeden temelde farklı.

---

## Bölüm 3 — Kilometre Taşları

### 3.1 DishBrain (2022) — Alanı Başlatan Çalışma

```
Yayın: Kagan et al., Neuron, Aralık 2022
Başlık: "In vitro neurons learn and exhibit sentience when
         embodied in a simulated game-world"

Yapılan:
  800,000 nöron, yüksek yoğunluklu elektrot dizisi
  Pong oyunu simüle edildi
  Top pozisyonu → elektrik uyarımı olarak nöronlara verildi
  Nöron aktivitesi → paddle hareketi olarak okundu

Sonuç:
  5 dakikada anlamlı öğrenme
  Rastgeleden belirgin biçimde iyi performans
  Kapalı döngü olmadan öğrenme YOK (embodiment şart)
```

### 3.2 Brain Organoid Reservoir Computing (2023)

```
Yayın: Cai et al., Nature Electronics 6, 1032-1039 (2023)
Kurum: Indiana University

Yapılan:
  3D beyin organoidi (petri kabında değil, küresel yapı)
  Reservoir computing yaklaşımı
  Konuşma tanıma ve tahmin görevleri

Sonuç:
  Organoid "rezervuar" olarak çalışıyor
  Sadece çıkış katmanı eğitiliyor (organoid sabit)
  Enerji verimliliği çok yüksek
```

### 3.3 Örneklem Verimliliği Karşılaştırması (2025)

```
Yayın: Cortical Labs, Ağustos 2025
Başlık: "Dynamic Network Plasticity and Sample Efficiency in
         Biological Neural Cultures"

Bulgu:
  Biyolojik nöronlar, derin pekiştirmeli öğrenme
  algoritmalarından DAHA HIZLI öğreniyor
  (örneklem verimliliği açısından)

Neden önemli:
  Modern RL milyonlarca deneme gerektiriyor
  Nöronlar yüzlerce denemede öğreniyor
  → Bu biyolojinin gerçek avantajı
```

---

## Bölüm 4 — Ticari Durum: CL1

Mart 2025'te Cortical Labs, CL1'i dünyanın ilk ticari
biyolojik bilgisayarı olarak tanıttı.

### 4.1 Teknik Özellikler

```
Ürün:        CL1 (Cortical Labs, Melbourne)
Fiyat:       ~$35,000
Boyut:       ayakkabı kutusu
Nöron:       iPSC türevi insan nöronları
Elektrot:    59 elektrot (planar dizi)
Yaşam:       6 aya kadar
Arayüz:      Python API, çift yönlü
Bulut:       "Wetware as a Service" — uzaktan erişim
Güç:         geleneksel sistemlerin çok altında
Bağımsız:    harici bilgisayar gerektirmiyor

Yaşam destek: filtreleme, besin dolaşımı,
              gaz karışımı, sıcaklık kontrolü
```

### 4.2 Ölçekleme Planı

```
Server stack: 30 CL1 ünitesi
Hedef:        4 stack (120 ünite) 2025 sonu
Erişim:       bulut üzerinden ("Cortical Cloud")

Minimal Viable Brain projesi:
  ~30 nöronluk sıkı organize kültürler
  Örüntü tanıma görevleri
  → Daha büyük biyo-hesaplama çekirdeklerine adım
```

### 4.3 Finansman ve Yatırımcılar

```
Toplam: ~$11M
Yatırımcılar:
  Horizons Ventures
  Blackbird Ventures
  LifeX, Radar Ventures
  In-Q-Tel  ← CIA'nin yatırım kolu (dikkat çekici)
```

---

## Bölüm 5 — Gerçek Uygulama Alanları

Hype'tan ayrılan somut kullanımlar:

### 5.1 İlaç Keşfi ve Toksikoloji (En Olgun)

```
Sorun:
  Yeni ilaç → hayvan testi → insan denemesi
  Hayvan modelleri insan beynini iyi temsil etmiyor
  %90 ilaç insan denemesinde başarısız

Biyolojik çip çözümü:
  İnsan nöronları üzerinde doğrudan test
  Etkiyi gerçek zamanlı elektriksel olarak ölç
  Hayvan testine etik alternatif

Bu bugün ÇALIŞIYOR — CL1'in birincil pazarı
```

### 5.2 Hastalık Modelleme

```
Hasta kaynaklı iPSC → hastalıklı nöron ağı

  Alzheimer, Parkinson, epilepsi, otizm
  → Hastanın kendi hücrelerinden model
  → Kişiselleştirilmiş ilaç tepkisi testi

Örnek: epilepsi hastasının nöronlarında
       nöbet aktivitesi gözlemlenip
       hangi ilacın işe yarayacağı test edilebilir
```

### 5.3 Robotik ve Adaptif Kontrol

```
Klasik AI: önceden programlanmış kurallar
Biyolojik: yeni ortama kendiliğinden uyum

Potansiyel:
  Bilinmeyen ortamda keşif robotu
  Az örnekle öğrenme gereken görevler
  Enerji kısıtlı otonom sistemler

Durum: araştırma aşaması, ürün yok
```

### 5.4 Enerji Verimli Hesaplama (Uzun Vade)

```
İnsan beyni: ~20 W ile devasa hesaplama
GPU kümesi:  aynı işi kW seviyesinde yapıyor

Teorik potansiyel: 1000× enerji verimliliği

AMA: bu bugün gerçekleşmiş değil
     CL1 genel amaçlı hesaplama yapmıyor
```

---

## Bölüm 6 — Dürüst Sınırlar

### 6.1 Alanın Kendi Uyarısı

Kasım 2025'te STAT News'te yayımlanan bir haber, beyin
organoidi öncülerinin şişirilmiş iddiaların geri tepmesinden
endişe duyduğunu aktardı.

```
Endişeler:
  "Biyolojik bilgisayar GPU'nun yerini alacak" → YANLIŞ
  "Bilinçli yapay zeka" → şu an geçerli değil
  "Yakında ticari genel hesaplama" → çok erken

Alan içindeki uzmanlar bu iddialardan rahatsız.
Aşırı vaat → fon kesintisi + düzenleyici tepki riski.
```

### 6.2 Temel Teknik Sınırlar

```
SINIR                        DURUM
────────────────────────────────────────────────────────
Yaşam süresi                 6 ay maksimum (CL1)
                             → sonra yeni kültür gerekli

Ölçek                        59 elektrot (CL1)
                             → GPU'da milyarlarca transistör

Hız                          Nöron: ~1-100 Hz
                             → Silikon: GHz (10⁷× hızlı)

Tekrarlanabilirlik           Her kültür farklı
                             → aynı sonuç garanti edilemez

Programlanabilirlik          Eğitim gerekiyor, kod yazılmıyor
                             → deterministik değil

Altyapı                      Yaşam destek sistemi şart
                             → besin, gaz, sıcaklık, sterilite

Maliyet                      $35K + sürekli işletme
                             → GPU çok daha ucuz

Genel amaçlılık              YOK
                             → sadece belirli görevler
```

### 6.3 Nerede Kesinlikle Kullanılamaz

```
✗ LLM çıkarımı (400B model çalıştırma)
✗ Genel amaçlı hesaplama
✗ Deterministik işlemler (finans, kriptografi)
✗ Yüksek hızlı işleme
✗ Gömülü/mobil cihazlar (yaşam destek gerekli)
✗ Uzun süreli güvenilir çalışma
```

---

## Bölüm 7 — Etik Boyut

Bu alan diğer teknolojilerden farklı olarak etik denetimi
**araştırmanın içine gömülü** hale getirdi.

```
2024: "Biocomputing through EnGINeering Organoid Intelligence"
      programı başlatıldı
      → Başvuru şartı: bir etik uzmanı eş-baş araştırmacı olmalı
      → Etik planı, araştırma planıyla eşit ağırlıkta değerlendirildi

Bu, bilim tarihinde nadir bir yaklaşım.
```

### Temel Etik Sorular

```
1. Bilinç sorusu:
   Beyin organoidi bilinçli olabilir mi?
   → Mevcut çalışmalar "bilinç" tanımının kendisinin
     yeterince net olmadığını gösteriyor
   → Cortical Labs: "kültürlerimiz bilinçli değil" (biyoetikçi onayı)

2. Rıza:
   Donör hücrelerinden türetilen nöronlar
   → Donör ne için rıza verdi?
   → "Hücrem düşünen bir sistem olacak" bilgisi verildi mi?

3. Düzenleme boşluğu:
   Mevcut yasal çerçeveler bu senaryoyu kapsamıyor
   → Ne tıbbi cihaz, ne bilgisayar, ne deney hayvanı
   → Yeni politika gerekiyor

4. Moral statü:
   Eğer bir sistem öğreniyor ve tepki veriyorsa,
   ona nasıl davranmalıyız?
```

---

## Bölüm 8 — Volt/Ternary Ekosistemi ile İlişki

### 8.1 Rakip Değil, Farklı Katman

```
Ternary/Nöromorfik silikon:      Biyolojik çip:
  Deterministik                    Olasılıksal
  GHz hız                          Hz-kHz hız
  Milyarlarca eleman               Yüzlerce elektrot
  Programlanır                     Eğitilir
  Kuru                             Yaşam destek gerekli
  Ürün olgunluğu: yüksek           Ürün olgunluğu: çok erken

Aynı sorunları çözmüyorlar.
```

### 8.2 Kesişim Noktası: Arayüz Donanımı

**Volt'un burada gerçek bir rolü olabilir** — biyolojik hesaplamada
değil, ona bağlanan silikon tarafında:

```volt
// Biyolojik arayüz donanımı — Volt ile tasarlanabilir

domain BiologicalDomain {
    clock = none              // nöronlar asenkron
    event_driven = true       // spike tabanlı
    timescale = millisecond   // biyolojik zaman ölçeği
}

domain DigitalDomain {
    clock = posedge
    frequency = 100.mhz
}

module NeuralInterface<const ELECTRODES: usize> {
    // Nöron tarafı: analog spike algılama
    in  electrode_in : [AnalogSignal; ELECTRODES] @BiologicalDomain

    // Dijital taraf: işlenmiş spike verisi
    out spike_events : Stream<SpikeEvent> @DigitalDomain

    // Geri besleme: uyarım
    in  stimulus_cmd : Stream<StimulusCmd> @DigitalDomain
    out electrode_out: [AnalogSignal; ELECTRODES] @BiologicalDomain

    // KRİTİK: biyolojik ↔ dijital geçişi
    // Volt tip sistemi bu geçişi denetliyor
    // Yanlış zamanlama → nöron hasarı → E3001 benzeri koruma

    invariant: stimulus_current < 100.uA   // güvenlik sınırı
    invariant: stimulus_charge_balanced     // DC bileşen yok
    // Dengesiz yük → elektrot korozyonu → nöron ölümü
}
```

**Somut değer:** Elektrot güvenliği kritik. Yük dengesizliği
elektrotları bozuyor ve nöronları öldürüyor. Bu tam olarak
formal doğrulama ile garanti edilebilecek bir özellik.

### 8.3 Ortak Kavram: Spike

```
Nöromorfik silikon:  Spike<Trit>  (yapay)
Biyolojik çip:       gerçek nöron ateşlemesi

Aynı soyutlama:
  olay tabanlı
  seyrek
  zaman kodlu

Volt'un Spike<T> tipi her ikisini de ifade edebilir.
```

---

## Bölüm 9 — Gerçekçi Zaman Çizelgesi

```
BUGÜN (2026):
  ✓ İlaç testi ve toksikoloji: ticari kullanımda
  ✓ Hastalık modelleme: araştırma laboratuvarlarında
  ✓ CL1 satın alınabilir ($35K) veya bulut erişimi
  ✗ Genel hesaplama: yok

2027-2030:
  ✓ İlaç keşfi standart araç haline gelir
  △ Robotik kontrol deneyleri
  △ Daha büyük elektrot dizileri (1000+)
  ✗ LLM veya genel AI: hayır

2030-2035:
  △ Hibrit sistemler (silikon + biyolojik)
  △ Özel görevlerde enerji avantajı gösterimi
  ? Düzenleyici çerçeve oturur

2035+:
  ? Belirsiz — alan çok yeni, tahmin spekülatif
```

---

## Bölüm 10 — Anahtar Kaynaklar

```
TEMEL YAYINLAR:

Kagan B.J. et al. (2022)
  "In vitro neurons learn and exhibit sentience when
   embodied in a simulated game-world"
  Neuron 110(23):3952-3969
  → Alanı başlatan çalışma (DishBrain)

Cai H. et al. (2023)
  "Brain organoid reservoir computing for artificial intelligence"
  Nature Electronics 6:1032-1039
  → Organoid tabanlı hesaplama

Smirnova L. et al. (2023)
  "Organoid intelligence (OI): the new frontier in
   biocomputing and intelligence-in-a-dish"
  Frontiers in Science 1:1017235
  → Alanın manifestosu

Hartung T. et al. (2023)
  "The Baltimore declaration toward the exploration of
   organoid intelligence"
  Frontiers in Science 1:1068159
  → Etik çerçeve

Sumi T. et al. (2023)
  "Biological neurons act as generalization filters in
   reservoir computing"
  PNAS 120:e2217008120

Wadan A.S. (2025)
  "Organoid intelligence and biocomputing advances"
  Brain Organoid and Systems Neuroscience Journal 3:8-14
  → Güncel derleme

KURUMLAR:
  Cortical Labs (Melbourne) — corticallabs.com
  Johns Hopkins OI programı — Thomas Hartung, Lena Smirnova
  Indiana University — Feng Guo grubu
  University of Essex — Michael Barros
```

---

## Özet

```
Biyolojik çip nedir:
  Canlı insan nöronları + silikon elektrot dizisi
  Elektrik ortak dil, çift yönlü arayüz
  Öğrenme: öngörülemezlikten kaçınma (Free Energy Principle)

Bugün gerçek olan:
  ✓ CL1 satın alınabilir ($35K, Mart 2025)
  ✓ İlaç testi ve hastalık modelleme çalışıyor
  ✓ Nöronlar RL algoritmalarından hızlı öğreniyor (örneklem verimliliği)
  ✓ Bulut erişimi mevcut

Bugün gerçek OLMAYAN:
  ✗ Genel amaçlı hesaplama
  ✗ LLM çalıştırma
  ✗ GPU'ya alternatif
  ✗ Bilinçli sistem

Alanın kendi uyarısı:
  Öncüler şişirilmiş iddiaların geri tepmesinden endişeli
  Etik denetim araştırmanın içine gömülü

Volt ile ilişki:
  Rakip değil — farklı katman
  Kesişim: nöral arayüz donanımı tasarımı
  Elektrot güvenliği (yük dengesi) formal doğrulama için ideal
```
