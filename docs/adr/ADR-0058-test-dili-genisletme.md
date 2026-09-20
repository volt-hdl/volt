# ADR-0058: Test Dili Genişletme — Yerel Değişken, Dizi, `for`, `read_hex`, `load`

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-20
> Etkilenen: volt-ast (`TestStmt::LetVar/For`, `TestExprKind` yeni
> biçimler, `TestBinOp`/`TestUnOp`), volt-syntax (`parser/test.rs`,
> `parser/test_expr.rs` YENİ), volt-hir (`sim.rs` kapsamlı yürüyücü,
> `sim_expr.rs` / `sim_load.rs` / `testdata.rs` YENİ), volt-sv-emit
> (`sim.rs`, `sim_script.rs` YENİ), volt-driver (`sim.rs`,
> `sim_lower.rs` YENİ), volt-diagnostics (E8507–E8511),
> docs/spec/grammar-full.ebnf (TestStmt/TestExpr), examples/riscv_sw/,
> tests/ui/ (pass 78–80, fail 59), `.test-baseline`, CHANGELOG.md.
> DOKUNULMADI: README.md.
> İlgili: ADR-0033 (test blokları — bu ADR onu genişletir, geçersiz
> kılmaz), ADR-0026 (üretilen kod İngilizce), ADR-0042 (Volt.toml proje
> kökü), ADR-0035 (dizi yazmaçları), ADR-0015 (determinizm).

## Sorun

ADR-0033 test gövdesini bilerek küçük tuttu: `let dut`, port atama,
`step`, `reset`, dört `assert_*`. `examples/riscv_sw/` turunda sınır
somutlaştı:

> Test dili yalnızca port atama, step ve assert destekliyor. Dizi, döngü
> ve dosya erişimi yok, dolayısıyla test bloğu belleği dolduramaz.

Sonuç: programı Volt'a taşımak için `hex2volt.py` ile ayrı bir
`hello_rom.volt` üretmek gerekti (4096 kelime sınırlı `const` tablo +
ROM modülü), ve UART çıktısını denetleyen test 20 kez kopyalanmış üç
satırdan oluştu. Aynı sınır veri odaklı her testte çıkar: tablo testleri
(S-box, katsayı dizisi), bellek yükleme (ROM/RAM başlangıç değeri), aynı
senaryoyu N girdiyle koşma.

## Karar

### 1. Değerler: sayı ve sayı dizisi

Test değerleri iki türdür: **64 bit işaretsiz sayı** ve **sayı dizisi**.
Donanım tip sistemi teste taşınmaz — betik, Verilator modelinin C++
arayüzüyle konuşur ve orada her port zaten bir tamsayıdır.

```volt
let n = len(expected) - 1;               // sayı
let expected = [0x63, 0x7c, 0x77, 0x7b]; // dizi — [u8; 4]
let rom = read_hex("hello.hex");         // dizi — [u32; N]
```

- Dizi yalnız `let` ile bağlanır; literal boş olamaz ve elemanları sabit
  sayıdır. İçerik **derleme zamanında bilinir**: uzunluk ve en büyük
  değer kapsamda taşınır, `load` boyut denetimi buna dayanır.
- Eleman tipi bilgilendiricidir (en büyük değere/hex basamak sayısına
  göre u8/u16/u32/u64); betik aritmetiği hep 64 bittir. Bağlayıcı olan
  tek yer `load`'dur: değer hedef elemana sığmalıdır (E8510).
- Erişim `ad[i]`, uzunluk `len(ad)`. İç içe dizi yoktur.
- İfadeler: `* / %`, `+ -`, `<< >>`, `&`, `^`, `|`, karşılaştırmalar,
  `&&`, `||`, `!`, parantez. Öncelik ve birleşim Volt donanım
  ifadeleriyle aynıdır (operator-precedence.md: bit düzeyi
  karşılaştırmadan, `< >` eşitlikten sıkı; karşılaştırma ve eşitlik
  zincirlenemez). Karşılaştırma ve mantıksal işlemler 0/1 üretir.
  Negatif sayı yoktur; çıkarma 64 bitte sarar.
- Adlar gölgelenemez (E8506); `for` gövdesindeki `let` gövdeye yereldir.

### 2. `for`: ÇALIŞMA ZAMANI döngüsü

```volt
for i in 0..16 {
    dut.a = i;
    dut.b = i;
    step(1);
    assert_eq(dut.sum, i * 2);
}
```

Döngü üretilen C++'ta **gerçek bir `for`'dur**, derleme zamanında
açılmaz. Gerekçe:

