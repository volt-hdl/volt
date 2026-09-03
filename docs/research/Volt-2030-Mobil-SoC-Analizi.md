> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# 2030 Sonrası Akıllı Telefonlarda Nöromorfik + Fotonik SoC

> Kısa cevap: Nöromorfik büyük ölasılıkla evet, fotonik çok daha zor.
> Ama "evet" ve "hayır" arasında detaylar kritik — hangi teknoloji,
> hangi kullanım amacı, hangi zaman dilimi.

---

## Bölüm 1 — Akıllı Telefon SoC'un Değişmeyen Kısıtları

Bu kısıtlar teknoloji ne olursa olsun geçerli:

```
Termal bütçe:
  Aktif kullanım: 3-8W sürekli (el ısısı sınırı)
  Arka plan: 0.5-1W
  Pik: 12-15W (kısa süreli, 30-60 saniye)
  Pasif soğutma: yalnızca metal kasa + ısı dağıtıcı

Boyut:
  Ana SoC: ~10mm × 10mm = 100 mm²
  Pil: 4000-5000 mAh → 1-2 günlük ömür
  Toplam kalınlık: ~7mm

Maliyet:
  SoC üretim: $30-80 (Snapdragon 8 Gen 3 ~ $150 perakende)
  Yüksek hacim: yüz milyonlarca adet/yıl → verim şart

Güvenilirlik:
  -20°C ile +50°C arasında çalışma
  Nem, titreşim, düşme toleransı
  3-5 yıl ömür beklentisi

Bu kısıtlar nöromorfik ve fotoniği çok farklı etkiliyor.
```

---

## Bölüm 2 — Nöromorfik: Gerçekçi ve Yakın

### 2.1 Neden Akıllı Telefona Uygun

```
Nöromorfik'in güçlü yanı: olay tabanlı, seyrek aktivasyon
Telefon kullanımının büyük çoğunluğu: "bekle ve dinle"

Uyku kelimesi algılama:
  Klasik NPU: 5mW sürekli
  Nöromorfik: 0.05-0.1mW sürekli
  Fark: 50-100x

24 saatlik etki:
  Klasik: 5mW × 86,400s = 432 J
  Nöromorfik: 0.1mW × 86,400s = 8.6 J
  Pil kazanımı: 423 J ≈ küçük ama gerçek

Yüz tanıma (her kilit açma):
  Spike tabanlı özellik çıkarımı: <1mJ/işlem
  CNN tabanlı: ~5-10mJ/işlem
  10 kez/gün: anlamlı pil tasarrufu

Sürekli sağlık izleme (EKG, SpO2):
  Anomali olmadığında: nöromorfik susturuluyor → 0W
  Anomali → spike → uyarı
  Bugünkü donanım: sürekli örnekliyor → pil tüketiyor
```

### 2.2 Bugün Var Olan Temeller

Tamamen yeni değil — zaten bu yönde ilerleme var:

```
Apple Neural Engine (A17 Pro):
  Olay tabanlı özellikler → nöromorfik ilham
  "Always-on" özellikler düşük güçte

Qualcomm Sensing Hub:
  Her zaman açık ses/hareket algılama
  Ana SoC uyurken çalışıyor → düşük güç

BrainChip Akida (şimdi mevcut!):
  Spike tabanlı çıkarım çipi
  Alan: ~1 mm²
  Güç: 1mW always-on
  Zaten IoT modüllerinde var
  Telefon entegrasyonu: teknik engel yok

Intel Loihi 2:
  Araştırma çipi, büyük ve pahalı
  Ama kanıtladığı: spike hesaplama ölçeklenebilir

FeFET teknolojisi (kritik bileşen):
  GlobalFoundries 22FDX: FeFET sunuyor
  Samsung: FeFET tabanlı ürünler
  TSMC: roadmap'te
  2026-2028: mobil SoC'ta FeFET gerçekçi
```

### 2.3 2030 Telefon Nöromorfik Senaryosu

