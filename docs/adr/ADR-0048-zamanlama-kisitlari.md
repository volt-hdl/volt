# ADR-0048: Zamanlama Kısıtları — Uygulanmayan Nitelikler W0021 Üretir, SDC Üretimi V1 Planı

> Statü: KABUL EDİLDİ (1. bölüm uygulandı; 2. bölüm ADR-0054 ile GERÇEKLENDİ —
> `@timing`/`@false_path`/`@multicycle` artık uygulanıyor, W0021 listesinden çıktı;
> bu ADR'nin 2. bölümü tarihsel plan olarak korunur, güncel karar ADR-0054'tedir)
> Tarih: 2026-09-14
> Etkilenen: grammar-full.ebnf §2/§18 (W0021 yeniden tanımı),
> volt-hir (attrs.rs — yeni geçit), volt-diagnostics (W0021 mesaj +
> explain), volt-driver (Volt.toml `[lint]`, aşama 1b), volt-lsp
> (analysis.rs), volt-syntax (KNOWN_ATTRIBUTES `allow`),
> examples/vga/, tests/ui/pass 63-64

## Sorun

`examples/vga/` keşfi (rapor 8d): `@timing(pix_clk = 25175000)`
ayrıştırılıyor, `KNOWN_ATTRIBUTES` içinde olduğu için W0020 üretmiyor,
ama hiçbir geçit okumuyor. SDC yazılmıyor, hiçbir denetim çalışmıyor,
hiçbir uyarı verilmiyor. Kullanıcı bir kısıt yazdığını sanıyor; boşluk
ancak üretici aracında ya da silisyumda ortaya çıkıyor.

Bu, UX Anayasası'nın "sessizce yok sayma" yasağının doğrudan ihlali.

### Durum tespiti — hangi nitelik ayrıştırılıyor, hangisi yorumlanıyor?

`crates/` altında `attrs` alanını okuyan yalnız üç yer var: mmio
desugar'ı (`parser/mmio.rs`, `parser/item.rs`), `timing.rs`
(`@strict_timing`) ve şimdi `attrs.rs`. Tablo (grammar-full.ebnf §2
sırasıyla):

| Nitelik | Aşama | Ayrıştırılıyor | Yorumlanıyor | Nerede |
|---|---|---|---|---|
| `@domain` | F2 | evet | **hayır** | — (port `@Ad` anotasyonu ayrı sözdizimidir ve yorumlanır) |
| `@mmio` `@reg` `@offset` `@access` `@reserved` `@self_clearing` `@w1c` | F3 | evet | evet | `parser/mmio.rs` desugar (ADR-0044) |
| `@budget` | F4 | evet | **hayır** | E6001 üreticisi yok |
| `@timing` | F4 | evet | **hayır** | SDC üreticisi yok |
| `@false_path` | F4 | evet | **hayır** | E6003 üreticisi yok |
| `@multicycle` | F4 | evet | **hayır** | E6004 üreticisi yok |
| `@version` `@abi_version` | F5 | evet | **hayır** | E7001/E7002 üreticisi yok |
| `@dft` `@debug_visible` `@debug_trace` `@synthesis_target` | V1 | evet | **hayır** | — |
| `@strict_timing` | ADR-0037 | evet | evet | `volt-hir/src/timing.rs` |
| `@allow` | bu ADR | evet | evet | `volt-hir/src/attrs.rs` |

Sonuç: tanınan 20 nitelikten **11'i** ayrıştırılıp atılıyordu.
`E6001/E6003/E6004/E7001/E7002` kodları tanımlı ama hiçbir yerde
üretilmiyor — nitelikler gibi bunlar da "rezerve".

Ayrıca `W0021` kodu spec §18'de "Kullanılmayan doc yorumu" olarak
rezerve edilmiş, ama hiçbir zaman üretilmemişti (mesaj tablosu ve
explain sayfası vardı, üretici yoktu).

## Karar

### Bölüm 1 — kısa vade (BU TURDA UYGULANDI)

#### 1.1 W0021: "nitelik ayrıştırılıyor ama henüz uygulanmıyor"