| | Açılım | Çalışma zamanı |
|---|---|---|
| `for i in 0..65536` | 65536 × gövde C++ satırı; g++ dakikalar sürer | gövde bir kez üretilir |
| Sınır bir port/değişken (`0..dut.count`, `0..len(rom)`) | imkânsız — değer derlemede yok | doğal |
| Hata raporu | açılmış satırlar aynı kaynak satırını gösterir, hangi yineleme belirsiz | sayaç değeri raporlanır |
| Determinizm (ADR-0015) | — | aynı: sınırlar döngüden önce bir kez hesaplanır |

Modül seviyesi `for` (ADR-0056) açılır, çünkü donanım üretir ve her
yineleme ayrı bir örnek/tel olmak zorundadır. Test `for`'u donanım
üretmez, betik yürütür; aynı sözdiziminin farklı yürütme modeli bu
yüzden tutarlıdır: **donanım yapısı açılır, davranış yürütülür.**

- Aralık yarı açıktır (`a..b`, `b` hariç); `a >= b` ise gövde hiç koşmaz.
- Sınırlar döngü başında BİR KEZ hesaplanır; gövde sınırı okuyan portu
  değiştirse de yineleme sayısı sabittir.
- `for` içinde DUT örneklenemez (E8506): bir test tek DUT sürer.
- Döngü içindeki hata raporu sayaçları taşır:

```
---- all_inputs ----
  assert_eq failed at adder_test.volt:24
    left:  8
    right: 7
    loop:  i = 2, j = 1
```

### 3. `read_hex`: derleme zamanında dosya okuma

`read_hex("yol")` Verilog `$readmemh` metnini okur: boşlukla ayrılmış
onaltılık kelimeler, `_`, `//` ve `/* */` yorumları, `@adres` (ELEMAN
cinsinden; boşluklar 0 ile dolar). Reddedilenler (E8508): onaltılık
olmayan basamak, `x`/`z`, 64 bitten geniş kelime, 1M elemanı aşan
adres, kapanmamış yorum, veri içermeyen dosya.

Dosya **derleme zamanında** okunur ve içerik testbench'e statik dizi
olarak gömülür. Çalışma zamanında okumak reddedildi: E8507/E8508 kaynak
konumlu derleyici tanıları olmalı, `load` boyutu derlemede
denetlenebilmeli ve simülasyon yürütülebiliri çalışma dizininden
bağımsız kalmalıdır.

**Yol kuralı.** Yol test dosyasının dizinine göre çözülür. Bir test
yalnız kendi projesindeki dosyaları okuyabilir: kök, en yakın
`Volt.toml`'un dizinidir; yoksa test dosyasının kendi dizini. Mutlak
yollar ve kökü `..` ile aşan yollar E8507'dir; sembolik bağ sözcüksel
kuralı dolanamasın diye gerçek (canonical) yol da kökün altında olmalıdır.
İşletim sistemine göre anlam değiştiren parçalar da reddedilir: `:`
(Windows'ta `sub/C:x.hex` sürücü-göreli yoldur ve `join` taban dizini
atar — sonuç çalışma dizinine bağlanırdı; `x.hex::$DATA` alternatif veri
akışıdır), kontrol karakterleri, sonda nokta ya da boşluk.
Gerekçe: dışarı uzanan test, sonucu koştuğu makineye bağımlı kılar ve
indirilen bir projenin testine keyfi dosya okuma yetkisi verirdi.

volt-hir dosya sistemi okumaz: erişim sürücünün verdiği
`TestFileLoader` üzerinden yapılır. Yükleyici yokken (tek dosyalık
`analyze`, LSP) yalnız sözcüksel kural işler ve kök test dosyasının
dizini sayılır — `Volt.toml` kökü altındaki `../veri/x.hex` yolu LSP'de
sahte E8507 verebilir; sürücü doğru kökle denetler. (Bilinen sınır.)

`read_csv` bu ADR'nin kapsamı DIŞINDADIR: iç içe dizi ister; ihtiyaç
somutlaşınca ayrı karar.

### 4. `load`: tasarımın belleğine yazma

```volt
test "run program" {
    let dut = Soc { };
    let rom = read_hex("hello.hex");
    load(dut.imem, rom);
    reset();
    step(5000);
    assert_true(dut.halted);
}
```

Hedef, DUT modülünde (`dut.imem`) ya da bir alt örnekte
(`dut.cpu.imem`) **dizi tipli bir `reg`** olmalıdır (E8509). Volt dizi
yazmaçlarını paketlenmemiş SV dizisi olarak üretir
(`logic [31:0] imem [0:1023]`); Verilator'da bu `VlUnpacked<T, N>`'dir.

**Uygulama — doğrudan bellek yazma, `$readmemh` değil:**

