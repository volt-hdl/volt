> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Ternary / Düşük Kesinlik AI Çip Yatırım Ortamı — Araştırma Raporu

> Kaynaklar: SemiEngineering, CNBC, SiliconANGLE, IEEE Xplore,
> arXiv, New Market Pitch. Tarih: Ağustos 2026

---

## Bölüm 1 — Dürüst Sonuç: Saf "Ternary Çip Şirketi" Yok

```
Araştırma bulgusu:

  "Biz ternary çip yapıyoruz" diyen finanse edilmiş şirket:  YOK
  Ternary'yi araç olarak kullanan finanse edilmiş şirket:    VAR
  Akademik ternary CIM araştırması:                          YOĞUN

Neden saf ternary şirketi yok:
  Ternary bir amaç değil, araç
  Yatırımcı "düşük kesinlik" değil "token/saniye" satın alıyor
  Şirketler kendini "ternary" değil "verimli çıkarım" olarak
  konumlandırıyor
```

Bu önemli bir stratejik gözlem: **ternary bir kimlik değil,
bir teknik.** Volt için de aynı ders geçerli.

---

## Bölüm 2 — En Kritik Gelişme: Taalas

Ağustos 2026'da olan şey, ternary tartışmasını doğrudan
ilgilendiriyor.

### 2.1 Ne Yaptılar

<cite index="140-1">Taalas'ın Hardcore AI mimarisi, model parametrelerini çalışma zamanında yazılımda çalıştırmak yerine doğrudan özel bir çipe gömüyor. Geleneksel AI hızlandırıcıları çıkarım sırasında model verisini bellek ile hesaplama arasında taşıyor, bu da gecikme ve güç yükü yaratıyor. Şirket, modeli silikona kazıyarak bu darboğazı ortadan kaldırmayı hedefliyor.</cite>

```
HC1 test çipi:
  TSMC 6nm süreci
  Llama 3.1 8B modeli
  ~17,000 token/saniye
  Nvidia H200'ün 1/10 gücünde
  73× daha hızlı (Şubat 2026 iddiası)

HC2 (geliştirme aşamasında):
  ~20 milyar parametreli modeller hedefleniyor
```

### 2.2 Yatırım ve Satın Alma

<cite index="141-1">Taalas, Şubat 2026'da Quiet Capital, Fidelity ve yarı iletken yatırımcısı Pierre Lamond'dan 169 milyon dolar topladı; toplam yatırım ~219 milyon dolara ulaştı.</cite>

<cite index="136-1">6 Ağustos 2026'da AMD, Taalas'ı satın almak için kesin anlaşma imzaladığını duyurdu. İşlem 2026'nın dördüncü çeyreğinde kapanması bekleniyor.</cite>

```
Zaman çizgisi:
  2023      Kuruluş (Ljubisa Bajic — eski Tenstorrent CEO'su,
            daha önce AMD direktörü)
  2024 Q1   $50M seed, stealth'ten çıkış
  2026 Şubat HC1 çipi tanıtıldı, $169M yeni tur
  2026 Ağustos AMD satın alma anlaşması

  Toplam süre: kuruluştan satın almaya 3 yıl
```

### 2.3 Neden Bu Ternary Tartışmasıyla İlgili

```
Taalas'ın çözdüğü sorun = ternary'nin çözdüğü sorun:
  BELLEK BANT GENİŞLİĞİ

Taalas yaklaşımı:  ağırlıkları transistöre kazı → bellek okuma yok
Ternary yaklaşımı: ağırlıkları 2 bite indir → bellek okuma 8× az

İkisi de aynı duvara farklı açıdan saldırıyor.
Ve Taalas'ın $219M toplayıp AMD'ye satılması,
bu duvarın gerçek ve değerli olduğunu kanıtlıyor.
```

<cite index="142-1">Bir AMD Helios rack'i HBM4 kullanıyor ve MI455X hızlandırıcı başına 19.6 TB/s sunuyor — H100'ün yaklaşık altı katı. Bu farkı daraltıyor ama kapatmıyor: HBM4 hâlâ DRAM, hâlâ her ileri geçişte yükleniyor, hâlâ aynı bant genişliği aritmetiğine tabi.</cite>

Bu cümle konuşmanın başındaki 400B analizinin endüstriyel doğrulaması.

---

## Bölüm 3 — Düşük Kesinlik Kullanan Finanse Edilmiş Şirketler

### 3.1 Nöromorfik / Düşük Bit

