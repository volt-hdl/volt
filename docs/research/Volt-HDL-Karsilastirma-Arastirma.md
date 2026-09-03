> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Volt Benzeri HDL Projeleri — Araştırma Raporu

> İnternet, GitHub ve akademik makaleler taranarak hazırlanmıştır.
> Tarih: Ağustos 2026

---

## Bölüm 1 — En Kritik Bulgu: Arch HDL (Nisan 2026)

Araştırmanın en önemli keşfi: **Volt'un çekirdek fikrini tam olarak uygulayan bir proje 3 ay önce yayımlandı.**

### Arch HDL — arxiv:2604.05983

<cite index="49,50">Arch (AI-native Register-transfer Clocked Hardware), Nisan 2026'da arxiv'e yüklenen bir çalışmayla tanıtıldı. Mikro-mimari spesifikasyonu ve AI destekli kod üretimi için tasarlanmış olan Arch, pipeline, FSM, FIFO, arbiter, register file ve saat alanı geçişleri için birinci sınıf dil yapıları sunuyor.</cite>

**Volt'a olan benzerliği:**

<cite index="52">Arch'ın GitHub deposuna göre: "Saat alanı güvenliği yok: SV'de saat alanları geçmek bir konvansiyondur. ARCH, Clock<Domain>'i tip sisteminde takip eder — Clock<SysDomain>'deki bir sinyal, açık bir senkronizör yapısı olmadan Clock<MemDomain> kaydedicisine atanamaz. Derleyici asenkron FIFO'lar için gray-code CDC otomatik ekler ve güvensiz geçişleri derleme zamanında reddeder."</cite>

<cite index="47">Arch'ın tasarım kararları arasında: "Temel bir tasarım kararı olarak, saat ve reset sıradan net'ler değil, parametreli tipler olarak ele alınmaktadır (Clock<D>, Reset<S,P,D?>). Bu yaklaşım, CDC ve reset alan analizini harici lint geçişlerinden derleme zamanı tip kurallarına dönüştürür."</cite>

**Volt'tan farkları:**
```
Arch:
  ✓ CDC tip sisteminde (Clock<D>) → Volt ile aynı fikir
  ✓ FSM, pipeline, FIFO birinci sınıf
  ✓ AI-native tasarım
  ✗ Ternary/çok değerli mantık: YOK
  ✗ Nöromorfik uzantılar: YOK
  ✗ Fotonik uzantılar: YOK
  ✗ CIRCT/MLIR arka ucu: YOK (doğrudan SV üretiyor)
  ✗ Formal doğrulama: sınırlı
  ✗ Çok paradigma vizyonu: YOK
  ? Lisans ve açık kaynak durumu: belirsiz (arch-com = commercial?)
  
  Yayım: Nisan 2026 — ÇOK YENİ
```

**Ne anlama geliyor:** Volt'un CDC-tipte-yakala fikrini bağımsız olarak aynı dönemde birisi de düşünmüş ve uygulamış. Bu hem doğrulama (fikir doğru, zamanlaması doğru) hem de rekabet sinyali.

---

## Bölüm 2 — Aktif Projeler Detaylı Karşılaştırma

### Spade — Linköping/Münih Üniversitesi

<cite index="23">Spade, İsveç'teki Linköping Üniversitesi'nde geliştirilen açık kaynak bir hardware description language. FPL 2022'de tanıtıldı, OSDA 2023'te genişletildi.</cite>

<cite index="26">Spade'in tip sistemi birçok çağdaş HDL'den daha güçlü. Dil, Rust gibi modern yazılım dillerinden ilham alarak güçlü statik tip kontrolü, pipeline yapıları ve port yönetimi için lineer tipler sunuyor.</cite>

```
Spade Özellikleri:
  ✓ Bağımsız dil (Scala/Python/Haskell gömülü değil)
  ✓ Rust sözdizimi ilham kaynağı
  ✓ Güçlü statik tip sistemi
  ✓ Birinci sınıf pipeline desteği
  ✓ Lineer tipler (port yönü güvenliği)
  ✓ Yardımcı hata mesajları
  ✓ Swim build sistemi (paket yönetimi)
  ✓ Aktif geliştirme (PhD tezi Ağustos 2025)
  ✓ NLNet finansmanı
  
  ✗ CDC tip sisteminde YOK
  ✗ Latch güvenliği: sınırlı
  ✗ Ternary/nöromorfik/fotonik: YOK
  ✗ ASIC hedefleme: zayıf (FPGA odaklı)
  ✗ Formal doğrulama: sınırlı

Volt'tan farkı: CDC yokluğu — en kritik açık
```

### Veryl — Açık Kaynak Topluluk

