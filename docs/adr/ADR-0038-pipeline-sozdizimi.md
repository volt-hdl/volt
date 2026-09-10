# ADR-0038: Pipeline Sözdizimi — `pipeline`, `stage`, `stall`, `flush`

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-10
> Etkilenen: grammar-full.ebnf §1/§4a/§17 (PipelineDecl, StageDecl,
> StallStmt, FlushStmt, StageRef), volt-syntax (keyword terfisi, parser,
> desugar), volt-ast (PipelineDecl yapıları, TimingInfo.pinned),
> volt-hir (timing.rs pinned tanımlar), volt-diagnostics
> (E5011–E5016), examples/riscv_pipeline.volt
> Uygulama aşaması: ADR-0037 (L1) devamı — L2 stall/flush semantiği

## Sorun

L1 (ADR-0037) hizalama hatalarını yakalıyor; ancak
`examples/riscv_pipeline.volt` deneyi gösterdi ki elle yazılmış bir
boru hattında derleyicinin göremediği dört yapısal yük kalıyor:

1. Aşama register'ları elle yazılıyor (10 register, 283 satırın
   büyük kısmı boilerplate).
2. Forwarding ~40 satır elle kurulmuş; her yol insan onaylı
   "yeniden zamanlama iddiası" gerektiriyor.
3. Aynı gecikmedeki farklı aşamalar tip sisteminde ayırt edilemiyor.
4. Stall/flush tamamen manuel; unutulan bir `!stall` muhafızı
   (riscv keşfindeki 3. hata sınıfı) derleyicinin görüş alanı
   dışında. ADR-0037 bu sınıfı açıkça L2'ye devretmişti:
   "`stall`/`flush` anahtar kelimeleri bu amaçla rezervedir."

## Karar

### 1. Yüzey sözdizimi

`pipeline`, `stage`, `stall`, `flush` ayrılmış kelime listesinden
aktif anahtar kelimelere terfi eder (E0003 üretmezler).

```volt
pipeline(5) RiscvCore {
    in  clk : clock
    in  instr : u32
    out pc : u32
    // kontratlar (invariant/cover/...) modüldeki gibi

    reg pc_r : u32 = 0            // mimari durum: modül seviyesinde

    stage Fetch {
        let next_pc : u32 = pc_r + 4
        pc_r <= next_pc            // aşama içi ardışık atama
    }
    stage Decode { ... }
    stage Execute { ... }
    stage Memory { ... }
    stage Writeback { ... }

    stall Fetch, Decode when hazard     // veya aşama içinde: stall when hazard
    flush Fetch, Decode when redirect

    pc = pc_r                      // modül seviyesi çıkış ataması
}
```

`pipeline(N)`: `N` tamsayı literali aşama sayısıdır; blok içindeki
`stage` sayısıyla eşleşmezse E5011. Aşama `i`'nin gecikmesi `i`'dir
(ilk aşama 0). Pipeline bildirimi örtük olarak `@strict_timing`
niteliği taşır — L1 denetimi her pipeline'da açıktır.

### 2. Aşamalar arası değer akışı — otomatik register

Bir aşamada `let x : T = ...` ile tanımlanan değer, sonraki
aşamalardan isimle referans edilebilir; derleyici gereken aşama
register'larını üretir. `i` aşamasında tanımlanan `x`, `b ≥ i`
sınırında `"<aşama_b>_x_r"` adlı register'a yazılır
(`fetch_instr_r`, `decode_rs1_v_r`, ...). `j > i` aşamasından `x`
referansı `"<aşama_{j-1}>_x_r"` okumasına çevrilir.

