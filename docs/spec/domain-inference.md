# Volt Domain Çıkarım Kuralları

> STATÜ: BAĞLAYICI — Volt'un temel değer önerisi
> İlgili: `type-inference.md`, UX Anayasası
> Aşama: F2c
> Karar kayıtları: ADR-0002 (`@Domain` anotasyonu ve birleşik domain §1), ADR-0020 (`trust_level` dördüncü boyut, K11)

---

## 0. Temel Gerilim

```
İKİ ÇELİŞEN İHTİYAÇ:

UX Anayasası:        "Tek saatli tasarımda domain görünmemeli"
Volt'un vaadi:       "CDC hatası derlenmemeli"

Çözüm: ÇIKARIM
  Domain her sinyalde VAR ama çoğu zaman YAZILMIYOR.
  Derleyici çıkarıyor. Belirsizlik olursa soruyor.
```

Bu belge o çıkarımın kurallarını tanımlar.

---

## 1. Domain Gösterimi

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainId {
    /// Belirli bir saat alanı
    Explicit(u32),
    /// Saatten bağımsız — sabitler, saf kombinasyonel
    Timeless,
    /// Henüz çözülmemiş (çıkarım sırasında)
    Unresolved(InferVar),
    /// Hata kurtarma — her domainle uyumlu
    Error,
}

pub struct DomainInfo {
    pub name: Name,
    pub clock: ClockSpec,
    pub reset: ResetSpec,
    // [V1] güç
    pub power: Option<PowerSpec>,
    // Güven seviyesi (ADR-0052): `trust_level` yazılmamışsa None =
    // sınıflandırılmamış; `trust_span` E3009 "trust level here" etiketi.
    pub trust: Option<TrustLevel>,
    pub trust_span: Option<Span>,
}
```

**`Timeless` kritik:** Sabitler ve saf kombinasyonel ifadeler
hiçbir saat alanına ait değil. Her domainle birleşebilir.

---

## 2. Çıkarım Kuralları

### K1 — Açık Anotasyon Kazanır

```volt
in data : u8 @Fast
```

Anotasyon varsa çıkarım yapılmaz. Kullanıcı ne dediyse o.

### K2 — Tek Saat Kuralı (En Önemli)

```volt
module Counter {
    in  clk    : clock      // ← tek clock portu
    in  enable : bool       // domain yazılmadı
    out count  : u8         // domain yazılmadı

    reg count_r : u8 = 0    // domain yazılmadı
    on clk { ... }
}
```

```
Modülde TEK clock portu varsa:
  → Anotasyonsuz her sinyal o domain'e atanır
  → CDC kontrolü trivially geçer
  → Kullanıcı 'domain' kelimesini hiç görmez
