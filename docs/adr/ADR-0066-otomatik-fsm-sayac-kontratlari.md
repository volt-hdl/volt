# ADR-0066: Otomatik FSM ve Sayaç Kontratları

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-23
> Etkilenen: volt-syntax (`parser/auto_contract/` YENİ — scan, fsm,
> counter, gen, print; `item.rs` desugar zinciri + `KNOWN_ATTRIBUTES`;
> `handshake.rs` ve `mmio.rs` köken bilgisi), volt-ast (`Contract::auto`,
> `AutoOrigin`, `AutoRule`), volt-sv-emit (`SvaProp::auto`, `AutoProp`,
> köken yorumu — `sva.rs`, `sim_contract.rs`), volt-driver
> (`sim/contracts.rs` "generated from" satırı ve cover etiketi;
> `verify.rs` E5001 köken notu + kapsam kipinde ulaşılamayan cover'ın
> satırı), `.github/workflows/ci.yml` (formal işine bir adım),
> `tests/ui/pass/89-90`, `tests/ui/fail/71`, docs/stdlib.md.
> DOKUNULMADI: volt-hir kaynağı (yalnız `ui_semantic_tests` +3), volt-diagnostics (yeni tanı kodu yok),
> examples/, README.md, docs/research/. Üretilen RTL bayt bayt aynı (§7).

## Sorun

`Handshake<T>` (ADR-0050) ve `@mmio` (ADR-0044) arayüz kontratlarını
otomatik üretiyor; `docs/research/rekabet-2026-09.md` (G satırı) Arch'ın
aynısını FIFO, sayaç ve FSM için de yaptığını not ediyor. ADR-0064'ten
sonra değer arttı: bir kontrat hem `volt verify`'da kanıtlanıyor hem her
`volt test` koşusunda izleyici olarak denetleniyor. ADR-0065 Aşama 5'te
VGA ve HybridTop formal'de 2. çevrimde düşüyordu ve kimse fark etmemişti
— kontrat yazılmamış yerde hata görünmez.

## Tespit (ADIM 1)

**Otomatik kontratların bugünkü yolu.** İkisi de volt-hir'de değil,
PARSER desugar'ındadır ve sonuç sıradan bir AST `Contract`'ıdır:

| | Nerede | Nasıl | Konum (span) |
|---|---|---|---|
| Handshake | `parser/bundle.rs` → `handshake.rs::handshake_contracts` (bundle düzleştirmesi) | `ExprBuilder` AST düğümü kurar; adlar benzersiz span alır (`fresh_name_span`) | port bildirimi |
| @mmio | `parser/mmio.rs` (`render_contracts` → sentetik Volt metni → `parse_generated`) | metin olarak üretilip ayrıştırılır | sentetik kaynak (`<mmio:Mod>:N`) |

Kullanıcı kontratlarıyla AYNI listeye (`ModuleDecl::contracts`) eklenirler;
isim çözümleme, tip denetimi, `prev()` kuralı (E5017), SVA, sby görevleri
ve ADR-0064 izleyicileri onları kullanıcınınkinden ayırt etmez. Kapatma
yalnız Handshake'te var (`@no_protocol_check`, port/modül); @mmio'da yok;
Volt.toml anahtarı yok. Kullanıcı kontratın nereden geldiğini yalnız
span'den çıkarabiliyordu: Handshake ihlali simülasyon raporunda
`invariant: out tx : Handshake<u8>` (port bildirimi metni) diye
görünüyordu.

**FSM tespiti için elde olan.** ADR-0032: `on` bloğunda `match` → `case`;
desenler yalnız literal, `A | B` ve `_`; **E0014 her match deyiminde `_`
kolunu zorunlu kılar**; enum desenleri SV'ye inmiyor (E0003). Örneklerdeki
bütün FSM'ler (`uart_tx`, `i2c_master`, `crypto/key_store`) `reg s : uN`
+ literal kollu `match` + `_` biçimindedir; `riscv_core.volt`'ta `match`
YOK (37 kollu bir match beklentisi doğru değil — çözücü bir `if` zinciri).

