# ADR-0050: `Handshake<T>` — Yerleşik Tek Saatli El Sıkışma Bundle'ı

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-14
> Etkilenen: volt-syntax (parser/handshake.rs YENİ, parser/bundle.rs
> sanal alan + otomatik kontrat, item.rs `@no_protocol_check`),
> volt-hir (handshake.rs YENİ — E4007, lib.rs geçit), volt-driver
> (E4007 geçidi), volt-diagnostics (E4007, explain), docs/stdlib.md,
> examples/axi4lite_slave.volt (+ test), examples/soc/{axi,bus,timer,
> uart,top}.volt (+ test), tests/ui/pass/66-67, tests/ui/fail/52

## Sorun

`examples/axi4lite_slave.volt` keşfi (rapor 4b): valid/ready el
sıkışması BEŞ kanalda tekrarlandı. Her kanal için aynı iki alan
(`out valid : bool`, `in ready : bool`) elle yazıldı, tutma kuralı
(`prev(valid) && !prev(ready) -> valid`) beş kez elle yazıldı (3
`assume` + 2 `invariant`), "veri el sıkışma tamamlanana dek sabit"
kuralı ise HİÇ yazılmadı — AXI'nin asıl yükümlülüğü kontratlarda yoktu.
SoC'de aynı desen `AxiToReg` köprüsünde (7 protokol satırı) ve üç
periferikte tekrarlandı.

Durum tespiti: mevcut `HandshakeSync<T>` bir CDC primitifidir —
`src_clk`/`dst_clk` arasında 4 fazlı req/ack ile TEK transfer (ADR-0027).
Tek saat alanında valid/ready protokolüyle ilgisi yoktur; port değil
örneklenen bir modüldür. Volt'ta tek alan içi el sıkışma için ne bir
tip ne bir yardımcı vardı; ADR-0039 bundle'ı yapı taşını verdi ama
generic `struct port` düzleştirilmiyordu.

## Karar

### 1. Yerleşik bundle

```volt
struct port Handshake<T> {      // yerleşik — kullanıcı yazmaz
    out data  : T
    out valid : bool
    in  ready : bool
}

module Producer { out tx : Handshake<u8>  ... }   // data/valid çıkış, ready giriş
module Consumer { in  rx : Handshake<u8>  ... }   // yönler terslenir (ADR-0039)
```

ADR-0039 mekanizması kullanılır: parser sonunda `flatten_bundles`
Handshake portunu `<port>_data`, `<port>_valid`, `<port>_ready` düz
portlarına açar; `in` yönleri tersler; gövde ve kontratlardaki
`tx.data`/`tx.valid`/`tx.ready` zincirleri düz isme yazılır. İsim
çözümleme, tip denetimi, domain çıkarımı ve SV üretimi Handshake'i
görmez (silme ilkesi); `Port::bundle` kaynağı `bundle: "Handshake"` ile
işaretlenir (E4005/E3013 ve E4007 bunu okur).

**Payload**: `T` sade (yönsüz, generic olmayan) bir `struct` ise
alanları ADR-0039 iç içe adlandırma kuralıyla açılır — `in aw :
Handshake<AxiAddr>` → `aw_data_addr`, `aw_data_prot`, `aw_valid`,
`aw_ready`; erişim `aw.data.addr`. Struct içinde struct özyineli açılır
(`tx_data_i_x`, derinlik sınırı bundle'la aynı). Gerekçe: SV çıktısında struct tipli
port yoktur, düz portlar Verilator/Yosys'te sorunsuzdur ve
`<port>_<alan>_<alt_alan>` zaten iç içe bundle'ın kuralıdır — yeni bir
adlandırma istisnası yaratılmadı. Skaler/dizi `T` tek `<port>_data`
portu olur.

Kullanıcı aynı adla `struct port Handshake` tanımlarsa (generic olsa
bile) kullanıcı tanımı kazanır ve yerleşik devre dışı kalır (stdlib
gölgeleme kuralı, docs/stdlib.md). `extern module` portları düzleşir
ama kontrat almaz (extern'in kontratı yoktur). Yerleşik
yalnız TAM BİR tip argümanıyla (`Handshake<T>`) devreye girer;
argümansız ya da iki argümanlı yazım olduğu gibi kalır ve isim
çözümleme raporlar.

### 2. Protokol yardımcıları: Seçenek B (bundle üzerinde alan gibi)

İki seçenek değerlendirildi:

| | A) Yerleşik fonksiyon `handshake_fired(tx)` | B) Sanal alan `tx.fired` |
|---|---|---|
| Yeni kavram | Bundle'ı DEĞER olarak argüman geçmek — ADR-0039 "bundle bir bütün olarak değer değildir" diyor; isim çözümleme, tip denetimi ve SV üretimi bu yeni kavramı öğrenmeli | Yok — `tx.fired` zaten `tx.valid` gibi bir alan zinciridir |
| Uygulama yeri | volt-hir prelude + typeck + volt-sv-emit | Yalnız parser/bundle.rs: zincir haritasında eşleşince `tx_valid && tx_ready` ifadesine yeniden yazılır |
| Kontratta/`on` bloğunda | Ayrı bağlam kuralları gerekir | Her yerde çalışır (yeniden yazma bağlamdan bağımsız) |
| Hata durumu | Argüman bundle değilse yeni tanı | Sol tarafta yazılırsa mevcut E1001 |