```

```rust
fn infer_module_domains(&mut self, m: &ModuleDecl) {
    let clocks: Vec<_> = m.ports.iter()
        .filter(|p| self.is_clock_type(p.ty))
        .collect();

    match clocks.len() {
        0 => {
            // Saat yok → tamamen kombinasyonel modül
            self.default_domain = DomainId::Timeless;
            // reg veya on bloğu varsa hata (K7)
        }
        1 => {
            // TEK SAAT — sessiz varsayılan
            self.default_domain = self.domain_of_clock(clocks[0]);
        }
        _ => {
            // ÇOK SAAT — anotasyon zorunlu (K3)
            self.default_domain = DomainId::Unresolved(fresh_var());
            self.multi_clock_mode = true;
        }
    }
}
```

### K2a — Çift Yönlü Port Okuması Haricidir (ADR-0051)

`inout` / `opendrain` portu K2 ile modülün alanına ATANIR (atama ve
örnekleme denetimleri için) ama OKUMASI harici sayılır: pad'in öbür
ucu kendi zamanlamasıyla başka bir aygıttır. `p.read()` (ya da çıplak
`p`) bir `sync()` çağrısının kaynağı değilse ve kontrat içinde değilse
**W3007** üretir; okuma yine modülün alanında değerlendirilir (E3012
kaskadı olmaz). `sync(p.read(), clk)` içinde kaynak alan `Timeless`
döner — hedefle aynı alan sayılmaz, W3002 çıkmaz.

### K11 — Güven Seviyesi: Domain'in Dördüncü Boyutu (ADR-0052)

```volt
domain SecureCore { clock = posedge, reset = sync active_high, trust_level = secret }
domain Debug      { clock = posedge, reset = sync active_high, trust_level = public }
```

Kafes `public < confidential < secret`; bilgi yalnız eşit ya da daha
yüksek seviyeye akar. Güven geçidi (`trust.rs`) saat çıkarımından SONRA
koşar ve aynı mekanizmayı kullanır:

- **Sinyalin seviyesi = alanının seviyesi** (K1/K2/K4 aynen): `@Ad`
  bildirimi trust_level taşıyorsa o, anotasyonsuz sinyal modülün tek
  alanının seviyesi, register yazıcı bloğunun alanı.
- **İfade en yüksek seviyeyi taşır** (K5 eşleniği, `join` = max).
- **Atama** (K6): hedef seviye < kaynak seviye → **E3009**. Kaynak =
  sağ taraf ⊔ içinde bulunulan `if`/`match` koşulu (örtük akış) ⊔ hedef
  indeksleri.
- **Örnekleme** (K8): giriş portuna bağlanan değer ≤ port seviyesi;
  `inst.out` port seviyesini taşır; sınıflandırılmamış çıkış, örneğin
  sınıflandırılmış girişlerinin en yükseğini (tutucu özet).
- **`sync()`** (K9) saati değiştirir, etiketi KORUR.
- **Sabitler** (Timeless) her seviyeyle uyumlu.
- **Sınıflandırılmamış sinyal** (alanında trust_level yok) kendisine
  yazılan en yüksek seviyeyi alır (sabit nokta) — anotasyonsuz register
  bir sırrı aklayamaz. Dosyada trust_level yoksa geçit hiç koşmaz.

**Saat boyutu takma adı.** trust_level taşıyan bir `@Ad` anotasyonu, bu
modülün hiçbir clock portu `Ad`'ı taşımıyorsa yeni saat alanı AÇMAZ:
tek saatli modülde sinyal modülün alanında kalır (yalnız güven boyutu
belirlenir); çoklu saatte E3010; bir clock portu taşıyorsa gerçek saat
alanıdır (E3001 korunur). Örneklemede de hedefin tek saatinden bağlanır.
trust_level'sız bildirimler için davranış değişmez.

**`declassify(expr, "gerekçe")`** tek meşru düşürme: sonuç public,
gerekçe zorunlu (**E0016**), her çağrı **W3008** iz kaydı. Parser'da
soyulur (`delay<K>` gibi); SV üretimi hiç görmez. Kontratlar gözlemdir,
denetlenmez; "sızıntı yok" iki-izli özellik olduğundan otomatik
invariant üretilmez (ADR-0052 §5).

```
error[E3009]: secret data flows to a public output
   ┌─ crypto.volt:92:5
20 │     trust_level = secret            -------------------- source trust level here
26 │     trust_level = public            -------------------- destination trust level here
92 │     debug_out = key_r[7:0] as u8
   │     ^^^^^^^^^   ----- @SecureCore (secret)
   │     @Debug (public)
   = reason: information from a higher trust level cannot reach a lower one; this could leak key material (ADR-0052)
   = help: if intentional, use declassify(expr, "reason")
```

### K3 — Çoklu Saat: Anotasyon Zorunlu

```volt
module CrossDomain {
    in  fast_clk : clock
    in  slow_clk : clock
    in  data     : u8       // ← HANGİSİ?
}
```

```
error[E3010]: sinyalin saat alanı belirsiz
  ┌─ design.volt:5:9
  │
3 │     in  fast_clk : clock
  │         -------- aday 1
4 │     in  slow_clk : clock
  │         -------- aday 2
5 │     in  data     : u8
  │         ^^^^ hangi alana ait olduğu belirsiz
  │
  = neden: modülde birden fazla saat var, sinyalin
           hangisine ait olduğu çıkarılamıyor
  = çözüm: in data : u8 @Fast
  = daha fazla: volt explain E3010
