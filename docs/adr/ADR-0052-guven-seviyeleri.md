# ADR-0052: Güven Seviyeleri — `trust_level`, Bilgi Akışı Denetimi (E3009) ve `declassify`

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-15
> Etkilenen: volt-ast (`DomainKey::TrustLevel`, `DomainValue::Trust`,
> `TrustLevel`, `TrustInfo` yan tablosu), volt-syntax (token.rs üç ayrılmış
> kelime serbest, item.rs `trust_level =` değeri, expr.rs `declassify`
> soyma + E0016), volt-hir (domain.rs `DomainInfo::trust`, K11 takma adı,
> K8 anahtar; trust.rs YENİ; lib.rs geçit), volt-driver (bir satır: geçit
> çağrısı), volt-diagnostics (E0016, W3008, E3009 artık V1 değil; explain
> en/tr), docs/spec (grammar-full.ebnf, domain-inference.md §1/K11/§5),
> examples/crypto (YENİ), tests/ui/pass/70-71, tests/ui/fail/55-56.
> DOKUNULMADI: crates/volt-sv-emit, README.md.

## Sorun

Bütünleşik mimari (v3 §2.5) domain'in dördüncü yüzü olarak güven
seviyesini tanımlamıştı:

```volt
domain SecureDomain {
    clock       = posedge
    reset       = sync active_high
    trust_level = secret        // secret | confidential | public
}
```

Durum tespiti (bu ADR'nin 1. adımı):

- Parser `trust_level` anahtarını tanımıyordu: `domain` gövdesinde
  **W0020 "bilinmeyen domain anahtarı"** üretiyordu; `secret`,
  `confidential`, `public` ise lexer'da AYRILMIŞ kelimeydi (ADR-0028
  listesi) ve her kullanımda **E0003** veriyordu. Yani mimari belgedeki
  örnek derlenemiyordu.
- `DomainKey::TrustLevel` yoktu; `DomainInfo`'da güven alanı yoktu
  (spec §1 `pub trust: TrustLevel` yazıyordu ama uygulanmamıştı).
- **E3009** yalnız tanı tablolarında yaşıyordu (`code.rs`, `messages`,
  `explain` — "[V1] … F-serisinde etkin değildir" notuyla); hiçbir
  analiz geçidi üretmiyordu. Saat (E3001/E3010/E3011/E3012), sıfırlama
  ve güç için hazır olan domain altyapısı güven için hiç kullanılmıyordu.

## Karar

### 1. Güven kafesi ve akış kuralı

`public < confidential < secret`. Bilgi yalnız **eşit ya da daha yüksek**
seviyeye akabilir:

| Akış | Sonuç |
|---|---|
| secret → confidential, secret → public, confidential → public | **E3009** |
| public → confidential, public → secret, confidential → secret | serbest |
| aynı seviye | serbest |
| sabit (Timeless) → her seviye | serbest |

### 2. Sinyalin güven seviyesi = alanının seviyesi (saatle aynı mekanizma)

Sinerji ilkesi (mimari §2.5: "ayrı sistem değil") olduğu gibi uygulandı.
Güven geçidi (`trust.rs`) saat çıkarımından SONRA koşar ve **onun
sonucunu** kullanır: `DomainResult::signal_domains` üzerinden her
sinyalin alanı, `DomainInfo::trust` üzerinden alanın seviyesi.

| Saat kuralı | Güven eşleniği |
|---|---|
| K1 açık anotasyon | `@Ad` bildirimi trust_level taşıyorsa o seviye; `@clk` ise o portun alanı |
| K2 tek saat | anotasyonsuz port/wire modülün alanının seviyesini alır (`in clk : clock @SecureCore` → her şey secret) |
| K4 register | `reg(X)` → X'in seviyesi; yoksa yazıcı `on` bloğunun alanı |
| K5 yayılım | ifade operandlarının **en yükseğini** taşır (`join` = max); tanık: seviyeyi taşıyan ilk alt ifade |
| K6 atama | hedef seviye < kaynak seviye → E3009 (sağ taraf ⊔ dal koşulu ⊔ hedef indeksleri) |
| K7 `on` bloğu | `if`/`match` koşulu **örtük akıştır**: dalın içindeki her yazma koşulun seviyesini taşır (`if key[0] { dbg <= 1 }` = `dbg = key[0]`) |
| K8 örnekleme | giriş portu: bağlanan değer ≤ portun seviyesi; `inst.out` okuması portun seviyesi; sınıflandırılmamış çıkış → **tutucu özet**: örneğin sınıflandırılmış girişlerinin en yükseği ⊔ hedef modülün kendi çıkarımı |
| K9 `sync()` | saati değiştirir, **etiketi korur**: `sync(secret_flag, slow_clk)` public alana E3009 |