**B seçildi**: daha az karmaşık — sıfır yeni aşağı akış kavramı, tek
dosyada ~40 satır. `tx.fired` = `tx_valid && tx_ready`, `tx.stalled` =
`tx_valid && !tx_ready`. Sanal alanlar yalnız okunur; `tx.fired = x`
yazımı düzleştirilmez ve isim çözümleme `tx` için E1001 üretir (özel
tanı ertelendi).

### 3. Otomatik protokol kontratları

Her Handshake portu için parser iki kural sentezler ve modül
kontratlarına ekler:

| Kural | Anlam |
|---|---|
| `prev(valid) && !prev(ready) -> valid` | valid, ready gelene dek düşmez |
| `prev(valid) && !prev(ready) -> data == prev(data)` | veri el sıkışma tamamlanana dek sabit (düz veri alanı başına bir kural; dizi/tuple payload atlanır — `==`/`prev` tanımsız) |

**Tür**: üretici tarafta (`out`) `invariant`, tüketici tarafta (`in`)
`assume`. Gerekçe ADR-0040 ile aynıdır: bir modül kendi girişini
kanıtlayamaz, ortamdan bekler; slave'in master'ı zorlaması mümkün
değildir. Kontratların span'i port bildirimidir: E5001 karşı örneği
porta işaret eder.

**Kapatma**: `@no_protocol_check` port düzeyinde (o port) ya da modül
düzeyinde (modülün bütün Handshake portları). Nitelik
`KNOWN_ATTRIBUTES`'a eklendi ve UYGULANIR (ADR-0048 listesinde değil,
W0021 üretmez). Kullanım gerekçesi: protokolü bilerek konuşmayan port —
örneğin yanıtı kendi ready'sini beklemeden yansıtan bir izleme/debug
çıkışı (`tests/ui/pass/67`, `dbg` portu).

`prev()` yalnız kontrat bağlamında geçerlidir (E5017); sentezlenen
ifadeler modül kontratı olarak eklendiğinden çözümleyici bunları
kontrat bağlamında görür — ek kural gerekmedi. Otomatik kontrat
`ready`yi okuduğundan üretici hiç okumasa da W1001 çıkmaz;
`@no_protocol_check` ile okunmayan ready yine W1001'dir.

### 4. E4007 — valid, ready'ye kombinasyonel bağımlı olamaz

Kontratlar davranışı formal akışta kanıtlar; bir yapısal kural ise
derleme zamanında denetlenir: üretici tarafta `valid`, `ready`'den
kombinasyonel türetilemez (AXI A3.3.1). İki taraf da karşısını beklerse
el sıkışma hiç tamamlanmaz. volt-hir/handshake.rs modül gövdesindeki
sürekli atamaları, `let` bağlarını ve `comb` bloklarını (koşullar
dâhil) bir bağımlılık grafiğine çevirir ve `<p>_valid` → `<p>_ready`
yolu arar; `on` blokları register olduğundan yolu keser, örnek
çıkışları opaktır. Tüketici `ready`yi `valid`den türetebilir; yalnız
üretici (`valid` çıkış, `ready` giriş) portları denetlenir.