```

**Kritik UX kararı:** Bu hata sadece **çoklu saat** durumunda çıkar.
Tek saatli tasarımlar hiç görmez.

### K4 — Register Domain'i `on` Bloğundan Gelir

```volt
reg counter : u8 = 0     // domain yazılmadı

on fast_clk {
    counter <= counter + 1    // ← burada belirleniyor
}
```

```rust
fn infer_reg_domain(&mut self, reg: &RegDecl) -> DomainId {
    // Açık: reg(fast_clk) x : u8 = 0
    if let Some(d) = reg.domain {
        return self.resolve_domain(d);
    }

    // Hangi 'on' bloklarında yazılıyor?
    let writers = self.find_sequential_writers(reg.name);

    match writers.len() {
        0 => {
            // Hiç yazılmıyor → sabit gibi
            self.warn(W3001, reg.span,
                "register hiçbir 'on' bloğunda yazılmıyor");
            DomainId::Timeless
        }
        1 => self.domain_of_on_block(writers[0]),
        _ => {
            let domains: HashSet<_> = writers.iter()
                .map(|w| self.domain_of_on_block(*w))
                .collect();
            if domains.len() == 1 {
                *domains.iter().next().unwrap()
            } else {
                // ÇOK CİDDİ HATA: aynı register iki saatten yazılıyor
                self.error(E3011, reg.span,
                    "register birden fazla saat alanından yazılıyor",
                    "her register tek bir saat alanına ait olmalı");
                DomainId::Error
            }
        }
    }
}
```

### K5 — Kombinasyonel Yayılım (Birleştirme)

Bu çıkarımın kalbi:

```volt
let sum = a + b;   // sum'un domain'i?
```

**Kural:** Kombinasyonel ifadenin domain'i, operandlarının
domain'lerinin **birleşimi** (join).

```rust
fn join_domains(&mut self, a: DomainId, b: DomainId,
                span: Span) -> DomainId {
    use DomainId::*;
    match (a, b) {
        // Hata yayılımı
        (Error, _) | (_, Error) => Error,

        // Timeless her şeyle birleşir (sabitler)
        (Timeless, x) | (x, Timeless) => x,

        // Aynı domain → sorun yok
        (Explicit(x), Explicit(y)) if x == y => Explicit(x),

        // FARKLI DOMAIN → CDC İHLALİ
        (Explicit(x), Explicit(y)) => {
            self.error_cdc_combinational(x, y, span);
            Error
        }

        // Çözülmemiş → kısıt biriktir
        (Unresolved(v), x) | (x, Unresolved(v)) => {
            self.add_constraint(v, x);
            x
        }
    }
}
```

**Kombinasyonel CDC neden yasak:**

```volt
let bad = fast_signal & slow_signal;
//        ^^^^^^^^^^^^^^^^^^^^^^^^^ E3001