1. Sürücü `load` hedeflerini toplar ve bir Verilator yapılandırma
   dosyası üretir (`load_<Modül>.vlt`):
   `public_flat_rw -module "Soc" -var "imem"`. Yalnız adı geçen
   bellekler açılır; `--public-flat-rw` ile bütün tasarımı açmak
   eniyilemeyi kapatır ve 59 testlik RISC-V koşusunu yavaşlatırdı.
   **Üretilen SV değişmez** — `$readmemh` SV'ye `initial` bloğu ve dosya
   yolu gömerdi; bu hem sentez çıktısını testin varlığına bağlar hem de
   ADR-0015'i (byte-aynı çıktı) zorlar.
2. Testbench `dut.rootp->Soc__DOT__cpu__DOT__imem` üzerinden yazar. Bir
   şablon yardımcı eleman tipini (`CData`/`SData`/`IData`/`QData`) ve
   boyutu Verilator modelinden alır, böylece üreteç genişlik bilmek
   zorunda değildir.
3. Yazmadan sonra `dut.eval()` çağrılır: Verilator 5 dışarıdan yazılan
   belleğe bağlı kombinasyonel mantığı kendiliğinden tazelemez
   (deneyle doğrulandı — `eval()` olmadan `assign q = mem[a]` eski
   değeri gösterir).

**Boyut ve genişlik (E8510).** Kaynak hedeften UZUN olamaz ve hiçbir
değer eleman genişliğini aşamaz: `load` asla kırpmaz. Kaynak KISA ise
kalan elemanlara dokunulmaz (`$readmemh` davranışı). Hedef boyutu düz
literal ya da düz literal `const` ise denetim derleme zamanındadır
(test denetimi isim çözümlemeden bağımsız çalışır — ADR-0033);
değilse aynı denetim testbench'te yapılır ve testi düşürür
(`load_too_long`, `load_value_too_wide`). Eleman genişliği biliniyorsa
testbench'e taşınır: C++ tipi 12 bitlik elemanı 16 bitte tutar, yalnız
tipe bakan denetim taşmayı göremezdi. Genişlik düz `const` üzerinden de
çözülür (`uint<W>`). 64 bitten geniş ya da sayı olmayan (iç içe dizi,
demet) elemanlar desteklenmez (E8509). E8509 konumu her zaman test
dosyasındaki `load` hedefidir — yazmaç kardeş dosyada olabilir ve onun
span'ı test dosyasının kaynak haritasında anlamsızdır.

**`reset()` ile etkileşim.** Volt dizi yazmaçları reset'te başlangıç
değerine döner; saf donanım anlamıyla `load` + `reset()` programı
silerdi — oysa en yaygın kullanım tam da budur (yükle, sıfırla, koştur).
Karar: **yüklenen bellekler başlatılmış depolamayı (ROM, FPGA
`initial` içeriği) modeller ve `reset()`'ten sağ çıkar.** Testbench her
`reset()` sonrasında o teste ait yüklemeleri sırayla yeniden uygular.
Testin başındaki örtük reset yüklemeden ÖNCE olduğundan, çekirdek o
çevrimlerde boş bellekle koşmuştur; programı temiz başlatmak için
`load` sonrası `reset()` çağrılır.

Hiç yazılmayan yazmaç W3001 üretir; saf ROM isteyen tasarım belleğe
gerçek bir yazma yolu (programlama portu) vermelidir — `HelloSoc`
`prog_we/prog_addr/prog_data` ile bunu yapar. W3001'i `load` hedefi
için susturmak reddedildi: modül derlenirken hangi testin onu
yükleyeceği bilinmez.

### 5. Çalışma zamanı hataları testi düşürür, çökertmez

Dizi indeksi, `/`, `%` ve kaydırmalar C++'ta tanımsız davranışa düşebilir.
Hepsi yardımcı işlevlerden geçer; hata bir bayrağa yazılır ve deyimden
sonra `VOLT-ASSERT-FAIL <tür> <konum> left= right=` satırına çevrilir
(`index_out_of_bounds`, `division_by_zero`). 64 ve üstü kaydırma 0 verir.
Sabit indeks derleme zamanında da denetlenir (E8511).

ADR-0058 özelliği kullanmayan betikler için üretilen testbench ADR-0033
çıktısıyla **bayt bayt aynıdır** (yardımcılar ve başlıklar eklenmez).
Tek istisna: test adı ve dosya adı artık printf biçim dizgesine kaçışlı
gömülür (`%` → `%%`); `%` içeren ad eskiden tanımsız davranıştı.

Test ifadeleri ve iç içe `for` donanım ifadeleriyle aynı derinlik
sınırına (200) tabidir. Sol-derin operatör zinciri de sayaca yazılır:
ağacı sonradan özyineli yürüyen denetim, indirgeme ve C++ üretimi
`1+1+…` gibi düz ama uzun bir ifadede yığını taşırırdı.

