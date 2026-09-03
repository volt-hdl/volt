> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Akademik HDL Projelerinden Yasal Bilgi Yeniden Kullanımı

---

## Bölüm 1 — Temel Hukuki Ayrım

Önce kritik kavramsal ayrım:

```
FİKİR                   KOD
──────────────────────────────────────────────────────
Telif hakkıyla          Telif hakkıyla
KORUNMAZ                KORUNUR

"CDC'yi tipte yakala"   Bu fikri uygulayan kod
→ Serbestçe kullanılır  → Lisansa tabi

"İki interval arasında  Bu algoritmayı uygulayan
 çakışma kontrolü"      somut fonksiyon
→ Serbest               → Lisansa tabi

Makale yayımlandı       Kodun deposu kapalı
→ Fikir tamamen serbest → "Lisans yok" = kullanma
```

**Temel kural:**
- Makale → **fikir** kaynağı → lisanssız kullanılır, atıf yapılır
- Kod → **ifade** kaynağı → lisansa göre kullanılır
- Bağımsız yeniden uygulama → her zaman serbest

---

## Bölüm 2 — Lisans Türleri ve Ne Yapılabilir

```
LİSANS         KULLANIMLAR                    KISIT
──────────────────────────────────────────────────────────────
MIT            Her şey: kullan, değiştir,     Telif hakkı notu
               dağıt, ticari kullan           koru (tek şart)

BSD-2, BSD-3   MIT ile neredeyse aynı         Telif + kaynak atfı

Apache 2.0     MIT gibi + açık patent izni    Telif + NOTICE dosyası
               (önemli fark!)

ISC            MIT'ten bile kısa, aynı güç   Telif notu

GPL v2/v3      Kullan, değiştir ama:          Türev çalışma
               türev çalışma da GPL olmak     da GPL olmak zorunda
               zorunda (copyleft)             → Ticari sorun

AGPL           GPL + ağ üzerinden sunma       En kısıtlayıcı
               da GPL kapsamında

Lisans YOK     Resmi olarak: kullanamassın   Varsayılan telif
               Gerçekte: risk taşır           hakkı geçerli
```

**Volt için pratik:**

```
MIT/BSD/Apache/ISC lisanslı kod:
  → Doğrudan kullanılabilir (atıf ile)
  → Ticari kullanım dahil

GPL kod:
  → Volt açık kaynak kalacaksa: kullanılabilir (Volt da GPL olur)
  → Volt'un kapalı/ticari kısmı varsa: KULLANMA

Lisanssız kod:
  → Makaleyi oku, bağımsız uygula (cleanroom)
  → Kodu kopyalama
```

---

## Bölüm 3 — Volt İçin Değerli Akademik Çalışmalar

### 3.1 Doğrudan Kullanılabilecek (Kod Seviyesi)

**Calyx / Futil (Cornell Üniversitesi)**

```
Lisans: MIT
Durum: Aktif (ama küçük ekip, çoğu zaman "yavaş")
Ne içeriyor:
  → Orta seviye IR, veri yolu + kontrol akışı ayrımı
  → Bu ayrım Volt HIR tasarımı için ders
  → Lowering geçiş altyapısı

Volt için:
  Calyx'in IR ayrım felsefesini öğren (makale)
  Kod: MIT → altta kullanma (Calyx zaten CIRCT içinde)
  
Makale: "A Compiler Infrastructure for Accelerator Generators"
        ASPLOS 2021
```

**Filament (Cornell)**

```
Lisans: Apache 2.0
Durum: Araştırma, yavaş geliştirme
Ne içeriyor:
  → Timeline tip sistemi (interval türleri)
  → Pipeline güvenliği derleme zamanında
  → Tam olarak Volt L2'nin hedeflediği şey!

Volt için:
  Makaleyi oku → L2 tip sistemi tasarımı için altın kaynak
  Kod: Apache 2.0 → referans olarak kullanılabilir
  Ama: Filament'in sözdizimi farklı, semantik farklı
       Volt L2 bağımsız uygulama olacak (cleanroom)
       
Makale: "Modular Hardware Design with Timeline Types"
        PLDI 2023
```

**Bluespec BSV (MIT kökenli)**

```
Lisans: ISC (2020'de açık kaynak oldu)
Durum: Şirket ürünü açık kaynak, aktif değil
Ne içeriyor:
  → Guarded Atomic Actions (korumalı atomik eylemler)
  → Kural tabanlı tasarım semantiği
  → Volt'un "rule" bloğuyla örtüşüyor

Volt için:
  ISC → doğrudan kullanılabilir
  Guarded action semantiği → Volt FSM ve rule tasarımına
  direkt ilham kaynağı
  
Makale serisi: Arvind et al., MEMOCODE konferansları
```