```
Gerçekçi mimari:

Ana SoC (TSMC 2nm, 2030):
┌─────────────────────────────────────────────────┐
│  CPU çekirdekleri (4+4 hibrit)    ~30mm²         │
│  GPU (grafik)                     ~20mm²         │
│  Ternary NPU (LLM çıkarımı)      ~15mm²         │
│  Nöromorfik Tile (FeFET)          ~5mm²  ← YENİ │
│  ISP, modem, vb.                  ~30mm²         │
└─────────────────────────────────────────────────┘

Nöromorfik Tile içeriği:
  FeFET sinaptik ağırlıklar: ~1M sinaps
  LIF nöron dizisi: ~100K nöron
  Spike yönlendirici: donanım
  Güç: 0.05-0.5mW (sürekli çalışma)

Ne yapıyor:
  Uyku kelimesi: her zaman dinliyor → 0.05mW
  Yüz algılama ön filtre: kamera açılmadan
  Hareket örüntüsü: yürüme, koşma, düşme
  EKG anomali: sürekli izleme → sadece alarm
  Titreşim analizi: kulak yakınında mı?

Ana CPU/NPU ne zaman uyanıyor:
  Nöromorfik → "önemli olay var" → spike
  Ana işlemci uyandırılır → karmaşık işlem
  Ortalama güç: nöromorfik baskın → ~0.5W
  Konuşurken: ~3-5W (NPU aktif)
```

### 2.4 Ternary NPU + Nöromorfik: Mükemmel Eşleşme

```
2030 telefon modeli senaryosu:

Ternary NPU (15mm², ~20 TOPS @ 2W):
  7B ternary model: 1.4 GB
  LPDDR6 (600 GB/s beklenen): 1.4 GB × 30 tok/s = 42 GB/s
  → LPDDR6 ile 30 tok/s 7B model MÜMKÜN!

Hibrit çalışma:
  Kullanıcı konuşmuyor → nöromorfik dinliyor (0.05mW)
  "Hey asistan" → spike → NPU uyanıyor
  NPU: 7B ternary ile anlık yanıt → 2-3W, 5-10 saniye
  Tamamlandı → NPU uyuyor, nöromorfik devam

Ortalama günlük güç:
  23 saat 50 dk nöromorfik: 0.05mW × 85,800s = 4.3J
  10 dk aktif konuşma: 2.5W × 600s = 1500J
  Toplam: ~1504J = ~418 mAh
  iPhone 15 pili: 3877 mAh → yaklaşık 9 saat AI kullanımı
  (normal telefon kullanımında: 24-48 saat)

Bu gerçekçi!
```

---

## Bölüm 3 — Fotonik: Çok Daha Zor

### 3.1 Telefon İçin Fotonik'in Temel Sorunları

```
Sorun 1 — Lazer:
  Si-fotonik CIM için lazer şart
  Lazer: 1-10mW sürekli güç tüketimi
  Telefon termal bütçesi: 3-8W
  Hesaplama için 1-10mW sürekli lazer:
  → Termal bütçenin önemli kısmını tüketiyor
  → LiDAR gibi anlık kullanım için OK
  → Sürekli AI hesaplama için sorunlu

Sorun 2 — Sıcaklık hassasiyeti:
  Ring resonatör: 1°C → 0.1nm dalga boyu kayması
  Telefon sıcaklık aralığı: -20°C ile +60°C = 80°C fark
  Kalibrasyon: her sıcaklıkta sürekli ayar
  TEC (termoelektrik soğutucu): ek güç + hacim
  → Telefon için gerçekçi değil

Sorun 3 — Boyut ve yoğunluk:
  En küçük Si-fotonik yapı: ~220nm
  Transistör (2nm): çok daha küçük
  Fotonik hücre alanı: elektronik belleğin 1000x büyüğü
  Fotonik RAM telefona sığmaz (mantıklı kapasite için)

Sorun 4 — Kırılganlık:
  Optik hizalama: nanometre hassasiyeti
  Telefon düşer: optik yollar bozulur
  Titreşim: faz gürültüsü artar
  → Tüketici cihazı için güvenilirlik sorunu
```

### 3.2 Fotonik'in Telefonlara Girebileceği Alan

Hesaplama için değil ama **veri taşıma** için fotonik telefona girebilir:

```
Chip-to-chip optik interconnect:
  Telefondaki çipler arası iletişim bugün: elektrik kablo
  2030-2035: kısa mesafe optik link (~1-3mm)
  
  Neden:
  Elektrik bağlantı bant genişliği: GHz sınırlı
  Optik: THz bant genişliği (kısa mesafede bile)
  Güç: elektrik uzun kabloda kayıp → optik verimli
  
  Apple Silicon zaten araştırıyor:
  M-serisi çiplerde chip-to-chip optik interconnect
  Bu telefona 2030-2035 civarı gelebilir

Optik sensörler (zaten var):
  LiDAR (iPhone Pro): fotonik sensör
  Face ID: kızılötesi nokta projektör
  Parmak izi okuma: optik (bazı modellar)
  
  Bu "hesaplama fotoniği" değil ama fotonik var telefonda
```

