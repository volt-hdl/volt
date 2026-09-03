> STATÜ: Arka plan araştırması. Bağlayıcı karar değildir.

# Ternary AI Hızlandırıcı Prototipi — Adım Adım Kılavuz

> Sıfırdan çalışan bir donanıma giden yol.
> Her aşama bir öncekinin çıktısı üzerine kuruluyor.
> Aşama atlamak = geri dönmek.

---

## Genel Bakış: Beş Aşama

```
AŞAMA 0  Model doğrulama (yazılım)          2-3 ay    $0
AŞAMA 1  RTL tasarım + simülasyon           3-4 ay    $0-5K
AŞAMA 2  FPGA prototipi                     4-6 ay    $3-15K
AŞAMA 3  Sistem entegrasyonu                3-4 ay    $5-20K
AŞAMA 4  ASIC (opsiyonel)                   18-24 ay  $500K-15M
─────────────────────────────────────────────────────────────
FPGA'ya kadar:                              12-17 ay  $10-40K
```

**Kritik gerçek:** Aşama 0-3 tek kişi tarafından yapılabilir.
Aşama 4 ekip ve ciddi sermaye gerektirir.

---

# AŞAMA 0 — Model Doğrulama (Yazılım)

Donanıma dokunmadan önce: ternary modelin gerçekten çalıştığını kanıtla.

## 0.1 Referans Model Seçimi

```bash
# BitNet b1.58 modelini indir ve çalıştır
git clone https://github.com/microsoft/BitNet
cd BitNet && pip install -r requirements.txt

# 2B ternary model (küçük başla)
huggingface-cli download microsoft/BitNet-b1.58-2B-4T-gguf

# CPU'da çalıştır — donanım yok, sadece doğrulama
python run_inference.py -m models/bitnet-2b.gguf \
    -p "Merhaba" -n 50
```

**Bu adımın amacı:** Ternary'nin gerçekten çalıştığını kendi
gözünle görmek. Teori değil, çalışan çıktı.

## 0.2 Ağırlık İstatistikleri Çıkarma

Donanım tasarımı bu sayılara bağlı:

```python
import numpy as np
from safetensors import safe_open

stats = {}
with safe_open("model.safetensors", framework="pt") as f:
    for key in f.keys():
        if "weight" not in key: continue
        w = f.get_tensor(key).numpy()

        # Ternary dağılımı
        total = w.size
        stats[key] = {
            "shape": w.shape,
            "zero_pct":  (w == 0).sum() / total,
            "pos_pct":   (w == +1).sum() / total,
            "neg_pct":   (w == -1).sum() / total,
        }

# Kritik çıktılar:
# 1. Sıfır oranı → zero-skip kazancı ne kadar?
# 2. Matris boyutları → sistolik dizi boyutu ne olmalı?
# 3. Katman sayısı → kaç kez tekrar kullanılacak?

avg_zero = np.mean([s["zero_pct"] for s in stats.values()])
print(f"Ortalama sıfır oranı: {avg_zero:.1%}")
# Tipik: %50-70 → enerji tasarrufu bu orandan gelir
```

## 0.3 Bit-Exact Referans Simülatör

Donanımın doğruluğunu ölçmek için altın referans:

```python
def ternary_matmul_reference(weights, activations):
    """Donanımın taklit edeceği tam davranış.
    Kayan nokta YOK — tam sayı aritmetiği."""
    M, K = weights.shape
    N = activations.shape[1]
    out = np.zeros((M, N), dtype=np.int32)

    for i in range(M):
        for j in range(N):
            acc = np.int32(0)
            for k in range(K):
                w = weights[i, k]        # -1, 0, +1
                a = activations[k, j]    # INT8
                if w == 1:    acc += a
                elif w == -1: acc -= a
                # w == 0 → atlanıyor (donanımda enerji yok)
            out[i, j] = acc
    return out

# Bu fonksiyonun çıktısı = donanımın üretmesi gereken çıktı
# Her RTL testi buna karşı karşılaştırılacak
np.save("golden_vectors.npy", test_cases)
```

**Aşama 0 çıktısı:**
- Çalışan ternary model
- Ağırlık istatistikleri (sıfır oranı, boyutlar)
- Bit-exact referans + altın test vektörleri

---

# AŞAMA 1 — RTL Tasarım ve Simülasyon

## 1.1 Mimari Boyutlandırma