**HardCaml (Jane Street)**

```
Lisans: MIT
Durum: Aktif kullanım (Jane Street içinde) ama gelişme yavaş
Ne içeriyor:
  → OCaml gömülü HDL
  → Güçlü tip sistemi, fonksiyonel RTL
  → Pipeline abstraction desenleri

Volt için:
  MIT → kullanılabilir
  Pipeline tip desenleri → Volt stdlib için ilham
  OCaml → Rust çevirisi bağımsız uygulama sayılır
```

**DAHLIA (Cornell)**

```
Lisans: Apache 2.0  
Durum: Makale çıktı, büyük ölçüde durdu
Ne içeriyor:
  → Affine tipler donanım için
  → "Bu bellek erişimi iki kez yapılamaz" garantisi
  → Kaynak kullanımını tipte izleme

Volt için:
  Bellek erişim analizi → ileride Volt'a eklenebilir
  Makale → fikir kaynağı (serbest)
  Kod → Apache 2.0 (referans kullanılabilir)
  
Makale: "Predictable Accelerator Design with Time-Sensitive Affine Types"
        PLDI 2020
```

---

### 3.2 Fikir Kaynağı Olarak Değerliler (Bağımsız Uygulama)

**Aetherling (Stanford)**

```
Lisans: MIT
Durum: Durdu (makale çıktı, geliştirici başka projeye geçti)
Ne içeriyor:
  → Uzay-zaman tür teorisi (space-time type theory)
  → Veri akışı paralelliğini tipte ifade etme
  → Vektörleştirme güvenliği

Volt için:
  Çok teorik → Volt MVP'de gerekmez
  Uzun vade: veri akışı semantiği için ilham
  MIT → kod kullanılabilir ama çok farklı paradigma
```

**LAPIS (TU Delft)**

```
Lisans: Açık kaynak (lisans belirsiz — DİKKAT)
Durum: Akademik proje, muhtemelen durdu
Ne içeriyor:
  → Donanım için güvenlik tip sistemi
  → Side-channel analizi HDL seviyesinde

Volt için:
  LİSANS BELİRSİZ → kodu kullanma
  Makaleyi oku → fikir kaynağı (serbest)
```

**Kiwi (Cambridge)**

```
Lisans: Akademik lisans (ticari kullanım kısıtlı)
Durum: Durdu
Ne içeriyor:
  → C# → donanım sentezi
  → Yüksek seviye sentez denemesi

Volt için:
  LİSANS KISITLI → kullanma
  Makale → HLS yaklaşımlarından ders çıkar (serbest)
```

---

## Bölüm 4 — "Cleanroom" Yeniden Uygulama

Bu yaklaşım hem yasal hem etik açıdan en güçlü konum.

**Nasıl çalışır:**

```
1. Makaleyi oku, anla
2. Kodu OKUMA
3. Algoritmayı bağımsız olarak yeniden uygula
4. Atıf yap (makaleye, koda değil)

Bu yaklaşımla:
  → Telif hakkı ihlali: yok (fikir korumasız)
  → Patent riski: minimal (uygulama farkı)
  → Etik sorun: yok (akademinin yayın amacı paylaşmak)
```

**Filament Timeline Tipleri — Cleanroom Örneği:**

```
Filament makalesi oku:
  "Pipeline güvenliği için interval türleri kullanılıyor.
   Bir sinyalin geçerli olduğu zaman aralığı [s, e)
   biçiminde ifade ediliyor. Çakışma analizi..."

Kodu OKUMA — sadece makaleyi kullan.

Volt L2'yi bağımsız tasarla:
  @['G+[0,1)) → Volt'un gösterimi
  Çakışma kontrolü → Volt tip çıkarımında
  
  Filament'in kodu ile % olarak örtüşme: minimal
  (Rust vs OCaml, farklı IR, farklı semantik)
  
Atıf yap (Volt dokümantasyonunda):
  "Timeline tipler üzerine Filament [PLDI 2023]'ten
   ilham alındı."
```

---

## Bölüm 5 — Patent Riski: Asıl Dikkat Edilmesi Gereken

Lisanstan daha tehlikeli olabilir.