Yukarıdaki tabloda "yorumlanıyor: hayır" olan her nitelik, her
kullanımında W0021 üretir:

```
warning[W0021]: attribute '@timing' is parsed but not yet enforced
   ┌─ examples/vga/vga_timing.volt:47:1
   │
47 │ @timing(pix_clk = 25175000)
   │ ^^^^^^^ no constraint or check is generated from this
   │
   = reason: timing constraint generation (SDC) is not implemented yet
   = note: the attribute is syntactically valid and will be honored in a
           future release; silence this with @allow(unenforced) on the
           same item or Volt.toml [lint] unenforced_attributes = "allow"
   = help: use vendor constraints (.xdc/.sdc) meanwhile, or silence with
           @allow(unenforced)
   = for more: volt explain W0021
```

`= reason:` nitelik ailesine göre değişir (`attrs.rs::reason_and_help`):

| Aile | reason | help |
|---|---|---|
| `@timing` | SDC üretimi yok | .xdc/.sdc kullan |
| `@false_path` `@multicycle` | set_false_path / set_multicycle_path ve yapısal kanıt (E6003/E6004) yok | .xdc/.sdc kullan |
| `@budget` | sentez kestirimi (E6001) yok | üretici kullanım raporu |
| `@version` `@abi_version` | sürüm denetimi (E7001/E7002) yok | inceleme notu |
| `@domain` | yorumlayıcı yok; alanlar port `@Ad` ile atanır | portu anotasyonla |
| V1 ipuçları | V1 özelliği | üretici akışında uygula |

**W0021'in yeniden tanımı.** Kod daha önce "kullanılmayan doc yorumu"
için rezerve edilmişti; hiç üretilmedi. Bu ADR kodu yeniden tanımlar
(belge öncelik sırası: ADR > spec). Doc yorumu uyarısı gerektiğinde
yeni bir kodla (W002x sırasındaki ilk boş kod) açılır. `docs/spec/` salt okunur olduğundan
§18 satırı bu ADR tarafından geçersiz kılınmış sayılır.

#### 1.2 Uyarı susturulabilir — iki kapı

1. **Öğe düzeyi:** `@allow(unenforced)`. Aynı düğümdeki (öğe, port,
   struct alanı, deyim) tüm uygulanmayan nitelikleri susturur. Öğe
   (modül/extern/struct) üzerindeki `@allow(unenforced)` portlarını,
   alanlarını ve gövde deyimlerini de kapsar; bir porttaki `@allow`
   kardeş portu kapsamaz.

   ```volt
   @timing(pix_clk = 25175000) @allow(unenforced)
   pub module VgaTiming { ... }
   ```

2. **Paket düzeyi:** Volt.toml

   ```toml
   [lint]
   unenforced_attributes = "allow"   # varsayılan "warn"
   ```

   Manifest, derlenen dosyanın dizininden yukarı aranır (ADR-0042 ile
   aynı keşif). Tanınmayan değer `warn`'a düşer — susturma sessizce
   "kazara" açılamaz.

`@allow` yalnız `@allow(unenforced)` biçiminde geçerlidir; başka ya da
eksik argüman **E0009** (geçersiz nitelik argümanı). Gerekçe: yanlış
yazılmış bir susturma hiçbir şeyi susturmaz, bunu da sessizce
geçirmek aynı yasağın ihlali olurdu. E0009 politikadan bağımsızdır.

#### 1.3 Nerede koşar

- `volt-hir/src/attrs.rs::check_attributes(ast, UnenforcedLint)` —
  çözümlemeden bağımsız, yalnız AST okur.
- Sürücü: aşama 1b (parse temizse, import çözümlemesinden önce);
  politika `Manifest::lint_unenforced`.
- LSP: `UnenforcedLint::discover(dosya dizini)` — sürücüyle aynı Volt.toml,
  aynı karar (editör ve `volt check` çelişmez).