### 3.3 2030'da Fotonik Bellek Telefonda?

```
FE+EO RAM için telefon analizi:

Yazma:
  FeFET (elektrik): enerji 1-5 pJ → OK
  Ama LiNbO₃ fotonik okuma için lazer gerekli

Okuma:
  Zayıf probe lazeri bile: 0.1-1mW sürekli
  24 saat × 1mW = 86.4J
  5000mAh pil = ~18,000J kapasitesi
  %0.5 pil → kabul edilebilir mi?
  Ama sıcaklık hassasiyeti hâlâ sorun

Gerçekçi alternatif — "Elektronik FeFET bellek":
  FeFET ağırlıkları (yazma: elektrik)
  Okuma: elektriksel direnç ölçümü (optik yok)
  → "Fotonik bellek değil" ama non-volatile AI bellek
  → Bu 2027-2028'de telefona girebilir

Voltun FE+EO konsepti telefon için:
  Değil (2030'da)
  Belki (2035+, eğer sıcaklık sorunu çözülürse)
```

---

## Bölüm 4 — Gerçekçi 2030 Telefon AI Mimarisi

### 4.1 En Olası Senaryo

```
2030 üst segment Android/iPhone SoC:

İşlem: TSMC 2nm (N2P) veya Samsung 2GAP
Çip alanı: ~100-120 mm²

KATMANLAR:
┌─────────────────────────────────────────────────┐
│  Klasik CPU (4P+4E çekirdek): 20mm²             │
│  Gelişmiş GPU (grafik + GPGPU): 25mm²           │
│  Ternary NPU (AI çıkarım): 15mm²               │
│    → 7B-13B ternary model: 30+ tok/s            │
│  Nöromorfik Tile (FeFET tabanlı): 5-8mm²       │
│    → Always-on algılama: <0.5mW                 │
│  PIM (işlemcili bellek): 5mm²                   │
│  ISP + modem + güvenlik: 25mm²                  │
│  Analog/RF: 10mm²                               │
└─────────────────────────────────────────────────┘

Bellek:
  LPDDR6 16-24 GB (600-800 GB/s beklenen)
  FeFET tabanlı non-volatile cache (yeni)

Fotonik: YOK (2030 için gerçekçi değil)
```

### 4.2 Kullanıcı Deneyimi Ne Değişiyor

```
2030 telefon AI kapasitesi:

Sürekli sağlık izleme (nöromorfik):
  EKG, SpO2, stres seviyesi
  Anomali → anında uyarı
  Güç: <0.1mW → pil etkisi minimal

Kişisel AI asistan (ternary NPU):
  7B yerel model, tamamen private
  30+ tok/s → akıcı konuşma
  İnternet bağlantısı gerekmez
  Verileriniz cihazdan çıkmaz

Proaktif algılama (nöromorfik + NPU):
  Düşme algılama: nöromorfik (anlık)
  Kazayı bağlama anlama: NPU
  Acil çağrı otomatik

Kamera AI (hibrit):
  Nöromorfik: yüz + hareket ön filtresi
  NPU: karmaşık tanıma + üretme
  Sonuç: daha hızlı, daha az pil

Dil modeli özellikleri:
  Mesaj önerisi: yerel 7B → gizli
  Çeviri: yerel, gerçek zamanlı
  Kod yardımı: 13B ternary → güçlü
```

---

## Bölüm 5 — Zaman Çizelgesi Özeti

