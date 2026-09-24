# ADR-0072: Küçük Tanı Düzeltmeleri — Kaynak Adı, E4011, Yerleşik Argüman Sayısı, Ölü Reset Zinciri

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-24
> Etkilenen: volt-ast (`GenerateInfo::source_names`/`source_name`;
> `reset_chain.rs` — YENİ: `chain_consumed`, `syncs_to`), volt-syntax
> (`mono/clone.rs` — açılım kaynak adı kaydı; `mono/unroll.rs` — sabit
> olmayan `for` sınırı E2021), volt-hir (`resolve/usage.rs` — W1001;
> `resolve/scope.rs` — E1003/W1002/W1003 kaynak adı; `typeck/synth.rs` —
> `sync`/`sync3` argüman sayısı E2003; `constraints/walk.rs` — yalnız
> tüketilen zincir kısıtlanır, `feeding_raw`/`has_reset`), volt-sv-emit
> (`instance.rs`, `builtin_prim.rs`, `lib.rs` — E4011, kaynak adı,
> `shown_name`, zincir kararı, okunmayan çıkış susturması, `count_ident`;
> `generate.rs` — blok `for` E2021/E2027/E2028; `const_array.rs` —
> E2021/E2003; `reset_sync.rs` — `chain_referenced`), volt-diagnostics
> (E4011; E2005 başlığı ve açıklaması), `tests/fixtures/parity/` (13 başlık
> + `p12b`, `p44`), testler.
> DOKUNULMADI: README.md, docs/spec/, docs/research/, examples/.
> Kapatır: ADR-0070 §"Sınırlar" 1-3, ADR-0068 §"Sınırlar" "üretilmiş ad
> sızması", ADR-0065 §5.3 "ölü zincir".

## 1. Açılmış gövdede üretilmiş ad sızıyordu

Modül seviyesi `for` (ADR-0056) gövdede bildirilen adları yineleme
sonekiyle yeniden adlandırır (`unused` → `unused_0`, iç içe `inner_0_1`).
Tanılar bu adı gösteriyordu:

```
warning[W1001]: unused binding: 'unused_0'
   = help: add a '_' prefix to silence: _unused_0      ← kaynağa yazılamaz
warning[W1001]: unused binding: 'unused_1'              ← mesaj farklı, katlanmaz
```

**Karar:** açılım, yeniden adlandırdığı her bildirim için üretilmiş adın
span'i (yineleme ctx'li) → kaynak adı kaydını `SourceFile::generate`
yan tablosuna (`source_names`) yazar. Tanılar `source_name(span, ad)` ile
kaynak adı gösterir; SV'deki adlar (`pe_0_y`) değişmez. Span anahtarı
kullanıcının kendi yazdığı `x_0`'ı etkilemez (tablo yalnız açılımın
ürettiklerini taşır).

**Tarama** (açılmış gövdede ad basan tanılar, yoklama dosyalarıyla):

| Tanı | Önce | Sonra |
|---|---|---|
| W1001 okunmayan bağlama | `'unused_0'`, 3 ayrı uyarı | `'unused'`, tek uyarı + "3 yinelemede" |
| E1003 çift tanım | `'v_0'`, `'v_1'` | `'v'`, tek hata |
| W1002/W1003 gölgeleme | aynı yol | kaynak adı |
| E4011 bağlanmamış giriş / literalde çıkış / reset / bidir | `instance 'pe_0'` | `instance 'pe'`, katlanır |
| E4011 yerleşik bağlanmamış port | `instance 'f_0'` | `instance 'f'` |
| E0003 hedefsiz örnek | `instance 'x_0'` | kaynak adı |
| W2012 | ADR-0070'te düzeltildi | — |

E1009 (modül adını basar), E2001/E2003 (tip basar), E3001 (ad basmaz) etkilenmez.

## 2. E2005 dört ayrı hatayı taşıyordu

`E2005 — "Literal width cannot be determined"` sv-emit'te ve açılımda
kullanılıyordu (18 nokta); ölçüm beş sınıf verdi:

| Sınıf | Nokta | Karar |
|---|---|---|
| Genişlik/uzunluk belirlenemiyor (literal, dönüşüm kaynağı, tipsiz `let`, `sync()` kaynağı, `bits<N>`/`[T; N]` boyu, indekssiz sabit dizi) | 7 | **E2005 kalır**, başlık "Width or length cannot be determined", `explain` bu listeyi sayar |
| Örnek portu bağlantısı (bağlanmamış giriş/saat — kullanıcı modülü, extern, yerleşik; literalde çıkış; bidir ifadeye bağlı; üst modülde reset yok; `inst.port = ...`) | 6 | **YENİ E4011** (bağlantı/sürücü ailesi E4xxx) |
| `for` sınırı sabit değil (modül ve blok seviyesi), sabit dizi elemanı hesaplanamıyor | 3 | **E2021** (zaten "Constant expression expected") |
| Blok `for` açılım sınırı aşıldı | 1 | **E2027** (modül seviyesi zaten E2027 veriyordu) |
| Sabit dizi eleman sayısı ≠ tip | 1 | **E2003** (tip uyuşmazlığı) |