Kurallar:
- Aşama sınırını geçen (veya `stage(...)` ile referans edilen) her
  `let` **açık skaler tip** taşımalıdır (`bool`, `uN`, `iN`,
  `bits<K>`): register bildirimi ve bubble değeri bu tipten türetilir
  (`bool → false`, sayısal → `0`). İhlal E5014. (F0'ın "reg tipi
  açık yazılmalı" kuralının — E2012 — pipeline karşılığıdır.)
- Aşama-yerel `let` isimleri pipeline genelinde benzersizdir (E5016);
  gölgeleme yoktur, üretilen SV isimleri birebir izlenebilir kalır.
- Aşama gövdesi `let` + ardışık atama (`<=`) + `if`/`match` içerir.
  Ardışık atamalar modül seviyesindeki mimari register'lara yazar ve
  içinde bulundukları aşamanın stall muhafızıyla (`if (!stall_<s>)`)
  sarılır.

### 3. Aşama referansları — `stage(X).y`

`stage(Ad).y`, `stage(+k).y`, `stage(-k).y`: `y` değerinin `X`
aşamasındaki görünümü. `y`'nin tanım aşaması `d(y)` olmak üzere
`X == d(y)` ise canlı kombinasyonel sinyal, `X > d(y)` ise
`"<aşama_{X-1}>_y_r"` register'ıdır. `X < d(y)` (değer henüz yok),
`X ≥ N`, bilinmeyen aşama adı veya bilinmeyen `y` → E5012.
Göreli biçim (`+k`/`-k`) aşama gövdesi dışında geçersizdir (E5012).
Erken aşamadan geç aşamaya `stage(X).y` okuması serbesttir — bu,
donanımdaki geri besleme telidir (örn. EX'ten PC yönlendirmesi).

**Tip kuralı:** `stage(X).y` ifadesi `X` aşamasının gecikmesinde bir
`Delayed<T, N_X>` değeridir. Başlatıcısında `stage(...)` geçen bir
aşama-yerel `let`, kendi aşamasının gecikmesine sabitlenmiş bir
*yeniden zamanlama iddiasına* (ADR-0037 §4) dönüşür: bilinçli aşama
karışımı tam olarak `stage(...)` yazımının görünür kıldığı yerdedir,
ayrıca el ile `Delayed` anotasyonu istenmez. Forwarding böylece
üç satırlık bir `if` zincirine iner.

Taşınan `let`'ler üretilen modülde bağımlılık sırasına dizilir
(çözümleme sıralı olduğundan); `stage(...)` üzerinden kombinasyonel
çevrim E5015.

### 4. Stall

- `stall when koşul` (aşama gövdesinde): o aşama dahil önceki tüm
  aşamalar durur; bir sonraki aşamaya bubble girer.
- `stall S1, ..., Sk when koşul` (modül veya aşama seviyesi):
  listelenen küme ilk aşamadan başlayan **bitişik bir önek**
  olmalıdır — boru hattının ortası, öncesi durmadan duramaz.
  İhlal, bilinmeyen aşama adı veya modül seviyesinde listesiz
  `stall when` → E5013.
- Etki: durdurulan aşamaların sınır register'ları güncellenmez
  (hold), önekten sonraki ilk sınıra bubble (init değerleri)
  yazılır, durdurulan aşamaların ardışık atamaları koşulmaz.
- Her `s` aşaması için `stall_<s> : bool` sinyali üretilir
  (kapsayan koşulların OR'u); kullanıcı koda görünürdür
  (`stall_o = stall_fetch` gibi debug çıkışları için).

### 5. Flush

- `flush S1, ..., Sk when koşul`: listelenen aşamalardaki uçuş
  halindeki komutlar ezilir — o aşamaların sınır register'larına
  init (bubble) değerleri yazılır. Küme serbesttir (önek şartı yok);
  bilinmeyen aşama adı E5013.
- `flush_<s> : bool` sinyali üretilir.
- Öncelik: aynı çevrimde flush > hold > bubble > normal yazım.
  Sınır `b` için üretilen şablon:

  ```systemverilog
  if (flush_<b>) begin <b>_x_r <= '0; ... end
  else if (!stall_<b+1>) begin
      if (stall_<b>) begin <b>_x_r <= '0; ... end   // bubble
      else begin <b>_x_r <= x; ... end
  end                                                // hold
  ```

  (Önek kuralı gereği `stall_<b> && !stall_<b+1>` yalnız önekin son
  aşamasında doğru olabilir.) V0 sınırı: stall ve flush koşullarının
  aynı çevrimde çakışmaması kullanıcı sorumluluğundadır (riscv'de
  yapısal olarak ayrıktır: load-use ↔ branch).

### 6. Üretilen SV ve desugar mimarisi

Pipeline bildirimi **parser içinde** (volt-syntax) sıradan bir
`ModuleDecl`'e indirgenir — ADR-0037'nin tip düzeyinde silme
ilkesinin L2 karşılığı. İsim çözümleme, tip denetimi, CDC, L1
zamanlama, kontrat/SVA üretimi ve SV emit **değişmeden** çalışır;
`volt-sv-emit` pipeline'ı hiç görmez. Üretilen SV, elle yazılmış
eşdeğerinden ayırt edilemez ve okunabilirdir (ADR-0012):
`<aşama>_<isim>_r` register'ları, `stall_<aşama>`/`flush_<aşama>`
telleri, tek `always_ff`.

Zamanlama sabitleme bilgisi `SourceFile.timing.pinned` yan
tablosuyla (bildirim adı span'i → çevrim) taşınır; `timing.rs`
bunları açık `Delayed` anotasyonu gibi okur. Pipeline'ın tek saat
portu olmalıdır (E5011); üretilen `on clk` bloğu bu portu kullanır.

### 7. Tanı kodları (5 parça, `volt explain` iki dilde)

| Kod | Anlam |
|---|---|
| E5011 | Pipeline yapısı: aşama sayısı ≠ N, yinelenen aşama adı, aşama yok, saat portu ≠ 1 |
| E5012 | Geçersiz aşama referansı: bilinmeyen aşama/değer, menzil dışı, değer henüz tanımsız, gövde dışında göreli biçim |
| E5013 | Geçersiz stall/flush: önek olmayan stall kümesi, bilinmeyen aşama, modül seviyesinde listesiz stall |
| E5014 | Aşama sınırını geçen değerde açık skaler tip yok |
| E5015 | `stage(...)` üzerinden kombinasyonel çevrim |
| E5016 | Aşama-yerel değer adı pipeline genelinde benzersiz değil |

### 8. V0 sınırları (bilinçli)

- Sınırı geçen değerler skalerdir; dizi/struct/tuple taşınmaz.
- Stall/flush koşulları kontrol sinyalidir, L1 denetimi dışındadır.
- Bubble değeri tipin sıfırıdır; özel NOP kodlaması kullanıcı
  kontrol bitlerinin sıfırında "geçersiz" anlamına gelecek şekilde
  kodlamalıdır (riscv'de `wb_en=false` yeterlidir).
- `pipeline` generic parametre almaz; `N` tamsayı literalidir.
- Aynı çevrimde çakışan stall+flush çözümü §5'teki önceliktir;
  koşulların ayrıklığı doğrulanmaz.

## Sonuçlar

- riscv_pipeline 283 → ~150 satıra iner; 10 elle yazılmış register
  ve ~40 satır forwarding boilerplate'i düşer.
- Keşif raporundaki 3. hata sınıfı (stall'da IF/ID güncellemesi)
  artık **dilde ifade edilemez**: aşama register'larını ve stall
  muhafızlarını derleyici ürettiğinden "muhafızı unutmak" diye bir
  yazım kalmaz. Yakalamaktan güçlü olan bu yapısal önlemenin bedeli,
  1. ve 2. hata sınıflarının (bypass eksikliği, yanlış aşamadan
  forwarding) `stage(...)` iddiaları içinde hâlâ denetim dışı
  kalmasıdır — ADR-0037'nin bilinen sınırı geçerliliğini korur,
  testler ve formal kanıt bu sınıfları örter.
- Geriye uyumluluk: `pipeline`/`stage`/`stall`/`flush` bugüne dek
  E0003 ürettiğinden hiçbir geçerli program bu kelimeleri
  kullanmıyordu; terfi kırıcı değildir. `module` bildirimleri
  değişmez.
