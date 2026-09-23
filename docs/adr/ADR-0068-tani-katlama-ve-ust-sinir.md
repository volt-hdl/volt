# ADR-0068: Açılan Kopyalarda Özdeş Tanı Katlama, Tanı Üst Sınırı ve Açılım Düğüm Bütçesi

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-23
> Etkilenen: volt-diagnostics (`fold.rs` — `fold_duplicates`, `Identity`;
> `Diagnostic::folded_ctxs`; W0023 mesaj + `volt explain` iki dilde),
> volt-syntax (`parser/mono/unroll.rs` — akış içi katlama, `MAX_UNROLL_NODES`,
> `GenerateIter.parent` kökü), volt-hir (`annotate_generate` — katlama +
> not), volt-driver (`--max-diagnostics`, `cap_diagnostics`, W0023),
> volt-ast (`Arena::len`, `parent` belgesi), `tests/fuzz_regressions/`,
> `crates/volt-{diagnostics,syntax,hir,driver}/tests`.
> DOKUNULMADI: volt-lsp, volt-sv-emit, examples/, README.md, docs/spec/.
> Düzeltir: ADR-0067 §4 "Tanı sayısına üst sınır — EKLENMEDİ" satırı.

## Sorun

Gecelik fuzz (PR #19; run 35891532644) 23,5 dakikada ikinci bulguyu
verdi: 1157 baytlık girdi (`tests/ui/pass/20_const_and_generate.volt`
mutasyonu) ASan altında 2 GB'ı aşıyor. Yerelde `volt check` 6,3 s
(çıktı yakalanınca 14,5 s), **65 365 tanı — 65 303'ü aynı E2005**, tepe
bellek 347 MB, 67 MB metin.

Girdide `const WIDTH = 283` ve hata kurtarmanın iç içe geçirdiği iki
`for i in 0..WIDTH`; en içte sınırı sabit olmayan üçüncü bir `for`
(`0..WIDTHch[156].f14`). Açılım (ADR-0056) her (dış, iç) yineleme
çiftinde iç gövdeyi klonlar ve `bounds()` aynı E2005'i yeniden üretir.

ADR-0067 §4 tanı sayısına üst sınırı "girdi 608 tanı üretti, sınır
gerçek hataları gizler" diye reddetmişti. O ilke FARKLI hatalar için
doğrudur; aynı hatanın kopyaları için değil. Bu ADR o kararı düzeltir.

## Tespit

### Kök neden doğrulaması — WIDTH ölçümü

| WIDTH | tanı (artifact) | tanı (temiz tek `for`, gövdede 2 hata) |
|---|---|---|
| 1 | 61 | 3 |
| 10 | 160 (100 × E2005) | 21 |
| 100 | 10 060 (10 000 × E2005) | 201 |
| 283 | 65 365 (65 303 × E2005) | 567 |

Tek `for` doğrusal, iç içe iki `for` **karesel** (WIDTH²); 283² = 80 089
olması gerekirken 65 303'te kalması u16 `ctx` bütçesinin (65 535, E2027)
kesmesidir — bu girdiyi bir üst sınır zaten kurtarıyordu, yalnız geç.
"Hata kurtarmanın modül çoğaltması" diye bir çarpan yok: modül tek, çarpan
ikinci `for` seviyesidir. 65 303 tanının hepsi AYNI kaynak konumuna
(31:34) ve AYNI mesaja işaret eder; yalnız `Span.ctx` ve "i = k
yinelemesinde" notu farklıdır. Tanı başına ~5 KB (mesaj, etiket, çözüm,
not, ikincil span) × 65 303 = 347 MB.

### Sınıf taraması — "şablon bir kez yazılır, çok kez açılır"

Her sınıf için hatalı bir şablonla ölçüldü (`volt check`, release):