```
Karar verilecekler:

Sistolik dizi boyutu:
  16×16   = 256 PE   → küçük FPGA, öğrenme
  32×32   = 1,024 PE → orta FPGA, gerçekçi başlangıç
  64×64   = 4,096 PE → büyük FPGA
  128×128 = 16K PE   → sadece büyük FPGA / ASIC

Hesap: performans = PE_sayısı × saat_frekansı × 2 (MAC)
  32×32 @ 200 MHz = 1024 × 200M × 2 = 410 GOPS
  Sıfır atlama %50 → efektif ~800 GOPS

Bellek hiyerarşisi:
  Ağırlık buffer:    dizi_boyutu × 2 bit × derinlik
  Aktivasyon buffer: dizi_boyutu × 8 bit × derinlik
  Akümülatör:        dizi_boyutu × 32 bit
```

## 1.2 Ternary İşlem Birimi (Temel Yapı Taşı)

```verilog
// Tek PE — çarpan YOK, sadece MUX + toplayıcı
module ternary_pe (
    input  logic               clk,
    input  logic               rst_n,
    input  logic signed [1:0]  weight,     // -1, 0, +1
    input  logic signed [7:0]  act_in,
    input  logic signed [31:0] acc_in,
    output logic signed [31:0] acc_out,
    output logic signed [7:0]  act_out     // sistolik geçiş
);
    logic signed [31:0] product;

    always_comb begin
        case (weight)
            2'b01:   product =  {{24{act_in[7]}}, act_in};  // +1
            2'b11:   product = -{{24{act_in[7]}}, act_in};  // -1
            default: product =  32'd0;                       //  0
        endcase
    end

    always_ff @(posedge clk) begin
        if (!rst_n) begin
            acc_out <= 32'd0;
            act_out <= 8'd0;
        end else begin
            acc_out <= acc_in + product;
            act_out <= act_in;
        end
    end
endmodule
```

**Kritik nokta:** `case` bloğu FPGA'da LUT'a, ASIC'te küçük
mantık hücresine sentezlenir. Çarpan devresi hiç kullanılmıyor.

## 1.3 Sistolik Dizi

```verilog
module ternary_systolic #(
    parameter int SIZE = 32
) (
    input  logic clk, rst_n,
    input  logic signed [1:0]  weights [SIZE][SIZE],
    input  logic signed [7:0]  act_west [SIZE],
    output logic signed [31:0] acc_south [SIZE]
);
    logic signed [7:0]  act_h [SIZE][SIZE+1];
    logic signed [31:0] acc_v [SIZE+1][SIZE];

    genvar i, j;
    generate
        for (i = 0; i < SIZE; i++) begin : row
            assign act_h[i][0] = act_west[i];
            for (j = 0; j < SIZE; j++) begin : col
                ternary_pe pe (
                    .clk(clk), .rst_n(rst_n),
                    .weight(weights[i][j]),
                    .act_in(act_h[i][j]),
                    .acc_in(acc_v[i][j]),
                    .acc_out(acc_v[i+1][j]),
                    .act_out(act_h[i][j+1])
                );
            end
        end
        for (j = 0; j < SIZE; j++) begin : init
            assign acc_v[0][j] = 32'd0;
            assign acc_south[j] = acc_v[SIZE][j];
        end
    endgenerate
endmodule
```

## 1.4 Doğrulama — Altın Referansa Karşı

```python
# cocotb testbench
import cocotb
import numpy as np
from cocotb.triggers import RisingEdge

@cocotb.test()
async def test_against_golden(dut):
    golden = np.load("golden_vectors.npy", allow_pickle=True)

    for case in golden:
        w, a, expected = case["weights"], case["acts"], case["out"]

        # Ağırlıkları yükle
        for i in range(32):
            for j in range(32):
                dut.weights[i][j].value = encode_trit(w[i,j])

        # Aktivasyonları besle, sonucu topla
        result = await run_systolic(dut, a)

        # BIT-EXACT eşleşme zorunlu
        assert np.array_equal(result, expected), \
            f"Uyumsuzluk! Beklenen {expected}, alınan {result}"
```

```bash
make SIM=verilator  # hızlı
make SIM=icarus     # alternatif
```

**Aşama 1 çıktısı:**
- Sentezlenebilir RTL
- %100 bit-exact test geçişi
- Kaynak tahmini (Yosys ile ön sentez)