"Açıklamayı genişlet" seçeneği reddedildi: bağlanmamış port bir
genişlik sorunu değildir; kullanıcı `volt explain E2005`'te literal
örneği görür, hatası başka yerdedir. Var olan kodların (E2021, E2027,
E2003) kısa açıklamaları yeni kullanımla zaten uyumlu; yalnız bağlantı
sınıfı için yeni kod gerekti.

Yan bulgu (düzeltildi): blok içi `for` ters aralıkta (`for i in 5..2`)
sessizce sıfır yineleme üretiyordu; modül seviyesi E2028 veriyordu. Blok
seviyesi de artık E2028.

## 3. `sync()` argüman sayısı E0003'tü

```
error[E0003]: not supported yet: sync() with 3 argument(s), expected sync(src, dst_clock)
```

"Henüz desteklenmiyor" yanlış: bu bir imza hatasıdır ve `sync3()` için
de "sync()" diyordu. **Karar:** typeck (`synth_call`) `sync`/`sync3`
çağrısında iki argüman ister; aksi E2003 "'sync3()' takes 2 arguments
(source, destination clock), 3 given". sv-emit'teki denetim HIR'siz
`emit()` çağrıları için aynı kod ve metinle kalır (analiz hatası emit'i
kapattığından çift tanı yok).

**Tarama — diğer yerleşikler:**

| Yerleşik | Yanlış argüman sayısı | Durum |
|---|---|---|
| `prev()` (kontrat) | E5017 "prev() takes a signal and an optional positive cycle count" | zaten doğru |
| `read_hex`, `load`, `step`, `assert_*` (test dili) | E8505 "'X' expects N argument(s), got M" | zaten doğru |
| `inout`/`opendrain` `drive`/`release`/`drive_low` | E4008 "takes N argument(s), M given" | zaten doğru |
| Handshake | yöntem çağrısı yok (`fired`/`stalled` alan) | — |
| `zext`, `sext`, `trunc`, `concat`, `replicate`, `popcount`, `clog2` | E0003 "function calls inside expressions" | argüman sayısından bağımsız: bu yerleşiklerin SV eşlemesi yok (doğru argümanla da E0003). Arity denetimi eşleme geldiğinde eklenmeli. |

## 4. HybridTb Verilator uyarıları

`examples/hybrid_accel` HybridTb `verilator --lint-only -Wall`: 5
`UNUSEDSIGNAL`.

**4a. Ölü reset zinciri (2 uyarı, ADR-0065 sonrası).** HybridTb'nin kendi
flop'u yok; ham `rst`'yi HybridTop'a geçirir, HybridTop kendi zincirini
kurar. Yine de sv-emit HybridTb'de her alan için bir bırakma zinciri
üretiyor, volt-hir `constraints` de SDC/XDC'de ona kısıt yazıyordu
(`get_cells rst_sync_t_clk_stage0_reg*` — sentezde budanan hücre).
ADR-0065 §5.3 bunu "sv-emit'te zincirsiz geçiş ayrı iş" diye bırakmıştı.
Kök neden emitter'da: zincir, tüketilip tüketilmediğine bakılmadan
üretiliyordu.

**Karar:** zincir yalnız tüketiliyorsa üretilir ve kısıtlanır. Kural
`volt_ast::reset_chain::chain_consumed` — iki taraf aynı fonksiyonu
çağırır; volt-hir RDC'nin `chain_used` kuralıyla aynıdır: `on clk` /
`on clk.reset`, `reg(clk)`, `sync(_, clk)`, flop'larını reset'leyen
yerleşik bağlaması ya da bir çocuğun OTOMATİK reset'li saatine bağlama.
Ek olarak kontratlı modül tutucu biçimde tüketiyor sayılır (SVA `disable
iff`/`initial assume`). sv-emit ayrıca son güvence olarak ürettiği metinde
(gövde, SVA, izleyiciler) zincir adı geçiyorsa zinciri tutar.

**4b. Okunmayan örnek çıkışları (3 uyarı, main'de de vardı).**
`top_t_push`, `top_t_act_east`, `top_b_act_east`: HybridTop'un HybridTb'nin
okumadığı çıkışları. Volt'ta örnek çıkışını okumamak meşrudur (tanı
yok). SV'de seçenekler ölçüldü:

| Biçim | Verilator -Wall |
|---|---|
| tel + bağlantı (önceki) | UNUSEDSIGNAL |
| boş bağlantı `.t_push()` | PINCONNECTEMPTY |
| tel + `// verilator lint_off UNUSEDSIGNAL` | temiz |

