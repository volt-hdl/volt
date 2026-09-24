# ADR-0047: Extern Modül Sınırında Domain Anotasyonu — Sembolik Saat Alanları

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-14
> Etkilenen: domain-inference.md K8a/§5/§8, name-resolution.md §2
> (DefKind::DomainParam), volt-hir (resolve.rs extern gövdesi +
> DomainParam, typeck.rs extern port tipleri, domain.rs K8 extern +
> E3014), volt-diagnostics (E3014, extern biçimli E3002),
> volt-lsp (hover.rs), volt-syntax (parser/bundle.rs anotasyon span'i),
> examples/vga/README.md, tests/ui 62/49/50

## Sorun

`examples/vga/` keşfi (rapor 8g): AsyncFifo gibi çift alanlı
primitifler `extern module` ile sarmalandığında CDC analizi extern
sınırında duruyordu. Durum tespiti (bu ADR'nin 1. adımı) üç kat derin
bir boşluk buldu:

1. **İsim çözümleme** extern portlarını hiç bildirmiyordu: `ItemKind::
   Extern` dalı yalnız port TİPLERİNİ çözüyor, portlara `DefId`
   vermiyor, `@Ad` anotasyonlarına hiç bakmıyordu. `@Src` gibi
   tanımsız bir alan E3002 bile üretmiyor, sessizce yutuluyordu.
2. **Tip kontrolü** yalnız `ItemKind::Module` üzerinde koşuyordu;
   extern portlarının `def_types` kaydı yoktu. Domain çıkarımı saat
   portlarını `Ty::Clock` üzerinden tanıdığından extern'in hiçbir portu
   saat sayılmıyordu.
3. **K8 haritası** (`check_instance`) bu yüzden boş kalıyordu:
   `port_domain_key` her extern portu için `None` → beklenen alan
   Timeless → `check_compat` hiçbir şey denetlemiyor. `wr_data`'ya
   PixDomain sinyali bağlamak SESSİZ geçiyordu.

Ayrıca volt-sv-emit extern örneğini üretemiyor ("SV generation of
module instance ... target module is not in this file", E0003);
yani extern bugün uçtan uca kullanılamıyor. Bu ADR yalnız analiz
katmanını kapsar; sv-emit kapsam dışıdır (bkz. "Sınırlar").

## Karar

### 1. Yüzey sözdizimi — gramer DEĞİŞMİYOR

```volt
extern module AsyncFifo {
    in  wr_clk   : clock @Src
    in  wr_data  : u8    @Src
    in  wr_en    : bool  @Src
    out wr_full  : bool  @Src

    in  rd_clk   : clock @Dst
    out rd_data  : u8    @Dst
    in  rd_en    : bool  @Dst
    out rd_empty : bool  @Dst
}
```

`@Src` ve `@Dst` **sembolik saat alanı parametreleridir**: hiçbir yerde
`domain Src { ... }` yoktur. Her örneklemede saat bağlantısı onları
gerçek alanlara bağlar; diğer portlar o haritaya göre denetlenir.

### 2. Sembolik alan bildirimi: ÖRTÜK (seçenek A)

İki seçenek değerlendirildi:

| | A) Örtük: extern içinde tanımsız `@X` parametredir | B) Açık: `extern module AsyncFifo<Src, Dst> { }` |
|---|---|---|
| Gramer | değişmez | `GenericParamKind::Domain` + `<...>` anlamı genişler |
| `<...>` ile çakışma | yok | `<T, const N>` tip/sabit generic'leri zaten burada; örnekleme `AsyncFifo<u14, 16> { }` bunları KONUMSAL geçer, domain argümanı ise hiç geçilmez (saat bağlantısından çıkar) → hayalet parametre, arity özel durumu |
| Yazım hatası (`@Sr`) | yeni sembolik alan açılır AMA onu taşıyan clock portu olmadığından **E3002** ile yakalanır | `<Src>` listesinde olmayan `@Sr` E3002 |
| Modüllerle tutarlılık | modül portları zaten `@clk_adı` ile clock portuna işaret edebiliyor; extern'de de aynı yol açık | ek kavram |
| Uygulama | resolve.rs'te tek dal; K8 kodu modülle ORTAK | ast + parser + resolve + mono etkileşimi |

**Seçim: A.** Daha az karmaşık, gramer dokunulmuyor, K8 kod yolu
modülle paylaşılıyor. Yazım hatası güvenliği "sembolik alan en az bir
clock portunda taşınmalı" kuralıyla korunuyor: `@Sr` yazan kullanıcı
E3002 alır ("sembolik alanın clock portu yok"), çünkü hiçbir clock
portu `@Sr` taşımaz. Bu kural olmadan A tehlikeli olurdu; kuralla
birlikte B'nin tek avantajı (açık liste) kalmıyor.

Çözümleme kuralı (`resolve_symbolic_domain_ref`):

1. `@Ad` kapsam zincirinde görünür bir isme çözülüyorsa olağan
   `resolve_domain_ref` (domain tanımı, extern'in kendi clock portu ya
   da daha önce açılmış sembolik alan).
2. Görünmüyorsa extern kapsamında `DefKind::DomainParam` açılır; ilk
   geçiş `use_spans`'e yazılır (`decl_spans`'e YAZILMAZ: bundle
   düzleştirmesi anotasyon span'ini port adı span'iyle paylaşır),
   sonraki `@Ad`'lar aynı tanıma çözülür.
