# ADR-0039: Bundle (Port Grubu) Desteği — `struct port`

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-13
> Etkilenen: grammar-full.ebnf §7 (StructField), sv-mapping.md §14,
> volt-syntax (parser/bundle.rs düzleştirme, struct port alan yönü),
> volt-ast (StructField.direction/domain, Port.bundle, BundleOrigin),
> volt-hir (resolve.rs E4005, domain.rs E3013), volt-diagnostics
> (E3013, E4005), examples/axi4lite_slave.volt

## Sorun

`examples/axi4lite_slave.volt` keşfi: beş AXI4-Lite kanalının 35 portu
elle yazıldı; 346 satırlık dosyanın 71 satırı yalnız port bildirimiydi.
Aynı tasarım SystemVerilog `interface` ile ~120 satır olurdu — Volt
%188 daha uzundu. Her master/slave çifti aynı sinyal listesini ters
yönlerle iki kez yazıyor, alan eklemek her kullanım noktasını
değiştirmeyi gerektiriyordu.

Gramer `StructDecl = "struct" [ "port" ] ...` biçimini zaten
ayrıştırıyordu (`is_port` bayrağı) ama HİÇBİR anlamı yoktu: `struct
port` tipli bir port E-hatasız derlenemiyordu.

## Karar

### 1. Yüzey sözdizimi

```volt
struct port AxiWriteAddr {
    out addr  : u32          // master sürer
    out valid : bool
    in  ready : bool         // slave sürer
}

module Master { out aw : AxiWriteAddr  ... }   // alanlar bildirildiği gibi
module Slave  { in  aw : AxiWriteAddr  ... }   // her alanın yönü TERSLENİR
```

- `struct port` alanları `in`/`out` yönü ile BAŞLAMAK ZORUNDADIR;
  yönsüz alan sözdizimi hatasıdır. Sıradan `struct` alanları yön
  taşıyamaz.
- Alanlar `@Domain` anotasyonu alabilir (`out data : u8 @Fast`).
- Bundle tipli port `in` ise alan yönleri tersine çevrilir; `out`
  (ve `inout`) bildirildiği gibi kalır.
- Alan tipi başka bir `struct port` olabilir (iç içe bundle); yönler
  bileşir: `in rx : Chan` alanı Chan'ı tersler, `in link : Link` portu
  Link'i bir kez daha tersler — iki tersleme özdeşliktir.
- Erişim: `aw.addr`, iç içe `link.rx.data`; sol taraf ve sağ tarafta,
  `on`/`comb` bloklarında ve kontratlarda.

### 2. Yön semantiği: Seçenek A (alan bazlı yön + `in` tersleme)

İki seçenek değerlendirildi:

| | A) Alan bazlı yön, `in` tersler | B) `Bundle::Master` / `Bundle::Slave` görünümü |
|---|---|---|
| Tanım | Tek struct, her alan yön taşır | Tek struct + iki adlandırılmış görünüm |
| Kullanım | `in aw : T` / `out aw : T` | `in aw : T::Slave` / `out aw : T::Master` |
| Parser | `struct port` alanına PortDir | Tip yolunda ilişkili ad (`T::Slave`) — yeni tip biçimi |
| Anlamsal | Tek kural: `in` ⇒ ters | Görünüm adı + yön + alan yönü: üç kaynak |
| İç içe | XOR ile doğal bileşir | Görünüm iç içe geçince belirsiz (`Link::Slave` içinde `Chan::?`) |
| Emsal | Chisel `Flipped(...)`, FIRRTL | SV `modport` |

**A seçildi**: daha az kavram (görünüm adı yok), tip dilbilgisi
değişmiyor, iç içe bundle'da bileşim tek bir XOR'dur ve mevcut
`in`/`out` port yönü zaten "modülün bakış açısını" söylüyor. B'de hem
alan yönü hem görünüm adı yazılmak zorunda; görünümler yalnız iki
tarafı (master/slave) kapsar, üç taraflı arayüzlerde (monitor)
uzatılamaz.

### 3. Uygulama: bundle PARSER'DA DÜZLEŞİR

ADR-0038'in "silme ilkesi" izlenir: `parse_source_file` sonunda
`flatten_bundles` (parser/bundle.rs) her modül/extern modül portunu
tarar; bundle tipli port `<port>_<alan>` (iç içe
`<port>_<alan>_<alt_alan>`) adlı düz `Port`lara açılır ve gövde +
kontratlardaki `aw.addr` alan zincirleri düz yola (`aw_addr`) yeniden
yazılır. İsim çözümleme, tip denetimi, domain çıkarımı, sürücü
analizi, L1 zamanlama ve SV üretimi bundle'ı HİÇ görmez.

