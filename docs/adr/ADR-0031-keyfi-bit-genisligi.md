# ADR-0031: Keyfi Bit Genişlikli Tamsayı Tipleri (u1..u64 / i1..i64)

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-09
> Etkilenen: grammar-full.ebnf §6/§17, volt-syntax (lexer + parser),
> volt-sv-emit (const katlama)
> Uygulama aşaması: F5 (examples/uart_tx.volt bulgusu)

## Sorun

`examples/uart_tx.volt` yazımı iki uçtan aynı engele çarptı:

1. Parser yalnız `u8/u16/u32/u64` ve `i8/i16/i32/i64` anahtar
   kelimelerini donanım tipi olarak tanıyordu. `u10` gibi bir isim
   `Path` olarak taşınıyor, typeck aileyi tanısa da (`widened_int`)
   SV eşlemesi `TypeRefKind::Path` için "kullanıcı tanımlı tip" hatası
   üretiyordu. Sonuç: 434 gibi bir bölücü sabitini tutacak `u10`
   sayacı yazılamıyor, tasarımcı `u16`'ya şişirmek zorunda kalıyordu.
2. Üst düzey `const` öğeleri parse + resolve + const-eval'den geçiyor
   ama SV üreticisi referansları İSİM olarak basıyor, eşleşen bir
   `localparam` üretmiyordu. Üretilen RTL tanımsız isim içerdiğinden
   Verilator lint'i geçmiyordu; örnek dosya `let` sabit-tel geçici
   çözümüne mahkûmdu.

Tip sistemi (`Ty::UInt { width: u16 }`) ve SV üretimi
(`logic [N-1:0]`) zaten genişlik-genel; kısıt yalnız sözdizimi
katmanında ve const emisyonundaydı.

## Seçenekler

**Seçenek A — Sekiz anahtar kelimeyi korumak, `bits<N>`'e yönlendirmek**
`bits<N>` işaretsiz ve aritmetiği kısıtlı (E2004); sayaç/karşılaştırma
desenlerini karşılamaz. Elenmiştir.

**Seçenek B — uN'i typeck'te Path üzerinden tanımaya devam etmek**
Bugünkü durum: SV eşlemesi olmayan "yarı tip". Tutarsız. Elenmiştir.

**Seçenek C — uN/iN tip ailesini lexer'a almak (SEÇİLDİ)**
`u[1-9][0-9]*` / `i[1-9][0-9]*` deseni tek `UIntType`/`SIntType`
tokenına gider; genişlik parser'da metinden okunur.

## Karar

- Lexer: `u8`..`i64` anahtar kelime tokenları kaldırıldı; yerlerine
  regex tabanlı `UIntType` (`u[1-9][0-9]*`) ve `SIntType`
  (`i[1-9][0-9]*`) geldi. Logos'un en-uzun-eşleşme kuralı `u8x`,
  `i2c_addr` gibi tanımlayıcıları Ident olarak korur; `u0`, `u_8`
  desene uymaz, Ident kalır.
- Parser: 1 ≤ N ≤ 64 için doğrudan `TypeRefKind::UInt(N)` /
  `SInt(N)`. N > 64 eski davranışla tek segmentli `Path` olarak
  taşınır: typeck genişletilmiş aileyi tanımaya devam eder, SV
  eşlemesi sonraki aşamaların işidir (bkz. `is_widened_int_type`).
- `uN`/`iN` adları artık tanımlayıcı olamaz — `u8` zaten anahtar
  kelimeydi, aile bütünüyle ayrılmış oldu (§17 notu).
- Literal SONEKLERİ değişmedi: `42u8` gibi sonekler §14'teki sekizli
  kümede kalır. Keyfi genişlik hedefe atama/karşılaştırma bağlamından
  çıkarılır; `const DIVISOR : u10 = 434` yeterlidir.
- Const katlama (sorunun 2. yarısı): SV üreticisi üst düzey `const`
  referanslarını bildirilen tipin genişliğiyle boyutlandırılmış
  literale katlar (`DIVISOR` → `10'd434`). `localparam` üretilmez —
  üretilen RTL'de tanımsız isim kalmaz, kullanılmayan localparam
  lint'i tetiklenmez. Modül sinyalleri ve yerleşik primitif örnekleri
  aynı adı gölgeler; katlanamayan referans (döngü, sinyal içeren
  ifade) isim olarak basılmaya devam eder ve mevcut tanılarla yakalanır.

## Sonuçlar

- `reg baud_cnt : u10 = 0` ve `const DIVISOR : u10 = 434` aynı tipte
  buluşur; `baud_cnt == DIVISOR - 1` doğrudan yazılır.
- grammar-full.ebnf §6 `UIntType`/`SIntType` üretimleri genelleşti,
  §17 anahtar kelime listesi aile notuna döndü (bu ADR kaynak).
- `tests/ui/pass/37_arbitrary_widths.volt` u3/u10/u17/i5 zincirini
  uçtan uca doğrular; lexer/parser birim testleri Ident sınırlarını
  (`u8x`, `u0`) sabitler.
- ADR-0025'in esnek genişlik aralığı değişmedi; bildirilen tipler
  somut tek genişlik olmaya devam eder.

## Emsal

SystemVerilog `logic [N-1:0]` ile, VHDL `unsigned(N-1 downto 0)` ile,
Chisel `UInt(n.W)` ile keyfi genişliği doğal sayar. Rust'ın sabit
sekizli ailesi yazılım ABI'sinden gelir; donanımda genişlik veri yolu
kadardır — dilin tip ailesi de öyle olmalıdır.