```
2024-2025 (BUGÜN):
  ✓ NPU hızlanıyor (her jenerasyon ~2x)
  ✓ GGUF/TNN ile 7B model telefonda (yavaş)
  ✓ BrainChip Akida: IoT'de var, telefonda değil
  ✗ Gerçek nöromorfik telefon SoC: yok
  ✗ Fotonik: telefonda yok (LiDAR hariç)

2026-2028:
  ✓ FeFET tabanlı non-volatile AI bellek
  ✓ BrainChip tarzı nöromorfik tile SoC'a giriyor
    (Qualcomm veya Apple ilk yapan olabilir)
  ✓ 7B ternary model: 15-20 tok/s telefonda
  ✓ Always-on algılama <0.5mW
  ✗ Fotonik compute: hâlâ yok

2028-2030:
  ✓ Nöromorfik tile standart üst segment SoC'ta
  ✓ 7B-13B ternary: 30+ tok/s
  ✓ LPDDR6 bant genişliği: 13B modeli besleyebilir
  ✓ FeFET AI bellek: small permanent model
  △ Chip-to-chip optik: deneysel, belki premium model
  ✗ Fotonik CIM: yok

2030-2035:
  ✓ Nöromorfik: standart (orta segment dahil)
  ✓ 13B-30B ternary: telefonda çalışabilir
  △ Chip-to-chip optik interconnect: geliyor
  △ Fotonik sensör entegrasyonu: genişliyor
  △ FE+EO bellek: küçük ölçekli, deneysel

2035+:
  △ Fotonik CIM (küçük ölçek, özel uygulama): mümkün
  ✓ Nöromorfik: yaygınlaşmış
  △ 30B+ yerel model: belki
  ✗ 400B yerel: hayır (fiziksel kısıtlar)

Semboller: ✓ gerçekçi, △ mümkün ama belirsiz, ✗ gerçekçi değil
```

---

## Bölüm 6 — Neden Fotonik Telefona Sığmaz (2030'a Kadar)

```
Temel fizik sorunu: ışık dalga boyu

En küçük Si fotonik yapı: ~200-500nm
En küçük transistör (2030): ~1-2nm
Fark: 100-500x

Telefon SoC alanı: 100mm²
400B ternary model için FE+EO:
  WDM×512 + 3D×32 → 156 cm² (hesaplandı)
  156 cm² >> 100 mm² (10,000x büyük!)

Bu kısıt temel fizik — mühendislik sinyali değil.
2030'da değil, 2050'de bile zor.

Telefon için gerçekçi fotonik:
  → Sensör (LiDAR, kamera): EVET
  → Chip-to-chip interconnect: 2030-2035
  → Hesaplama (CIM): HAYIR (ölçek sorunu)
  → AI bellek (FE+EO): HAYIR (boyut + sıcaklık)
```

---

## Bölüm 7 — Volt Ekosisteminin Bu Tablodaki Yeri

```
2030 telefon SoC Volt ile nasıl tasarlanır:

Nöromorfik tile:
  Volt v2 → Spike<Trit> + on spike { }
  NeuroCompiler → FeFET topolojiye eşle
  Formal doğrulama: spike yönlendirme güvenli mi?

Ternary NPU:
  Volt MVP → sistolic dizi RTL
  TNN ağırlıkları FeFET'e yükle
  volt build → TSMC 2nm sentez

Hibrit zamanlayıcı (mimari OS):
  hardware_descriptor NeuromorphicTile { ... }
  hardware_descriptor TernaryNPU { ... }
  Görev: nöromorfik → NPU geçiş yönetimi

Volt'un değeri bu SoC için:
  Farklı paradigmaları tek dilde
  (nöromorfik tile + ternary NPU + CPU)
  CDC güvenliği paradigmalar arası
  Tek semantik model → chip üretimde sürpriz yok
```

---

## Özet

```
Nöromorfik telefonda 2030:
  EVET — büyük olasılıkla
  Özellikle: always-on algılama, sağlık izleme
  FeFET tabanlı, düşük güçlü nöromorfik tile
  Mevcut akıllı telefon mimarisine doğal eklenti
  İlk olabilecek: Apple (Neural Engine geliştirme)
                  veya Qualcomm (Sensing Hub geliştirme)

Fotonik hesaplama telefonda 2030:
  HAYIR — fiziksel ölçek engeli
  Dalga boyu limiti aşılamaz (2030'a kadar)
  Sıcaklık hassasiyeti tüketici cihazında sorun
  
Fotonik sensör/interconnect telefonda:
  EVET/BELKİ — zaten LiDAR var
  Chip-to-chip optik: 2030-2035 premium modelde
  
Gerçek devrim 2030 telefonda:
  Nöromorfik tile + Ternary NPU kombinasyonu
  → 7B-13B yerel model, 30+ tok/s, private
  → Always-on sağlık izleme, <0.5mW
  → Ortalama AI güç tüketimi: ~0.5W/gün
  Bu bugünün GPT-4 kalitesini offline, private,
  pil dostu yapıyor — gerçek dönüşüm bu.
```
