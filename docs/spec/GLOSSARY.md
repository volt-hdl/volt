# Volt HDL — Terminoloji Sözlüğü / Terminology Glossary

> STATÜ: BAĞLAYICI — çeviri tutarlılığının temeli
> STATUS: BINDING — foundation of translation consistency
>
> Bu tabloya uymayan çeviri kabul edilmez.
> Translations not following this table are rejected.

---

## 0. Kural / Rule

```
DEĞİŞMEZ (her iki dilde aynı kalır):
NEVER TRANSLATED (identical in both languages):

  Anahtar kelimeler    module, in, out, reg, on, comb, let, wire
  Tip isimleri         bool, clock, u8, i16, bits<N>, Trit
  Hata kodları         E3001, W1002
  Fonksiyon isimleri   sync(), clog2(), zext()
  Dosya adları         grammar-full.ebnf, type-inference.md
  Rust kodu            fn, struct, enum, impl, match
  CLI komutları        volt build, volt check
```

---

## 1. Temel Kavramlar / Core Concepts

| Türkçe | English | Not / Note |
|---|---|---|
| donanım tanımlama dili | hardware description language | HDL kısaltması ortak |
| saat alanı | clock domain | |
| saat alanı geçişi | clock domain crossing | CDC kısaltması ortak |
| sıfırlama alanı | reset domain | RDC kısaltması ortak |
| güç alanı | power domain | PDC kısaltması ortak |
| alan / etki alanı | domain | tek kelime tercih: "alan" |
| sinyal | signal | |
| kaydedici | register | "register" de kabul |
| tel | wire | |
| kapı | gate | |
| kenar | edge | posedge/negedge çevrilmez |
| metastabilite | metastability | |
| geçici darbe | glitch | |
| kurulum süresi | setup time | |
| tutma süresi | hold time | |
| senkronizatör | synchronizer | |
| boru hattı | pipeline | "pipeline" da kabul |
| aşama | stage | |
| durum makinesi | state machine | FSM kısaltması ortak |
| silisyum | silicon | |
| sentez | synthesis | |
| yerleştirme ve yönlendirme | place and route | P&R kısaltması ortak |
| netlist | netlist | çevrilmez |
| tape-out | tape-out | çevrilmez |
| yeniden dönüş | re-spin | |

---

## 2. Derleyici / Compiler

| Türkçe | English | Not |
|---|---|---|
| derleyici | compiler | |
| sözcüksel çözümleyici | lexer | "lexer" de kabul |
| ayrıştırıcı | parser | "parser" de kabul |
| ayrıştırma | parsing | |
| belirteç | token | "token" de kabul |
| gramer | grammar | |
| sözdizim | syntax | |
| anlambilim | semantics | |
| soyut sözdizim ağacı | abstract syntax tree | AST kısaltması ortak |
| somut sözdizim ağacı | concrete syntax tree | CST kısaltması ortak |
| ara temsil | intermediate representation | IR kısaltması ortak |
| tip çıkarımı | type inference | |
| tip kontrolü | type checking | |
| isim çözümleme | name resolution | |
| kapsam | scope | |
| bağlama | binding | |
| gölgeleme | shadowing | |
| tanım | definition | |
| bildirim | declaration | |
| ileri referans | forward reference | |
| derleme zamanı | compile time | |
| çalışma zamanı | runtime | |
| sabit değerlendirme | constant evaluation | const eval |
| düşürme | lowering | "lowering" de kabul |
| kod üretimi | code generation | codegen |
| artımlı derleme | incremental compilation | |
| hata kurtarma | error recovery | |
| senkronizasyon noktası | synchronization point | |
| kaskad hata | cascading error | |
| tanı | diagnostic | |
| önbellek | cache | |

---

## 3. Tip Sistemi / Type System

| Türkçe | English | Not |
|---|---|---|
| tip | type | |
| bit genişliği | bit width | |
| işaretli | signed | |
| işaretsiz | unsigned | |
| taşma | overflow | |
| taşma genişlemesi | overflow widening | Volt'a özgü |
| genişletme | widening / extension | |
| daraltma | narrowing / truncation | |
| işaret genişletme | sign extension | |
| sıfır genişletme | zero extension | |
| tip dönüşümü | cast | |
| örtük | implicit | |
| açık | explicit | |
| atanabilirlik | assignability | |
| çift yönlü | bidirectional | |
| sentez modu | synthesis mode | tip çıkarımında |
| kontrol modu | checking mode | |
| dengeli üçlü | balanced ternary | Trit |
| yerleşik | builtin | |
| ön tanımlı | prelude | "prelude" de kabul |
| lineer tip | linear type | |
| ters tel | inverted wire | `&inv` |
| genel tip parametresi | generic parameter | |

---

## 4. Doğrulama / Verification

| Türkçe | English | Not |
|---|---|---|
| doğrulama | verification | |
| biçimsel doğrulama | formal verification | "formal" de kabul |
| iddia | assertion | |
| değişmez | invariant | |
| ön koşul | precondition | `requires` |
| son koşul | postcondition | `ensures` |
| kapsam hedefi | coverage goal | `cover` |
| karşı örnek | counterexample | |
| sınırlı model denetimi | bounded model checking | BMC |
| kapsam | coverage | |
| test tezgahı | testbench | |
| benzetim | simulation | "simülasyon" da kabul |
| dalga formu | waveform | |
| test vektörü | test vector | |
| kesin eşleşme | bit-exact | |