// Nedeni: iki farklı saatten gelen sinyaller kapıda
// birleşince geçici darbe (glitch) üretir.
// Bu darbe sonraki register'da yanlış yakalanabilir.
```

### K6 — Atama Domain Uyumu

```volt
out slow_out : u8 @Slow
slow_out = fast_data;     // ← E3001
```

```rust
fn check_assign_domain(&mut self, lhs: &LValue, rhs: Idx<Expr>) {
    let lhs_dom = self.lvalue_domain(lhs);
    let rhs_dom = self.expr_domain(rhs);

    match (lhs_dom, rhs_dom) {
        // Sabit her yere atanabilir
        (_, DomainId::Timeless) => {}
        (DomainId::Error, _) | (_, DomainId::Error) => {}
        (a, b) if a == b => {}
        (a, b) => self.error_cdc_assignment(a, b, lhs.span),
    }
}
```

### K7 — `on` Bloğu İçi Kısıtlar

```volt
on slow_clk {
    slow_reg <= fast_signal;   // ← E3001
}
```

Sıralı blokta:
- Yazılan sinyal bloğun domain'inde olmalı
- Okunan sinyaller de aynı domain'de olmalı
- Sabit ve `Timeless` istisna

### K8 — Modül Örnekleme

```volt
let uart = Uart {
    clk:  slow_clk,      // Uart'ın clk portu → Slow domain
    data: fast_data,     // ← E3001 (data portu Slow bekliyor)
};
```

```rust
fn check_instance_domains(&mut self, inst: &InstanceDecl) {
    let module = self.resolve_module(&inst.module_path);

    // 1. Saat portlarından modülün domain haritasını çıkar
    let mut mapping = HashMap::new();
    for binding in &inst.bindings {
        let port = module.port(binding.port_name);
        if self.is_clock_type(port.ty) {
            let actual = self.expr_domain(binding.value);
            mapping.insert(port.domain_id, actual);
        }
    }

    // 2. Diğer portları bu haritaya göre kontrol et
    for binding in &inst.bindings {
        let port = module.port(binding.port_name);
        if self.is_clock_type(port.ty) { continue; }

        let expected = mapping.get(&port.domain_id)
            .copied()
            .unwrap_or(DomainId::Timeless);
        let actual = self.expr_domain(binding.value);

        if !self.domains_compatible(expected, actual) {
            self.error_cdc_port(binding, expected, actual);
        }
    }
}
```

#### K8a — Extern Modül Sınırı (ADR-0047)

K8 `extern module` hedefleri için de geçerlidir. Extern gövdesi
olmadığından port anotasyonları sınırdaki TEK sözleşmedir:

```volt
extern module AsyncFifo {
    in  wr_clk  : clock @Src      // @Src, @Dst: SEMBOLİK alan
    in  wr_data : u8    @Src      // hiçbir yerde 'domain Src' yok
    out wr_full : bool  @Src
    in  rd_clk  : clock @Dst
    out rd_data : u8    @Dst
    in  rd_en   : bool  @Dst
}

let f = AsyncFifo {
    wr_clk:  sys_clk,     // @Src := SysDomain (adım 1)
    wr_data: pix_d,       // ← E3001: wr_data @Src = SysDomain bekliyor
    rd_clk:  pix_clk,     // @Dst := PixDomain
    rd_en:   pix_en,      // ✓
}
pix_q = f.rd_data         // f.rd_data @Dst = PixDomain (K8 haritası)
```

- `extern module` içinde çözülemeyen `@Ad` E3002 DEĞİL, extern'e özel
  **sembolik alan parametresidir** (`DefKind::DomainParam`,
  name-resolution.md §2). Bilinen bir `domain` adı ya da extern'in
  kendi clock portu (`@wr_clk`) olağan biçimde çözülür.
- Sembolik alan en az bir `clock` portunda taşınmalıdır; aksi halde
  hiç bağlanamaz → **E3002** (extern biçimi: "sembolik alanın clock
  portu yok").
- Birden fazla clock portu olan extern'de saat dışı her port anotasyon
  taşımalıdır → **E3010** (K3 sınırda uygulanır). Tek saatli extern K2
  gibi davranır (anotasyonsuz portlar o saatin alanında), saatsiz
  extern portları Timeless'tır.
- K8 adım 1'de aynı anahtara (sembolik ya da açık alan) iki farklı
  alandan saat bağlanırsa → **E3014**; anahtar `Error`'a düşer, port
  denetimleri kaskad E3001 üretmez. Bu kural sıradan modüller için de
  geçerlidir (iki clock portu aynı `@Alan`ı taşıyorsa).
- Bağlanmamış sembolik alan (`wr_clk` bağlanmadı) `Error`'dur:
  denetlenemez, yanlış pozitif de üretmez.

### K9 — `sync()` Köprüsü

Tek meşru CDC geçiş yolu:

```volt
out slow_data : u8 @Slow
slow_data = sync(fast_data, slow_clk);   // ✓ geçerli
```

```rust
// sync yerleşik fonksiyon — özel tip kuralı
fn synth_sync_call(&mut self, args: &[Idx<Expr>], span: Span) -> TypeId {
    if args.len() != 2 {
        return self.err_arity(2, args.len(), span);
    }

    let data_ty = self.synth(args[0]);
    let src_dom = self.expr_domain(args[0]);
    let dst_dom = self.clock_arg_domain(args[1]);

    // Aynı domain → gereksiz senkronizatör
    if src_dom == dst_dom {
        self.warn(W3002, span,
            "sync() aynı saat alanı içinde gereksiz",
            "doğrudan atama yeterli");
    }

    // Çok bitli veri uyarısı
    if let Some(w) = self.width_of(data_ty) {
        if w > 1 {
            self.warn(W3003, span,
                format!("{}-bit sinyal için iki-flop senkronizasyonu \
                         bit tutarlılığı garanti etmez", w),
                "gray kodlama veya AsyncFifo kullanın");
        }
    }

    self.set_expr_domain(span, dst_dom);   // çıkış hedef domain'de
    data_ty
}
```

**Neden çok bitli uyarısı:** İki-flop senkronizatör her biti
bağımsız senkronize eder. Bitler farklı saat kenarlarında
yakalanabilir → geçersiz ara değer. Bu Arch HDL'in de kabul
ettiği bir sınır.

---

## 3. Çıkarım Akışı

```
1. MODÜL TARAMASI
   Clock portlarını topla
   Tek saat → default_domain belirle
   Çok saat → multi_clock_mode aç