**Sayaç tespiti.** Örneklerde 19 `r <= r + 1` yazması: sınırlı olanlar
(`uart_tx` clk/bit sayaçları, `vga_timing` h/v, `vga_top` wx/wy,
`key_store` cnt, `i2c` bit sayacı) bir karşılaştırmanın `else`'inde
(`if r == LIM - 1 { r <= 0 } else { r <= r + 1 }`) ya da `>=` korumasında
artar; serbest sayaçlar (`riscv_core` mcycle/minstret/div_cnt, `timer`
count, `i2c` phase) korumasızdır ya da başka biçimde yazılır.

## Seçenekler — nerede üretilmeli?

**A — volt-hir'de, isim çözümlemeden sonra.** Tipler ve tanımlar kesin
bilinir. Ama SV üretimi ve izleyiciler AST kontratlarını okur; HIR AST'yi
değiştiremez, eklenen ifadelerin adları çözümlenmemiş olur (ikinci bir
çözümleme turu gerekir). Elendi.

**B — parser desugar'ı, Handshake/@mmio ile aynı yer (SEÇİLDİ).** Birim
sonu zincirinin (mono → `for` açılımı → bundle → çift yönlü port) SONUNA
`add_auto_contracts` eklenir: somut, düzleşmiş modülleri ve `@mmio`
gövdesini görür; üretilen kontratlar bütün alt geçitlerden kullanıcı
kontratı gibi geçer — "aynı deseni izle". Bedeli: tanıma sözdizimseldir;
bu yüzden muhafazakârdır (§2).

Bu seçim görevin önerdiği kapsamı (`volt-hir`, `volt-diagnostics`) değiştirdi:
değişiklik volt-syntax/volt-ast'a gider, volt-hir'e dokunulmaz; "generated
from" satırı raporda basıldığı için volt-driver'a (sim raporu, E5001 notu)
küçük eklemeler gerekti. Yeni tanı kodu gerekmedi.

## Karar

### 1. Tanıma (sözdizimsel, muhafazakâr)

Aday: `uN` / `uint<N>` register'ı (1..64 bit), modülün İLK saat portunun
`on` bloklarında soneksiz yazılır. Kombinasyonel atama, sonekli hedef
(`r[3] <=`), `for` içi yazma ya da başka saatin bloğu register'ı aday
dışı bırakır ("kirli"). Kontratlar SVA'da ilk saatle örneklendiğinden
başka saatteki register'ın `prev()`'i anlamsız olurdu.

**FSM:** her yazma sabit (`2`, `IDLE`, `LIM - 1`; yerel adla gölgelenmemiş
`const`), reset değeriyle birlikte en az iki farklı değer, ve register
üzerinde muhafızsız, literal/joker kollu bir `match`.

**Sayaç:** her yazma ya sabit ya `r + 1` / `1 + r`; HER artış bir sınır
karşılaştırmasının korumasında —
`else` kolunda `r == E` / `r >= E` (ya da bunları içeren `||`), `then`
kolunda `r != E` / `r < E` (ya da bunları içeren `&&`); ayna biçimleri
(`E == r`) de. `E` sabit ifade, bütün artışlarda aynı değer; reset değeri
ve her sabit yazma `0..=E`. O zaman `r <= E` tümevarımsaldır: artış yalnız
`r < E` iken olur.

### 2. Hangi kontratlar? (ADIM 2)