---

## 5. Kontrat Sistemi / Contract System

| Türkçe | English |
|---|---|
| kontrat | contract |
| kaynak bütçesi | resource budget |
| zamanlama kısıtı | timing constraint |
| yanlış yol | false path |
| çok döngülü yol | multicycle path |
| test edilebilirlik | design for test (DFT) |
| tarama zinciri | scan chain |
| hata kapsamı | fault coverage |
| yeniden üretilebilirlik | reproducibility |
| belirlenimcilik | determinism |
| anlamsal sürümleme | semantic versioning |
| kırıcı değişiklik | breaking change |

---

## 6. Donanım Teknolojisi / Hardware Technology

| Türkçe | English |
|---|---|
| bellek duvarı | memory wall |
| bant genişliği | bandwidth |
| gecikme | latency |
| verim | throughput |
| bellekte hesaplama | compute-in-memory (CIM) |
| bellek içi işlem | processing-in-memory (PIM) |
| sistolik dizi | systolic array |
| çarp-topla | multiply-accumulate (MAC) |
| işlem birimi | processing element (PE) |
| sıfır atlama | zero skipping |
| seyreklik | sparsity |
| niceleme | quantization |
| kalıcı bellek | non-volatile memory |
| dayanıklılık | endurance |
| kayma | drift |
| yalıtım hücresi | isolation cell |
| seviye kaydırıcı | level shifter |
| tutma | retention |
| nöromorfik | neuromorphic |
| ateşleme | spike |
| sinaps | synapse |
| fotonik | photonic |
| dalga kılavuzu | waveguide |
| faz kayması | phase shift |
| atım gürültüsü | shot noise |
| gürültü marjı | noise margin |

---

## 7. Hata Mesajı Şablonu / Error Message Template

Beş parça her iki dilde de zorunlu:

| Türkçe | English |
|---|---|
| `error[E3001]:` | `error[E3001]:` |
| `= neden:` | `= reason:` |
| `= çözüm:` | `= help:` |
| `= not:` | `= note:` |
| `= daha fazla:` | `= for more:` |

**Örnek / Example:**

```
TR:
error[E3001]: iki farklı saat alanı doğrudan bağlanamaz
  = neden: sinyal kararsız bir anda yakalanabilir
  = çözüm: result = sync(data, slow_clk)
  = daha fazla: volt explain E3001

EN:
error[E3001]: cannot connect two different clock domains directly
  = reason: the signal may be sampled during an unstable window
  = help: result = sync(data, slow_clk)
  = for more: volt explain E3001
```

---

## 8. Belge Başlıkları / Document Headers

| Türkçe | English |
|---|---|
| STATÜ: BAĞLAYICI | STATUS: BINDING |
| STATÜ: Arka plan araştırması | STATUS: Background research |
| Bağlayıcı karar değildir | Not a binding decision |
| İlgili | Related |
| Aşama | Phase |
| Tasarım İlkeleri | Design Principles |
| Gerekçe | Rationale |
| Alternatifler | Alternatives |
| Sonuçlar | Consequences |
| Uygulama Sırası | Implementation Order |
| Test Vektörleri | Test Vectors |
| Hata Kodları | Error Codes |
| Kural | Rule |
| Örnek | Example |
| Not | Note |
| Uyarı | Warning |
| Karar | Decision |

---

## 9. Sık Yapılan Çeviri Hataları / Common Mistakes

```
✗ "clock area"           ✓ "clock domain"
✗ "sign expansion"       ✓ "sign extension"
✗ "type deduction"       ✓ "type inference"
✗ "name solving"         ✓ "name resolution"
✗ "wrong path"           ✓ "false path"
✗ "memory calculation"   ✓ "compute-in-memory"
✗ "fire"                 ✓ "spike"
✗ "reset area"           ✓ "reset domain"
✗ "compile moment"       ✓ "compile time"
✗ "declaration order"    ✓ "forward reference" (bağlama göre)
```

---

## 10. Dosya Adlandırma / File Naming

```
Spec dosyaları (İngilizce, tireli):
  grammar-full.ebnf
  type-inference.md
  domain-inference.md

Türkçe çeviriler (tr/ alt dizininde, AYNI isim):
  docs/spec/tr/type-inference.md
  docs/spec/tr/domain-inference.md

Araştırma (Türkçe, Volt- önekli):
  Volt-Optik-Gurultu-Analizi.md
  → İngilizce sürüm YOK (kişisel arka plan)
```

---

## 11. Çeviri Kontrol Listesi / Translation Checklist

```
□ Terminoloji bu sözlüğe uygun mu?
□ Kod blokları DEĞİŞMEDEN kopyalandı mı?
□ Hata kodları (E3001) aynı mı?
□ Anahtar kelimeler (module, reg, on) çevrilmedi mi?
□ Hata mesajı şablonu (= reason/help/note) doğru mu?
□ Başlık numaraları eşleşiyor mu? (§3.2 ↔ §3.2)
□ Tablo satır sayısı aynı mı?
□ Dosya sonundaki test vektörleri korundu mu?
```
