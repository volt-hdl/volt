# Volt Domain Çıkarım Kuralları

> STATÜ: BAĞLAYICI — Volt'un temel değer önerisi
> İlgili: `type-inference.md`, UX Anayasası
> Aşama: F2c

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
    // [V1] güç ve güven
    pub power: Option<PowerSpec>,
    pub trust: TrustLevel,
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
E3009  Bilgi akışı ihlali (trust_level) [V1]
E3010  Domain belirsiz (çoklu saat, anotasyon yok)
E3011  Register birden fazla domainden yazılıyor
E3012  'on' bloğunda yabancı domain sinyali okunuyor

W3001  Register hiç yazılmıyor
W3002  Gereksiz sync() (aynı domain)
W3003  Çok bitli sync() — bit tutarlılığı garanti değil
W3004  Kullanılmayan domain tanımı
```

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
Aynı domain içinde sync()                ⚠ W3002
8-bit sync()                             ⚠ W3003
```
