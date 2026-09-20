# ADR-0059: Test Bloğunda Port Genişlik Denetimi (E8512)

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-20
> Etkilenen: volt-hir (`sim_port.rs` YENİ, `sim.rs`, `sim_load.rs`),
> volt-sv-emit (`sim.rs` `TbStep::SetPortChecked` / `TbPortCheck`,
> `sim_script.rs` `PORT_PRELUDE`), volt-driver (`sim_lower.rs`, `sim.rs`
> rapor), volt-diagnostics (E8512), tests/ui/fail/60, `.test-baseline`,
> CHANGELOG.md.
> DOKUNULMADI: README.md, examples/, docs/spec/.
> İlgili: ADR-0033 (test blokları), ADR-0058 (test dili genişletme — bu
> ADR onun "bilinen sınır"ını kapatır), ADR-0056 (paketlenmiş dizi
> portları), ADR-0026 (üretilen kod İngilizce).

## Sorun

ADR-0058 "Bilinen sınırlar":

> hesaplanmış değer porta maskesiz yazılır (`addr : u3` iken
> `dut.addr = i`, i ≥ 8 — ADR-0033'te literal için de böyleydi; döngüyle
> artık olağan).

Testbench `dut.addr = <ifade>;` satırını olduğu gibi üretiyordu;
`SimPort` yalnız ad/yön/saat taşıyordu, genişlik teste hiç ulaşmıyordu.

### Ölçüm: ne oluyordu?

Verilator 5.x, Docker; `in addr : u3`, `in wide : uint<12>`,
`reg tbl : [u8; 8]` (içerik 10..17), `dut.addr = 8`:

| Gözlem | Ölçülen | Donanımın vereceği (8 & 7 = 0) |
|---|---|---|
| `echo = addr` (`out echo : u3`) | **8** — u3 portta imkânsız değer | 0 |
| `data = tbl[addr]` | **0** (Verilator sınır koruması) | 10 (`tbl[0]`) |
| `big = addr > 5` | **1** | 0 |
| `wide = 0xFFFF`, `wsum = wide + 1` (13 bit) | **0** | 0x1000 |

Yani ne kırpma ne taşma: Verilator giriş portlarını **maskelemez**; u3
port 8 bitlik `CData`'da tutulur ve üst bitler kirli kalır. Simülasyon
donanımın asla bulunamayacağı bir durumu yürütür, sonuç üretilen C++'ın
ayrıntısına bağlıdır ve hiçbir uyarı çıkmaz. Test "geçer" ya da yanlış
nedenle düşer — "hata imkânsız olsun" ilkesinin tam tersi.

## Seçenekler

- **A — Yalnız derleme zamanı.** `dut.addr = 8` yakalanır, ama sorunun
  asıl kaynağı olan `for i in 0..16 { dut.addr = i }` yakalanmaz. RED.
- **B — Yalnız çalışma zamanı.** Her şeyi yakalar; ama sabit hata için
  Verilator derlemesini beklemek ve konumu yalnız satır olarak görmek
  gereksiz. RED (tek başına).
- **Maskeleme** (`dut.addr = i & 7`). Donanımla aynı sonucu verir ama
  hatayı GİZLER: 16 elemanlı tabloyu 8 adresle tarayan test sessizce
  ilk yarıyı iki kez okur. RED.
- **C — İkisi birden.** Sabit → E8512; hesaplanmış → testbench'te
  denetim, test düşer. KABUL.

## Karar

### 1. Genişlik bilgisi: AST port tipinden, `load` ile aynı yoldan

Test denetimi isim çözümlemeden bağımsız çalışır (ADR-0033; kardeş dosya
kuralı), dolayısıyla genişlik HIR tip tablosundan değil **AST port
tipinden** çözülür — ADR-0058'in `load` eleman genişliği için kurduğu
yol (`sim_load::elem_width`) `sim_port::scalar_width` olarak ortaklaştı:

| Port tipi | Genişlik |
|---|---|
| `bool` | 1 |
| `u8` / `i16` / `u3` … | N, işaret tipten |
| `uint<N>` / `sint<N>` / `bits<N>` | N düz literal ya da düz literal üst düzey `const` ise N |
| `[u8; 4]` (paketlenmiş vektör, ADR-0056) | eleman × uzunluk = 32 |
| takma ad, enum, generic parametre | **bilinmiyor** |

`volt_hir::test_port_width(sources, modül, port)` sürücüye,
`PortWidth { bits, kind }` denetleyiciye hizmet eder. Maliyet: tip
çözümleme zaten vardı; yeni olan ~60 satır ve `SetPort` başına bir port
araması.

"Bilinmiyor" sessiz geçiş DEĞİLDİR: testbench denetimi Verilator
modelindeki C++ depolama tipine göre yapar
(`volt_port_fits_type(dut.port, v)`). Bu, 12 bitlik portun 16 bitlik
depolamadaki 4 bitini göremez (bilinen sınır), ama tipi aşan her değeri
yakalar.

### 2. Kabul kuralı

Test değerleri 64 bit işaretsizdir; negatif sayı `0 - n` ile yazılır ve
64 bitte sarar (ADR-0058). `out` portları hep **bit deseni** olarak
okunur (`i8` −1 → 0xFF; `riscv_core_test` `0xFFFFFFFE` ile karşılaştırır).

- **İşaretsiz W bit:** `v < 2^W`.
- **İşaretli W bit:** `v < 2^W` (bit deseni — okumayla simetrik:
  `dut.a = dut.q` ve `0xFF` alışkanlığı çalışmaya devam eder) **ya da**
  `v`, 64 bitte sarmış ve aralıktaki bir negatif sayıdır
  (`[63 : W-1]` bitlerinin hepsi 1, yani −2^(W−1) ≤ v < 0). İkinci
  biçim porta W bitlik desenine indirgenerek yazılır — bu kırpma
  değildir, değer kayıpsız temsil edilir. `i8`: −128..255 geçer;
  256 ve −129 düşer.
- **W ≥ 64:** her değer sığar.

Yalnız "−128..127" kuralı reddedildi: okunan desenle yazılan desen
ayrışır, `dut.b = dut.a_out` gibi geri beslemeler ve mevcut `0xFF…`
alışkanlığı yanlış alarm verirdi.

Aynı kural iki yerde yaşar ve testlerle eşlenir: Rust
`PortWidth::accepts` (derleme zamanı) ve C++ `volt_port_fits`
(çalışma zamanı).

### 3. Derleme zamanı: E8512

`volt_hir::const_test_value` yalnız literallerden oluşan ifadeyi üretilen
C++ ile AYNI anlamla katlar (64 bit sarma, ≥64 kaydırma 0; sıfıra bölme
sabit sayılmaz — çalışma zamanında `division_by_zero` olarak düşer).
`dut.addr = 8`, `dut.addr = 4 + 4`, `dut.addr = 1 << 3` hepsi E8512'dir.
Değişken okuyan ifade (`let n = 8; dut.addr = n`) sabit sayılmaz ve
çalışma zamanına kalır: veri akışı analizi bu ADR'nin kapsamı dışıdır.

```
error[E8512]: value does not fit in port width
   ┌─ lookup_test.volt:25:16
   │
25 │     dut.addr = 8;
   │         ----   ^ value 8
   │         │
   │         port 'addr' is u3 (max 7)
   │
   = help: use a value in range 0..7
   = for more: volt explain E8512
```

### 4. Çalışma zamanı: test düşer, değer porta ULAŞMAZ

İndirgeme (`sim_lower::set_port`) üç yoldan birini seçer:

1. Sabit + bilinen genişlik → `TbStep::SetPort` (desen olarak; `0 - 1`
   → `0xFF`). Denetim derlemede yapıldı, koruma üretilmez.
2. Sığdığı kanıtlı → `SetPort`: 0/1 her porta sığar; bir `out` portunun
   okunan deseni en az o kadar geniş porta sığar.
3. Diğer her şey → `TbStep::SetPortChecked { check: TbPortCheck }`:

```cpp
{ const unsigned long long volt_pv = v_i;
if (!volt_port_fits(volt_pv, 3U, false)) {
    std::printf("VOLT-ASSERT-FAIL port_overflow t_test.volt:14 left=%llu right=%llu loop=i=%llu port=addr:u3\n", volt_pv, 3ULL, v_i);
    dut.final();
    return false;
}
dut.addr = volt_port_bits(volt_pv, 3U); }
```

Değer bir kez hesaplanır; ifadenin kendi hatası (indeks, bölme) port
denetiminden ÖNCE raporlanır. Protokol satırına `port=<ad>:<tip>`
belirteci eklendi; ayrıştırıcı `loop=` ve `port=`'u sıradan bağımsız
okur. Rapor:

```
---- table_lookup ----
  port 'addr' (u3) cannot hold value 8 at table_test.volt:14
    range: 0..7
    loop:  i = 8
```

Sarmış negatif okunur basılır: `cannot hold value
18446744073709551487 (-129)`.

(1) ve (2) sayesinde ADR-0058'in güvencesi sürer: yalnız literal/port
kullanan betiklerin testbench'i bayt bayt aynıdır; `PORT_PRELUDE` yalnız
denetimli yazma varsa eklenir.

### 5. Kapsam taraması: aynı sorun başka nerede?

| Yer | Durum |
|---|---|
| `assert_eq(dut.out, 300)`, `out : u8` | Sessiz DEĞİL (hep düşer) ama `assert_ne(dut.out, 300)` **sessizce hep geçer**. Sabit, portun okunabilir aralığını (0..2^W−1) aşıyorsa E8512 — iki argüman sırası da. İşaretli `out` için `assert_eq(dut.s, 0 - 1)` de E8512: port 0xFF okur. Hesaplanmış beklenen değer denetlenmez: `assert_eq` zaten yüksek sesle düşer. |
| `load()` elemanları | E8510 (derleme) + `load_value_too_wide` (çalışma) yeterli: `load` asla kırpmaz. Eleman genişliği çözülemezse C++ tipine düşer — port için yukarıdaki sınırın aynısı. Dizi literalleri negatif tutamaz, işaret sorunu doğmaz. |
| `read_hex` | Kelimeler 64 bitle sınırlı (E8508); hedefe sığma `load`'da denetlenir. |
| `step(<ifade>)`, `for` sınırları | Porta yazmaz; genişlik sorunu yok (`step` = 0 sınırı ADR-0058'de kayıtlı, ayrı konu). |
| `volt run` duman stimulusu | Her girişe 1 yazar; her porta sığar. |

### 6. Mevcut testler

Docker'da 13 dosya / 143 test (59 RISC-V testi dahil) yeni denetimle
koştu: **hepsi geçti; gerçek hata da yanlış alarm da çıkmadı.**

## Sonuçlar

- `dut.addr = 8` artık derlenmez; `for i in 0..16 { dut.addr = i }`
  8. yinelemede port, tip, değer, aralık ve sayaçla düşer.
- İşaretli porta negatif yazmak ilk kez DOĞRU çalışır: eskiden
  `uint`/`sint<12>` porta `0 - 1` 16 bitlik depolamaya 0xFFFF bırakırdı.
- Yeni kod: **E8512** (`volt explain` iki dilde).
- Bilinen sınırlar: genişliği çözülemeyen port (takma ad, enum, generic
  parametre) C++ depolama tipine göre denetlenir — 12 bitlik takma adlı
  portta 4 bitlik pencere açık kalır ve işaret de bilinmediğinden `0 - n`
  biçimindeki negatif sayı reddedilir (test düşer, sessiz kalmaz). Bu yol
  bugün pratikte erişilmez: kullanıcı tanımlı tipli port SV üretiminde
  E0003 verir, generic modül doğrudan DUT olamaz (E8501). Kullanıcı tipli
  portlar SV'ye inince `scalar_width` takma adı izlemelidir; değişken üzerinden gelen sabit
  (`let n = 8; dut.addr = n`) derlemede değil koşuda yakalanır; 64 bitten
  geniş portlar testten sürülemez (ADR-0033'ten beri — `VlWide` ataması
  C++'ta derlenmez).
