# ADR-0090: Üretilen Ad Çarpışmaları ve `let x = sync(...)`

> Statü: Uygulandı
> İlgili: ADR-0070 (tanı paritesi), ADR-0078 (ad değiştirmeme ilkesi), ADR-0065/ADR-0054 (SDC hücre adları), ADR-0084 §5 bulgu 4, ADR-0079 (çıktı doğrulama ağı).
> Tarih: 2026-09-27
> Etkilenen: volt-sv-emit (`sv_collisions.rs` — YENİ: modül metninde
> çift bildirim denetimi + köken iletisi + yardımcı ad seçimi; `lib.rs`
> `emit_unit` kancası, `sync()` köprüsü `SyncTarget`, `let` köprüsü
> `emit_let_sync`; `past.rs` zincir tabanı; `sim_contract.rs` sayaç adı;
> `expr.rs` ifade içi `sync()` iletisi), volt-hir (`constraints/walk.rs`
> `let` köprüsü SDC/XDC'de; `typeck/synth.rs` `sync()` tipi),
> volt-diagnostics (E1003 açıklaması iki dilde), tests/ui (pass 131,
> fail 189–197), tests/fixtures/parity (gn01–gn14, p24 artık `ok`),
> volt-sv-emit/volt-driver testleri

## Sorun

Volt birçok SV adını kendisi kurar. Kurulan ad kullanıcının bir adıyla
(ya da başka bir kurulan adla) aynı çıkınca `volt check` temiz diyor,
üretilen SV'de aynı ad iki kez bildiriliyordu. İlk kez SoC keşfinde
(`timer` örneği + `irq` çıkışı → `timer_irq` portu), son olarak #55–57
turunda (`hs` + `valid` → `hs_valid`) görüldü. ADR-0070'in paritesi
tanılar içindir; bu sınıf ÇIKTININ geçerliliğidir (ADR-0078 ile aynı
sınıf: "Volt tamam diyor, çıktı geçersiz").

## 1. Ölçüm (önce ölç)

Her ad kaynağı için bir sonda (`build/g1/gen.py`): kurulan ad bir
kullanıcı port/reg/wire/let adıyla ya da başka bir kurulan adla aynı.
Verilator `--lint-only` ve Yosys `read_verilog -sv; hierarchy; proc`
Docker'da (`build/g1/tools.sh`). "sessiz" = `check` ve `build` temiz
ama çıktı geçersiz ya da anlamı değişmiş.

| # | Kaynak | Üretilen ad | Sonda | check | build | Verilator | Yosys | Önce | Sonra |
|---|---|---|---|---|---|---|---|---|---|
| a1 | örnek çıkışı | `<örnek>_<port>` | + port | 0 | 0 | Duplicate | sessiz birleştirme | **sessiz** | E1003 |
| a2 | örnek çıkışı | 〃 | + reg | 0 | 0 | Duplicate | sessiz | **sessiz** | E1003 |
| a3 | örnek çıkışı | 〃 | + let | 0 | 0 | Duplicate | sessiz | **sessiz** | E1003 |
| a4 | örnek çıkışı | 〃 | + wire | 0 | 0 | Duplicate | sessiz | **sessiz** | E1003 |
| a5 | örnek çıkışı × 2 | `a_b`+`c` = `a`+`b_c` | üretilen×üretilen | 0 | 0 | Duplicate | sessiz | **sessiz** | E1003 |
| a6 | örnek çıkışı | 〃 | + örnek adı | 0 | 0 | CELL/variable | hata | **sessiz** | E1003 |
| a7 | örnek çıkışı | 〃 | + blok `let` | 0 | 0 | temiz | temiz | tekilleştirilmiş (ADR-0083) | aynı |
| b1–b3 | bundle alanı | `<port>_<alan>` | + port/let/reg | E1003 | E1003 | — | — | hata (ayrıştırıcı) | aynı |
| b4 | bundle × 2 | `a_b.c` = `a.b_c` | üretilen×üretilen | E1003 | E1003 | — | — | hata | aynı |
| b5 | bundle × örnek çıkışı | `hs_data` | örnek `hs` + bundle `hs` | 0 | 0 | Duplicate | sessiz | **sessiz** | E1003 |
| c1–c5 | struct alanı | `<sinyal>_<alan>` | + port/let/reg/bundle, struct×struct | E1003 | E1003 | — | — | hata | aynı |
| d1 | reset zinciri | `rst_sync_<saat>_stage<i>` | + let | 0 | 0 | Duplicate | uyarı + birleştirme | **sessiz** | E1003 |
| d2 | reset zinciri | 〃 | + port | 0 | 0 | Duplicate | sessiz | **sessiz** | E1003 |
| e1 | otomatik reset portu | `rst` | + let | 0 | 0 | Duplicate | sessiz | **sessiz** | E1003 |
| e2 | otomatik reset portu | `rst` | + reg | 0 | 0 | Duplicate | sessiz | **sessiz** | E1003 |
| e3 | otomatik reset portu | `rst_n` | + port | 0 | 0 | Duplicate port | sessiz | **sessiz** | E1003 |
| f1 | `sync()` köprüsü | `sync_<k>_stage<i>` | + let | 0 | 0 | Duplicate | sessiz | **sessiz** | E1003 |
| f2 | `sync()` köprüsü | `sync_<k>_src` | + port | 0 | 0 | Duplicate | sessiz | **sessiz** | E1003 |
| g1 | `prev()` yardımcısı | `past_<x>_<n>` | + reg (verify kipi) | 0 | 0 | Duplicate | sessiz | **sessiz** | tekilleştirme (`past_x_2_1`) |
| g2 | cover sayacı | `volt_hits_<id>` | + reg (test kipi) | 0 | 0 | Duplicate (test) | — | **sessiz** | tekilleştirme (`volt_hits_cov_0_2`) |
| h1–h2 | fn açılımı | `<fn>_<n>_<yerel>` | + let/port | E1003 | E1003 | — | — | hata (ADR-0081) | aynı |
| i1–i2 | `@mmio` | `mmio_*`, bundle portları | + let/port | E1003 | E1003 | — | — | hata (ayrıştırıcı) | aynı |
| j1–j2 | `for` açılımı | `<ad>_<i>` | + let/port | E1003 | E1003 | — | — | hata (ADR-0056) | aynı |
| k1 | enum localparam | `<Enum>_<Varyant>` | + let | E1003 | E1003 | — | — | hata (ADR-0074) | aynı |
| l1 | yerleşik primitif | `<örnek>_<port/iç>` | + let | 0 | 0 | Duplicate | sessiz | **sessiz** | E1003 |

**En ağır bulgu Yosys'te:** çift bildirim hata DEĞİL — Yosys iki
`logic` bildirimini tek tele birleştirir (d1'de yalnız "reg assigned in a
continuous assignment" uyarısı). Verilator'suz bir akışta (yalnız Yosys
sentezi, `volt verify`) tasarım hatasız sentezlenir ama iki sürücü aynı
tele bağlanmıştır: donanım sessizce değişir.

Ayrışma: bundle, struct, `@mmio`, `for`, fn ve enum kaynakları zaten
hataydı — hepsi adı ayrıştırıcıda ya da açılımda kuruyor ve çözümleme
aynı kapsamda görüyor. Sessiz olanların hepsi adı **emitter'da** kuruyor
(çözümleme görmez): örnek çıkışı, yerleşik primitif, reset portu, reset
zinciri, `sync()` köprüsü, kontrat yardımcıları.

## 2. Karar — hata mı, tekilleştirme mi

İlke (ADR-0078): kullanıcının gördüğü adlar SESSİZCE değişmez.

| Kaynak | Karar | Neden |
|---|---|---|
| otomatik `rst`/`rst_n` portu | **E1003** | Port: modül arayüzü, üst modül ve testbench adıyla bağlar. |
| örnek çıkışı `<örnek>_<port>` | **E1003** | sv-mapping.md §9 adı sözleşme olarak yazar (`f_result`); dalga biçiminde ve hiyerarşik adla hata ayıklamada kullanıcının aradığı ad. Yeniden adlandırmak (`timer_irq_2`) ekrandaki adla kaynağı koparır. |
| yerleşik primitif `<örnek>_<ad>` | **E1003** | CDC primitiflerinin iç register'larına SDC/XDC atıf yapar (`constraints/walk.rs` `builtin_bridge`, `<örnek>_<reg>`). |
| reset zinciri `rst_sync_<saat>_stage<i>` | **E1003** | SDC `set_false_path`/ASYNC_REG hedefi (ADR-0065). |
| `sync()` köprüsü `sync_<k>_src`/`_stage<i>` | **E1003** | SDC/XDC köprü kısıtının hedefi (ADR-0054). |
| `past_<x>_<n>` (`prev()`) | **tekilleştirme** | Yalnız `volt verify`/`volt test` çıktısında, modül içinde yaşar; atıf yapan tek yer aynı emit'in ürettiği kontrat metnidir. SDC/XDC, SVA dosyası (`$past`), testbench ve LSP bu adı hiç görmez. |
| `volt_hits_<id>` (cover sayacı) | **tekilleştirme** | Yalnız `volt test` çıktısında; rapor kimlikle (`"Top.cov_0"`) yapılır, sayaç adıyla değil. |
| blok `let` (ADR-0083), fn yerel teli (ADR-0081) | tekilleştirme (değişmedi) | Zaten öyle. |
| bundle/struct/`@mmio`/`for`/fn/enum | E1003 (değişmedi) | Zaten hata. |

Tekilleştirme kuralı fn açılımıyla aynı: çakışırsa `_2`, `_3`, …
(kaynak sırası, kararlı). `prev()` zincirinde taban tekilleştirilir
(`past_start` → `past_start_2`, halkalar `past_start_2_1..N`): taban,
`<taban>_<sayı>` biçiminde hiçbir modül adı olmayan ilk adaydır. Çakışma
yoksa ad eskisiyle aynıdır — çıktı bayt bayt değişmez.

### Hata kodu: E1003, yeni kod YOK

Aynı sınıfın iki tanısı zaten E1003'tü: fn açılımı ("inlining 'parity'
generates the signal 'parity_0_x', which is already declared") ve enum
localparam'ı ("the SystemVerilog name 'S_Idle' of an enum variant
clashes"). E1003'ün anlamı "aynı kapsamda iki tanım" — SV modülü bir
kapsamdır. Yeni kod aynı hatayı iki koda bölerdi. `volt explain E1003`
iki dilde üretilen adları anlatacak şekilde genişletildi.

İleti adın nasıl kurulduğunu söyler; iki etiket iki konumu gösterir
(birincil: adı kuran yer, ikincil: öteki):

```
error[E1003]: 'timer_irq' is both port 'timer_irq' and output 'irq' of instance 'timer'
   ┌─ top.volt:12:9
 3 │     out timer_irq : bool
   │         --------- also 'timer_irq'
12 │     let timer = Timer { clk }
   │         ^^^^^ becomes 'timer_irq'
   = help: rename one of them — Volt does not rename SystemVerilog names: ...
```

Köken kalıpları: port (bundle alanıysa "field 'data' of bundle port
'hs'"), register, wire, let, örnek adı; örnek çıkışı, yerleşik
primitif sinyali, otomatik reset portu, reset zinciri aşaması, `sync()`
yakalama register'ı/aşaması. İki kurulan ad çakışırsa ikisi adlandırılır
("'a_b_c' is both output 'c' of instance 'a_b' and output 'b_c' of
instance 'a'").

### Denetim nerede

Üretilen modül metni üzerinde (`sv_collisions.rs`), ADR-0078 güvenlik
ağının yanında: modül düzeyindeki (dört boşluk girintili) bildirim
satırları (`input/output/inout/logic/wire/tri1/localparam/longint/int`
ve `Mod örnek (`) okunur, iki kez bildirilen her ad bir E1003'tür.
Gerekçe: gerçek ölçüt SV'de iki bildirimdir; ad kaynakları emitter'ın
birçok yerine dağılmış ve yenisi eklendiğinde denetim kendiliğinden
kapsar. Kökeni tanınmayan bir çift genel iletiyle yine hatadır
("the SystemVerilog generated for module 'M' declares 'x' more than
once"). Ayrıştırıcı tutucudur — tanımadığı satırı atlar; iki bildirim
her zaman geçersiz SV olduğundan yanlış alarm veremez (korpus: 0).
Aynı ad için başka bir E1003 (fn, enum) varsa tekrar bildirilmez.

`volt check` ve LSP aynı emit'i çıktısız koşar (ADR-0070): üç yol aynı
tanıyı görür (`parity_tests.rs`, gn01–gn14). `volt verify` ve `volt test`
kiplerinin kendi yardımcıları tekilleştirildiği için bu kiplerde yeni
bir çakışma kalmaz; kalırsa aynı denetim E1003 verir.

Kapsam dışı (izleme): b4'ün iletisi (`'a.b_c' is already defined`)
ayrıştırıcının bundle düzleştirmesinden gelir ve iki kurulan adı
adlandırmaz — hata olduğu için parite sorunu değil, ileti iyileştirmesi
volt-syntax'ta ayrı iş.

## 3. `let x = sync(...)`

**Ölçüm:** `let s = sync(x, clk)` E0003 veriyordu, yalnız emitter'da
(analiz, alan çıkarımı ve RDC kabul ediyordu). `wire s : bool` +
`s = sync(x, clk)` şunu üretir: `logic s;`, köprü (`sync_x_src`,
`sync_x_stage0/1`), `assign s = sync_x_stage1;`.

**Karar: desteklenir.** `let` aynı çıktıyı üretir — gövde `wire`
sürümüyle bayt bayt aynıdır (`sync3` dahil); genişlik açık tipten, yoksa
kaynaktan. SDC/XDC köprü kısıtı `let` için de üretilir (ölçüm: kısıt
yürüyücüsü köprüyü yalnız atamada arıyordu — `let` desteği yalnız
emitter'da yapılsaydı SDC'de köprü sessizce eksik kalırdı).

Bağlı düzeltme: typeck `sync()`/`sync3()` çağrısına hata tipi veriyordu
(yalnız `prev()` argüman tipini taşıyordu); tipsiz `let s = sync(x8, clk)`
sonra `o : bool = s` sessizce 8→1 bit kesiliyordu — atamada da aynı
(`o = sync(x8, clk)`). Artık `sync(x, clk)` x'in tipini taşır: iki biçim
de E2003. Korpusta yeni hata yok.

İfade içinde ya da blokta `sync()` E0003 kalır (köprü modül düzeyinde bir
register zinciridir); ileti artık "modül düzeyinde sağ tarafın tamamı
olarak yazın: `let s = sync(x, clk)` ya da `s = sync(x, clk)`" der.
ADR-0084 §5 bulgu 4 kapatıldı. `cdc` şablonu `wire` + atama ile kalır
(geçerli ve ADR-0088'in `@Alan` açıklamasını gösteriyor).

## 4. Doğrulama

- Sondalar: tablonun her satırı `tests/fixtures/parity/gn01–gn14`
  (check = build = LSP) ve yeni hatalar için `tests/ui/fail/189–197`.
- Korpus (examples/, templates/, tests/ui/pass, tests/fixtures; iki SVA
  kipi, 1100 derleme): **yeni hata veren dosya 0** (A: 0, B: 0). Tek
  değişen `parity/p24_sync_src_width.volt`: E0003 → derleniyor (beklenti
  `ok`, bu ADR'nin konusu).
- Golden: çakışma içermeyen tasarımların 515 çıktı dosyası bayt bayt
  aynı (`build/g1/corpus_before` ↔ `corpus_after`).
- Mutasyon: her karar noktası tek tek bozuldu, testler düştü (PR
  gövdesinde tablo).
- ADR-0079 çıktı ağı yeşil.