```
BrainChip (ASX: BRN) — Halka açık, Avustralya
  Akida çipi: 1, 2, 4 bit ağırlık desteği
  2 bit ≈ ternary'ye çok yakın
  IoT modüllerinde ticari kullanımda
  → En yakın "ternary üretimde" örneği

Innatera (Hollanda) — Seed/Series A
  Spiking neural network çipi
  Analog nöromorfik, düşük kesinlik doğal
  Edge sensörler hedefli

SynSense (Çin/İsviçre)
  DYNAP çipleri, olay tabanlı görme
  Nöromorfik + düşük bit
```

### 3.2 Bellekte Hesaplama (CIM)

```
Mythic AI — analog CIM, flash tabanlı
  ~$165M toplam yatırım
  Analog ağırlık depolama (ternary değil ama düşük kesinlik)
  2023'te finansal zorluk yaşadı, yeniden yapılandı

Everspin (NASDAQ: MRAM) — halka açık
  MRAM tabanlı nöral hızlandırıcı
  CHEETA programı (Purdue + DoD)
  2025'te $10.5M geliştirme sözleşmesi
  2026'da üretime hazır demo hedefi

Axelera AI (Hollanda) — ~$120M
  Dijital CIM, INT8 odaklı
  Edge AI hızlandırıcı
```

### 3.3 Genel AI Çip Ortamı (Bağlam)

<cite index="139-1">AI çip pazarı 2024'te ~$2.19 milyar, 2025'te ~$3.48 milyar, 2026'nın ilk yarısında ~$4.16 milyar fon topladı. Medyan tur büyüklüğü 2024'te ~$91M, 2025'te ~$72M iken 2026 yılbaşından itibaren $350M'a çıktı.</cite>

<cite index="139-1">Çıkarım (inference) en güçlü alt kategori: 2024'te $896M, 2025'te ~$1.66B, 2026'nın ilk yarısında $2.0B — önceki tam yıl toplamını çoktan aştı.</cite>

**Kritik uyarı:**

<cite index="139-1">2025'te ilk finansmanlar işlemlerin %35'ini ve sermayenin yaklaşık %19'unu temsil ediyordu; 2026 yılbaşından itibaren hiç nitelikli ilk finansman olmadı.</cite>

```
Bu ne demek:
  Yeni AI çip şirketi kurmak için PENCERE KAPANDI
  Yatırımcılar mevcut oyunculara takviye yapıyor
  Sıfırdan başlayan şirket fon bulamıyor

  Yani: 2023'te Taalas kurulabilirdi
        2026'da aynı şirket kurulamaz
```

---

## Bölüm 4 — Akademik ve Kurumsal Araştırma

Ticari şirket yok ama araştırma yoğun:

```
T-CIM (16T1C bitcell) — IEEE JSSC
  Ternary CIM işlemci
  Kompakt alan + iyi doğrusallık
  ADC'yi ortadan kaldıran şarj tabanlı toplayıcı
  → ADC enerjisi sistem enerjisinin büyük kısmıydı

TiM-DNN (Purdue) — arXiv:1909.06892
  Ternary in-memory hızlandırıcı
  Desteklenen gösterimler:
    ağırlıksız (-1, 0, 1)
    simetrik ağırlıklı (-a, 0, a)
    asimetrik ağırlıklı (-a, 0, b)

Microsoft BitNet
  b1.58 modelleri (yazılım tarafı)
  BitNet.cpp — CPU çıkarım kütüphanesi
  Donanım şirketi kurmadılar → yazılım katmanında bıraktılar
```

**Dikkat çekici:** Microsoft ternary'yi icat etti ama çip
şirketi kurmadı. Yazılım olarak açık kaynak yaptı.

---

## Bölüm 5 — Neden Saf Ternary Şirketi Yok

### Neden 1 — Ternary Tek Başına Yeterli Değil

```
Ternary'nin kazancı:
  Bellek: 8× az (FP16 → 2 bit)
  Enerji: 3-5× az (ASIC'te)

Taalas'ın kazancı:
  Bellek okuma: SIFIR (ağırlık transistörde)
  Hız: 73×

Yatırımcı perspektifi:
  "8× iyileşme" → ilginç
  "73× iyileşme" → yatırım yaparım
```

### Neden 2 — Doğruluk Kaybı Satılamıyor

```
Ternary: %2-3 doğruluk kaybı
Müşteri: "Neden %3 kalite kaybedeyim?"
Satıcı:  "Çünkü daha ucuz"
Müşteri: "Kalite benim ürünüm"

Bu konuşma çoğu B2B satışta kaybediliyor.
```