- `volt_hir::analyze` (tek dosya API'si, testler): varsayılan `Warn`.
- Generic modüllerin monomorph klonları aynı kaynak konumunu taşır;
  W0021 konum başına BİR kez raporlanır (`ctx` yok sayılır).

#### 1.4 Yorumlanan bir nitelik listeden çıkar

`UNENFORCED_ATTRIBUTES` tek doğruluk kaynağıdır. Bir nitelik
gerçekten uygulanmaya başlayınca (ör. `@timing` SDC yazınca) listeden
ve `reason_and_help` tablosundan silinir; W0021 o nitelik için
kendiliğinden kesilir. Geride kalan `@allow(unenforced)` zararsızdır.

### Bölüm 2 — uzun vade: `@timing` → SDC (YALNIZ PLAN, UYGULANMADI)

Aşağıdaki hiçbir madde bu turda kodlanmadı; hedef sürüm **V1**.

#### 2.1 Sözdizimi ve birimler

```volt
@timing(pix_clk >= 25.175.mhz)        // en düşük frekans (create_clock)
@timing(pix_clk = 25.175.mhz)         // tam frekans
@timing(sys_clk = 100.mhz, jitter = 50.ps)
@false_path(from = cfg_r, to = pix_out)
@multicycle(2, from = acc, to = sum)
```

Ön koşullar (bugün eksik):
- **Ondalık literal** (`25.175`) — lexer bugün `25` `.` `175` üretir
  (E0001 "expected field name"). Gramer değişikliği ayrı ADR ister
  (ADR-0049 adayı: birimli sayısal literal `25.175.mhz`, `50.ps`).
- **Birim son ekleri** `hz/khz/mhz/ghz`, `ps/ns/us` — const-eval'de
  `ConstValue::Frequency(u64 Hz)` / `ConstValue::Time(u64 ps)`.
- Domain bildirimindeki `frequency` anahtarı (`DomainKey::Frequency`)
  bugün aynı şekilde yok sayılıyor; `@timing` ile aynı kaynaktan
  beslenmeli — tek sayı, tek yer: domain'de bildirilir, `@timing`
  modül sınırında onu doğrular/daraltır.

#### 2.2 SDC eşlemeleri

| Volt | SDC | Kaynak bilgi |
|---|---|---|
| `domain D { frequency = 25.175.mhz }` + `in clk : clock @D` | `create_clock -name D -period 39.722 [get_ports clk]` | domain + port eşlemesi (K1/K8) |
| `@timing(clk >= F)` (domain'siz tek saat) | `create_clock -period 1/F [get_ports clk]` | modül portu |
| `@timing(a -> b <= 5.ns)` | `set_max_delay 5 -from [get_pins a*] -to [get_pins b*]` | ad çözümlemesi (DefId → SV ad) |
| iki farklı domain | `set_clock_groups -asynchronous -group {A} -group {B}` | domain çıkarımı (ADR-0023) — **en ucuz ve en değerli çıktı**, hiçbir yeni sözdizimi gerektirmez |
| `@false_path(from = x, to = y)` | `set_false_path -from ... -to ...` | E6003: yol gerçekten yoksa hata (yapısal erişilebilirlik — drivers.rs grafiği) |
| `@multicycle(N, from = x, to = y)` | `set_multicycle_path N -setup -from ... -to ...` ve `N-1 -hold` | E6004: register kademesi sayısı N ile uyuşmalı (timing.rs gecikme çıkarımı yeniden kullanılır) |
| `sync()` / `AsyncFifo` üretilen register'lar | `set_false_path -to [get_pins *sync_r0*/D]` ya da `ASYNC_REG` özniteliği | ADR-0027 primitif üretimi — CDC yolları zaten biliniyor |

#### 2.3 Üretim ve dosya düzeni

- `volt build` → `build/constraints/<Top>.sdc` (ADR-0024 ile aynı
  adlandırma: modül adı). `--emit=sdc` yalnız kısıt üretir.
- Üretici lehçeleri: `.sdc` (Synopsys/Quartus/Yosys) varsayılan;
  `.xdc` (Vivado) `get_ports/get_pins` aynı, `ASYNC_REG` ek. İlk
  sürümde yalnız SDC; XDC tek ek satırlık farkla V1.1.
- SV çıktısındaki hiyerarşik adlar (ADR-0024 modül başına dosya)
  SDC'de `-from/-to` için deterministik olmalı: sv-emit ad üretimi
  (`sv_name`) tek kaynak olarak dışa açılır (E9002 determinizm).

#### 2.4 Doğrulama katmanı (derleme zamanı, araçtan bağımsız)

- `@timing(clk >= F)` iki domain'in aynı porta bağlandığı yerde
  (E3014) çelişkili frekans → yeni kod önerisi (E600x sırasında) "zamanlama
  kısıtı çelişkisi".
- `@false_path` → E6003 ancak yol yapısal olarak KANITLANIRSA kabul
  (bkz. `volt explain E6003`).
- `@multicycle` → E6004 gecikme sayımı `timing.rs` altyapısıyla.
- Bu üçü gerçeklenince nitelikler `UNENFORCED_ATTRIBUTES`'tan çıkar;
  `@budget`, `@version`/`@abi_version` ve V1 ipuçları ayrı ADR'lerle
  aynı yolu izler.

#### 2.5 Aşamalandırma

| Aşama | İçerik | Yeni sözdizimi? |
|---|---|---|
| V1.0 | `set_clock_groups -asynchronous` domain çıkarımından; `sync()`/FIFO false-path'leri | hayır |
| V1.0 | `create_clock` — domain `frequency` (Hz tamsayı) | hayır (anahtar zaten gramerde) |
| V1.1 | ondalık + birimli literal (ADR-0049), `@timing(clk >= 25.175.mhz)` | evet |
| V1.1 | `@false_path` + E6003 kanıtı, `@multicycle` + E6004 | hayır |
| V1.2 | `set_max_delay` (`a -> b <= T`), `.xdc` lehçesi | evet (`->` kısıt ifadesi) |

## Sınırlar

- Bölüm 2 kod DEĞİL, plan: hiçbir SDC satırı üretilmiyor, ondalık
  literal hâlâ E0001. W0021 kullanıcıya tam olarak bunu söyler.
- W0021 nitelik ARGÜMANLARINI doğrulamaz: `@timing(pixel_clock >=
  25175000)` var olmayan bir adla da yalnız W0021 üretir. Argüman
  çözümlemesi nitelik uygulanınca gelir (aksi hâlde yarım bir
  denetim "kısıt var" izlenimi verir — aynı yasak).
- `@allow` bugün tek bir lint adı tanır (`unenforced`). Genel bir
  `@allow(W1001)` mekanizması ayrı karardır; bu ADR o kapıyı
  kapatmaz, yalnız açmaz.
- Test blokları (`test "..." { }`) nitelik taşımaz; `Test` öğesinin
  kendi `attrs` listesi denetlenir, gövdesi değil.
- Önceden var olan boşluk (bu ADR kapsamı dışı, ayrı iş): parser,
  kontrat anahtar kelimesi ve pipeline `stage/stall/flush` önündeki
  nitelikleri ve `@mmio` register bildirimlerinin (`mmio_regs`) ek
  niteliklerini AST'ye taşımadan düşürür; oralara yazılan `@timing`
  ya da `@allow(unenforced)` hiç görülmez. Aynı "sessizce yok sayma"
  sınıfı; parser düzeltmesi ayrı bir ADR/PR ister.

## Ölçütler (bu tur)

- `volt check examples/vga/vga_top.volt` → `0 error(s), 2 warning(s)`
  (W0021 vga_timing.volt:47, W3006 frame_buffer.volt). Örnek `@timing`i
  bilerek korur; uyarı istenen durumdur.
- `tests/ui/pass/63_unenforced_attribute_warns.volt` yalnız W0021;
  `64_allow_unenforced_silences.volt` sıfır tanı.
- `volt explain W0021` iki dilde, NOT bölümü bu ADR'ye işaret eder.
- `cargo test --all` yeşil; +35 test (23 attrs_tests, 6 CLI, 2 explain, 2 LSP, 2 manifest); mevcut testler değişmeden geçer.
