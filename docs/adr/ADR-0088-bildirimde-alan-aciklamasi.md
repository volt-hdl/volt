# ADR-0088: Sinyal Bildirimlerinde Saat Alanı Açıklaması

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-27
> Etkilenen: volt-ast (`WireDecl::domain`, `LetDecl::domain`), volt-syntax
> (`parser/stmt.rs` — `wire`/`let`/`reg`, `parse_domain_annot` ortak,
> `mono/clone.rs`), volt-hir (`resolve/stmt.rs`, `resolve/block.rs`,
> `domain/walk.rs`); spec: grammar-full.ebnf, ast-nodes.md,
> domain-inference.md K1; `templates/cdc`
> Kaynak: ADR-0084 §5 bulgu 3

## Sorun

`wire x : bool @Slow` → **W0020 "unknown attribute: '@Slow'"**. Port
alan yazabiliyor (`in d : u8 @Fast`), tel yazamıyor. Ölçüm (sonda
`build/c4/gen.py`, main 9fb467e):

| Yazım | Önce |
|---|---|
| `wire s : bool @Slow` | W0020 — `@Slow` bir sonraki satırın niteliği sayılıp **atılıyor**; hiçbir şey denetlenmiyor |
| `wire s : bool @Nope` (yazım hatası) | yine yalnız W0020 — **tanımsız alan adı sessiz** |
| `let s : bool @Slow = e` | E0001 "expected '='" + W0020 |
| `reg r : bool @Slow = false` | E0001 + W0020 |
| blok `let t : bool @Slow = b` | E0001 + E0006 |
| struct alanı `a : bool @Fast` | E2013 (ADR-0077) |
| fn parametresi `x: bool @Fast` | E0001 |

Kullanıcı tel alanını yazdığını ve denetlendiğini sanıyordu; açıklama
uyarıyla atılıyordu (tanımsız alan adı dahil).

## Karar

**Desteklenir: `wire`, `let` (modül ve blok) ve `reg` bildirimleri
`@Alan` taşıyabilir; açıklama DENETLENEN bir niyet beyanıdır** — tip
açıklamasıyla (`let x : u8 = e`) aynı mantık: çıkarım yine yapılır,
açıklama sinyalin alanını sabitler, çelişki hatadır.

| Bildirim | `@Alan` | Anlam | Çelişki |
|---|---|---|---|
| port | ✓ (değişmedi) | arayüz sözleşmesi | — |
| `wire x : T @A` | ✓ **yeni** | tel `A` alanında; sürücüleri K6 ile denetlenir | E3001 |
| `let x : T @A = e` (modül) | ✓ **yeni** | `e`'nin alanı `A` olmalı (sabit her alana uyar) | E3001 |
| `let x : T @A = e` (blok, `on`/`comb`) | ✓ **yeni** | aynı | E3001 |
| `reg x : T @A = v` | ✓ **yeni** | ≡ `reg(<A'nın saati>)`; yazan `on` bloğu `A`'da olmalı (K7) | E3001 |
| `reg(clk) x : T @A` | ✗ | iki açıklama | **E0001** "two domain annotations" |
| bundle (port struct) alanı | ✓ (ADR-0039, değişmedi) | | |
| düz struct alanı | ✗ E2013 (ADR-0077, değişmedi) | struct değer tipidir, alanı yok | |
| fn parametresi / dönüş | ✗ E0001 (değişmedi) | fn saf ve alandan bağımsızdır (ADR-0081) | |
| tanımsız alan adı | — | **E3002** (önce W0020 ile sessiz) | |

Gerekçe:

- **Sessiz atılan açıklama en kötü seçenek.** "Yalnız portta"
  kararıyla bile W0020 yerine açık bir hata gerekirdi; desteklemek
  aynı maliyette daha değerli: CDC geçiş noktası kaynakta görünür ve
  denetlenir (`wire synced : bool @Slow` + `sync()`).
- **Tip açıklamasıyla tutarlı:** Volt yerel tipleri çıkarır ama
  yazdırır da; alan ikinci tip boyutudur (domain-inference.md §1).
- **Hata kodu E3001:** açıklama ile değerin alanı çelişkisi bir CDC
  ihlalidir — ileti ve öneri (`sync()`/AsyncFifo) aynen geçerli. Yeni
  kod açılmadı. Çelişen `let` bir kez raporlanır; kullanımları kaskad
  üretmez (bağlamanın alanı hata alanına düşer).
- **`reg x @A` ≡ `reg(clk)`** — mevcut K1 yolu (`RegDecl::domain`)
  alan adı da kabul ediyordu; ikisini birden yazmak hangisinin geçerli
  olduğunu belirsiz bırakır.
- Ayrıştırma portla aynı (`parse_domain_annot`): `@ad(` biçimi ve
  tanınan nitelik adları sonraki öğenin niteliği kalır (`@max_fanout(4)`
  bir teli izleyen satırda yine nitelik).
- **SV değişmez:** açıklama yalnız alan çıkarımına girer (test: aynı
  tasarım açıklamalı/açıklamasız bayt aynı SV).

## Doğrulama

- `domain_annot_tests.rs`: ui/fail 185 (tel yabancı sürücü E3001), 186
  (let E3001), 187 (tanımsız alan E3002 — önce sessiz W0020), 188
  (`reg(clk)` + `@A` E0001) işaretli satırda TEK tanı; ui/pass 130
  (tel + `sync()`, modül/blok `let`, `reg`) temiz ve SV açıklamasızıyla
  bayt aynı; Verilator `-Wall` temiz.
- Sonda tablosu `build/c4/gen.py` (17 biçim, önce/sonra).
- Mutasyon (`build/c4/mutate.py`, D1–D4): tel açıklamasını yok saymak,
  let değerini denetlememek, teli ayrıştırmamak, alan adını çözmemek —
  hepsi düştü.
- `templates/cdc`: geçiş teli `wire toggle_s : bool @Slow` (şablon
  zinciri — check/build uyarısız, test, verify — geçti).

## Sonuçlar

- (+) CDC geçiş noktaları ve niyet kaynakta, derleyici denetiminde.
- (+) Yazım hatalı alan adı artık hata (E3002), sessiz değil.
- (−) `@ad` telden sonraki satırda PARANTEZSİZ ve bilinmeyen bir nitelik
  olarak kastedildiyse artık alan adı sayılır (E3002) — port için zaten
  böyleydi.