| Sınıf | Girdi | Tanı (önce) | Patlama | `ctx` |
|---|---|---|---|---|
| `for` açılımı (ADR-0056), tek | 283 yineleme, gövdede 2 hata | 567 | doğrusal | yineleme ctx'i, `generate` tablosunda |
| `for` açılımı, iç içe | 283 × 283, iç sınır sabit değil | 65 303 | **karesel** | aynı |
| Generic mono (ADR-0041), elle | hatalı `W<K>` × 10 farklı argüman | 30 (3 × 10) | doğrusal (örnekleme sayısı) | klon ctx'i, tabloda YOK |
| Generic mono, `for` içinden | `for i in 0..283 { W<i> }` | 849 (3 × 283) | doğrusal × yineleme | aynı |
| Bundle dizisi `[Bus; 256]` | alan tipi bilinmiyor | 257 × E1001 | doğrusal (eleman sayısı) | **ctx 0** — düzleştirme span'i paylaşır |
| `[Handshake<Nope>; 256]` | payload tipi bilinmiyor | 256 × E1001 | doğrusal | ctx 0 |
| Pipeline desugar (ADR-0038) | 3 aşamada hata | 1 | yok — her aşama bir kez yazılır | — |
| `@mmio` register üretimi (ADR-0044) | 3 register, alan tipi bozuk | 1 | yok — E0015 erken durur | — |
| Test dili `for` (ADR-0058) | 283 yinelemede hatalı `set` | 1 | yok — çalışma zamanı, açılım yok | — |

Üç sınıf patlar (for, mono, bundle dizisi); üçünde de kopyalar aynı
kaynak konumu + aynı mesajı taşır. Bundle dizisinde kopyalar `ctx` ile
bile ayrılmaz — katlama `ctx`'e bakmadan, kaynak konumu + içerik
üzerinden yapılmalıdır.

### Kalan sınıf — AST boyutu