---

# AŞAMA 2 — FPGA Prototipi

## 2.1 Kart Seçimi

```
BAŞLANGIÇ (öğrenme, 16×16 dizi):
  Digilent Arty A7-100T          $250
  → Artix-7, 101K LUT, 240 DSP
  → Vivado ücretsiz sürüm yeterli

ORTA (gerçekçi, 32×32 dizi):
  Digilent Nexys Video           $500
  → Artix-7 200T, HDMI, DDR3
  Alinx AXU3EG                   $600
  → Zynq UltraScale+, ARM çekirdek dahil (ÖNERİLEN)

CİDDİ (64×64+, DDR4 gerekli):
  Xilinx Kria KV260              $250 (!)
  → Zynq UltraScale+, ticari kullanıma hazır
  Alveo U50                      $2,500
  → HBM2 8GB, PCIe, veri merkezi sınıfı
```

**Öneri:** Kria KV260 fiyat/performans olarak en iyisi.
ARM Cortex-A53 dahil → host yazılımı aynı kartta çalışıyor.

## 2.2 Sentez ve Kaynak Analizi

```bash
# Vivado batch modu
vivado -mode batch -source build.tcl
```

```tcl
# build.tcl
create_project ternary_accel ./build -part xck26-sfvc784-2LV-c
add_files [glob ./rtl/*.sv]
add_files -fileset constrs_1 ./constraints/kv260.xdc
set_property top ternary_top [current_fileset]

launch_runs synth_1 -jobs 8
wait_on_run synth_1

# Kaynak raporu
open_run synth_1
report_utilization -file utilization.rpt
report_timing_summary -file timing.rpt
```

**Beklenen sonuçlar (32×32 dizi, Kria KV260):**

```
LUT kullanımı:      ~35,000 / 117,000  (%30)
FF kullanımı:       ~28,000 / 234,000  (%12)
BRAM:               ~40 / 144          (%28)
DSP:                0 (!)              ← ternary'nin gücü
Fmax:               ~200-250 MHz
Güç (tahmini):      ~3-4 W
```

**DSP kullanımının sıfır olması kritik gösterge** — çarpan
devresi gerçekten kullanılmıyor demektir.

## 2.3 Kart Üzerinde Doğrulama

```python
# PYNQ ile (Kria/Zynq kartlarda Python'dan erişim)
from pynq import Overlay, allocate
import numpy as np

ol = Overlay("ternary_accel.bit")
accel = ol.ternary_systolic_0

# Tampon ayır
w_buf = allocate(shape=(32,32), dtype=np.int8)
a_buf = allocate(shape=(32,64), dtype=np.int8)
o_buf = allocate(shape=(32,64), dtype=np.int32)

# Altın vektörlerle test
golden = np.load("golden_vectors.npy", allow_pickle=True)
for case in golden[:100]:
    w_buf[:] = case["weights"]
    a_buf[:] = case["acts"]
    w_buf.flush(); a_buf.flush()

    accel.write(0x10, w_buf.physical_address)
    accel.write(0x18, a_buf.physical_address)
    accel.write(0x20, o_buf.physical_address)
    accel.write(0x00, 1)                      # başlat

    while accel.read(0x00) & 0x2 == 0: pass   # bekle
    o_buf.invalidate()

    assert np.array_equal(o_buf, case["out"])

print("✓ 100/100 test geçti — donanım doğru")
```

## 2.4 Ölçüm ve Karşılaştırma

```python
import time

def benchmark(fn, iterations=1000):
    start = time.perf_counter()
    for _ in range(iterations):
        fn()
    return (time.perf_counter() - start) / iterations

t_fpga = benchmark(run_on_fpga)
t_cpu  = benchmark(lambda: np.matmul(w, a))

print(f"FPGA:  {t_fpga*1e6:.1f} µs")
print(f"CPU:   {t_cpu*1e6:.1f} µs")
print(f"Hızlanma: {t_cpu/t_fpga:.1f}x")

# Güç ölçümü (Kria'da yerleşik sensör)
# cat /sys/class/hwmon/hwmon0/power1_input
```

**Aşama 2 çıktısı:**
- Kart üzerinde çalışan, doğrulanmış hızlandırıcı
- Kaynak/güç/performans ölçümleri
- CPU ile karşılaştırma verisi

---

# AŞAMA 3 — Sistem Entegrasyonu