**Karar:** emitter, modülün okumadığı (bağlantı dışında hiç geçmeyen)
örnek çıkış tellerini tek bir `lint_off/lint_on UNUSEDSIGNAL` bloğunda
bildirir. Emsal: yerleşik izleyicilerde `lint_off WIDTH`. Örnek
(`examples/hybrid_accel`) değiştirilmedi.

**Sonuç:** HybridTb `verilator --lint-only -Wall` çıkış kodu 0, uyarı yok;
`volt test examples/hybrid_accel/hybrid_test.volt` 6/6.

## Ölçüm

- Golden (ADR-0070 araçları, ADR-0071 dalı ↔ bu dal): 273 dosyadan 257
  birebir; build farkı 4 (hepsi beklenen), check farkı 13 (tümü kod
  değişikliği yapılan parite sondaları + yeni `p12b`, `p44`).
- Geçerli tasarımların build çıktısındaki değişiklikler (yalnız §4):

  | Dosya | Fark |
  |---|---|
  | `examples/hybrid_accel/hybrid_test.volt` → `HybridTb.sv` | iki ölü zincir (−26 satır); 3 okunmayan çıkış susturma bloğunda |
  | → `HybridTb.sdc` / `.xdc` | ölü zincir yorumları (−2) / `ASYNC_REG` kısıtları (−4) |
  | `tests/ui/pass/68_inout_bidirectional.volt` → `Board.sv` | `s_driving` susturma bloğunda |
  | `tests/ui/pass/69_opendrain_basic.volt` → `OpenDrainBus.sv` | okunmayan çıkış susturma bloğunda |

  SDC/XDC: 187 dosyada `--emit=sdc,xdc`, fark yalnız HybridTb.
- Mutasyon (10): W1001/E1003/sv-emit örnek adı üretilmiş ada döndü,
  typeck arity kapalı, E4011 → E2005, zincir hep üretilir (sv-emit),
  zincir hep kısıtlanır (SDC), okunmayan çıkış susturulmaz, blok `for`
  ters aralık denetimi kapalı → **9/10 yakalandı**. Kaçan: iç içe açılımda
  kaynak adını dış yinelemenin kaydından taşıyan zincir araması — iç
  gövde şablondan klonlandığı için gereksizdi; kaldırıldı.

## Sınırlar / Yeni bulgular (ayrı iş)

- **`let` bağlamasına atama sessizce kabul ediliyor**: `let v = a` sonra
  `v = 3` hiçbir tanı vermeden `wire [7:0] v = a; assign v = 8'd3;` (çift
  sürücü) üretir; açılmış `for` gövdesinde de. E4001 (Double driver)
  bekleniyordu. Bu ADR'nin kapsamı dışında.
- `zext`/`sext`/`trunc`/`concat`/`replicate`/`popcount`/`clog2` SV'ye
  inmiyor (E0003); arity denetimi eşleme geldiğinde.
- `docs/spec/` (`type-inference.md` E2005 başlığı; `const-eval.md` ve
  `sv-mapping.md`'deki "sınır sabit değilse E2005", "bağlanmamak E2005"
  atıfları) bu ADR ile eskidi; spec salt okunur (yeni anlamlar burada
  tanımlıdır).
- Flop'suz ama otomatik reset'li alanlı modül kullanılmayan `rst` portu
  taşır (ADR-0071 "Sınırlar"; ör. ui/pass/62 Bridge). Portu kaldırmak
  modül arayüzünü değiştirir.

## Test

- `crates/volt-hir/tests/generate_semantic_tests.rs` — W1001 kaynak adı +
  katlama, iç içe açılım, E1003, elle yazılmış `x_0` korunur (+4).
- `crates/volt-hir/tests/builtin_tests.rs` — `sync`/`sync3` 0/1/3
  argüman E2003 + 2 argüman temiz (+2).
- `crates/volt-hir/tests/constraints_tests.rs` — yalnız tüketilen zincir
  kısıtlanır (+1); üç mevcut test flop'lu hâle getirildi (niyetleri
  besleme kuralı), hiyerarşi testi ara seviyenin ölü zincirini artık
  listelememeyi belgeler.
- `crates/volt-sv-emit/tests/generate_emit_tests.rs` — açılmış gövdede
  E4011 kaynak adı, blok `for` ters aralık E2028 (+2).
- `crates/volt-sv-emit/tests/reset_sync_tests.rs` — ara seviye zincirsiz,
  flop'lu seviye zincirli, otomatik reset'li çocuğu besleyen zincir kalır,
  okunmayan çıkış susturması / hepsi okunuyorsa blok yok (+5).
- Parite sondaları: 13 başlık yeni koda (E4011 ×7, E2021 ×2, E2027,
  E2003 ×1 — `p20_sync_one_arg`), yeni `p12b_for_reversed` (E2028),
  `p44_sync_arity` (E2003, `sync3`).
- Mevcut testlerde kod güncellemesi (E2005 → E4011/E2021): 11 test.