| Aday | Karar | Gerekçe |
|---|---|---|
| **F1** durum her zaman geçerli değerlerden biri | **ÜRETİLMEZ** | E0014 her `match` deyiminde `_` kolunu zorunlu kılar → görevdeki kural gereği ("wildcard varsa gereksiz") her FSM'de gereksiz. Ayrıca tanınan FSM'in bütün yazmaları sabit olduğundan "yazılan değerlerden biri" tanım gereği doğrudur (hiç düşemez). "Literal kollardan biri" biçimi ise YANLIŞ ALARM üretir: `uart_tx`'te STOP durumu `_` koludur (değer 3). |
| **F2** her durum erişilebilir (cover) | **ÜRETİLİR — yalnız hiçbir geçiş cover'ının hedefi olmayan durumlar için** | Geçiş cover'ı `a → b` tetiklendiyse `s == b`'ye ulaşılmıştır; ikisini birden üretmek gürültü. Reset değeri 0. çevrimde kendiliğinden tutar: üretilmez. `match` dışından girilen durum (`if abort { s <= 3 }`) F2 alır. |
| **F3** her geçiş kullanılabilir (cover, kol başına) | **ÜRETİLİR** — `cover: prev(s) == a && s == b` | Kol `a` içindeki her `s <= b` (b ≠ a) bir geçiş; `A \| B` kolu `(prev(s) == A \|\| prev(s) == B)`, joker kol "adı geçen hiçbir literal değil" kaynağıdır (`_ -> 0`). Öz-döngü üretilmez. **Sınır:** FSM başına 16 geçiş (`MAX_TRANSITIONS`); aşılırsa F3 yerine F2 (her durum için); durumlar da 16'yı aşarsa hiç cover yok — büyük FSM'in kapsamını yazar seçer. |
| **F4** kilitlenme yok (IDLE'a dönülebilir) | **Ayrı kontrat ÜRETİLMEZ** | Liveness; BMC kanıtlayamaz, Volt'ta liveness yok (ADR-0040). BMC'nin ifade edebildiği kısmı ("reset durumuna dönüş erişilebilir") zaten F3'tür: hedefi reset değeri olan geçiş cover'ları (`_ -> 0`). |
| **C1** değer genişlik içinde | **ÜRETİLMEZ** | `uN` sarar; `r <= 2^N-1` tip gereği doğrudur. |
| **C2** açık sınır `r <= E` (invariant) | **ÜRETİLİR** (E < 2^N−1 ise) | §1 koşullarıyla tümevarımsal. Tasarım kodunu yeniden yazdığı için kodun KENDİSİNDEKİ yanlış sabiti yakalayamaz (bkz. `deep_violation`: 100'de saran sayaç `c <= 100` alır); değeri üç yerdedir: `--mode prove`'da tümevarım yardımcısı (elle yazılan "induction helper"ların bir kısmı), reset kapsaması hatası (reset almayan/geç alan register formal'de keyfi başlar — ADR-0065'te VGA'nın düştüğü sınıf) ve simülasyonda ucuz sürekli denetim. |
| **C3** sarma noktası erişilebilir (cover) | **ÜRETİLİR** — `cover: r == E` | Sayacın hiç tam tur atıp atmadığı; simülasyonda test kapsamı sinyali. Derin sayaçlarda (VGA 800) BMC kapsam kipinde ulaşılamaz — bkz. §6 ölçüm ve §5 sınıflandırma. |
| **C4** etkin değilken değişmez | **ÜRETİLMEZ** | Yol koşulu çıkarımı ister, birden çok yazma/sıfırlama yolu varken yanlış alarm riski yüksek; doğru olduğu yerde kodun satır satır tekrarıdır. |

**Kullanıcı kontratıyla tekilleştirme:** aynı tür ve aynı metinli kullanıcı
kontratı varsa otomatik olan üretilmez (`23_provable_invariant`: elle
yazılan `count_r <= 10` kalır, yalnız sarma cover'ı eklenir).

**Numaralandırma:** otomatik kontratlar kullanıcınınkilerden SONRA eklenir;
mevcut property adları (`inv_0`, `cov_3`) kaymaz — sby görev eşlemesi,
eski raporlar ve `volt test` kimlikleri aynı kalır.

### 3. Kontrol mekanizması (ADIM 3)

**Varsayılan AÇIK** — Handshake ve @mmio gibi. Kapatma Handshake'in
deseniyle nitelikle:

- `@no_auto_contracts` modül üzerinde: modülün bütün FSM/sayaç kontratları;
- `@no_auto_contracts` `reg` deyimi üzerinde: yalnız o register.

Nitelik `KNOWN_ATTRIBUTES`'ta (W0020 yok) ve uygulanır (ADR-0048
listesinde değil, W0021 yok). Handshake/@mmio kontratlarını KAPATMAZ:
onlar arayüz spesifikasyonudur (tüketici tarafı `assume`'dur, kapatmak
anlamı değiştirir), bunlar ise uygulamadan türetilir. Volt.toml anahtarı
EKLENMEDİ: parser manifest görmez, mevcut desende de yok; tüm test
koşusunda kapatmak için `volt test --no-contracts` (ADR-0064) zaten var.

### 4. Kaynak konumu ve mesaj (ADIM 4)

Her otomatik kontrat `Contract::auto: Option<AutoOrigin>` taşır: kural
(`AutoRule`), Volt sözdizimiyle metin (kaynakta yazılı değil — raporlar
metni buradan alır), köken anlatımı ve köken konumu. FSM/sayaç
kontratlarının ifade span'i kökendir; Handshake'inki port, @mmio'nunki
kullanıcının `@mmio` niteliği (ifade sentetik kaynakta kalır). Bu tur
Handshake ve @mmio da köken aldı — "her otomatik kontratta" koşulu.

| Kural | Köken konumu | Köken anlatımı |
|---|---|---|
| F3 geçiş | `s <= b` ataması | `match on s, transition 1 -> 2` |
| F2 durum | `match s` deyimi | `match on s` |
| C2/C3 | koruyan karşılaştırma (`r == LIM - 1`) | `wrap check on r` |
| Handshake | port bildirimi | `Handshake port tx` |
| @mmio | `@mmio(...)` niteliği | `@mmio register map of Mod` |

`volt test` ihlali:

```
---- frame ----
  contract violated: auto-generated counter bound invariant (uart_tx.volt:66)
    invariant: clk_count_r <= CLKS_PER_BIT - 1
  at cycle 47
  in instance: dut
  generated from: uart_tx.volt:66 (wrap check on clk_count_r)
```

Cover özeti: `UartTx.cov_7 (uart_tx.volt:99, auto FSM transition)  NEVER HIT`.

`volt verify` E5001: birincil etiket kökende; `= note: auto-generated
counter bound contract 'r <= 9', generated from wrap check on r`; köken
ifadeden ayrıysa (@mmio) "generated from here" ikincil etiketi.
SVA/izleyici yorumu:
`// invariant (auto counter bound: tick_r <= 9) generated from div.volt:10 (wrap check on tick_r)`.

İstenen örnekteki `actual: 4` satırı üretilmedi: izleyici DPI çağrısı
yalnız kimlik taşır; değer basmak her kontrat için ifade başına ayrı
argüman gerektirir (ADR-0064 §6 kapsamı dışı, ertelendi).

Kapsam kipinde ulaşılamayan cover, sby logunda `Unreached cover statement
at dosya.sv:N` satırıdır; önceden bu satır okunmuyor ve E5001 modülün İLK
kontratına düşüyordu. Otomatik cover'lar kapsam kipini sık kullandıracağı
için satır artık okunur: E5001 gerçekten ulaşılamayan cover'ı gösterir.

### 5. Mevcut kodun taranması (ADIM 5)

Otomatik kontratlar açıkken bütün `tests/ui/` fikstürleri ve örnekler
derlendi (golden, §7); kontrat taşıyan her örnek `volt verify` (Docker
sby) ve test dosyası olan her örnek `volt test` (Docker Verilator 5.050)
ile önce/sonra koşuldu. Üretilen kontratlar:

| Modül | Üretilen |
|---|---|
| `UartTx` (uart_tx, riscv_core, soc, hello_soc) | 4 geçiş cover'ı (0→1, 1→2, 2→3, _→0; F2 yok: durumlar geçiş hedefi), `clk_count_r <= CLKS_PER_BIT - 1` + sarma, `bit_count_r <= 7` + sarma |
| `I2cMaster` | 9 geçiş cover'ı (i2c `_` kolu dahil), `bit_count_r <= 7` + sarma; `phase_r` (korumasız, u2 sarması) ve `clk_count_r` (`tick` let'i ile korunuyor) TANINMADI |
| `KeyStore` | 3 geçiş cover'ı, `cnt_r == LOAD_CYCLES` sarma cover'ı (sınır tipin en büyüğü → C2 yok) |
| `VgaTiming` | `h_cnt_r <= H_TOTAL - 1`, `v_cnt_r <= V_TOTAL - 1` + iki sarma |
| `VgaTop` | `wx_r <= 79`, `wy_r <= 59` + iki sarma |
| `RiscvCore`, `Timer`, `HybridTop`, FIR, systolic, AXI | hiç (match yok; sayaçlar serbest ya da maskeli yazma) |

**`volt test` (16 test dosyası, önce/sonra):** çıkış kodları bire bir aynı;
hiçbir otomatik invariant ihlal edilmedi. Cover özetleri:

| Test | Otomatik cover | Sonuç |
|---|---|---|
| riscv_core_test | UartTx 4 geçiş + 2 sarma | hepsi tetiklendi (21–213) |
| soc/top_test | UartTx 4 geçiş + 2 sarma | hepsi tetiklendi (1–11) |
| i2c_test | 9 geçiş + 1 sarma | hepsi tetiklendi (1–562) |
| vga_top_test | 4 sarma | hepsi tetiklendi (120–1528) |
| deep_violation | `c == 100` sarma | tetiklendi (1); ihlal eden yine KULLANICININ `count < 100`'ü |
| uart_tx_test | 4 geçiş + 2 sarma | **`_ -> 0` (STOP→IDLE) NEVER HIT** |

**`volt verify`:** bmc 20 (soc bmc 12) önce/sonra hepsi aynı sonuç: A
sınıfı hata yok, otomatik invariant düşmedi. Kapsam kipinde (i2c/vga
derinlik 24) yeni ulaşılamayan cover'lar çıktı — aşağıda.

**Sınıflandırma:**

| Bulgu | Sınıf | Açıklama |
|---|---|---|
| Gerçek tasarım hatası | **A — bulunmadı** | Otomatik invariant'lar tümevarımsal olduğundan (§2 C2) mevcut örneklerde düşmedi; reset kapsaması hataları ADR-0065 Aşama 5'te zaten kapatılmıştı. |
| Yanlış alarm | **B — yok** | Hiçbir otomatik kontrat bmc/sim'de yanlış düşmedi; tanıma kuralı daraltılmadı. |
| uart_tx_test `_ -> 0` NEVER HIT | **C — cover anlamlı, kapsam boşluğu DEĞİL** | `frame is 10 bits` testi `busy`'nin düştüğü çevrimde biter; izleyiciler kenardaki (önceki) değeri örneklediğinden geçiş bir kenar SONRA görülür. Tasarım davranışı testte `assert_false(dut.busy)` ile zaten doğrulanıyor; riscv_core/soc testlerinde aynı cover 21/1 kez tetikleniyor. |
| i2c kapsam kipi, derinlik 24: 8 geçiş + bit sarması ulaşılamadı | **C — derinlik sınırı** | Bir I2C biti ≥ 8 çevrim (4 faz × 2); simülasyonda hepsi tetiklendi. Önceden de kullanıcının 5 cover'ı aynı nedenle ulaşılamıyordu. |
| VGA kapsam kipi: h/v (800/525) ve wx/wy (80×60) sarmaları ulaşılamadı | **C — derinlik sınırı** | BMC'nin ulaşamayacağı derinlik; simülasyonda 120–1528 kez tetiklendi. VgaTiming'in kullanıcı cover'ları da önceden aynı nedenle ulaşılamıyordu (ADR-0064 notu). |
| fail/71 ölü kol | **C — gerçek kapsam boşluğu (kasıtlı fikstür)** | Tasarım 1 durumuna hiç girmiyor: `1 -> 2` ve `_ -> 0` hiçbir derinlikte ulaşılamaz. Otomatik cover'ın asıl değeri budur. |

ADR-0064'te hiç tetiklenmeyen `Axi4LiteSlave.cov_2/3` ve `I2cMaster.cov_5`
kullanıcı cover'larıdır; bu tur değişmediler (otomatik kontrat bunlarla
ilgili değil).

### 6. Formal süresi (ADIM 6)

Windows `volt verify -j 8` + Docker `hdlc/formal` sby, iki tur, duvar
süresi (s; tur1 / tur2). "özellik" sütunu önce → sonra.

| Dosya | Kip | Özellik | Önce | Sonra | Değişim |
|---|---|---|---|---|---|
| uart_tx | bmc 20 | 7 → 15 | 1.3 / 1.2 | 1.3 / 1.2 | aynı |
| i2c_master | bmc 20 | 13 → 24 | 2.0 / 1.8 | 1.8 / 1.7 | aynı (gürültü) |
| vga_top | bmc 20 | 15 → 23 | 2.4 / 2.3 | 2.6 / 2.4 | +5 % |
| crypto/key_store | bmc 20 | 6 → 10 | 1.4 / 1.3 | 1.3 / 1.3 | aynı |
| soc/top | bmc 12 | 137 → 145 | 10.5 / 10.3 | 10.0 / 9.8 | aynı (gürültü) |
| uart_tx | cover 48 | 7 → 15 | 3.4 / 3.2 | 6.0 / 5.8 | **≈ 1.8×** |
| key_store | cover 48 | 6 → 10 | 1.5 / 1.1 | 1.4 / 1.3 | aynı |
| i2c_master | cover 24 | 13 → 24 | 8.0 / 7.8 | 12.3 / 9.5 | +22…54 % |
| vga_top | cover 24 | 15 → 23 | 2.7 / 2.4 | 2.4 / 2.3 | aynı |
| 23 / fail 24 / deep_violation | bmc 20 | 1/1/2 → 2/3/4 | 1.1–1.3 | 1.1–1.2 | aynı |

(İlk ölçüm turu, durdurulan bir önceki koşunun yetim kalan bash
döngüsüyle — arka planda bir riscv_core sby'si dahil — eşzamanlı koştuğu
için atıldı; tablo boş makinede tekrarlanan iki turdur.)

Ölçülmeyenler: `riscv_core` — `RiscvCore` görevinin kendisi derinlik 5'te
bile 11 dakikayı aştı (otomatik kontrat almıyor, önceden bilinen
yavaşlık); değişen tek görev `UartTx`, yukarıdaki uart_tx ile aynı
modüldür. `hybrid_top` — SVA bayt bayt aynı (otomatik kontrat yok),
bmc 20 önce 637.7 s; tekrar koşulmadı.

**Karar: varsayılan AÇIK kalır.** Varsayılan kip (bmc) etkilenmiyor:
fark ölçüm gürültüsü içinde (≤ %5; soc'ta 8 özellik eklenmesine rağmen
aynı) — invariant'lar birer karşılaştırma, cover'lar bmc'de çözülmez.
Artış yalnız açıkça istenen kapsam kipinde ve kontrat SAYISIYLA değil,
ulaşılması gereken EN DERİN hedefle büyüyor: uart_tx'te STOP→IDLE geçişi
kullanıcının en derin cover'ından (STOP) sonra gelir ve smtbmc her
ulaşılan cover için ayrı iz üretir (key_store'da +4 cover, fark yok).
1.8× (3.3 s → 5.9 s) mutlak olarak küçük ve karşılığında bütün
geçişlerin erişilebilirliği kanıtlanıyor. Cover sayısını daha da
sınırlamak (ör. yalnız F2) i2c'deki artışı azaltırdı ama "her geçiş
kullanılabilir" bilgisini kaybettirirdi; 16 geçişlik FSM sınırı
patlamayı zaten keser.
Maliyet istenmiyorsa modülde `@no_auto_contracts`.

### 7. Değişmeyenler — golden (ADIM 7)

Referans: ADR-0065 Aşama 5 sonrası `main` (4e9c80e), `build/wt-main`
worktree'sinde ayrı hedef dizinle derlendi. `build/afc_golden.py` 794
dosyanın (tests/ui, tests/fixtures, examples, typeck/domain korpusları)
her biri için `volt check --format=json` ve
`volt build --emit=sva,sdc,xdc` çıktısını karşılaştırır.

| Çıktı | Fark |
|---|---|
| `rtl/*.sv` (üretilen RTL) | **0 dosya** |
| `.sdc` / `.xdc` | 0 |
| `volt check` tanı kodları | 0 (26 korpus dosyasında W0020 yardım metnine `@no_auto_contracts` eklendi — bilinen nitelik listesi) |
| SVA — yeni otomatik property | 16 dosya: uart_tx (+riscv_core, riscv_core_test, hello_soc, soc/uart), i2c (2), key_store, vga (3), deep_violation, fail/24, pass/23; `37_arbitrary_widths` ve `38_match_sequential` ilk kez SVA dosyası aldı (kontratsızdı) |
| SVA — yalnız yorum satırı | 13 dosya: Handshake (66, 67, 77, soc) ve @mmio (57, 58, 72, soc, axi4lite) kontratlarının yorumuna kural/metin/köken eklendi; ifadeler aynı |
| Yeni fikstür | pass/89, pass/90, fail/71 |

`volt test`'in simülasyon SV'si (`SvaMode::Simulation`) izleyici eklediği
için değişir; Docker'da `sim_golden_tests` (2) ve `sim_contract_tests`
(20) geçer, testbench C++'ı değişmedi.

## Sonuçlar

- Örneklerde 5 modül otomatik kontrat aldı: 16 FSM geçiş cover'ı, 7
  sayaç sınırı, 8 sarma cover'ı (+ i2c/key_store'daki durumlar geçişle
  kapsandığı için F2 hiç gerekmedi). Hiçbiri yanlış alarm vermedi; bmc'de
  ve 16 test dosyasının simülasyonunda hepsi tuttu.
- Gerçek tasarım hatası BULUNMADI. Değer bugün iki yerde görünür:
  simülasyon cover özeti (bir testin bir geçişi hiç yürütmediği:
  uart_tx_test `_ -> 0`) ve kapsam kipi (ölü kol — fail/71).
- Handshake ve @mmio kontratları da köken aldı: ihlal raporu artık port
  bildirimi metnini değil kontratın kendisini ve "generated from"u yazar.
- `volt verify --mode cover` E5001'i artık modülün ilk kontratına değil,
  sby'nin bildirdiği ulaşılamayan cover'a bağlar (önceden var olan
  hatalı eşleme; otomatik cover'larla sıklaşacağı için düzeltildi).
- CI formal işine `fail/71 --mode cover → exit 6` adımı eklendi; SoC
  adımı artık 145 özellik doğrular (adım adı 137 → 145 güncellendi).

### Testler

- `crates/volt-syntax/tests/auto_contract_tests.rs` — 34 test (FSM
  geçiş/durum/sınır/öz-döngü/joker, sayaç koruma biçimleri ve red
  koşulları, kapatma, köken, benzersiz ad span'leri, Handshake kökeni).
- `crates/volt-sv-emit/tests/auto_contract_emit_tests.rs` — 6 test (RTL
  otomatik kontratlı/kontratsız bayt bayt aynı, SVA yorumu, SvaProp
  kökeni, izleyici DPI kimliği, numaralandırma sırası, @mmio kökeni).
- volt-hir `ui_semantic_tests` +3 (pass/89, pass/90, fail/71 tanısız
  derlenir, beklenen kontrat metinleri).
- volt-driver: `sim/contracts.rs` +3 (ihlal raporunda "generated from",
  metin derleyiciden, cover özeti etiketi), `verify.rs` +2 (E5001 köken
  notu/ikincil etiket, kapsam logundan ulaşılamayan cover satırı);
  `sim_contract_tests` gerçek Verilator koşusunda otomatik sarma cover'ını
  doğrular.
- Değişen beklentiler: `cli_tests::verify_fake_sby_pass_exit_0` (1 → 2
  özellik: pass/23'e sarma cover'ı), `verify_parallel_tests` fikstürü
  `@no_auto_contracts` aldı (modül başına tek özellik varsayar),
  `sim_contract_tests` cover özeti hizası.

### Mutasyon (tek tek, `CARGO_BUILD_JOBS=2`, `--test-threads=2`)

`build/afc_mutate.py` — 18/18 yakalandı:

| Mutasyon | Yakalayan (örnek) |
|---|---|
| F3 geçiş üretimi kapalı | 13 test (ui 89/71, SVA yorumu, parser) |
| F2 durum üretimi kapalı | 3 test |
| C2 sınır üretimi kapalı | 10 test |
| C3 sarma üretimi kapalı | 5 test |
| **Joker muafiyeti bozuk → sahte F1 invariant'ı** | 10 test (`fsm_valid_state_invariant_is_never_generated`, ui 89/71, …) |
| Joker içi döngü (`_ -> joker değeri`) geçiş sayılır | `fsm_wildcard_writing_a_wildcard_value_is_not_a_transition` |
| 16 geçiş sınırı kaldırıldı | 2 test |
| Sabit olmayan yazma atlanır | 1 test |
| then/else koruma yönü ters | 11 test |
| Reset değeri ≤ sınır denetimi yok | 1 test |
| İlk saat süzgeci yok | 1 test |
| Kirli register süzgeci yok | `register_with_an_unrecognised_write_gets_nothing` |
| Modül / reg `@no_auto_contracts` yok sayılır | 2 + 2 test |
| Kullanıcı kontratıyla tekilleştirme yok | 2 test |
| Rapordaki "generated from" satırı silindi | 1 test |
| SVA yorumunda köken yerine ifade konumu | @mmio köken testi |
| E5001 köken notu silindi | 1 test |

İlk koşuda iki mutasyon (joker içi döngü, kirli register) KAÇTI; iki test
eklenip yeniden koşuldu.

## Sınırlar / Ertelenen

- Tanıma sözdizimseldir: `let tick = cnt == LIM` üzerinden korunan artış
  (`i2c` clk_count), çok yazmalı ya da `r <= r + STEP` sayaçları, `match`
  dışı sonraki-durum mantığı (`s <= nx` + comb) tanınmaz — kontrat
  ÜRETİLMEZ, yanlış alarm da yok.
- Enum desenleri (F3 enum) gelince F1 "durum enum varyantlarından biri"
  biçiminde anlam kazanabilir (joker zorunluluğu kalkarsa); yeni ADR.
- İlk saat dışındaki FSM/sayaçlar kontrat almaz (kontratlar ilk saatte
  örneklenir — çok saatli modüllerde kontrat saati ayrı bir iştir).
- `actual: <değer>` satırı (§4) ertelendi.
- Liveness (F4) Volt'ta yok.