Beş parça: kod E4007, konum (valid'i süren deyim), açıklama
(`'tx' el sıkışması: valid, ready'ye kombinasyonel bağımlı`), öneri
(register'lanmış karar: `reg valid_r ...; on clk {...}; tx.valid =
valid_r`), gerekçe notu (kombinasyonel yol `tx.valid <- go <- tx_ready`
+ kilitlenme açıklaması, ADR-0050) + ikincil etiketler (ready okuması,
port bildirimi). `volt explain E4007` iki dilde. Sürücü geçidi
`check_handshakes` L1 zamanlamadan sonra koşar.

## Sonuçlar

`examples/axi4lite_slave.volt` (ADR-0040 durumu → ADR-0050):

| | ADR-0040 | ADR-0050 |
|---|---|---|
| Satır | 163 | 131 |
| Bundle tanımı | 5 `struct port`, 34 satır (valid/ready 5 kez) | 4 payload `struct`, 4 satır |
| Elle yazılan protokol kontratı | 5 tutma kuralı (3 assume + 2 invariant), veri kararlılığı yok | 0 — 14 kural otomatik (aw/w/ar: 3'er assume; b: 2, r: 3 invariant) |
| Kanıtlanan özellik | 16 | 25 (11 elle + 14 otomatik) — `prove 3 --engine boolector`, `bmc 12`, `cover 12` |
| Sim | 5/5 (Verilator) | 5/5 |
| Lint | `-Wall` temiz | `-Wall` temiz |

SoC (ADIM 6): AXI kanalları `Handshake<Payload>` olduğundan `AxiToReg`,
`Timer`, `UartCtrl` ve `SocTop` otomatik kontrat aldı; `AxiToReg`'in 7
elle yazılmış protokol satırı 3'e indi (yalnız `r : AxiReadCtl` —
Handshake DEĞİL, veri periferikte — tutma kuralı elle kaldı).
`SocTop`'ın otomatik `b`/`r` kontratları gerçek bir hata buldu:
`BusDecoder` bir yanıt teslim edilirken başka bir periferiğe yeni istek
kabul ediyor, sahip register'ı (`wsel_r`/`rsel_r`) değişip yanıt
muxu teslimat ortasında sahibini değiştiriyordu (valid düşer, veri
değişir). Düzeltme: `wr_pending`/`rd_pending` kapısı — yanıt
bekleniyorken istek yönlendirilmez ve kabul edilmez. `BusDecoder`'ın
kendi `AxiSel` bundle'ı (yalnız valid/ready/resp yönlendirir, payload
host'tan doğrudan gider) Handshake'e çevrilmedi: `aw`/`w`/`ar` için
payload'sız bir Handshake anlamsız, `r_data` ise başka kanalda.
Ayrıca W yalnız AW ile birlikte yönlendirilir: AXI4-Lite master W'yi
önce sunabilir; tek başına yönlendirilen `w_valid`, AW başka sayfayı
seçince düşer ve periferiklerin `w` girişinde varsaydığı tutma kuralını
bozardı. `Gpio` (`@mmio`, ADR-0044) üretilen düz AXI adlarını
(`aw_addr`, ...) korur; `SocTop` onu eski, Handshake'li periferikleri
yeni adlarla (`aw_data_addr`, ...) bağlar. SoC doğrulaması: 5/5 sim
testi (Verilator), sekiz modül `-Wall` temiz, 137 özellik (74 →
otomatik kontratlarla) `bmc 12` / `prove 3 --engine boolector` /
`cover 48`. `top_test.volt`'ta HEAD'de zaten başarısız olan
`gpio_write_then_read` (volatile `DATA_IN` register'ının pinleri bir
kenar geç örneklemesi) test zamanlaması düzeltilerek geçirildi.

Testler: `tests/ui/pass/66_handshake_basic.volt` (üretici/tüketici/
bağlantı, `fired`/`stalled`), `67_handshake_contracts.volt` (struct
payload, elle + otomatik kontrat, `@no_protocol_check`),
`tests/ui/fail/52_handshake_protocol_violation.volt` (E4007);
volt-syntax/tests/bundle_tests.rs (+17), volt-hir/tests/
bundle_semantic_tests.rs (+13), volt-sv-emit/tests/bundle_emit_tests.rs
(+6), volt-diagnostics explain (114 kod). 66/67 formal: `prove 3`
(6 / 10 özellik), 67 `cover 12`.

## Sınırlar / Ertelenen

- `Handshake<AxiWriteAddr>` gibi `struct port` payload desteklenmez —
  yön taşıyan payload anlamsızdır; generic struct payload açılmaz.
- Düzleştirilmiş port için W1001 fix-it'i (`_tx_ready` öneki) sentetik
  ada işaret eder — ADR-0039'dan gelen sınır; `@no_protocol_check`
  kullanıldığında `tx.ready`yi okumak ya da niteliği kaldırmak asıl
  çözümdür (tanı metni V1).
- Sentetik isim span'leri port bildirimi + öğe sonu bütçesiyle sınırlı;
  çok sayıda Handshake portu olan çok kısa bir modülde span'ler
  çakışabilir (yalnız LSP referans aramasını etkiler).
- `tx.data` bir bütün olarak değer değildir (ADR-0039 kuralı sürer);
  struct payload'da alan alan erişilir.
- `tx.fired`/`tx.stalled` sol tarafta yazılamaz (E1001); özel tanı V1.
- `@no_protocol_check` Handshake olmayan bir porta yazılırsa etkisizdir
  ve uyarı üretilmez (yanlış yerleştirilmiş nitelik sınıfı için genel
  bir tanı yok; V1).
- `@mmio` (ADR-0044) üretilen AXI bundle'ları Handshake kullanmaz;
  birleştirme ayrı ADR.
- E4007 örneklerin içinden ve `for` (generate) gövdelerinden geçen
  yolları görmez; kontrat kanıtı (tutma kuralı) bu boşluğu kapatır.
- `HandshakeSync<T>` değişmedi: CDC (iki saat) için o, tek alan için
  `Handshake<T>` (docs/stdlib.md karar tablosu).