**Sınıflandırılmamış sinyaller.** Alanında `trust_level` olmayan sinyal
(anotasyonsuz `clk`'nın örtük alanı dahil) bir seviye TAŞIMAZ; kendisine
yazılan verinin **en yüksek seviyesini alır** (sabit nokta, en çok
kafes yüksekliği kadar tur). Böylece `reg tmp; tmp <= key; dbg = tmp`
bir sırrı aklayamaz — ve dosyada hiç trust_level yoksa hiçbir sinyalin
etiketi olmaz, hiçbir tanı çıkmaz.

**Kontratlar** gözlemdir, akış değil: denetlenmez (bkz. §5).

### 3. K11 — trust_level'lı anotasyon saat alanı AÇMAZ

Bu ADR'nin asıl tasarım kararı. Domain sistemi bugüne kadar "domain =
saat alanı" idi: `in key : u8 @SecureCore` bir `@SecureCore` clock portu
olmadan modülde ayrı bir saat alanı sayılır, `@Debug` çıkışa her atama
E3001 (CDC) olurdu. Mimari örneği ise **tek saatli** bir modülde secret
giriş + public çıkış istiyor; `debug_out = key[7:0]`'ın CDC hatası değil
E3009 olması gerekir, `busy = declassify(...)`'ın da `sync()`suz
geçmesi.

Kural: `@Ad` anotasyonu, `Ad` **trust_level taşıyan** bir domain
bildirimiyse ve **bu modülün hiçbir clock portu onu taşımıyorsa**:

- tek saatli (ya da saatsiz) modülde sinyal saat boyutunda modülün alanında
  kalır (K2 / Timeless); anotasyon yalnız güven boyutunu belirler;
- çoklu saatli modülde hangi saate ait olduğu belirsizdir → **E3010**
  (aday listesiyle) — trust'lı anotasyon saat söylemez;
- bir clock portu onu taşıyorsa (`in dclk : clock @Debug`) gerçek saat
  alanıdır, E3001 aynen korunur;
- örneklemede (K8) hedef modülde clock portu taşımayan trust'lı anotasyon
  saat anahtarı değildir; port hedefin tek saatinden bağlanır.

**Geriye uyumluluk:** kural yalnız `trust_level` yazılmış bildirimlere
uygulanır. trust_level'sız `@Other` anotasyonu eskisi gibi ayrı saat
alanıdır (test: `annotation_without_trust_level_keeps_old_behaviour`).
Hiç trust_level ve declassify içermeyen dosyada geçit hiç koşmaz.

### 4. `declassify(expr, "gerekçe")` — tek meşru düşürme

- Sonuç **public**; gerekçe ZORUNLU, boş olmayan dize literali; eksikse
  **E0016** (parser hatası, uyarı değil).
- Her çağrı **W3008** "bilinçli güven düşürme" uyarısı: mesajda gerekçe,
  birincil etikette kaynak seviye (`@SecureCore (secret) → public here`),
  gerekçe literaline ikincil etiket. Uyarılar, sınıflandırılmış bilginin
  bilinçli olarak açıklandığı yerlerin **eksiksiz listesidir** — güvenlik
  incelemesi derleyici çıktısını okumaya iner.
- **Tip seviyesinde soyulur** (ADR-0037 `delay<K>(x)` kalıbı): parser
  çağrıyı ayrıştırır, AST'de yalnız iç ifade yaşar, kayıt
  `SourceFile::trust.declassify` yan tablosuna düşer. volt-sv-emit hiç
  görmez; üretilen SV değişmez (görev kısıtı: SV üretimi dokunulmadı).
- Bağlamsaldır: yalnız `declassify(` biçiminde; tek başına `declassify`
  sıradan bir isimdir. Yalnız iç ifadeyi kapsar: `declassify(a, "r") & key`
  içindeki `key` hâlâ secret.

### 5. Kontrat üretimi: HAYIR — tip sistemi yeterli

Görev sorusu: "invariant: debug_out ifadesi secret içermez" otomatik
üretilsin mi? **Üretilmez.** Gerekçe:

1. "Sızıntı yok" bir **izleme (trace) özelliği değil, iki-izli
   (hyperproperty, non-interference)** özelliğidir: tek bir yürütme
   üzerinde SVA `assert property` ile ifade edilemez; iki kopya ve
   girişleri eşitleyen bir kurulum gerekir (self-composition) — bu, üretilen
   SV'yi ve formal sarmalayıcıyı değiştirmek demektir, görev kısıtına
   aykırı.
2. Tip seviyesi denetim **daha güçlüdür**: sabit nokta bütün akışları
   statik olarak kapsar, BMC derinliğine bağlı değildir, ve
   `declassify` noktalarını da (W3008) iz kaydı olarak listeler.
3. Kontrat, kullanıcı isterse yine yazılabilir (ör. `invariant: !key_valid
   -> key == 0`) — davranışsal bir özellik olarak; güven boyutuyla
   ilgisi yok.

### 6. Ayrılmış kelimeler

`secret`, `confidential`, `public` ADR-0028 ayrılmış listesinden
ÇIKARILDI; ADR-0023 kalıbıyla **bağlamsal**: yalnız `trust_level =`
değer konumunda seviyedir, başka her yerde Ident (`in public : bool`
geçerli). `declassify` de bağlamsal (yalnız `(` izliyorsa).

## Hata mesajı (5 parça)

```
error[E3009]: secret data flows to a public output
   ┌─ crypto.volt:92:5
   │
20 │     trust_level = secret
   │     -------------------- source trust level here
26 │     trust_level = public
   │     -------------------- destination trust level here
92 │     debug_out = key_r[7:0] as u8
   │     ^^^^^^^^^   ----- @SecureCore (secret)
   │     @Debug (public)
   = reason: information from a higher trust level cannot reach a lower one; this could leak key material (ADR-0052)
   = help: if intentional, use declassify(expr, "reason")
   = for more: volt explain E3009
```

Hedef türü mesajda adlandırılır: output / register / wire / port.

## Sınırlar

- **Örnek özeti tutucudur**: sınıflandırılmamış bir alt modülün çıkışı,
  o örneğin sınıflandırılmış girişlerinin en yükseğini taşır (port→port
  bağımlılık özeti yok). Bir toplayıcının `sum`'u, `a` secret ise
  `b`'den bağımsız olarak secret'tır. Kesin özet (çıkış → giriş kümesi)
  sonraki iş.
- **Çoklu saat + trust-only anotasyon** E3010'dur; kullanıcı güven
  bölgesini bir clock portunda taşımalı (`in dclk : clock @Debug`).
- Yalnız iki kaynak seviyesi ve bir hedefin en yüksek tanığı raporlanır
  (ilk tanık); aynı atamada birden çok gizli operand tek E3009.
- Extern modül portları yalnız anotasyon (ya da tek clock portunun
  anotasyonu) ile sınıflandırılır; gövdesi yok, çıkarım yok.
- `sync()` etiketi korur ama `AsyncFifo` gibi yerleşik primitifler tutucu
  özetle geçer (bütün çıkışlar bütün girişlerin en yükseği).
- Zamanlama/güç yan kanalları (`@constant_time`, `@no_power_leak`) bu
  ADR'nin kapsamı dışında; W0021 ile "uygulanmıyor" der (ADR-0048).

## Sonuçlar

- `examples/crypto/key_store.volt`: AES anahtar kaydı + yükleme durum
  makinesi, iki domain tek saat; `busy` / `key_valid` / `debug_out` üç
  `declassify` = 3 W3008; `debug_out = key_r[7:0] as u8` → E3009 (yorumda
  bırakıldı, README'de gösterildi). `volt build` 65 satır SV, Verilator
  `-Wall` temiz, `bmc 12` / `prove 3` / `cover 12` 6/6.
- Yeni kodlar: **E0016** (gerekçesiz declassify), **W3008**; **E3009**
  artık V1 değil. Toplam 118 kod, `volt explain` iki dilde.
- Testler: volt-syntax +10 (trust_level 3 seviye / tek başına / kötü
  değer, bağlamsal kelimeler, declassify soyma + yan tablo, E0016 üç
  biçim, öncelik, çıplak isim), volt-hir `trust_tests.rs` +41 (kafes,
  yayılım, register/wire, sınıflandırılmamış çıkarım + sabit nokta,
  örtük akış, indeks sızıntısı, declassify/W3008, sync, K11 üç durum +
  geriye uyumluluk, K8 dört durum, tanı biçimi), ui +4 (pass/70-71,
  fail/55-56), explain +2, ui sayaçları 60.
- Geriye uyumluluk: trust_level yazılmayan tüm mevcut fixture'lar ve
  örnekler değişmeden geçer; geçit trust_level/declassify yoksa erken döner.