```
Üniversite patent durumu:

MIT, Stanford, Carnegie Mellon, Cornell:
  → Aktif tech transfer ofisleri
  → Araştırma sık sık patentleniyor
  → "Açık kaynak" ile "patent yok" FARKLI şeyler

MIT lisansı bir kodun patentten korunduğu anlamına gelmez!
Sadece kodun kullanım izni verir.

Apache 2.0 farklı:
  → Açık patent izni içeriyor
  → Katkıcılar patent iddiasından vazgeçiyor
  → En güvenli lisans bu nedenle

MIT lisanslı ama patentli kod:
  → Kodu kullanabilirsin (MIT izin veriyor)
  → Ama uyguladığın METHOD patentli olabilir
  → Teorik risk (pratikte akademik HDL'de nadir)
```

**Pratik risk değerlendirmesi:**

```
Volt için patent riski:
  Yüksek risk: sıfır (HDL algoritmalar nadiren patentlenir)
  Orta risk:   tip çıkarım algoritmaları (ama yaygın bilgi)
  Düşük risk:  CIRCT altyapısı (Apache 2.0 → patent izni dahil)

Dikkat edilmesi gereken VARSA:
  Bluespec BSV → Bluespec Inc. patent portföyü olabilir
  → ISC lisans var ama patentler ayrı mesele
  → Guarded atomic actions kullanırken araştır
```

---

## Bölüm 6 — Pratik Kontrol Listesi

Bir akademik projeden yararlanmadan önce:

```
□ Lisansı kontrol et
  → MIT/BSD/Apache: yeşil ışık
  → GPL: Volt da GPL olacak mı karar ver
  → ISC: yeşil ışık
  → "Telif hakkı saklıdır" veya lisans yok: kullanma

□ Üniversite patent araştırması
  → Google Patents → üniversite adı + konu ara
  → Genellikle temiz ama 5 dakika harcamaya değer

□ Makale mi, kod mu kullanıyorsun?
  → Makale fikri: her zaman serbest, atıf yap
  → Kod: lisansa göre hareket et

□ Cleanroom mı, doğrudan kullanım mı?
  → Cleanroom: güçlü yasal konum
  → Doğrudan: hızlı ama lisans şartlarına uy

□ Atıf ver
  → Akademik dürüstlük + topluluk güveni
  → "X projesinden ilham alındı" ifadesi yeterli
  → Zorunlu değil ama güçlü itibar etkisi var
```

---

## Bölüm 7 — Volt İçin Öncelikli Çalışmalar

```
ÇALIŞMA                    LİSANS     VOLT KULLANIMI       ÖNCELİK
─────────────────────────────────────────────────────────────────────
Filament (Cornell)         Apache 2.0 L2 tip sistemi        YÜKSEK
  makale PLDI 2023

Calyx/Futil (Cornell)      MIT        HIR tasarımı + IR     YÜKSEK
  makale ASPLOS 2021

Bluespec BSV               ISC        rule semantiği        ORTA
  kaynak: github/B-Lang-org

DAHLIA (Cornell)           Apache 2.0 bellek erişim         ORTA
  makale PLDI 2020

HardCaml (Jane Street)     MIT        pipeline abstraction  DÜŞÜK
  kaynak: github/janestreet

Aetherling (Stanford)      MIT        uzun vade veri akışı  DÜŞÜK
  makale PLDI 2019
─────────────────────────────────────────────────────────────────────

En acil okuma listesi:
  1. Filament PLDI 2023 → L2 tip sistemi tasarımından önce
  2. Calyx ASPLOS 2021 → HIR tasarımından önce
  3. Bluespec makaleleri (Arvind, MEMOCODE) → rule semantiği için
```

---

## Özet

```
Yasal olarak güvenli:
  ✓ Makalelerdeki fikir ve algoritmaları uygulamak
  ✓ MIT/BSD/Apache/ISC lisanslı kodu kullanmak (atıf ile)
  ✓ Cleanroom yeniden uygulama (makale oku, kod yazma)
  ✓ GPL kodunu kullanmak (Volt da GPL olursa)

Dikkat:
  ⚠ "Lisans yok" = kullanma (bağımsız uygula)
  ⚠ MIT lisans ≠ patent izni (Apache 2.0 güvenli)
  ⚠ Bluespec patent portföyünü araştır

Volt için altın kural:
  Filament + Calyx makalelerini oku (serbest)
  Kodlarını referans al (MIT/Apache izin veriyor)
  Volt'u bağımsız uygula (cleanroom → güçlü konum)
  Akademik atıf yap (zorunlu değil, ama itibar kazandırır)
  
En değerli varlık:
  Bu projelerin BAŞARISIZLIK NEDENLERİ
  → Topluluk olmadan ölüm
  → Makale sonrası terk
  → Yanlış hedef kitle
  Bu bilgi tamamen serbest — ve Volt'un
  farklılaşmasının temeli.
```
