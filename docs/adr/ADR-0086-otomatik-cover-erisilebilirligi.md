# ADR-0086: Otomatik Cover'ların Yapısal Erişilebilirliği — Cover Kipinde Yanlış E5001 Yok

> Statü: Uygulandı
> Önceki karar: ADR-0066 — cover kipinde otomatik cover E5001
> Tarih: 2026-09-27
> Etkilenen: volt-ast (`AutoOrigin::reach`, `AutoReach`), volt-syntax
> (`auto_contract/counter.rs`, `fsm.rs`, `gen.rs`), volt-sv-emit
> (`AutoProp::reach`, `CoverReach`, `COVER_HARNESS_STEPS`), volt-driver
> (`verify_depth.rs` yeni, `verify.rs`, `verify_jobs.rs`,
> `verify_report.rs`); cli-contract.md §8a
> Kaynak: ADR-0084 §5 bulgu 5; ADR-0066 §5 "C — derinlik sınırı" sınıfı

## Sorun

ADR-0084 şablonları yazılırken: doygun sayaç (`if c != 255 { c <= c + 1 }`)
ADR-0066'nın otomatik "counter wrap" cover'ını (`c == 255`) erişilemez
kılıyor, `volt verify --mode cover` E5001 veriyor; şablon sayaç yerine
yapışkan bayrak kullandı. ADR-0074'teki erişilemez `_` kolundan sonra
ADR-0066'dan kalan ikinci yanlış cover sayıldı. Görev: "sarma yolu
olmayan sayaçta wrap cover'ı üretme; tespit yapısal; emin olunamazsa
üretme — yanlış alarm, eksik cover'dan kötü".

### Ölçüm — neden doygunluk değil, derinlik

Sonda `build/c2/gen.py` (16 sayaç biçimi), `volt verify --mode cover`,
hdlc/formal + boolector, tek konteyner:

| Sayaç | Derinlik 12 |
|---|---|
| doygun, sınır 5 (`!=`, `<`, `en &&`, `clr` ile, reset sınırda, başka koşulda sıfırlama) | **ulaşılır** (6/6) |
| sarmalı, sınır 5 (else/then/en/yükleme) | ulaşılır |
| doygun, sınır 255 | E5001 |
| **sarmalı, sınır 255** | **E5001** (aynı) |
| doygun 255, `ld` ile 250 yüklemesi | ulaşılır |

Doygun sayacın sınırı erişilebilir — cover yanlış değil. E5001'in nedeni
0'dan 255'e 255 artış: derinlik 12'de hiçbir sayaç oraya varamaz, sarmalı
da. "Doygun sayaçta wrap cover'ı üretme" hem yanlış teşhis hem geçerli
bir kapsam sinyalini (sayaç doyuyor mu?) siler, sarmalı 255'i de
düzeltmez. Kök neden ADR-0066 §5'in "C — derinlik sınırı" sınıfıdır
(VGA 800/525, i2c): sınırlı BMC'de **ulaşılmadı ≠ ulaşılamaz**, ama
volt verify ikisini de "contract violated" diye raporluyordu — üstelik
kullanıcının yazmadığı bir kontrat için.

### Kalibrasyon — koşum ofseti

Formal koşum `initial assume (rst)` + saat kenarında örneklenen
`if (!rst) cover (...)`. Tam ölçüm (derinliği birer artırarak):

