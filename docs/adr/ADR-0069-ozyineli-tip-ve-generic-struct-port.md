# ADR-0069: Özyineli Tiplerin Tek Tip Çizgesi Denetimi ve Generic Struct Port Reddi

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-24
> Etkilenen: volt-syntax (`parser/type_graph.rs` — YENİ: tip çizgesi,
> yinelemeli Tarjan, E4009 ve generic struct port E0003 üretimi;
> `parser/bundle.rs` — `find_cycles`/`Cycles`/`err_recursive_bundle`
> kaldırıldı, `collect_bundle_defs` `recursive_types` kullanır;
> `parser/handshake.rs` — `err_recursive_payload` kaldırıldı, `PlainDefs.cyclic`
> `recursive_types`'tan; `parser/item.rs` — `finish_unit_desugar` başı;
> `parser/mod.rs` — `Parser::recursive_types`), volt-diagnostics (E4009 kısa
> açıklama + `volt explain` iki dilde genelleşti, E0003 `explain` notu),
> `tests/ui/fail/74-82`, `tests/ui/pass/91`,
> `crates/volt-syntax/tests/type_graph_tests.rs`.
> DOKUNULMADI: volt-hir, volt-sv-emit, volt-lsp, examples/, README.md, docs/spec/.
> Genişletir: ADR-0067 (E4009'un kapsamı; §"Sınırlar" ilk iki madde kapanır).

## Sorun

ADR-0067, fuzz'ın bulduğu bellek taşmasını kapatırken iki komşu açığı
kapsam dışı bıraktı (issue #21 ile izlendi):

1. **Sade özyineli struct**: `struct P { d : u8, f : P }` bir modül
   portunun tipi olarak `volt check`'ten hatasız geçiyor.
2. **Generic struct port**: `struct port G<T> { out d : T }` + `in g : G<u8>`
   + `o = g.d` hatasız geçiyor.

ADR-0067'nin dersi burada da geçerli: asıl hata bir sınır eksikliği değil,
bir **tanı** eksikliğidir — geçersiz ya da desteklenmeyen bir yapı sessizce
kabul ediliyor.

## Tespit

### Zincir boyunca davranış (düzeltme öncesi, `main` edc0e84)

| Aşama | Sade özyineli struct (`in a : P`) | Generic struct port (`in g : G<u8>`, `g.d`) |
|---|---|---|
| `volt check` | çıkış 0, yalnız W1001 | çıkış 0, hiç tanı yok |
| `volt build` | çıkış 1, **E0003** "SV mapping of user-defined types" (ilgisiz) | çıkış 1, **E0003** aynı ilgisiz metin |
| `build --emit=sva` | aynı E0003, çıktı yok | aynı |
| `build --emit=rust,c` | aynı E0003 (mmio yok) | aynı |
| `volt test` | derleme adımında aynı E0003 | aynı |
| LSP tanıları | yalnız W1001 | boş |
| LSP hover (`a`/`g`) | `a : struct` | `g : struct` |
| Süre / bellek / panik | 25 ms, < 10 MB, panik yok | aynı |

Yani iki bulgu da **patlamıyor**, **yanlış çıktı üretmiyor** (SV hiç
üretilmiyor) ama **anlamsız bir sonuçla kabul ediliyor**: kontrol ve editör
"temiz" der, derleme alakasız bir "henüz desteklenmiyor" hatasıyla kırılır.
Sonsuz tipin kendisi hiçbir yerde söylenmez.

### Sınıf taraması (31 örnek, `volt check`, düzeltme öncesi)

| Sınıf | Örnek | Önce | Sonra |
|---|---|---|---|
| Sade struct, doğrudan | `struct P { f : P }` (portta / kullanılmamış / `reg`) | **sessiz** (reg'de ilgisiz E2003) | E4009 |
| Karşılıklı | `A { b : B }`, `B { a : A }` | **sessiz** | E4009 × 2 |
| Dizi üzerinden | `struct S { x : [S; 4] }` | **sessiz** | E4009 |
| Demet üzerinden | `struct S { t : (u8, S) }` | **sessiz** | E4009 |
| Enum tuple varyantı | `enum E { A, V(E) }` | **sessiz** | E4009 |
| Enum struct varyantı | `enum E { A, V { e : E } }` | **sessiz** | E4009 |
| Enum temel tipi | `enum E : E { A }` | **sessiz** | E4009 |
| Enum ⇄ struct | `enum E { V(S) }`, `struct S { e : E }` | **sessiz** | E4009 × 2 |
| Takma ad, kendisi | `type T = T` | **sessiz** | E4009 |
| Takma ad, karşılıklı | `type T = U`, `type U = T` | **sessiz** | E4009 × 2 |
| Takma ad ⇄ struct | `type T = S`, `struct S { t : T }` | **sessiz** | E4009 × 2 |
| Takma ad, dizi | `type T = [T; 2]` | **sessiz** | E4009 |
| Generic, kendisi | `struct W<T> { x : W<T> }` (kullanılmış / kullanılmamış) | **sessiz** | E4009 |
| Generic, büyüyen | `struct W<T> { x : W<W<T>> }` | **sessiz** | E4009 |
| Generic argüman | `struct P { w : W<P> }`, `W<T> { x : T }` | **sessiz** | E4009 |
| Generic, sonlu | `W<W<u8>>` | temiz | temiz (doğru) |
| Handshake payload'ı, doğrudan | `Handshake<P>`, `P { f : P }` | E4009 (ADR-0067) | E4009 |
| Handshake payload'ı, dizi | `P { f : [P; 2] }` | **sessiz** (`a_data_f : [P; 2]` açıldı) | E4009, `data` opak |
| `Handshake<Handshake<u8>>` | iç içe | E1001 "undefined name" (özyineleme değil; metni zayıf) | aynı |
| struct port, dizi alanı | `struct port S { out x : [S; 4] }` | **sessiz** (`a_x : [S; 4]`) | E4009 |
| struct port ⇄ sade struct | `struct port A { out p : P }`, `P { a : A }` | **sessiz** | E4009 × 2 |
| Bundle içinde sade özyineli | `struct port B { out p : P }`, `P { f : P }` | **sessiz** | E4009 (P) |
| Bundle içinde generic sade | `struct port B { out w : W<u8> }` | temiz (build'de E0003) | aynı — kapsam dışı |
| Bundle içinde generic struct port | `struct port B { out g : G<u8> }` | **sessiz** | E0003 (G) |
| Generic struct port | kullanılmış / kendini içeren / kullanılmamış / `let` | **sessiz** (let'te ilgisiz E2003) | E0003 (+E4009 kendini içerende) |

31 örneğin **27'si sessizdi**; yalnız doğrudan Handshake payload'ı (E4009)
ve iç içe Handshake (E1001) tanılanıyordu; kalan ikisi (`W<W<u8>>`, bundle içinde
generic sade struct) geçerli tasarım.

**Patlama**: hiçbiri patlamadı (hepsi ~25 ms). Sebep: sade struct, enum ve
takma ad hiçbir yerde açılmıyor; generic struct'lar hiç örneklenmiyor (mono
yalnız modüllerde, yalnız const generic — ADR-0041). Açılan tek yollar
(bundle, Handshake payload'ı) ADR-0067 ile zaten sınırlıydı; dizi alanı orada
yaprak sayıldığı için sonsuz açılmıyordu. Bu yüzden ne E4010 ne
`MAX_UNROLL_NODES` ne de `fold_duplicates` burada devreye giriyordu —
sorun tamamen **tanı eksikliği** idi. Yan bulgu: ADR-0067'nin `find_cycles`'ı
büyük bir `struct port` halkasında süper-doğrusaldı (2000 halka: 0,79 s
release); yeni denetim doğrusal (0,05 s). Patlama sayılmaz, fuzz regresyonu
eklenmedi; ölçek testi `type_graph_tests.rs`'te.

## Karar 1 — Özyineli tipler: E4009 genelleşir, denetim tek yerde

**Anlam.** Her Volt tipi sabit genişlikli bir bit vektörüdür; port grubu
derleme zamanında düz portlara açılır. Kendini içeren tip (doğrudan,
karşılıklı, dizi/demet/enum payload'ı/takma ad/generic argüman üzerinden)
donanımda anlamsızdır ve **her biçimi hatadır** — kullanılmasa bile
(Rust'taki gibi; tanımın kendisi geçersiz).

**Kod: E4009 genelleşir, yeni kod açılmaz.** Seçenekler:

| Aday | Karar | Gerekçe |
|---|---|---|
| Yeni kod (tip çıkarımı ailesinde "özyineli tip") + E4009 yalnız bundle | Elendi | Aynı kural iki kodla söylenir; `struct port` ⇄ sade struct döngüsü hangi kodu alır belirsizleşir; iki denetim yeri kalır. |
| Yeni kod, E4009 emekliye | Elendi | Kod emekliye ayırmak (`explain`, fixture 72, fuzz regresyonu, CODE_COUNT) kazanç getirmeden kırılganlık üretir. |
| **E4009'u "özyineli tip"e genişletmek** | **SEÇİLDİ** | Tek kural, tek kod, tek `volt explain` sayfası; ADR-0067 fixture'ı ve fuzz regresyonu aynen geçer. Kod ailesi (E4xxx bağlantı/bundle) tarihseldir; kısa açıklama artık "Recursive type". |

**Yer: parser birim sonu desugar'ı, tek geçit (`type_graph.rs`).**
ADR-0067'de iki ayrı döngü araması vardı (bundle: yalnız `struct port`;
Handshake: yalnız sade struct, yalnız argümansız tek segmentli yol). İkisi
de kaldırıldı; `finish_unit_desugar`'ın ilk adımı `check_type_graph` bütün
adlandırılmış tipleri tek çizgede denetler ve döngüye **ulaşan** adları
`Parser::recursive_types`'a yazar. Bundle ve Handshake açılımı yalnız bu
kümeye bakar (kendi aramaları yok). HIR'da değil, çünkü:

- Açılım (bundle, Handshake) parser'da koşar; döngü bilgisi açılımdan ÖNCE
  gerekir (ADR-0067 Seçenek C ile aynı gerekçe). HIR'da ikinci bir denetim
  aynı kuralı ikinci kez yazmak olurdu.
- Fuzz hedefi yalnız parser'ı koşar; tanı orada olunca fuzz da görür.
- Birim tek kök kapsamdır (ADR-0042): çok dosyalı birimde çizge bütün
  dosyaları kapsar (`parse_unit` sonrası tek geçit; test var).

**Çizge.** Düğüm: `struct`, `struct port`, `enum`, `type`. Üye: alan,
varyant (tuple ya da struct payload'ı), enum temel tipi, takma ad hedefi.
Kenar: üye tipinde geçen her bildirilmiş ad; diziler, demetler ve generic
argümanlar derinlemesine izlenir. Kesinlik kuralları:

- Tipin kendi generic parametreleri kenar değildir (`struct W<T> { x : T }`
  içindeki `T`, aynı adlı bir üst düzey tip olsa bile).
- Generic bir tipin argümanı ancak o parametre tipin üyelerinde geçiyorsa
  kenardır: `Box<P>` → `P` yalnız `struct Box<T> { v : T }` ise;
  `struct Tag<T> { v : u8 }` için `Tag<P>` sonludur (yanlış pozitif yok).
  Bilinmeyen/yerleşik tiplerin (`Handshake<P>`) argümanları hep izlenir.
- Aynı ad iki kez bildirilmişse sonuncusu (ADR-0067 ile aynı kural; yinelenen
  ad tanısı çözümleyicide).

**Algoritma.** Yinelemeli Tarjan (O(V+E), derin zincirde yığın taşmaz —
20 000'lik zincir testi), ulaşma kümesi ters kenarlarda BFS. Döngüdeki düğüm:
bileşeni > 1 ya da öz-döngülü. ADR-0067'nin O(n²) `find_cycles`'ının yerini
alır.

**Tanı biçimi** (ADR-0067 ile aynı iskelet, deterministik — bildirim
sırası):

- Döngüdeki **her** tip bir E4009 alır; döngüye yalnız ulaşan tip almaz
  (kaskad yok) ama açılmaz da.
- Birincil etiket tip adında ("recursive type"), ikincil etiket döngüyü
  kapatan ilk üyede ("this field / variant / type closes the cycle").
- Not: döngü yolu, `cycle: A.b → B.c → C.a → A` (bileşen içinde en kısa
  yol, BFS bildirim sırasıyla). Bileşen 64 tipten büyükse yalnız
  `cycle: N types` — dev bir halkada her düğüm için BFS karesel olurdu.
- Mesaj türe göre: `struct port 'Req' contains itself (field 'req' leads
  back to 'Req')`, `enum 'E' … (variant 'V' …)`, `type alias 'T' … (the
  target type …)`, `enum 'G' … (the base type …)`.
- ADR-0067'nin Handshake'e özgü port tanısı ("the payload of Handshake port
  'x' is a recursive struct") kaldırıldı: struct'ın kendisi raporlanır, port
  `data` opak kalır — aynı hata için ikinci tanı kaskaddır.

**ADR-0068 güvenceleriyle ilişki.** Özyineli tip hiçbir açılımdan geçmez:
çizge denetimi açılımlardan önce koşar ve döngüye ulaşan tipler açılmaz.
Bu yüzden E4010 bütçesi, `MAX_UNROLL_NODES` ve `MAX_NESTING` bu sınıf için
tetiklenmez — onları **dil kuralı** değil, döngüsüz ama büyük çizgeler için
**güvence** olarak bırakıyoruz. Tanılar farklı adlar taşıdığından
`fold_duplicates` birleştirmez (doğru: farklı tipler); sayıları tip sayısıyla
sınırlıdır ve `--max-diagnostics` sürücüde geçerlidir.

## Karar 2 — Generic struct port: açık E0003, destek yok

**Mekanizma var mı?** Yok. Bundle düzleştirmesi (ADR-0039) yalnız generic
olmayan `struct port`'ları açar; ADR-0041 monomorfizasyonu yalnız
modüllerde ve yalnız const generic'tir — tip parametresine argüman E0003'tür
("type generic arguments on user modules are not supported yet"). Dilde
**tip parametresi ikamesi hiçbir yerde yok**.

**Destek küçük mü?** Hayır. Gerekenler: alan tiplerinde `T` → argüman
ikamesi (dizi uzunluğu, `bits<W>` gibi const ifadeleri için ifade klonlama),
iç içe generic bundle'lar, Handshake payload'ında generic struct'lar, bundle
dizileri ve E4005/E3013 köken bilgisi; HIR'da generic struct tipinin alan
tipi çözümü. Bu, ADR-0041'in "tip generic'leri ileride" kararını tek bir
yapı için öne çekmek olur; ayrı bir ADR'nin konusu.

**Karar: bildirimde E0003**, mevcut "henüz desteklenmiyor" sözleşmesiyle:
`generic struct ports are not supported yet ('G')`, birincil etiket
struct adında, çözüm "her somut tip için ayrı, generic olmayan bir struct
port bildirin", not ADR-0041/0069. Bildirimde (kullanımda değil), çünkü:

- Kullanım biçimleri çok (port, bundle dizisi, başka bundle'ın alanı,
  `let` tipi); bildirimde tek tanı hepsini kapsar, kaskad üretmez.
- Kullanılamayan bir bildirim de yanıltıcıdır; const generic
  (`struct port B<const W: u32>`) dahil.

Sade generic struct (`struct W<T>`) bu kararın dışında: portta kullanılırsa
bütün kullanıcı tanımlı tipler gibi `build`'de E0003 alır; bildirimi
sonlu olduğu sürece geçerlidir. Sessiz kabul yalnız `struct port` içindi
(parser açmıyor, HIR `g.d`'yi hata tipine çözüp susuyordu).

## Sonuçlar

- Özyineli tiplerin hiçbir biçimi sessiz geçmiyor (27/27 sessiz örnek artık
  E4009/E0003); generic struct port açık E0003.
- Geçerli tasarımlar değişmedi: 187 dosyalık golden (`tests/ui`,
  `tests/fixtures`, `examples` — `check` insan + JSON, `build --emit=sva`
  çıktı dosyası hash'leri) PR #23 (`main` edc0e84) ile birebir; tek fark
  geçersiz fixture 72'nin E4009 metni (etiket, not, yardım).
- Davranış değişikliği: özyineli sade struct / enum / takma ad ve generic
  struct port bildirimi artık `volt check`'te hatadır (eskiden `build`'de
  ilgisiz E0003 ile düşüyordu ya da kullanılmıyorsa hiç düşmüyordu).
  Örneklerde ve testlerde kullanan yok.
- Kod sayısı değişmedi (134).

## Sınırlar / Ertelenen

- `Handshake<Handshake<u8>>` E1001 "undefined name: 'Handshake'" alıyor —
  özyineleme değil (iç içe yerleşik bundle), tanı var ama metni yanıltıcı.
  Ayrı iş.
- LSP hover struct tipli portu `a : struct` diye gösteriyor (ad yok);
  `Ty::display` bilinen sınırı, ayrı iş.
- Yalnız tek segmentli tip yolları çizgeye girer (birim tek kök kapsam;
  çok segmentli tip yolu bugün çözümlenmiyor).
- Döngü yolu notu 64 tipten büyük bileşende yalnız boyutu söyler.