Tanılar katlanınca aynı girdi hâlâ 274 MB: 65 535 yineleme × klonlanan
gövde (~40 düğüm) ≈ 2,6 M AST düğümü. `MAX_UNROLL` (4096) yineleme
sayısını, u16 ctx toplam yinelemeyi sınırlar; **gövde × yineleme
çarpımını** hiçbir sınır yakalamıyordu (ADR-0067 §4 "mono ve for
açılımının kendi sınırları zaten var" eksik bir gözlemdi).

## Seçenekler

### Tekilleştirme nerede?

**A — Açılım sırasında, sınıf başına (unroll, mono, bundle ayrı ayrı).**
Üç yerde üç kural; bundle dizisinin kopyaları `ctx` taşımaz, mono'nun
kopyaları HIR'da (typeck) üretilir — açılım katmanı onları hiç görmez.
Elendi.

**B — Tanı toplayıcısında, tek mekanizma (SEÇİLDİ) + akış içi çağrı.**
Mekanizma tek: `volt_diagnostics::fold_duplicates` — kimlik = kod + önem
+ mesaj + tüm etiketli span'ler (`ctx` hariç) + notlar. Politika tek:
`volt_hir::annotate_generate` — sürücünün her çıkış yolu (parse
başarısızlığı, anlamsal başarısızlık, emit sonrası) ve `volt_hir::analyze`
zaten buradan geçer; katlama buraya girer, not metni burada yazılır.
İki çağrı yeri vardır çünkü bellek gerekçesi ayrıdır: parser'ın kendi
E2005/E0003'leri toplayıcıya gelene kadar 65 303 kopya olarak birikirdi
(fuzz hedefi yalnız parser'ı koşar). `expand_for` her yinelemeden sonra
kendi kuyruğunu (`from = mark`) katlar; bellek O(farklı tanı) kalır.

**C — Tanı yapısını değiştirmeden çıktı katmanında (render) katlamak.**
JSON/LSP tüketicileri ve `summary` sayıları yine 65 303 görür; bellek
çözülmez. Elendi.

### Çözüm metni kimliğe dahil mi?

Açılım gövde adlarını yeniden adlandırır (`t` → `t_0`, `t_1`) ve W2012'nin
çözümü bu üretilmiş adı taşır ("make it explicit by writing let t_0 :
i32 = ..."). Çözüm kimliğe dahil olsaydı 283 W2012 katlanmazdı. Karar:
çözüm ve öneriler kimliğe DAHİL DEĞİL; aynı konum + mesaj için ilk
kopyanın çözümü kalır (üretilmiş adın sızması ayrı bir kusurdur, bkz.
Sınırlar).

### Genel üst sınır (ADIM 4)

| Seçenek | Değerlendirme |
|---|---|
| A — Sınır yok (ADR-0067 kararı) | Katlama kök nedeni çözer; ama bilinmeyen sınıflar için 65 MB'lık terminal çıktısı ve 67 MB JSON'a karşı hiçbir güvence kalmaz. |
| B — Yüksek sınır (1000), `--max-diagnostics` ile değiştirilebilir (SEÇİLDİ) | Katlama sonrası sınıra yalnız FARKLI tanılar ulaşır; binin üstünde listeyi kimse okumaz. Clang varsayılan 20'de durur (`-ferror-limit`), GCC `-fmax-errors`; 1000 mevcut test külliyatının (en çok birkaç yüz tanı) çok üstündedir, `//~ ERROR` fixture'ları etkilenmez. Kapanış tanısı W0023 gizlenen sayıyı ve çözümü söyler — sessiz kayıp yok. |
| C — İnsan çıktısında düşük (50), JSON/LSP sınırsız | 67 MB JSON CI anotasyon araçlarını da boğar; iki ayrı davranış iki ayrı sözleşme demektir. Elendi. |

## Karar

### 1. Katlama (`fold_duplicates`, `Diagnostic::folded_ctxs`)

- `Identity` = kod + önem + mesaj + `[(dosya, başlangıç, bitiş, etiket,
  birincil)]` + notlar. Hash ve eşitlik tek tanımdan türer (mutasyon
  M5 bunu ister — yalnız `same_identity` mutasyonu hash tarafından
  gizleniyordu).
- İlk görülen kalır, sıra korunur; katlanan kopyanın birincil `ctx`'i ve
  kendi `folded_ctxs`'i kalana eklenir. `from` ile yalnız kuyruk katlanır.
- `Diagnostic` yeni alan `folded_ctxs: Vec<u16>`; kurucular boş başlatır,
  kimliğe dahil değildir. JSON şeması DEĞİŞMEZ: sayı not olarak taşınır.

### 2. Not metni (`annotate_generate`)

Katlanmış tanıya, kopyaların `ctx`'leri yineleme zincirine (değişken
başına `min..max`) ve köklerine ayrıştırılarak tek not yazılır:

```
= note: reported once; occurs in 283 unrolled 'for' iterations (i = 0..282) (ADR-0068)
= note: reported once; occurs in 8 generic instantiations (ADR-0068)
= note: reported once; occurs in 32 copies: unrolled 'for' iterations (i = 0..3) across 8 generic instantiations (ADR-0068)
= note: reported once; 257 identical occurrences (ADR-0068)
```

İç içe döngü `(i = 0..282, i = 0..230)` — ADR-0056'nın "y = 1, x = 2"
sırasıyla (dıştan içe) tutarlı. Katlanmamış tek kopya eski notunu korur
("in the unrolled 'for' iteration i = 2"). Kök tespiti için
`GenerateIter.parent` en dışta `for` deyiminin kendi `ctx`'ini taşır
(monomorf klonda klonun ctx'i; elle yazılmış modülde 0).

Farklı tanılar ayrı kalır: `for i in 0..5 { for j in i..1 {} }` üç ayrı
E2028 ("2..1", "3..1", "4..1"); `res[i]` taşması yalnız i = 2 ve 3'te →
iki ayrı E2006, her biri kendi yineleme notuyla.

### 3. Üst sınır (W0023, `--max-diagnostics`)

`compile()` çıkışında `cap_diagnostics`: sınır (varsayılan 1000, 0 =
sınırsız) aşılırsa hatalar önce (JSON zarfıyla aynı sıra), ilk N kalır,
sonuna **W0023** "too many diagnostics: 1000 shown, 1400 hidden" +
"--max-diagnostics=N" çözümü. `summary` gösterilen tanıları sayar; gizli
sayı W0023 mesajındadır. Tek yer: `check`, `build`, `test`, `run`,
`verify` hepsi `compile()`den geçer. Kod sayısı 133 → 134.

### 4. Açılım düğüm bütçesi (`MAX_UNROLL_NODES = 262 144`, E2027)

Modül başına açılımın ürettiği deyim + ifade düğümü; her yinelemeden
sonra denetlenir (E4010 ilkesi: sonradan saymak patlamayı önlemez).
Aşımda tek E2027 ("exceeded the AST node budget"), `budget_exhausted`
bayrağı kalan döngüleri açmaz (kaskad yok). 4096 yineleme × 64 düğümlük
gövde sığar; `examples/` ve test külliyatındaki hiçbir tasarım
bütçenin %10'una yaklaşmaz.

### 5. Kalıcı regresyon

`tests/fuzz_regressions/oom_nested_for_diag_flood_1157b.volt` — ham
artifact; `fuzz_regression_tests` onu 1 s sınırıyla ayrıştırır ve < 100
tanı ister. Sınıf başına regresyon: unroll (iç içe E2005 tek, 2499
katlanmış), mono (6 elle / 8 `for` içinden örnekleme), bundle dizisi
(`[Bus; 16]`), farklı-kalır testleri (E2028 × 3, E2006 × 2), düğüm bütçesi,
`--max-diagnostics` (JSON 1001 / 0 sınırsız / insan 5).

## Sonuçlar

| Ölçüm | Önce | Sonra |
|---|---|---|
| artifact, `volt check` süre | 6,3 s (14,5 s çıktı yakalanınca) | 0,27 s |
| tepe bellek | 347 MB | 73 MB |
| tanı sayısı | 65 365 | 63 |
| çıktı | 67 MB | 24 KB |
| `fuzz_regression_tests` (debug, 3 girdi) | — | 0,25 s |
| temiz 283 yinelemeli tasarım, gövdede 2 hata | 567 tanı | 3 tanı (283 kopya notta) |

- Golden: `tests/ui`, `tests/fixtures`, `examples` — 187 dosya, `check`
  insan + JSON, önce/sonra **sıfır fark**. Hiçbir fixture'ın tanısı
  değişmedi (külliyatta özdeş kopya üreten girdi yok).
- Davranış değişikliği: aynı hatanın açılmış kopyaları artık BİR tanıdır;
  `summary.errors` kopya değil hata sayar. ADR-0056'nın "her yineleme ayrı
  tanı" testi bu sözleşmeye güncellendi.
- Mutasyon: 5 mutasyon (unroll akış içi katlama, toplayıcı katlama, üst
  sınır, düğüm bütçesi, kimlikten mesaj) → 5/5 yakalandı.

## Sınırlar / Ertelenen

- **volt-lsp `annotate_generate` çağırmıyor** (bu ADR öncesinden):
  editörde ne yineleme notu ne katlama var — 65 303 tanı LSP'ye olduğu
  gibi gider. Kapsam dışı; ayrı iş (tek satırlık çağrı + LSP testi).
- **Üretilmiş ad sızması:** W2012 çözümü `t_0` diyor (kullanıcı `t` yazdı).
  Katlama bunu gizler, düzeltmez; typeck'in kaynak adı taşıması ayrı iş.
- **Farklı mesajlı kopyalar katlanmaz** — tasarım gereği (bilgi kaybı
  yok): `[Bus; 256]` × W1001 "unused input port: 'x_k_c'" 256 ayrı uyarı
  kalır (MAX_FLAT_PORTS 4096 ile sınırlı), yineleme değerini taşıyan
  E2006/E2028 ayrı kalır. Üst sınır (1000) bunları keser.
- **HIR katmanında akış içi katlama yok:** typeck'in ara `Vec`'i 65 535 ×
  gövde hatası kadar büyüyebilir (fuzz edilmiyor; AST'nin kendisi
  MAX_UNROLL_NODES ile sınırlı olduğundan tanı sayısı da sınırlı).
- Bütçe değeri (262 144) ölçümle değil, `MAX_UNROLL` × makul gövdeyle
  seçildi; gerçek bir tasarım aşarsa yükseltilir (E2027 açıkça söyler).
- Gecelik fuzz'ın 30 dakikayı bulgusuz tamamlaması merge sonrası elle
  tetiklenerek doğrulanacak; yeni bulgu ayrı ADR.