<cite index="24">Veryl, "SystemVerilog'a Modern Bir Alternatif" olarak tanımlanıyor. SystemVerilog'a dayalı sözdizimi kullanıyor, HDL'e özgü optimizasyon yapıyor ve mevcut projelerle yüksek birlikte çalışabilirlik sağlıyor. DVCon Japan 2024'te ve ISCA 2025 workshop'ta sunuldu.</cite>

<cite index="24">Veryl'in mevcut alternatif HDL'lerden farkı şöyle açıklanıyor: "Mevcut alt-HDL'lerin çoğu bir programlama dilinin iç DSL'i. Bu yaklaşımın avantajları var ama sözdizimi donanım tanımı için tam uygun değil. Ayrıca kısa ve sofistike koddan çok büyük Verilog kodu üretiliyor."</cite>

```
Veryl Özellikleri:
  ✓ Rust benzeri sözdizimi
  ✓ Okunabilir SV üretimi (FF seviyesi değiştirilebilir)
  ✓ Paket yöneticisi
  ✓ LSP desteği
  ✓ arXiv makalesi (DVCon 2024)
  ✓ ISCA 2025 workshop
  ✓ Aktif geliştirme

  ✗ SV semantiğini koruyor (gerçek yeni dil değil)
  ✗ CDC dil seviyesinde yok
  ✗ Ternary/nöromorfik/fotonik: YOK
  ✗ Formal doğrulama: YOK
  ✗ Tek semantik model: YOK (SV semantiği)

Volt'tan farkı: "SV'yi güzelleştiriyor", çözmüyor
```

### SUS — pc2 (Paderborn)

<cite index="33,35">SUS, geleneksel sentezlenebilir Verilog ve VHDL'nin doğrudan rakibi olmayı hedefliyor. Temel amacı, netlist oluşturmak için sezgisel ve ince bir sözdizimi sunmak. Tasarımcıya belirli bir paradigma dayatmıyor. Tek kısıtı: tasarımın synchronous olma zorunluluğu.</cite>

```
SUS Özellikleri:
  ✓ Bağımsız dil
  ✓ Rust ile yazılmış derleyici
  ✓ Latency Counting (gecikme sayımı) → pipeline güvenliği
  ✓ LSP desteği (vim/neovim)
  ✓ Aktif geliştirme

  ✗ CDC: YOK (latency counting var ama CDC değil)
  ✗ Ternary/nöromorfik/fotonik: YOK
  ✗ Formal doğrulama: YOK
  ✗ Paket yöneticisi: YOK

Volt'tan farkı: CDC yok, dar kapsam
```

### Clash — Haskell HDL

<cite index="67,68">Clash, Haskell programlama dilinin hem sözdizimini hem anlambilimini ödünç alan fonksiyonel bir HDL. VHDL, Verilog veya SystemVerilog'a derleniyor. Güçlü tip çıkarımıyla birlikte hem güvenli hem hızlı prototipleme imkânı sunuyor. Tip-güvenli çoklu saat alanı ve saat alanı geçişi desteği var.</cite>

<cite index="75">Clash'in CDC desteği pratikte şöyle kullanılıyor: "Clash'in tip sistemi saat alanlarını kodluyor, örtük (veya yanlışlıkla yapılan) geçişleri engelliyor. Diğer donanım tasarım dillerinde bu, sentez sonrasında ancak yakalanan yaygın bir hata kaynağı."</cite>

```
Clash Özellikleri:
  ✓ Tip-güvenli CDC (Signal<domain, type> tipi)
  ✓ Güçlü Haskell tip sistemi
  ✓ Auto-pipelining
  ✓ VHDL/Verilog/SV çıktısı
  ✓ REPL ile interaktif simülasyon
  ✓ Aktif topluluk
  ✓ Endüstriyel kullanım (bittide projesi)

  ✗ Haskell öğrenmek şart → çok dik öğrenme eğrisi
  ✗ Gömülü dil (Haskell bağımlılığı)
  ✗ Ternary/nöromorfik/fotonik: YOK
  ✗ CIRCT arka ucu: YOK

Önemli not: Clash CDC'yi tip sisteminde yapıyor —
Volt'un bu özelliği Clash'te ZATEN VAR (Haskell ile)
```

### Filament — Cornell Üniversitesi (PLDI 2023)

<cite index="11,13">Filament, statik olarak zamanlanmış pipeline'lar için zamanlama ve yapısal kısıtları belirtip zorlayan bir dil. Timeline tipler kullanıyor — bunlar belirli bir sinyalin hangi saat döngüsünde mevcut veya gerekli olduğunu tanımlayan aralıklar.</cite>

<cite index="12">Filament, kısmen pipeline'lanmış modüllere izin veriyor ve tel, register, modüller gibi kaynakların zaman içinde nasıl yeniden kullanıldığını statik olarak garanti ediyor. Filament, tüm yapısal tehlikeleri derleme zamanında ortadan kaldıran ilk dil.</cite>