2. PORT ATAMASI
   Açık @Domain varsa kullan
   Yoksa: tek saat → default; çok saat → E3010

3. REGISTER ATAMASI
   reg(clk) varsa kullan
   Yoksa: 'on' bloklarından çıkar (K4)

4. KOMBİNASYONEL YAYILIM
   İfade ağacında aşağıdan yukarı join (K5)
   Her join'de CDC kontrolü

5. ATAMA KONTROLÜ
   lhs.domain vs rhs.domain (K6)

6. ÖRNEK KONTROLÜ
   Saat bağlantılarından harita, sonra port kontrolü (K8)

7. KISIT ÇÖZÜMÜ
   Kalan Unresolved değişkenler
   Çözülemezse → E3010
```

---

## 4. Hata Mesajları

### E3001 — Kombinasyonel CDC

```
error[E3001]: farklı saat alanları kombinasyonel olarak birleşemez
  ┌─ design.volt:14:15
  │
14│     let bad = fast_sig & slow_sig;
  │               ^^^^^^^^   ^^^^^^^^ @Slow (satır 6)
  │               │
  │               @Fast (satır 5)
  │
  = neden: iki saat alanından gelen sinyaller kapıda
           birleşince geçici darbe (glitch) üretir;
           bu darbe sonraki register'da yanlış yakalanır
  = çözüm: önce senkronize edin
           let synced = sync(fast_sig, slow_clk);
           let ok = synced & slow_sig;
  = daha fazla: volt explain E3001
```

### E3001 — Atama CDC

```
error[E3001]: saat alanları arasında doğrudan atama
  ┌─ design.volt:12:16
  │
12│     slow_out = fast_data;
  │     ^^^^^^^^   ^^^^^^^^^ @Fast (100 MHz)
  │     │
  │     @Slow (25 MHz)
  │
  = neden: hedef register kaynak sinyali kararsız anda
           yakalayabilir (metastabilite)
  = çözüm: slow_out = sync(fast_data, slow_clk);
  = not: 8-bit veri için AsyncFifo daha güvenli olabilir
  = daha fazla: volt explain E3001
```

### E3010 — Belirsiz Domain

```
error[E3010]: sinyalin saat alanı belirlenemiyor
  ┌─ design.volt:5:9
  │
3 │     in  fast_clk : clock
  │         -------- aday: @Fast
4 │     in  slow_clk : clock
  │         -------- aday: @Slow
5 │     in  data     : u8
  │         ^^^^
  │
  = neden: modülde birden fazla saat alanı var
  = çözüm: in data : u8 @Fast
  = daha fazla: volt explain E3010