Düzleştirilmiş port yalnız `Port::bundle : Option<BundleOrigin>`
(kaynak port adı, bundle tipi, noktalı alan yolu, bildirilen yön,
terslendi mi) taşır; iki tanı bunu okur. Bildirim span'leri
benzersizdir (decl_spans anahtarı): port bildirimi içindeki tek
karakterlik konumlar, tükenirse öğe sonundan geriye sayan sıfır
genişlikli konumlar (pipeline desugar'ı öğe başından ileri sayar).

Struct port bildirimi modülden SONRA da gelebilir (düzleştirme tüm
öğeler okunduktan sonra koşar). Generic `struct port` ve argümanlı
tip yolları düzleştirilmez (V1'e ertelendi).

### 4. Tanılar

- **E4005 — bundle alanı yön ihlali** (volt-hir/resolve.rs,
  `resolve_lvalue`): etkin yönü giriş olan bir bundle alanına atama
  (`in req : Req` içinde `req.addr = 0`, ya da `out hs : Handshake`
  içinde `hs.ready = ...`). Beş parça: kod, konum (atama hedefi),
  açıklama (`'req.addr' yönü giriş`), öneri (`out req : Req` olarak
  bildirin), gerekçe notu (alan `'out'` bildirilmiş, `in` port tersler,
  ADR-0039) + ikincil etiket (bundle portu bildirimi).
- **E3013 — bundle alanları farklı saat alanlarında** (volt-hir/
  domain.rs, port ataması sonrası): aynı bundle portunun düzleştirilmiş
  alanları farklı `DomainId`'ye düştüyse. Kaynak: alan anotasyonlarının
  birbiriyle ya da port anotasyonuyla çelişmesi. Beş parça: kod, konum
  (bundle port adı), açıklama (`'data' @Fast ama 'ready' @Slow`),
  öneri (portun tamamını `@Fast` ile anotasyonlayın ya da arayüzü iki
  bundle'a ayırın), gerekçe notu (bundle tek arayüzdür, ADR-0039).
- Sözdizimi: yönsüz `struct port` alanı ve yönlü sıradan struct alanı
  mevcut E0001 ailesiyle (beklenen-öğe) raporlanır.

Kural: alan anotasyonu port anotasyonunu geçersiz kılar; hiçbiri yoksa
port düz portlar gibi varsayılan alanı alır (çok saatli modülde E3010).

### 5. SV üretimi: DÜZ portlar, `interface` YOK

```systemverilog
module Slave (
    input  logic [31:0] aw_addr,
    input  logic        aw_valid,
    output logic        aw_ready
);
```

Gerekçe: SV `interface`/`modport` sentez ve lint araçlarında tutarsız
desteklenir (parametreli arayüzler, hiyerarşi düzleştirme, modport
yön denetimi araçtan araca değişir); düz portlar Verilator, Yosys ve
ticari araçların hepsinde çalışır, Volt kaynağıyla bire bir
izlenebilirlik korunur ve mevcut sim/formal köprüleri (`dut.aw_addr`,
SVA kontratları) değişmeden kalır. Isimlendirme `<bundle_port>_<alan>`.

Test blokları (ADR-0033) ve örnekleme bağlamaları düz adı kullanır:
`dut.aw_valid = true;`.

## Sonuçlar

`examples/axi4lite_slave.volt` (bundle ile yeniden yazıldı):

| | Keşif (elle) | ADR-0039 |
|---|---|---|
| Satır | 346 | 150 |
| Port bildirimi satırı | 71 | 11 (5 bundle + clk + status + 4 reg) |
| SV port sayısı | 35 | 26 (+ üretilen `rst`) |
| Bundle tanımı | — | 5 struct port, 34 satır, master/slave ortak |

Doğrulama: 5 sim testi (Verilator, Docker), Verilator `-Wall` lint
temiz, 9 kontrat (5 invariant + 4 cover) SymbiYosys/boolector ile
prove (derinlik 3), bmc ve cover modlarında geçti.

İç içe bundle çalışır (`tests/ui/pass/48_bundle_nested.volt`, çift
tersleme). Testler: `tests/ui/pass/47_bundle_basic.volt`,
`48_bundle_nested.volt`, `tests/ui/fail/35_bundle_direction.volt`
(E4005), `36_bundle_domain.volt` (E3013); crate testleri
volt-syntax/tests/bundle_tests.rs, volt-hir/tests/
bundle_semantic_tests.rs, volt-sv-emit/tests/bundle_emit_tests.rs.

## Sınırlar / Ertelenen

- Bundle bir bütün olarak değer değildir: `x = aw` ya da bundle'ı
  örnekleme bağlamasına tek parça geçmek desteklenmez (düz alanlar
  bağlanır). Bilinmeyen alan (`aw.bogus`) düzleştirilmez, isim
  çözümleme `aw` için E1001 üretir — özel tanı V1.
- Generic `struct port` (`Chan<T>`) düzleştirilmez.
- `inout` bundle portunda tersleme uygulanmaz.
- Kullanılmayan bundle alanı (`aw.prot`) W1001 üretir; uyarı etiketi
  sentetik span nedeniyle port bildirimi satırında bir karaktere
  işaret eder.
- Alan bazlı `@Domain` yalnız `struct port` içinde kabul edilir.