| Tasarım | Kenar alt sınırı | İlk ulaşılan sby derinliği |
|---|---|---|
| reset değeri zaten sınırda | 0 | 3 |
| 0 → 5 (+1) | 5 | 8 |
| `ld` ile 250, sonra 255'e | 1 + 5 | 9 |
| 0 → 255 | 255 | 258 (257'de ulaşılamaz) |
| asenkron, aktif düşük, negedge reset, 0 → 5 | 5 | 8 |
| FSM `0 -go-> 1 -> 2 -> 0`: `0→1`, `1→2`, `_→0` | 1, 2, 3 | 4, 5, 6 |

Ofset her biçimde **3** (`COVER_HARNESS_STEPS`): reset adımı, ilk
örneklenen kenar, örnekleme gecikmesi. Çok saatli koşum (`multiclock on`)
kenar başına daha çok adım harcar — değer yine alt sınırdır.

## Karar

**Cover üretimi değişmez; `volt verify --mode cover` bir OTOMATİK
cover'ın ulaşılmamasını yalnız yapısal kanıt varsa E5001 sayar.**

1. **Yapısal erişilebilirlik tanıyıcıda hesaplanır** (`AutoOrigin::reach`):
   - Sayaç sarma cover'ı `r == E`: sayaç yalnız +1 artar ya da sabit
     yazılır (ADR-0066 tanıma kuralı) → en az `min(E - init, 1 + E - k)`
     kenar (`k` sabit yazmalar). `AtLeast(n)`.
   - FSM (F3 geçiş, F2 durum): durum register'ı yalnız sabit yazılır →
     durum grafiği, resetten BFS. Kenar = her yazma, kaynağı yazmanın
     bulunduğu kol (değerler, joker = adı geçmeyenler, match dışı = her
     durum). Geçiş `a → b`: `mesafe(a) + 1`; durum `v`: `mesafe(v)`.
     Kaynak hiç ulaşılabilir değilse **`Never`** — kanıt.
   - Register'ın kendi yazmaları dışındaki koşullar (girişler, başka
     sayaçlar) yok sayılır: her kenarda en elverişli seçim — `AtLeast`
     ALT sınırdır, `Never` her koşulda doğrudur. İfade tahmini yok.
   - Handshake / `@mmio` / F1: `Unknown`.
2. **sv-emit** ofseti ekler: `CoverReach::MinDepth(n + 3)`; reset'siz
   alanda başlangıç serbest olduğundan her şey `Unknown` (resetten BFS
   anlamsız).
3. **verify, cover kipi:** sby'nin BÜTÜN "Unreached cover" satırları
   okunur (önce yalnız ilki). Ulaşılmayan cover:

   | Cover | Sonuç |
   |---|---|
   | kullanıcı cover'ı | **E5001** (değişmedi — kullanıcı yazdı, derinliği o seçer) |
   | otomatik, `Never` | **E5001** + not "structurally unreachable: no sequence of states from reset leads here, at any depth" |
   | otomatik, `MinDepth(d)`, `d > derinlik` | not: "needs --depth d or more; unreachable at depth N by construction, not a design error" |
   | otomatik, diğer | not: "not reached within depth N; not proven unreachable, so not an error — try a deeper run or 'volt test'" |

   Notlar başarısızlık değildir: görev `ok`, çıkış 0 (yalnız notlar
   varsa); ilerleme satırı ve `--fail-fast` düzeltilmiş sonucu görür.
   Özet: `Result 1 of 2 properties verified, 1 auto cover(s) not reached
   at this depth`. JSON `properties[].status`: `needs-depth` (+
   `min_depth`) ya da `not-reached`.

### Gerekçe