```
Filament Özellikleri:
  ✓ Timeline tipler (Volt L2'nin karşılığı)
  ✓ Pipeline güvenliği derleme zamanında
  ✓ Kaynak çakışması analizi
  ✓ CIRCT/Calyx'e lowering
  ✓ Apache 2.0 lisans
  ✓ PLDI 2023 (saygın yayın)

  ✗ Sadece statik pipeline — genel amaçlı değil
  ✗ CDC desteği: YOK
  ✗ Ternary/nöromorfik/fotonik: YOK
  ✗ Genel RTL: HAYIR (çok dar kapsam)

Volt ile ilişki: Filament'in timeline tip fikri
Volt L2'nin doğrudan ilham kaynağı (cleanroom uygulama)
```

### Hardcaml — Jane Street (2023)

<cite index="9">Hardcaml, OCaml programlama dilinde gömülü bir donanım tasarım DSL'i. HLS'den farklı olarak maksimum üretkenlik için altta yatan donanım üzerinde düşük seviye kontrol sağlıyor. OCaml'ın zengin tip sistemi, özel tip tanımlama, tip-güvenli parametrik modüller ve elaborasyon zamanı bit genişliği çıkarımı özelliklerine sahip.</cite>

```
Hardcaml Özellikleri:
  ✓ OCaml güçlü tip sistemi
  ✓ Tip-güvenli parametrik modüller
  ✓ Bit genişliği çıkarımı
  ✓ SAT ispatlama ve formal doğrulama araçları
  ✓ Endüstriyel kanıt (ZPrize 2022 FPGA birincisi)
  ✓ MIT lisansı
  
  ✗ OCaml gömülü (bağımlılık)
  ✗ CDC: sınırlı
  ✗ Ternary/nöromorfik/fotonik: YOK
```

---

## Bölüm 3 — Volt'un Bu Tablodaki Yeri

### Özellik Karşılaştırma Matrisi

```
ÖZELLİK              Volt   Arch   Spade  Clash  Veryl  SUS   Filament
──────────────────────────────────────────────────────────────────────────
Bağımsız dil          ✓      ✓      ✓      ✗(H)   ✓*     ✓      ✓
CDC tip sisteminde    ✓      ✓      ✗      ✓      ✗      ✗      ✗
Latch önleme          ✓      ✓      ✗      ✗      ✗      ✗      ✗
Pipeline tip güv.     ✓(L2)  ✓      ✓      ✓(kısm)✗     ✓      ✓✓
Formal doğrulama      ✓      ✗      ✗      ✓(SAT) ✗      ✗      ✗
CIRCT arka ucu        ✓      ✗      ✗      ✗      ✗      ✗      ✓(Calyx)
Ternary/Trit          ✓      ✗      ✗      ✗      ✗      ✗      ✗
Nöromorfik (v2)       ✓      ✗      ✗      ✗      ✗      ✗      ✗
Fotonik (v3)          ✓      ✗      ✗      ✗      ✗      ✗      ✗
Tek semantik model    ✓      ✓      ✗      ✗      ✗      ✗      ✓(kısm)
AI native             ✓      ✓✓     ✗      ✗      ✗      ✗      ✗
Paket yöneticisi      ✓(plan)✗      ✓      ✗      ✓      ✗      ✗
LSP                   ✓(plan)✗      ✓      ✗      ✓      ✓      ✗
Aktif geliştirme      plan   ✓      ✓      ✓      ✓      ✓      ✓(yavaş)
Endüstri kullanımı    ✗      ✗      ✗      ✓      ✗      ✗      ✗
──────────────────────────────────────────────────────────────────────────
* Veryl SV transpiler'a daha yakın
H = Haskell gömülü
```

### Volt'un Gerçek Farkı

```
CDC tipte + bağımsız dil + CIRCT + çok paradigma:
  Arch: CDC tipte ✓, ama CIRCT yok, çok paradigma yok
  Clash: CDC tipte ✓, ama Haskell bağımlı
  Spade: bağımsız ✓, ama CDC yok
  
  Bu üçünü birleştiren: henüz yok.
  Volt bu boşluğu doldurmayı hedefliyor.

Çok paradigma vizyon (Ternary + Nöromorfik + Fotonik):
  Hiçbir projede yok.
  Bu Volt'un en özgün katkısı.
  
  Risk: Bu katkı MVP'de değil — Yıl 3-5 hedefi.
  MVP'de Volt = Arch veya Spade ile rekabet.
  MVP farklılaşması: CDC + latch + CIRCT birlikte.
```

---

## Bölüm 4 — Kritik Çıkarımlar

### Çıkarım 1 — Arch Bir Uyarı

Arch'ın Nisan 2026'da çıkması iki şeyi gösteriyor:

- **Doğrulama:** "CDC tipte, AI-native, birinci sınıf yapılar" fikri doğru zamanda — biri daha düşünmüş.
- **Rekabet:** Arch 3 ay önce yayımlandı. Volt için öncelik önemli.

Arch'ın zayıflığı: dar kapsam. Güçlü yönü: odak.
Volt'un şansı: daha geniş vizyon + CIRCT + çok paradigma.

### Çıkarım 2 — Spade En Yakın Pratik Rakip

Spade hem en aktif hem en olgun bağımsız alternatif. PhD tezi Ağustos 2025'te tamamlandı. NLNet finansmanı var. Topluluk büyüyor.

Spade'in eksik olduğu yer: CDC. Bu Volt'un birincil farklılaşma fırsatı.

### Çıkarım 3 — Clash Göz Ardı Edilmemeli

Clash zaten CDC'yi tip sisteminde yapıyor — ve yıllardır. Volt bunu "yenilik" olarak sunacaksa Clash'i çok iyi bilmek gerekiyor.

Clash'in sorunu: Haskell. Öğrenmesi çok zor. Bu Volt'un açığı.
Volt mesajı: "Clash'in CDC güvenliği, Haskell olmadan."

### Çıkarım 4 — Veryl ile Dikkatli Olunmalı

Veryl çok aktif (ISCA 2025, arXiv, GitHub) ve Volt'a yüzeysel benziyor. Ama "SV transpiler" kategorisinde. Volt'un "neden Veryl değil?" sorusuna cevabı net olmalı: Veryl SV semantiğini koruyor, Volt yeni semantik kuruyor.

### Çıkarım 5 — Filament'in Timeline Tipleri

Filament Volt L2'nin doğrudan öncülü. Apache 2.0 lisansıyla cleanroom uygulama için referans kaynak. CIRCT'e lowering yapıyor — Volt ile ortak arka uç.

---

## Bölüm 5 — Nasıl Konumlandırılmalı

```
"Bu projeler varken neden Volt?"

Arch: CDC ✓, ama ternary/nöromorfik/fotonik yok
Spade: bağımsız ✓, ama CDC yok, çok paradigma yok
Clash: CDC ✓, ama Haskell öğrenmek şart
Veryl: aktif ✓, ama SV kökenli, gerçek semantik yok
Filament: timeline ✓, ama sadece statik pipeline

Volt'un yanıtı:
  "Clash'in CDC güvenliğini,
   Spade'in bağımsız sözdizimini,
   Filament'in timeline tiplerini,
   Arch'ın AI-native yaklaşımını
   tek çatı altında getiriyoruz.
   
   Ve üstüne: ternary, nöromorfik, fotonik —
   hiçbirinde olmayan vizyon."

Dürüst ek:
  Bu vizyon MVP'de kanıtlanmalı.
  MVP'de Volt = "CDC + bağımsız + Rust/Elm kalite hata mesajı"
  Bu üçü birlikte henüz yok.
```

---

## Bölüm 6 — Önerilen Eylemler

```
Hemen yapılacaklar (bu hafta):

1. Arch HDL'i incele (arxiv:2604.05983):
   Tam olarak ne yapıyor?
   Nasıl farklılaşırız?
   Toplulukla bağlantı kurulabilir mi?

2. Spade geliştirici Frans Skarman ile iletişim:
   "CDC desteği planlıyor musunuz?"
   Cevap yoksa: Volt'un fırsatı
   Cevap varsa: işbirliği mi, rekabet mi?

3. Clash topluluğunu incele:
   Neden Haskell'i değiştirmiyorlar?
   Kullanıcılar ne istiyor?
   Bu kitle Volt'un hedef kullanıcısı mı?

4. Veryl farkını dokümante et:
   "Volt neden Veryl değil" — net yazılı yanıt
   Her yerde bu soru gelecek

5. Filament kaynak kodunu incele (Apache 2.0):
   L2 implementasyonu için referans
   CIRCT lowering yaklaşımı öğren
```

---

## Özet

```
Araştırma öncesi varsayım:
  "Volt benzeri proje muhtemelen yok"

Araştırma sonrası gerçek:
  "Volt'un parçalarını yapan projeler var,
   ama hepsini birleştiren yok"

En önemli keşif:
  Arch HDL (Nisan 2026) — 3 ay önce
  CDC tipte + AI-native + birinci sınıf yapılar
  Volt'un ana fikrinin bağımsız doğrulaması

En önemli fırsat:
  CDC + bağımsız dil + çok paradigma üçlüsü
  Şu an hiçbir projede yok

En önemli risk:
  Arch büyürse ve çok paradigmayı eklerse
  Volt'un rekabet avantajı daralır
  → Hız kritik

Sonuç:
  Başlamak için doğru zaman.
  Ama "doğru zaman" hızla kapanıyor.
```
