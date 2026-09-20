# Melez Sistolik Dizi — Sentez Ölçümü (ternary PE ↔ binary PE)

> Statü: **ÖLÇÜM NOTU.** Kod, gramer, spec, ADR ve örnekler değişmedi.
> Tarih: 2026-09-20
> Girdi: `examples/hybrid_accel/` (commit `16d2004`), `volt build` çıktısı
> Araç: Yosys 0.36+42 (`hdlc/formal` Docker imajı), yalnız sentez + `stat`
> Kapsam: kaynak sayımı (LUT / DSP / hücre). Fmax, güç ve gerçek donanım **yok** (§5).

---

## 1. Özet

| Soru | Ölçülen cevap |
|------|---------------|
| Ternary PE'de DSP kullanımı 0 mı? | **Evet.** İki hedefte, DSP çıkarımı açıkken de 0 (tek PE ve 8×8 dizi). |
| DSP'siz karşılaştırmada LUT oranı (ternary / binary, PE başına, dizi içinde) | **iCE40: 0,251 (≈ 4,0× az)** · Xilinx 7: 0,149 (≈ 6,7× az) |
| DSP serbestken LUT oranı | **Ternary lehine DEĞİL.** iCE40: 2,43 (ternary 2,4× **fazla** LUT). Xilinx 7: binary PE 0 LUT kullanıyor, oran tanımsız. |
| i32 birikimin payı | iCE40'ta ternary PE LUT'larının **%57'si**, binary PE'nin (DSP'siz) %15'i. |

Tek cümle: ternary PE çarpanı gerçekten ortadan kaldırıyor ve DSP harcamıyor;
ama "daha az LUT" iddiası yalnız **binary çarpan LUT'a serildiğinde** doğru.
DSP bloğu olan bir hedefte binary PE çarpımı (Xilinx'te birikimi de) DSP'ye
gömüyor ve LUT sayısında ternary'nin **altına** iniyor. Gerçek takas
"LUT ↔ DSP bloğu" takasıdır, "az LUT ↔ çok LUT" değil.

---

## 2. Ölçüm tabloları

`volt build examples/hybrid_accel/hybrid_top.volt` 5 kaynak dosyadan 7 SV
modülü üretti: `TernaryPe` (41 satır), `TernaryArray` (1018), `BinaryPe` (39),
`BinaryArray` (282), `TernaryCtl` (45), `BinaryFront` (61), `HybridTop` (230).
Aşağıda ilk dördü ölçüldü.

"LUT" = hedefin LUT hücreleri toplamı (iCE40: `SB_LUT4`; Xilinx: `LUT1..LUT6`
toplamı). "Hücre" = Yosys `Number of cells` (LUT + elde + FF + DSP + diğer).
"PE başına" = dizi LUT'u / PE sayısı.

### 2.1 Hedef 1 — `synth_ice40` (DSP çıkarımı KAPALI — varsayılan)

LUT sayımının net olduğu, iki PE'nin aynı kaynak türüyle karşılaştırıldığı koşu.

| Modül               | LUT  | DSP | Hücre | PE başına LUT | Elde (`SB_CARRY`) | FF (`SB_DFFSR`) |
|---------------------|-----:|----:|------:|--------------:|------------------:|----------------:|
| TernaryPe           |   56 |   0 |   134 | —             |   38 |   40 |
| BinaryPe            |  209 |   0 |   291 | —             |   42 |   40 |
| TernaryArray (8×8)  | 3150 |   0 |  7533 | 3150/64 = **49,22** | 2007 | 2376 |
| BinaryArray (4×4)   | 3133 |   0 |  4192 | 3133/16 = **195,81** |  483 |  576 |

**Oran (PE başına LUT, ternary / binary):** dizi içinde 49,22 / 195,81 = **0,251**
(≈ 3,98× az); tek PE 56 / 209 = 0,268 (≈ 3,73× az).

Yan gözlem: 64 ternary PE (3150 LUT) ile 16 binary PE (3133 LUT) aynı LUT
bütçesine sığıyor.

### 2.2 Hedef 1b — `synth_ice40 -dsp` (SB_MAC16 çıkarımı AÇIK)

| Modül               | LUT  | DSP (`SB_MAC16`) | Hücre | PE başına LUT | Elde | FF |
|---------------------|-----:|----:|------:|--------------:|-----:|-----:|
| TernaryPe           |   56 |   0 |   134 | —             |   38 |   40 |
| BinaryPe            |   32 |   1 |   104 | —             |   31 |   40 |
| TernaryArray (8×8)  | 3150 |   0 |  7533 | **49,22**     | 2007 | 2376 |
| BinaryArray (4×4)   |  324 |  16 |  1228 | 324/16 = **20,25** |  312 |  576 |

**Oran:** 49,22 / 20,25 = **2,43** — ternary PE, DSP'li binary PE'den 2,4× **fazla**
LUT kullanıyor (tek PE: 56 / 32 = 1,75). Binary PE'nin 32 LUT'u tam olarak
i32 toplayıcıdır (§3); çarpan bütünüyle `SB_MAC16`'ya gitti.

TernaryPe satırı `-dsp` ile yeniden koşulmadı; TernaryArray `-dsp` ile koşuldu
ve DSP'siz koşuyla hücre hücre aynı çıktı (0 `SB_MAC16`), TernaryPe değerleri
§2.1'den taşındı.

### 2.3 Hedef 2 — `synth_xilinx` (7 serisi, DSP çıkarımı AÇIK — varsayılan)

| Modül               | LUT  | DSP (`DSP48E1`) | Hücre | PE başına LUT | `CARRY4` | FF (`FDRE`) | `INV` |
|---------------------|-----:|----:|------:|--------------:|-----:|-----:|----:|
| TernaryPe           |   32 |   0 |    91 | —             |   11 |   40 |   8 |
| BinaryPe            |    0 |   1 |     9 | —             |    0 |    8 |   0 |
| TernaryArray (8×8)  | 1716 |   0 |  5244 | 1716/64 = **26,81** |  640 | 2376 |  512 |
| BinaryArray (4×4)   |    0 |  16 |   144 | 0/16 = **0**  |    0 |  128 |   0 |

**Oran:** tanımsız (payda 0). Binary PE tek bir `DSP48E1` + 8 FF'ten ibaret:
çarpım, i32 toplama **ve 32 bitlik `acc` yazmacı** DSP bloğunun içine
(çarp-topla + P yazmacı) gömüldü; dışarıda yalnız `act_r` (8 FF) kaldı.
LUT ayrıntısı: TernaryPe 32×`LUT5`; TernaryArray 108×`LUT4` + 1608×`LUT5`.

### 2.4 Hedef 2b — `synth_xilinx -nodsp` (DSP çıkarımı KAPALI)

| Modül               | LUT  | DSP | Hücre | PE başına LUT | `CARRY4` | FF | `MUXF7`/`MUXF8` |
|---------------------|-----:|----:|------:|--------------:|-----:|-----:|--------:|
| TernaryPe           |   32 |   0 |    91 | —             |   11 |   40 | 0 / 0   |
| BinaryPe            |  194 |   0 |   296 | —             |   12 |   40 | 39 / 11 |
| TernaryArray (8×8)  | 1716 |   0 |  5244 | **26,81**     |  640 | 2376 | 0 / 0   |
| BinaryArray (4×4)   | 2888 |   0 |  4413 | 2888/16 = **180,50** |  160 |  576 | 612 / 177 |

**Oran:** 26,81 / 180,50 = **0,149** (≈ 6,7× az); tek PE 32 / 194 = 0,165.
Ternary satırları §2.3'ten taşındı (ternary'de DSP adayı olmadığı §2.3'te
ölçüldü; `-nodsp` ile ayrıca koşulmadı).
BinaryPe LUT kırılımı: 39 `LUT2` + 13 `LUT3` + 27 `LUT4` + 17 `LUT5` + 98 `LUT6`.
BinaryArray: 1 `LUT1` + 423 `LUT2` + 205 `LUT3` + 439 `LUT4` + 285 `LUT5` + 1535 `LUT6`.

### 2.5 Oran özeti

| Karşılaştırma | Ternary PE başına LUT | Binary PE başına LUT | Oran T/B | Binary DSP/PE |
|---------------|----------------------:|---------------------:|---------:|--------------:|
| iCE40, DSP'siz          | 49,22 | 195,81 | **0,251** | 0 |
| iCE40, `-dsp`           | 49,22 |  20,25 | **2,43**  | 1 |
| Xilinx 7, varsayılan    | 26,81 |   0,00 | tanımsız (B = 0) | 1 |
| Xilinx 7, `-nodsp`      | 26,81 | 180,50 | **0,149** | 0 |

---

## 3. i32 birikimin payı

İki PE'de de ortak olan kısım: `acc <= acc_in + sext(product)` (32 bit toplayıcı
+ 32 FF) ve `act_r` (8 FF). Bunu ayırmak için çarpım/seçim mantığı çıkarılmış,
`product`'ı doğrudan port olarak alan bir taban modül (`AccOnly`, **elle yazılmış
SV — Volt çıktısı değil**, `build/olcum/rtl/AccOnly.sv`) aynı komutlarla
sentezlendi. `product` genişliği 9 (ternary) ve 16 (binary) için sonuç aynı:

| Hedef | LUT | Elde | FF |
|-------|----:|-----:|---:|
| iCE40      | 32 (`SB_LUT4`) | 31 `SB_CARRY` | 40 |
| Xilinx 7   | 32 (`LUT2`)    | 8 `CARRY4`    | 40 |

Taban çıkarıldığında çarpım/seçim mantığının kendi maliyeti:

| Hedef (DSP'siz) | Ternary "çarpım" (MUX + negasyon) | Binary çarpım (8×8 işaretli) | Oran |
|-----------------|-----------------------------------|------------------------------|-----:|
| iCE40    | 56 − 32 = **24 LUT** + 7 `SB_CARRY` | 209 − 32 = **177 LUT** + 11 `SB_CARRY` | 0,136 (≈ 7,4×) |
| Xilinx 7 | 32 − 32 = **0 LUT** + 3 `CARRY4` + 8 `INV` | 194 − 32 = **162 LUT** + 4 `CARRY4` + 50 `MUXF7/8` | 0 |

Pay olarak:

- **iCE40:** birikim, ternary PE LUT'larının 32/56 = **%57**'si; binary PE'nin
  (DSP'siz) 32/209 = %15'i; `-dsp` binary PE'nin 32/32 = %100'ü.
  FF'lerin tamamı (40/40) iki PE'de de birikim + `act_r`'dir; çarpım mantığı
  FF eklemiyor.
- **Xilinx 7:** ternary PE'nin LUT **sayısı** tabanla aynı (32). MUX + negasyon
  ayrı LUT harcamadı; toplayıcının `LUT2`'leri `LUT5`'e genişleyerek seçimi
  içine aldı. Ek maliyet 3 `CARRY4` + 8 `INV`. Yani LUT sayısı ölçütüyle
  ternary PE'nin %100'ü birikimdir — ama bu LUT'lar daha geniştir (aşağıda sınır).

Sonuç: PE seviyesinde ternary'nin 4× (iCE40) kazancı, çarpım mantığındaki
7,4× kazancın ortak 32 LUT'luk toplayıcıyla seyrelmiş hâlidir. Ternary PE'de
baskın maliyet artık çarpım değil, **i32 birikimdir**; daha fazla kazanç
ancak birikim genişliği daraltılarak gelir (ölçülmedi).

### Dizi içinde PE başına değer neden tek PE'den düşük?

TernaryArray 49,22 < TernaryPe 56; BinaryArray 195,81 < BinaryPe 209.
Doğrulanan kısım: her satırın ilk PE'sinde `acc_in = 0` sabit, toplayıcı
düşüyor ve `acc` yazmacı `product` genişliğine iniyor. FF sayıları bunu
tam tutuyor: ternary 64×40 − 8×(32−9) = 2376 ✓, binary 16×40 − 4×(32−16) = 576 ✓.
LUT farkının geri kalanı (düzleştirme sonrası modüller arası eniyileme)
ayrıştırılmadı.

---

## 4. Yorum — beklentiyle karşılaştırma

**Beklentiye UYAN:**

1. Ternary PE'de DSP = 0. Çıkarım açıkken (`synth_xilinx` varsayılanı,
   `synth_ice40 -dsp`) bile tek PE'de ve 64 PE'lik dizide DSP hücresi yok.
2. Binary PE'de `$signed` çarpım DSP'ye eşleniyor: PE başına tam 1 DSP
   (16 PE → 16 `DSP48E1` / 16 `SB_MAC16`).
3. DSP'siz karşılaştırmada ternary PE belirgin biçimde küçük: iCE40'ta ≈ 4×,
   Xilinx 7'de ≈ 6,7× az LUT.

**Beklentiye UYMAYAN (olduğu gibi):**

1. **DSP'li hedefte binary PE, ternary PE'den daha AZ LUT kullanıyor.**
   iCE40 `-dsp`: 20,25'e karşı 49,22 LUT/PE. Xilinx 7: 0'a karşı 26,81 LUT/PE.
   "Ternary = daha az LUT" ifadesi DSP'si olan bir FPGA'da yanlıştır.
   Doğru ifade: ternary PE, PE başına 1 DSP bloğunu ≈ 27–29 LUT ile takas eder
   (iCE40: 49,22 − 20,25 ≈ 29; Xilinx: 26,81 − 0 ≈ 27, ayrıca +32 FF).
2. **Xilinx 7'de binary PE FF'te de küçük:** 8 FF'e karşı ternary 40 FF, çünkü
   `acc` yazmacı DSP48E1'in P yazmacına gömülüyor. Hücre sayısında fark
   çarpıcı: BinaryArray 144 hücre, TernaryArray 5244 hücre (PE başına 9'a
   karşı 82).
3. **4× kazanç çarpanın değil toplam PE'nin kazancı ve i32 birikim bunu
   sınırlıyor.** Çarpım mantığı tek başına 7,4× küçülüyor ama PE'nin %57'si
   (iCE40) ortak toplayıcı olduğu için PE seviyesinde 4×'te kalıyor.
4. **Kazanç dizinin tamamına yansımıyor:** bu tasarımda 64 ternary PE ≈ 16
   binary PE LUT bütçesi (3150 ≈ 3133, iCE40 DSP'siz). Ternary dizi 4× fazla
   PE içerdiği için toplam LUT'ta küçülme **yok**; kazanç "aynı alana 4× PE"
   biçiminde okunmalı, "daha küçük çip" biçiminde değil.

**Hangi durumda ternary gerçekten kazanır (ölçümden çıkan, iddia değil koşul):**
DSP'siz hedef (ör. iCE40 HX/LP ailesi) veya DSP bloklarının başka iş için
tükendiği / PE sayısının DSP sayısını aştığı durum. Bu koşulların hiçbiri bu
turda bir cihaza yerleştirilerek sınanmadı; Yosys `stat` cihaz kapasitesini
denetlemez.

---

## 5. Metodoloji

**SV üretimi** (depo kökünden, `cargo build` sonrası güncel ikili):

```
target\debug\volt.exe build --target-dir build\olcum examples\hybrid_accel\hybrid_top.volt
```

**Sentez** — her koşu ayrı konteyner, tam log `build/olcum/log/<ad>.log`'a,
bağlama yalnız son `stat` bloğunun hücre satırları alındı
(`build/olcum/synth.ps1`; `build/` git tarafından yok sayılır):

```
docker run --rm -v "C:/Dev/volthdl/build/olcum:/work" -w /work/rtl hdlc/formal \
  yosys -q -l /work/log/<ad>.log -p "read_verilog -sv <dosyalar>; synth_<hedef> -top <Modül> -flatten <ek>; stat"
```

| Koşu | `<hedef>` ve `<ek>` |
|------|---------------------|
| §2.1 | `synth_ice40 -flatten` |
| §2.2 | `synth_ice40 -flatten -dsp` |
| §2.3 | `synth_xilinx -flatten -noiopad -noclkbuf` (aile varsayılanı: xc7) |
| §2.4 | `synth_xilinx -flatten -noiopad -noclkbuf -nodsp` |
| §3   | aynı komutlar, `AccOnly.sv`; PW=16 için `chparam -set PW 16 AccOnly` |

- Dosyalar: tek PE için `TernaryPe.sv` / `BinaryPe.sv`; diziler için PE + dizi dosyası.
- `-flatten`: dizi toplamı tek `stat` bloğunda okunsun ve PE'ler arası eniyileme
  gerçek akıştaki gibi çalışsın diye.
- `-noiopad -noclkbuf`: modüller bağlam dışı ölçülüyor; aksi hâlde her port biti
  `IBUF`/`OBUF` olarak hücre sayısına giriyor (TernaryPe'de 176'ya karşı 91 hücre).
- Sürüm: `Yosys 0.36+42 (git sha1 70d35314d, clang 11.0.1-2 -fPIC -Os)`, ABC
  bu sürümle gelen; Docker 29.7.2; imaj `hdlc/formal:latest`.
- Toplam 17 sentez koşusu; hiçbirinde `ERROR` yok.
- Ağırlıklar her iki dizide de **çalışma zamanı portu** (`weight[127:0]`); sabit
  değil. Sentez ağırlık değerlerinden yararlanamıyor.

---

## 6. Sınırlar

- **Güç tüketimi ölçülmedi.** LUT/DSP sayısı güç vekili değildir; DSP bloğu ile
  eşdeğer LUT mantığının dinamik gücü karşılaştırılmadı.
- **Gerçek FPGA'da test edilmedi.** Yer-rota yapılmadı; sayılar Yosys teknoloji
  eşlemesi sonrası `stat` çıktısıdır, yerleştirme sonrası kullanım değil.
  Cihaz kapasitesi (LUT/DSP adedi) denetlenmedi.
- **Fmax ölçülmedi.** İmajda `nextpnr`/`icetime` yok; tasarımdaki 400 MHz
  (ternary) / 200 MHz (binary) hedefleri bu turda **doğrulanmadı**. Ternary
  PE'nin 32 bitlik elde zinciri ile DSP'nin iç çarp-topla yolu arasındaki
  zamanlama farkı bilinmiyor.
- **Model doğruluğu değerlendirilmedi.** Ternary ({−1, 0, +1}) ağırlıkların
  i8 ağırlıklara göre ağ doğruluğuna etkisi bu ölçümün dışındadır; "PE başına
  4× az LUT" eşit iş çıkardığı anlamına gelmez.
- **Tek araç, eski sürüm.** Yalnız Yosys 0.36 + ABC. Vivado / Radiant / yeni
  Yosys (`-abc9`) farklı sayılar verebilir; özellikle LUT'a serilmiş 8×8
  çarpanın 177–194 LUT'luk maliyeti araca duyarlıdır.
- **LUT sayısı ≠ LUT alanı (Xilinx).** Ternary `LUT5`, binary `LUT2..LUT6`
  karışımı kullanıyor; 7 serisinde fiziksel LUT6'ya paketleme (iki LUT5 → bir
  LUT6) yapılmadı. 512 `INV` hücresi gerçek akışta komşu LUT/elde girişine
  soğurulur; burada ayrı sayıldı ve LUT sütununa katılmadı.
- **Ağırlık kodlaması.** Ternary ağırlık `i2` + elle MUX ile yazıldı; Volt'un
  `Trit` tipi SV'ye eşlenemiyor (E0003). Gerçek `Trit` eşlemesi farklı mantık
  üretebilir.
- **Sabit ağırlık senaryosu ölçülmedi.** Ağırlıklar sabit olsaydı ternary PE'de
  MUX tamamen düşer (0 / geçir / eksi), binary çarpan da sabit-katsayı
  çarpanına inerdi; iki tarafın oranı bu durumda bilinmiyor.
- **Dizi boyutları eşit değil** (8×8 ↔ 4×4); karşılaştırma yalnız PE başına
  normalize edilerek anlamlıdır, dizi toplamları doğrudan kıyaslanamaz.
- **Taban modül elle yazıldı** (§3); Volt derleyicisinden geçmedi. Yalnız
  birikim payını ayırmak için kullanıldı, ana tabloya girmedi.
- Ölçülmeyen modüller: `TernaryCtl`, `BinaryFront`, `HybridTop` (AsyncFifo
  köprüsü dahil sistem toplamı yok).