- **Yanlış alarm, eksik uyarıdan kötü** (görevin önceliği): kanıtsız
  otomatik cover için E5001 yanlış alarm olabilir; kanıt varsa (`Never`)
  gerçek bulgudur ve kalır (ADR-0066 fail/71: "otomatik cover'ın asıl
  değeri budur").
- **Cover'ı üretmemek yerine sınıflandırmak:** cover simülasyonda
  (`volt test`) kapsam sinyali olarak değerli (ADR-0066 §5: VGA sarmaları
  simülasyonda 120–1528 kez tetiklendi); derinlik kullanıcının
  seçimidir, üretim zamanında bilinmez.
- **Sayaç kesin, FSM ölü tespiti kesin; aradaki (başka sayaçla kapılı
  geçiş) kanıtsız:** uart'ın `Data → Stop` geçişi ~37 kenar ister (4 +
  8×4 baud) ama bunu kanıtlamak iç içe sayaç "tutma" analizi ister
  (kapı sayaç `C == K` iken bir sonraki kenarda `K`'de kalabilir mi?);
  genel durumda sağlam değil — kanıtsız kalır, not alır.
- Doygunluk ayrımı yok: doygun ve sarmalı sayaç aynı cover'ı hak eder.

## Sınıf taraması ve korpus (derinlik 12, tek tek)

ADR-0066'nın ürettiği diğer cover'lar yapısal olarak erişilemez olabilir
mi? FSM F2/F3 **evet** — kaynak durumu resetten hiç girilmeyen kol
(fail/71); BFS bunu `Never` olarak kanıtlar. Sayaç C3 **hayır** — sayaç
her kenarda artabilir, `E ≤ tip maksimumu` (tanıma şartı). F1/Handshake/
`@mmio` cover değil ya da çözümlenmez.

`build/c2/scan.py`: `templates/`, `examples/`, `tests/ui/pass/` içinde
otomatik cover'ı olan 20 dosya + fail/71, `volt verify --mode cover
--depth 12 -j 1`, sby logundaki bütün "Unreached" satırları eşlenerek.

| Dosya | Önce: otomatik E5001 | Sonra |
|---|---|---|
| vga_timing | h/v sarması | not: `--depth 802` / `527` |
| vga_top | h/v, wx/wy sarması | not: `802` / `527` / `82` / `62` |
| ui/pass/23_provable_invariant | `count_r == 10` | not: `--depth 13` |
| ui/pass/37_arbitrary_widths | `baud_cnt == DIVISOR - 1` | not: `--depth 436` |
| uart_tx (+ riscv_core, hello_soc, soc/uart, soc/top) | `bit_count_r == 7`, `Data → Stop`, `Stop → Idle` | not: kanıtsız (baud sayacıyla kapılı) |
| i2c_master | `bit_count_r == 7` + 7 geçiş | not: kanıtsız (faz sayacıyla kapılı) |
| key_store, gpio, mmio şablonu, ui/pass 38/57/58/72/89/90/94 | — | — |
| **ui/fail/71** (kasıtlı ölü kol) | `1 → 2`, `_ → 0` | **E5001 kalır** + "structurally unreachable" |

Otomatik cover'dan yanlış E5001: **önce 10 dosya, sonra 0**. Kalan E5001'ler
kullanıcı cover'ları (uart `state_r == Stop`, i2c 5, VgaTiming 3) —
ADR-0066 §5'te "önceden de aynı nedenle ulaşılamıyordu" diye kayıtlı;
değişmedi. Ölçüm çıktıları `build/c2/scan-before.txt`,
`scan-after.txt`.

## Doğrulama

- `verify_cover_depth_tests.rs`: sahte sby ile (araçsız) — yalnız derin
  otomatik cover → çıkış 0 + not + JSON `needs-depth`; yanında kullanıcı
  cover'ı → E5001 kullanıcı satırında; sınırı derinlikten küçük kanıtsız
  cover → not; yapısal ölü FSM (fail/71) → E5001 + kanıt notu. Gerçek sby
  varsa (verify işi): 5 artışlık sayaç derinlik 7'de `needs --depth 8`,
  8'de doğrulanır — ofset değişirse test düşer.
- `verify_depth.rs` birim, `verify_report.rs` özet/JSON, `verify.rs`
  bütün "Unreached" satırları, `auto_contract_tests.rs` (sayaç sınırı,
  yükleme, doygun, reset sınırda; FSM mesafe 1/2/3, ölü kol `Never`,
  match dışı yazma).
- Golden: check/build/SVA çıktısı değişmedi (`reach` SV'ye yazılmaz).
- Mutasyon (`build/c2/mutate.py`): 9/9 düştü — düzeltme kancasını
  kaldırmak, `Never`'ı nota çevirmek, koşum ofsetini 2 yapmak, reset'siz
  alanda sınır vermek, sabit yüklemeyi saymamak, joker kolu her durumdan
  saymak (ölü tespiti kaybolur), yalnız ilk "Unreached" satırını okumak,
  derinliği aşmayan sınırı `needs-depth` saymak, FSM geçişinde kaynağa
  varış kenarını saymamak.

## Sonuçlar

- (+) Şablonlarda ve örneklerde `--mode cover` otomatik cover yüzünden
  düşmez; derin sayaçlar için kesin derinlik önerilir.
- (+) Gerçek ölü FSM kolu kanıtla raporlanır.
- (−) Kapılı geçişi başka bir koşul yüzünden hiç tetiklenmeyen (ama
  grafikte ulaşılabilir) otomatik cover cover kipinde not olur, hata
  değil; simülasyonda "NEVER HIT" raporu (ADR-0064) sürer.
- (−) JSON durum kümesi iki değer büyüdü (cli-contract.md §8a).

## Gelecek İş

1. Kapı sayaç analizi (her kolda yazılan sayaçta dönem = sınır − sıfırlama
   değeri + 1) — uart/i2c geçişlerine kesin alt sınır.
2. `Never` otomatik cover'ı derleme zamanında uyarı olarak da vermek
   (ölü FSM kolu formal koşmadan görünür).
