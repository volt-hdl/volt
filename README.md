# Volt HDL

![durum](https://img.shields.io/badge/durum-F0_tamamland%C4%B1_—_erken_alpha-orange)
![testler](https://img.shields.io/badge/testler-215_ge%C3%A7iyor-brightgreen)
![fuzz](https://img.shields.io/badge/fuzz-525k_ko%C5%9Fu%2C_0_panik-blue)

> ⚠️ **Erken alpha.** F0 (kavram kanıtı) tamamlandı; dil ve araçlar aktif
> geliştirme aşamasındadır. Sözdizimi haber verilmeden değişebilir.
> Üretimde kullanmayın.

Volt, modern bir sözdizimiyle SystemVerilog üreten ve **saat alanı
geçişlerini (CDC) derleme zamanında yakalayan** bir donanım tanımlama
dilidir. Her sinyal bir saat alanına aittir; alanlar arası güvensiz
geçişler tip hatası olarak raporlanır.

## Çalışan Örnek

`tests/fixtures/counter.volt`:

```volt
/// 8-bit yukarı sayaç
/// enable yüksekken her saat kenarında artar
module Counter {
    in  clk    : clock
    in  enable : bool
    out count  : u8

    reg count_r : u8 = 0

    on clk {
        if enable {
            count_r <= count_r + 1
        }
    }

    count = count_r
}
```

Derleme:

```console
$ volt build tests/fixtures/counter.volt
   Derleniyor tests/fixtures/counter.volt
    Tamamlandı 0.01s
     Çıktı build/rtl/counter.sv (34 satır)
```

Üretilen SystemVerilog (`build/rtl/counter.sv`, ilk 15 satır):

```systemverilog
// Bu dosya Volt tarafından otomatik üretilmiştir.
// Kaynak: counter.volt
// Volt sürümü: 0.1.0
//
// DÜZENLEMEYİN — değişiklikler kaynak dosyada yapılmalıdır.

`default_nettype none

// 8-bit yukarı sayaç
// enable yüksekken her saat kenarında artar
module Counter (
    input  logic       clk,
    input  logic       rst,
    input  logic       enable,
    output logic [7:0] count
);
```

Reset portu ve reset bloğu otomatik üretilir; çıplak `always`, `reg`,
`initial` veya `#` gecikme asla üretilmez (sv-mapping.md §11).

## Kurulum

```console
$ git clone https://github.com/volthdl/volthdl
$ cd volthdl
$ cargo build --release
$ ./target/release/volt build tests/fixtures/counter.volt
```

## Durum — F0 tamamlandı

- [x] Lexer (logos; 53 anahtar kelime, iç içe blok yorum)
- [x] Parser (LL(2) iniş + Pratt ifadeler, hata kurtarma, fuzz'lı)
- [x] Tanı altyapısı (86 hata kodu, insan/short/JSON çıktı)
- [x] SystemVerilog üretimi (counter.volt → birebir referans çıktı)
- [x] CLI: `volt build` / `volt check` (sözleşmeli çıkış kodları)
- [ ] F1: tam gramer, `sync()` CDC köprüsü (ADR-0023)
- [ ] F2: HIR — gerçek tip çıkarımı ve CDC doğrulaması
- [ ] F3: CIRCT lowering

## Belgeler

Belge indeksi: [docs/README.md](docs/README.md) —
bağlayıcı spesifikasyon [docs/spec/](docs/spec/), mimari kararlar
[docs/adr/](docs/adr/) altında.