3. Sembolik alan **yalnız `extern module` içinde** açılır; sıradan
   modülde tanımsız `@Ad` E3002 olmaya devam eder.
4. Görünür isim yalnız **alan anlamı taşıyorsa** (domain, sembolik alan,
   clock portu, import) 1. kuralı tetikler; kök kapsamdaki `struct Src`
   ya da `const Src` sembolik `@Src`'yi gölgelemez. Aksi halde başka bir
   dosyadan `use`'la gelen bir isim daha önce derlenen extern'i bozar ve
   beklenen alan Timeless'a düşüp denetim sessizce kapanırdı.
5. Bundle (ADR-0039) düzleştirmesi alan anotasyonunun span'ini de port
   başına benzersizleştirir; aynı `struct port`'u kullanan iki extern'in
   sembolik alan tanımları birbirini ezmez.

### 3. Extern bildirimi denetimleri (domain.rs `check_extern_decl`)

| Durum | Sonuç |
|---|---|
| Sembolik alan hiçbir clock portunda değil | **E3002** (extern biçimi): "symbolic domain '@Dst' has no clock port in extern module 'X'"; öneri `in dst_clk : clock @Dst` |
| ≥2 clock portu, saat dışı port anotasyonsuz | **E3010** (K3 sınırda), adaylar clock portları |
| Tek clock portu, anotasyonsuz portlar | K2: o saatin alanı |
| Clock portu yok | Timeless (kombinasyonel extern) |
| Aynı port adı iki kez | E1003 (portlar artık tanım) |
| `@port` clock tipinde değil | E3002 (modülle aynı metin) |
| Port adı kök kapsamdaki bir öğeyle aynı | uyarı YOK (W1002 bilerek üretilmez: gövdesiz extern'de gölgeleme karışıklık yaratamaz) |

Extern portları W1001 (kullanılmayan giriş) ÜRETMEZ: dış SV modülüne
aittir; `ScopeKind::Extern` kullanım raporunda atlanır.

### 4. K8 extern'e uygulanır (domain.rs `check_instance`)

Modülle aynı kod yolu; tek fark anahtarın sembolik olabilmesi:

1. Saat bağlamalarından harita: `@Src := domain(sys_clk)`.
2. Diğer portlar haritaya göre `check_compat` → ihlal **E3001**
   (mevcut atama biçimi, beklenen/gerçek alan etiketleri).
3. `instance_ports` doldurulur → `f.rd_data` okuması `@Dst`'nin
   bağlandığı alanı taşır (K5-K7 doğal çalışır).
4. Bağlanmamış sembolik alan → `DomainId::Error`: denetlenemez, yanlış
   pozitif de üretmez.

### 5. Yeni kod E3014 — aynı alana iki farklı saat

```
error[E3014]: symbolic domain '@Core' is bound to two different clocks: @SysDomain and @PixDomain
   ┌─ x.volt:38:9
37 │         wr_clk:  sys_clk,
   │         ---------------- '@Core' was already bound to @SysDomain here
38 │         rd_clk:  pix_clk,
   │         ^^^^^^^^^^^^^^^^ this clock is @PixDomain
   = reason: one domain annotation stands for exactly one clock domain per instantiation; ... (ADR-0047)
   = help: drive both clock ports of '@Core' from one clock, or give the second port its own domain in the declaration (e.g. @Dst)
```

- Beş parça: kod, konum (ikinci bağlama), açıklama, öneri, ADR notu;
  ilk bağlama ikincil etiket.
- Çakışan anahtar `Error`'a düşer → port denetimleri kaskad E3001
  üretmez (tek kök neden, tek hata).
- Kural **sıradan modüller için de geçerli**: `in a_clk : clock @Fast`,
  `in b_clk : clock @Fast` portlarına farklı alanlardan saat bağlamak
  E3014. Önceden K8 ikinci bağlamayı sessizce üzerine yazıyordu.
- Çelişki yalnız iki taraf da `Explicit` ve farklıysa; Timeless/Error
  taraflar çelişki sayılmaz.

### 6. `examples/vga/frame_buffer.volt`

Dosyada `extern module` YOK: FIFO yerleşik `AsyncFifo<u14, 16>`
(ADR-0027), RAM yerleşik `DualPortRam`. Anotasyon eklenecek extern
bulunmadı. Kasıtlı ihlal denemesi yerleşik yolda yapıldı: `wr_data:
rd_x as u14` (PixDomain portu yazma tarafına) → **E3001** üretti,
sonra geri alındı. Extern yolundaki kasıtlı ihlal `tests/ui/fail/49`
ile kalıcı test altında. README'deki "extern module sorunu derleyicinin
denetlemediği SV dosyasına iter" cümlesi güncellendi: sınır artık
denetleniyor, kara kutunun içi denetlenmiyor.

## Sonuçlar

- **Güvenlik açığı kapandı** (analiz katmanında): extern portuna
  yanlış alandan sinyal bağlamak artık E3001; aynı alana iki saat
  E3014; anotasyonsuz çok saatli extern E3010; taşınmayan sembolik
  alan E3002.
- Etkilenen extern modül sayısı depoda **0** (tests/ui, examples/ ve
  fixtures'ta hiç `extern module` yoktu — açık, kullanıcıya açık ama
  hiç kullanılmamış bir yol üzerindeydi).
- Değişen dosyalar: volt-hir (resolve/typeck/domain), volt-diagnostics
  (code, messages, explain), volt-lsp/hover.rs (DefKind eşlemesi
  joker kolsuz olduğundan zorunlu tek satır), docs/spec (ADR kaynaklı
  K8a/§5/§8 + DefKind), volt-syntax bundle.rs (anotasyon span'i),
  tests/ui 3 fixture, +36 test.

## Sınırlar

- ~~**volt-sv-emit** extern örneğini üretemiyor~~ — ADR-0071 ile
  kapandı (adlandırılmış bağlantılı örnekleme, örtük reset portu yok).
  Özgün not: (E0003, `module_decl_named`
  yalnız `ItemKind::Module` arıyor). Bu ADR'nin kapsamı dışında;
  extern'in SV'ye eşlenmesi ayrı ADR ister (sv-mapping.md §9 eki:
  adlandırılmış port bağlantılı örnekleme + dış dosya referansı).
- Kara kutunun İÇİ denetlenmez: extern bildirimi yanlışsa (SV
  çekirdeği aslında tek saatliyken `@Src`/`@Dst` yazılmışsa) derleyici
  bunu bilemez. Sözleşme kullanıcının sorumluluğundadır; bu, extern'in
  doğasıdır.
- Extern generic'leri (`extern module X<T>`) monomorfize edilmez;
  `T` tipli port `resolve_type_ref` ne döndürürse onu taşır. Mevcut
  davranış korundu.

## Test

- `tests/ui/pass/62_extern_domains.volt` — iki sembolik alanlı FIFO +
  tek saatli extern, doğru bağlanmış, çıkış okuması bağlanan alanı
  taşıyor.
- `tests/ui/fail/49_extern_domain_violation.volt` — E3001.
- `tests/ui/fail/50_extern_clock_conflict.volt` — E3014.
- `crates/volt-hir/tests/extern_domain_tests.rs` — 32 test: tanım/
  tip/bildirim denetimleri, K8 haritası, E3014 beş parça ve kaskad
  bastırma, modül E3014, regresyonlar (modülde E3002, W1001/W1002 yok,
  kök `struct`/`const` gölgelemez, aynı extern'in iki örneği, üç saat
  tek E3014, bundle alan anotasyonu iki extern'de).
- Test taban çizgisi 1484 → 1527 (+36 bu ADR, +7 önceki commitlerde
  güncellenmemiş).
- `explain_tests.rs` — 113 kod, iki dilde tam açıklama, E3014
  metninde "extern".