Tek matris çarpımı ≠ çalışan LLM. Şimdi tam sistem.

## 3.1 Kontrol Katmanı

```
Gerekli bileşenler:

Ağırlık yükleyici:
  DDR'den → on-chip buffer → sistolik dizi
  Katman katman, çift tamponlama (gecikme gizleme)

Aktivasyon pipeline'ı:
  Giriş → dizi → akümülatör → aktivasyon fn → çıkış

Katman zamanlayıcı:
  Katman N bitince → katman N+1 ağırlıkları hazır olmalı

Yardımcı işlemler:
  LayerNorm/RMSNorm  (FP16 veya INT32)
  Softmax            (dikkat katmanında)
  Aktivasyon fn      (SiLU, GELU)
  → Bunlar ternary DEĞİL, ayrı donanım veya ARM'de
```

## 3.2 Model Dönüştürücü

```python
def convert_model_to_hw_format(model_path, output_path):
    """GGUF/SafeTensors → donanım formatı"""
    layers = []
    for name, weight in load_ternary_weights(model_path):
        # 32×32 bloklara böl (sistolik dizi boyutu)
        blocks = tile_matrix(weight, tile=32)

        # 2 bit paketleme: 4 trit = 1 byte
        packed = pack_trits(blocks)

        layers.append({
            "name": name,
            "shape": weight.shape,
            "scale": compute_scale(weight),   # FP16 ölçek
            "data": packed,
        })

    write_hw_binary(output_path, layers)

def pack_trits(arr):
    """4 trit'i 1 byte'a paketle"""
    # -1 → 0b11, 0 → 0b00, +1 → 0b01
    encoded = np.where(arr == -1, 3, np.where(arr == 1, 1, 0))
    return np.packbits(
        encoded.reshape(-1, 4).astype(np.uint8), axis=-1
    )
```

## 3.3 Uçtan Uca Test

```python
# Aynı prompt, üç yol
prompt = "Türkiye'nin başkenti"

out_reference = bitnet_cpp_inference(prompt)   # referans
out_hardware  = fpga_inference(prompt)          # donanım

# Token dizisi aynı olmalı (greedy decoding ile)
assert out_reference == out_hardware, \
    f"Uyumsuzluk: {out_reference} vs {out_hardware}"

# Performans
tokens_per_sec = measure_throughput(fpga_inference)
print(f"Hız: {tokens_per_sec:.1f} token/s")
```

**Gerçekçi beklentiler (32×32 dizi, Kria KV260):**

```
Model boyutu    Token/s    Not
────────────────────────────────────────
125M ternary    50-100     rahat çalışır
700M ternary    15-30      DDR bant genişliği sınırlı
2B ternary      5-10       bellek darboğazı
7B ternary      1-3        DDR4 yetersiz
```

**Aşama 3 çıktısı:**
- Uçtan uca çalışan LLM çıkarımı
- Token/saniye ölçümü
- Referansla bit-exact uyum

---

# AŞAMA 4 — ASIC (Opsiyonel, Uzun Yol)

## 4.1 Karar Kriterleri

```
ASIC'e geçmeden önce şunlar olmalı:

□ FPGA prototipi tam çalışıyor
□ Ölçülmüş performans avantajı var
□ Hacim gerekçesi var (>10K adet veya güç kritik)
□ 18-24 ay zaman ayrılabiliyor
□ Sermaye mevcut

Yoksa: FPGA'da kal. Kria KV260 ile ürün yapılabilir.
```

## 4.2 Yol Seçenekleri

```
A) Tiny Tapeout — $300, öğrenme amaçlı
   Sky130 130nm, 160×100 µm alan
   → Sadece ~8×8 dizi sığar
   → Değeri: "gerçek silikon" deneyimi
   → Süre: 6-9 ay (shuttle takvimi)

B) Efabless/ChipIgnite — $10-15K
   Sky130, ~10 mm², 4 hafta submission
   → 32×32 dizi sığar
   → Süre: 9-12 ay

C) MPW (Multi-Project Wafer) — $50-150K
   TSMC/UMC 65-28nm, üniversite programları
   → 64×64+ dizi, ciddi performans
   → Süre: 12-18 ay

D) Tam maske seti — $500K-15M
   TSMC 16nm: ~$500K
   TSMC 7nm:  ~$3M
   TSMC 3nm:  ~$15M
   → Üretim hacmi gerektirir
```

