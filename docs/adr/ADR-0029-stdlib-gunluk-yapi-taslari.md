# ADR-0029: Stdlib Genişletmesi — Tek Saatli Günlük Yapı Taşları

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-08
> Etkilenen: volt-ast (builtin tablosu), volt-hir (E2025 genellemesi,
> W3006, port tipleri), volt-diagnostics (W3006), volt-sv-emit
> (primitif SV/kontrat üretimi), tests/ui/pass/30-35, docs/stdlib.md
> Önceki karar: ADR-0027 (stdlib mimarisi — Rust'ta yerleşik)

## Sorun

ADR-0027 yalnız CDC primitiflerini (AsyncFifo, HandshakeSync,
PulseSync) getirdi. Günlük tasarımın yapı taşları — FIFO, RAM, sayaç,
kaydırma, arbitrasyon, kenar algılama — hâlâ elle yazılıyor. Aynı
mimari desenle (Seçenek B: Rust'ta yerleşik) sekiz tek saatli primitif
eklenir.

## Karar

`SyncFifo<T, DEPTH>`, `Ram<T, DEPTH>`, `DualPortRam<T, DEPTH>`,
`Counter<WIDTH>`, `ShiftRegister<T, LEN>`, `RoundRobinArbiter<N>`,
`PriorityArbiter<N>`, `EdgeDetect` yerleşik primitif tablosuna eklenir
(volt-ast/builtin.rs). Ayrıntılı imza/port/kontrat referansı:
docs/stdlib.md.

### Tek saat modeli

CDC primitiflerinden farklı olarak bu primitiflerin TEK `clk` portu
vardır ve tüm portlar aynı alandadır (DomainRole::Src). Emisyonda
`dst_clock == src_clock` kabul edilir; `multiclock` akışı değişmez
(tek saatli modüller clk2fflogic'e girmez).

### Sabit generic kuralları (E2025 genellemesi)

E2025 ("geçersiz genişlik/boyut") tüm sabit generic argümanlara
genellenir; kural tablosu volt-ast'ta yaşar ve hem volt-hir hem
volt-sv-emit aynı kuralı uygular:

- `DEPTH` (SyncFifo/Ram/DualPortRam/AsyncFifo): iki kuvveti, 2..=65536.
  İki kuvveti kısıtı adres/sayaç sarmasını yapısal kılar ve
  `addr < DEPTH` invariant'ını kanıtlanabilir yapar.
- `WIDTH` (Counter): 1..=64. `LEN` (ShiftRegister): 2..=64 (LEN=1'in
  "kaydırması" tek register'dır; seri-paralel amaç en az iki aşama
  ister). `N` (arbiterler): 2..=64 (tek istekçiye arbiter gerekmez).

### Adres/vektör port tipleri

`uN` yalnız 8/16/32/64 için var olduğundan adresler ve vektör portları
ham bit vektörüdür: `addr : bits<clog2(DEPTH)>`, `req/grant/count :
bits<...>`, `taps : bits<LEN * width(T)>`. Bu, her derinlik için ifade
edilebilirlik sağlar; aritmetik gerektiğinde kullanıcı açık dönüşüm
yapar (§5 felsefesiyle tutarlı).

### Yeni tanı kodu: W3006 (bu ADR tanım kaynağıdır)

**W3006 — DualPortRam yazma-yazma çakışması.** İki port aynı çevrimde
aynı adrese yazarsa üretilen bellek B portunun verisini bırakır (A
önce, B sonra yazar — tek always_ff, Verilator MULTIDRIVEN'dan
kaçınmak için). Adresler çalışma zamanı değeri olduğundan çakışma
statik dışlanamaz; derleyici her DualPortRam örneklemesinde W3006
üretir (W3005 kalıbı; spec salt okunur olduğundan kod ADR ile
tanımlanır — ADR-0025/0027 emsali).

### Formal kontratlar

Her primitif Immediate modda kontratlarını üretir (docs/stdlib.md'de
tam liste). Öne çıkan kararlar:

- **SyncFifo `!(full && empty)` kanıtlanır** — AsyncFifo'da iki-flop
  gecikmesi yüzünden zayıflatılan kontrat, tek saatte tam haliyle
  geçerlidir; bayraklar tek doluluk sayacından türetilir.
- **Arbiter bir-sıcaklık** `$countones` yerine
  `(grant & (grant-1)) == 0` saf mantığıyla yazılır (her aracın
  desteklediği biçim). Her istekçi için `cover grant[k]` sınırlı
  erişilebilirliği belgeler.
- **Kenar bazlı "iz ortası reset yok" varsayımı yalnız çift saatli
  primitiflerde üretilir.** Tek saatli (clk2fflogic'siz) modelde her
  BMC adımı bir kenar örneklemesidir; `always @(posedge clk) assume
  (!rst)` 0. adımda `initial assume (rst)` ile çelişir (PREUNSAT).
  Tek alanda ADR-0027'nin korktuğu "kısmi reset" zaten imkânsızdır —
  varsayım gereksizdir.
- Zayıflatılan kontrat YOKTUR: tüm yeni invariant'lar tam haliyle
  kanıtlanır.

### İsim çakışması notu

Üretilen sinyaller `<örnek>_<port>` önekini taşır (ADR-0027 kalıbı).
Örnek adı + port adı, modülün başka bir sinyaliyle çakışabilir
(ör. `rr` örneği + `grant` portu = `rr_grant` sinyali). Bu, mevcut CDC
primitiflerinde de bulunan bilinen bir sınırdır; çakışma sentezde çift
bildirim hatası olarak yakalanır. Derleyici tarafı sezimi ayrı bir işe
bırakılmıştır.

## Sonuçlar

- **volt-ast**: BuiltinPrim 8 varyant, PortKind'e Addr/Dim/Taps,
  ConstRule tablosu, has_dst_clock(). (ADR-0027 tabloyu volt-ast'a
  koyduğundan kapsam genişlemesi buradan geçer.)
- **volt-hir**: Ty::Builtin'e `dim` alanı; E2025/E2008/E2003 metinleri
  primitif adı/parametre adıyla genellendi; W3006 domain denetiminde
  üretilir.
- **volt-diagnostics**: W3006 kod tablosu + messages/{en,tr} +
  explain/{en,tr} (92 kod).
- **volt-sv-emit**: primitif başına gövde + Immediate kontrat üretimi.
- **tests/ui**: pass/30-35 (Verilator --lint-only -Wall temiz,
  volt verify Docker/sby ile kanıtlı), fail/25-26 (E2025).
- **docs/stdlib.md**: tüm stdlib modüllerinin referansı.

## Emsal

ADR-0027'nin FIRRTL/LLVM intrinsic emsali aynen geçerlidir. Tek saatli
yapı taşları için ek emsal: SpinalHDL `StreamFifo`/`Counter` ve Chisel
`util.Queue`/`util.Arbiter` — standart kütüphane, dil değil araç/kit
tarafında çözülür.