### Neden 3 — Model Ekosistemi Bağımlılığı

```
Ternary çip → ternary model gerekiyor
Ternary model → BitNet dışında yaygın değil
Yaygın değil → müşteri modelini dönüştürmeli
Dönüştürme → doğruluk riski + iş yükü

Tavuk-yumurta problemi.
Taalas bunu çözdü: mevcut Llama'yı alıyor, dönüştürmüyor.
```

### Neden 4 — FPGA'da Görünmüyor

```
Ternary'nin avantajı ASIC'te ortaya çıkıyor
FPGA prototipinde: LUT tabanlı, avantaj yok
→ Yatırımcıya erken kanıt gösterilemiyor
→ "Önce $10M ver, sonra göstereyim" → zor satış
```

---

## Bölüm 6 — Volt İçin Çıkarımlar

### 6.1 Konumlandırma Dersi

```
YANLIŞ:
  "Volt ternary donanım için HDL"
  → Ternary bir pazar değil

DOĞRU:
  "Volt CDC güvenli HDL, ternary dahil düşük kesinlik
   tiplerini native destekliyor"
  → CDC gerçek pazar, ternary bonus
```

Taalas'ın kendisi bunu yapıyor: "ternary şirketi" değil,
"model-specific integrated circuit" şirketi.

### 6.2 Fırsat: Taalas Tarzı Şirketler Volt'un Müşterisi

```
Taalas gibi bir şirketin ihtiyacı:
  Model → RTL otomatik dönüşüm akışı
  Her model için yeni çip → hızlı iterasyon şart
  CDC hataları → tape-out gecikmesi → milyonlar

Volt'un sunduğu:
  Trit tipi native
  CDC/RDC derleme zamanında
  Formal doğrulama dilde
  Parametrik jeneratör (Python API)

Bu eşleşme gerçek.
Ama: Taalas artık AMD'nin. Yeni müşteri aramak gerek.
```

### 6.3 Zamanlama Uyarısı

```
AI çip pazarında ilk finansman penceresi 2026'da kapandı.

Volt için anlamı:
  ✗ "Volt tabanlı çip şirketi kuralım" → fon bulunamaz
  ✓ "Volt'u mevcut şirketlere araç olarak sat" → mümkün
  ✓ "Açık kaynak + danışmanlık" → sürdürülebilir

Araç şirketi olmak, çip şirketi olmaktan
şu an daha gerçekçi.
```

---

## Bölüm 7 — Kimler Volt'un Potansiyel Müşterisi

```
KATEGORİ                  ÖRNEKLER              İHTİYAÇ
──────────────────────────────────────────────────────────────
Nöromorfik çip şirketi    BrainChip, Innatera,  Spike + düşük bit
                          SynSense              tip sistemi

CIM şirketleri            Mythic, Axelera,      Analog/dijital
                          Everspin              karışım, domain

Model-specific ASIC       (Taalas modeli)       Hızlı iterasyon,
                          yeni girenler         model → RTL akışı

Edge AI çip              EdgeCortix,           Düşük kesinlik,
                          Hailo, Kneron         güç bütçesi

Akademik gruplar          Purdue, KAIST,        Ternary CIM
                          IMEC, ETH             araştırması

Savunma/uzay             CHEETA programı       Radyasyon toleransı,
                          DoD projeleri         formal doğrulama
```

---

## Özet

```
Saf ternary çip şirketi:
  ✗ YOK — ternary kimlik değil, teknik

Ternary kullanan finanse edilmiş şirketler:
  ✓ BrainChip (1/2/4 bit, üretimde)
  ✓ Innatera, SynSense (nöromorfik, düşük bit)
  ✓ Everspin (MRAM nöral, DoD sözleşmeli)

En kritik gelişme:
  Taalas — ağırlıkları silikona kazıyor
  $219M yatırım → AMD satın alması (Ağustos 2026)
  Aynı problemi (bellek duvarı) farklı yoldan çözüyor
  → Problemin gerçek ve değerli olduğunun kanıtı

Pazar uyarısı:
  AI çip ilk finansman penceresi 2026'da kapandı
  Yeni çip şirketi kurmak için geç
  Araç/altyapı katmanı hâlâ açık

Volt için ders:
  Ternary'yi kimlik yapma, özellik yap
  Hedef müşteri: nöromorfik + CIM + model-specific ASIC şirketleri
  Konumlandırma: "CDC güvenli HDL" (ternary bonus)
```