### 6. Hata kodları

| Kod | Anlam |
|---|---|
| E8507 | Test veri dosyası bulunamadı ya da proje dışında |
| E8508 | Bozuk hex veri dosyası (satır numarasıyla) |
| E8509 | `load()` hedefi bir bellek dizisi değil |
| E8510 | `load()` kaynağı hedefe sığmıyor (uzunluk ya da eleman genişliği) |
| E8511 | Test ifadesinde tip uyuşmazlığı (sayı ↔ dizi, boş/sabit olmayan dizi literali, sabit indeks sınır dışı) |

E8511 görev tanımında yoktu; sayı/dizi ayrımı gelince "diziyi porta
atama" gibi hataların bir kodu olması gerekti ve E8505 (yerleşik çağrı)
ya da E8506 (ad) bunu dürüstçe karşılamıyordu. Tanımsız ad ve gölgeleme
E8506'da, bilinmeyen değer yerleşiği ve argüman sayısı E8505'te kaldı.
Hepsinin `volt explain` açıklaması iki dildedir.

### 7. Dilbilgisi

```ebnf
TestStmt  = "let" Ident "=" Ident "{" "}" ";"
          | "let" Ident "=" TestExpr ";"
          | "for" Ident "in" TestExpr ".." TestExpr "{" { TestStmt } "}"
          | Ident "." Ident "=" TestExpr ";"
          | Ident "(" [ TestExpr { "," TestExpr } ] ")" ";" ;
```

`let x = Ad {` DUT örneklemedir, diğer her `let` yerel değişkendir —
tek token ileri bakış yeter. `for` deyimi `;` ile bitmez. Bozuk `for`
başlığında ayrıştırıcı gövdeyi de tüketir; aksi hâlde gövdenin `}`'si
test bloğunu erken kapatır ve hata kaskadı üretirdi.

## Sonuçlar

- `examples/riscv_sw/`: `hex2volt.py` ve `hello_rom.volt` SİLİNDİ.
  `HelloSoc` programı `reg imem : [u32; 1024]` içinde tutar; test
  `read_hex("riscv_sw/hello.hex")` + `load(dut.imem, program)` +
  `reset()` ile yükler, beklenen metni dizide tutar ve 20 kopya yerine
  tek `for` ile karşılaştırır. `hello.hex` artık bayt değil 32 bit kelime
  dökümüdür (`objcopy --verilog-data-width=4`): bir `$readmemh` elemanı
  = bir `u32`. Sonuç aynı: 59/59 test, 913 çevrim, 847 komut.
- **4096 kelime sınırı** `const` tabloların sınırıydı (ADR-0041 case
  fonksiyonu). `load` hedefi bir yazmaç dizisidir; sınır
  `MAX_ARRAY_LEN` = 1M elemandır. 65536 kelimelik bellek + 60001
  elemanlı hex ile uçtan uca doğrulandı. `const` tablo sınırı değişmedi.
- objcopy `--verilog-data-width=4` ile bile `@` satırlarına BAYT adresi
  yazar; `$readmemh`/`read_hex` eleman sayar. 0'dan başlayan tek
  bölümlü görüntüde zararsızdır; çok bölümlü görüntüde adresler 4 kat
  kayar (riscv_sw/README.md'de not edildi).
- Bilinen sınırlar (kod incelemesinden, bu ADR'de çözülmedi; ilki
  **ADR-0059 ile KAPANDI** — sabit E8512, hesaplanmış değer testi düşürür):
  hesaplanmış değer porta maskesiz yazılır (`addr : u3` iken
  `dut.addr = i`, i ≥ 8 — ADR-0033'te literal için de böyleydi; döngüyle
  artık olağan, port genişliğini `SimPort`'a taşıyan ayrı bir karar
  ister); `step(<ifade>)` 0 verirse sessizce hiç çevrim koşmaz (yalnız
  literal `step(0)` E8505); `Volt.toml` araması yukarı doğru tavansızdır
  (ADR-0042 davranışı) — kendi manifest'i olmayan bir proje, üst
  dizindeki başıboş bir `Volt.toml`'u kök sayar ve okuma alanı genişler;
  `read_hex` dosyası `volt test`te denetim ve indirgemede ayrı ayrı
  okunur (arada değişirse "içsel hata").
- Diğer sınırlar: `read_csv` ve iç içe dizi yok; `generate`-`for`
  içindeki örneklerin belleklerine `load` yolu yok (`dut.u[3].mem`);
  alt örnek çıkış portu okunamaz (`dut.cpu.pc`); yükleyicisiz analizde
  kök varsayımı (yukarıda).