```

---

## 5. Hata ve Uyarı Kodları

```
E3001  Saat alanı uyumsuzluğu (CDC)
E3002  Tanımsız saat alanı
E3003  Sıfırlama alanı uyumsuzluğu (RDC)
E3004  Sıfırlama sekans ihlali
E3005  Koşullu sıfırlama karşılanmadı
E3006  Güç alanı geçişi izolasyonsuz [V1]
E3007  Güç sekans ihlali [V1]
E3008  Retention eksik [V1]
E3009  Bilgi akışı ihlali (trust_level) (ADR-0052, K11)
E3010  Domain belirsiz (çoklu saat, anotasyon yok)
E3011  Register birden fazla domainden yazılıyor
E3012  'on' bloğunda yabancı domain sinyali okunuyor
E3013  Bundle alanları farklı saat alanlarında (ADR-0039)
E3014  Aynı sembolik saat alanına iki farklı saat bağlandı (ADR-0047)

W3001  Register hiç yazılmıyor
W3002  Gereksiz sync() (aynı domain)
W3003  Çok bitli sync() — bit tutarlılığı garanti değil
W3004  Kullanılmayan domain tanımı
W3007  Harici çift yönlü sinyal senkronizasyonsuz okunuyor (ADR-0051)
W3008  Bilinçli güven düşürme (declassify) — gözden geçirilmeli (ADR-0052)
```

E0016 (gerekçesiz declassify) sözdizimi kodudur; grammar-full.ebnf §18.

---

## 6. UX Doğrulama Testi

Bu kuralların UX Anayasası'na uygunluğu:

```
SENARYO                          KULLANICI GÖRÜR MÜ?
──────────────────────────────────────────────────────
Tek saatli sayaç                 Hayır — domain görünmez
İki saat, anotasyonsuz           EVET — E3010, öğretici
İki saat, anotasyonlu, doğru     Hayır — sessiz geçer
İki saat, CDC ihlali             EVET — E3001, çözüm gösteriyor
sync() ile doğru köprü           Hayır — sessiz geçer
```

**Kural:** Kullanıcı sadece **gerçek bir sorun** olduğunda
domain kavramıyla karşılaşır. Bu UX Anayasası'nın
"cezalandırma" testinden geçer.

---

## 7. Uygulama Sırası

```
F2c-1 (1 hafta):
  DomainId, DomainInfo yapıları
  K1, K2 (açık anotasyon + tek saat)
  → Tek saatli tasarımlar çalışıyor

F2c-2 (1 hafta):
  K4, K5 (register + kombinasyonel yayılım)
  E3001 kombinasyonel hatası
  → CDC ihlali yakalanıyor ← VOLT'UN VAADİ

F2c-3 (1 hafta):
  K3, K6 (çoklu saat, atama)
  E3010 belirsizlik hatası

F2c-4 (1 hafta):
  K8, K9 (örnekleme, sync())
  Hata mesajı cilası
  → F2 tamamlandı
```

**F2c-2 sonu = projenin en önemli anı.** İlk kez CDC hatası
derlenmiyor.

---

## 8. Test Vektörleri

```
DURUM                                    BEKLENEN
──────────────────────────────────────────────────────────
Tek saat, anotasyonsuz                   ✓ derlenir
Tek saat, açık @Domain                   ✓ derlenir
İki saat, tümü anotasyonlu, uyumlu       ✓ derlenir
sync() ile CDC köprüsü                   ✓ derlenir
Sabit atama (Timeless)                   ✓ derlenir
──────────────────────────────────────────────────────────
İki saat, anotasyonsuz sinyal            ✗ E3010
Farklı domain doğrudan atama             ✗ E3001
Kombinasyonel domain karışımı            ✗ E3001
Register iki 'on' bloğunda               ✗ E3011
Tanımsız domain referansı                ✗ E3002
Extern: sembolik alan, doğru bağlanmış   ✓ derlenir (ADR-0047)
Extern: yanlış alandan port bağlama      ✗ E3001 (ADR-0047)
Extern: sembolik alana iki farklı saat   ✗ E3014 (ADR-0047)
Extern: sembolik alanın clock portu yok  ✗ E3002 (ADR-0047)
Extern: çok saat, anotasyonsuz port      ✗ E3010 (ADR-0047)
Aynı domain içinde sync()                ⚠ W3002
8-bit sync()                             ⚠ W3003
```