## 4.3 ASIC Akışı (Açık Kaynak)

```bash
# OpenLane 2 ile RTL → GDSII
pip install openlane
openlane --dockerized config.json
```

```json
{
  "DESIGN_NAME": "ternary_accel",
  "VERILOG_FILES": "dir::rtl/*.sv",
  "CLOCK_PORT": "clk",
  "CLOCK_PERIOD": 5.0,
  "FP_CORE_UTIL": 45,
  "PL_TARGET_DENSITY": 0.5,
  "SYNTH_STRATEGY": "AREA 0"
}
```

**ASIC'te ternary'nin gerçek kazancı:**

```
                    Ternary      INT8       Fark
─────────────────────────────────────────────────
PE alanı            ~150 µm²    ~800 µm²   5.3× küçük
PE enerjisi         ~0.1 pJ     ~0.5 pJ    5× az
Ağırlık depolama    2 bit       8 bit      4× az
Çarpan devresi      YOK         var        —

FPGA'da bu kazançlar görünmez (LUT tabanlı).
ASIC'te ortaya çıkar.
```

---

## Volt ile İlişki

Bu proje Volt'un **ilk gerçek vaka çalışması** olabilir:

```verilog
// Bugün: SystemVerilog
module ternary_pe (
    input logic signed [1:0] weight,  // -1,0,+1 → derleyici bilmiyor
    ...
);
```

```volt
// Volt ile: Trit tipi native
module TernaryPE {
    in  weight : Trit          // tip sistemi biliyor
    in  act_in : i8
    out acc    : i32

    // Trit × i8 → i8 kuralı tip sisteminde
    // Sıfır atlama derleyici tarafından biliniyor
    @zero_skip_rate(0.6)       // profil verisinden
}
```

**Karşılıklı fayda:**
- Hızlandırıcı → Volt için gerçek kanıt ("Volt ile tasarlandı")
- Volt → hızlandırıcı için tip güvenliği ve CDC garantisi

Ama **sıra önemli:** Önce SystemVerilog ile çalışan prototip
yapın. Volt olgunlaştığında (F5+) port edin.

---

## Risk Haritası

```
RİSK                        OLASILIK  ETKİ    AZALTMA
──────────────────────────────────────────────────────────────
DDR bant genişliği darboğazı  Yüksek   Yüksek  Küçük model ile başla
Zamanlama kapanmıyor          Orta     Orta    Pipeline derinliği artır
Sonuç referansla uyuşmuyor    Yüksek   Yüksek  Bit-exact test, adım adım
FPGA kaynağı yetmiyor         Orta     Orta    Dizi boyutunu küçült
Beklenen hızlanma yok         Orta     Yüksek  Aşama 0'da profil çıkar
Kapsam genişlemesi            Yüksek   Yüksek  Her aşamayı bitir, sonra geç
```

---

## İlk Hafta Kontrol Listesi

```
□ BitNet.cpp indir, 2B modeli CPU'da çalıştır
□ Ağırlık istatistiklerini çıkar (sıfır oranı?)
□ Referans matmul fonksiyonunu yaz
□ 100 altın test vektörü üret ve kaydet
□ Verilator + cocotb kur
□ 4×4 sistolik dizi yaz (küçük başla!)
□ İlk testi geçir

Bu yedi adım tamamlanmadan FPGA kartı sipariş etme.
```

---

## Özet

```
Aşama 0: Modeli anla    → 2-3 ay,  $0
Aşama 1: RTL yaz        → 3-4 ay,  $0
Aşama 2: FPGA'da çalış  → 4-6 ay,  $500-2K
Aşama 3: Sistem kur     → 3-4 ay,  $0
Aşama 4: ASIC (ops.)    → 18+ ay,  $10K-15M

En kritik kural:
  Her aşamanın çıktısı bir sonrakinin girdisi.
  Aşama atlanırsa geri dönülür — kayıp katlanır.

En yaygın hata:
  Aşama 0'ı atlayıp doğrudan RTL yazmak.
  Sonuç: donanım çalışıyor ama doğru mu bilinmiyor.

Gerçekçi ilk hedef:
  32×32 ternary sistolik dizi, Kria KV260 üzerinde,
  125M parametreli model, 50+ token/s.
  Süre: 12-15 ay. Maliyet: ~$500.
```
