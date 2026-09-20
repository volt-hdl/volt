# ADR-0060: Test Bloğunda Sabit Yayılımı

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-20
> Etkilenen: volt-hir (`sim_const.rs` YENİ — `TestConsts`; `sim.rs`,
> `sim_expr.rs`, `sim_port.rs`), volt-driver (`sim_lower.rs`),
> volt-diagnostics (E8512 açıklaması), tests/ui/fail/61, tests/ui/pass/81,
> `.test-baseline`.
> DOKUNULMADI: README.md, examples/, docs/spec/.
> İlgili: ADR-0059 (port genişlik denetimi — bu ADR onun "bilinen
> sınır"ını kapatır), ADR-0058 (test dili: `let`, `for`), ADR-0033 (test
> denetimi isim çözümlemeden bağımsızdır).

## Sorun

ADR-0059 "Bilinen sınırlar":

> `let n = 8; dut.addr = n` derlemede değil koşuda yakalanıyor; veri
> akışı analizi yapmadım.

`const_test_value` yalnız literallerden oluşan ifadeleri katlıyordu;
ifadede bir ad geçtiği an değer "hesaplanmış" sayılıyor ve denetim
testbench'e bırakılıyordu. Hata kaybolmuyordu ama geç geliyordu:
kullanıcı Verilator derlemesini (saniyeler–dakikalar) bekleyip testin
düştüğünü görüyordu; `volt check` ve LSP ise hiçbir şey söylemiyordu.
Oysa `n`nin 8 olduğu kaynakta yazılıdır.

## Gözlem: test dili zaten tek atamalıdır

Genel bir dilde bu iş veri akışı analizi ister (dallanma birleşimleri,
yeniden atama, döngü sabit noktaları). Test dilinde (ADR-0033/0058) üçü
de YOKTUR:

| Soru | Yanıt | Kanıt |
|---|---|---|
| Koşullu atama? | Test dilinde `if` yok | `TestStmt`: LetDut, SetPort, Call, LetVar, For |
| Yeniden atama? | Yok; `ad = değer` deyimi yok, yalnız `dut.port = değer` | aynı |
| Gölgeleme? | Aynı adla ikinci `let` E8506 | `duplicate_name` |

Yani her `let` bağlandığı andan kapsamı bitene kadar TEK değere sahiptir.
Sağ tarafı sabitse ad da sabittir; analiz bir ad → değer ortamına iner.

## Seçenekler

1. **Olduğu gibi bırak (koşuda yakala).** Doğru ama geç; `volt check`
   sessiz. Reddedildi.
2. **Genel const değerlendiriciyi (`ConstEvaluator`) test bloğuna aç.**
   İsim çözümleme sonucu ister; test denetimi bilerek ondan bağımsızdır
   (ADR-0033: kardeş dosya kuralı, tek dosyalık `analyze`/LSP). Test
   değerlerinin anlamı da farklıdır (64 bit sarmalı, C++ ile birebir).
   Reddedildi.
3. **Kapsam çerçeveli sabit ortamı (SEÇİLDİ).** `TestConsts`: üst düzey
   literal `const`lar + çerçevelerde `let` bağlamaları; katlama mevcut
   `fold_binary` ile (üretilen C++ ile aynı anlam).

## Karar

### 1. Ortam: `volt_hir::TestConsts`

Her yerel ad `Some(değer)` (sabit) ya da `None` (çalışma zamanı) olarak
bağlanır. `None` da bir bağlamadır: dış çerçevedeki ya da üst düzeydeki
aynı adlı sabiti GÖLGELER (`for LIMIT in 0..4` içinde `LIMIT` sayaçtır).

| Biçim | Sonuç |
|---|---|
| `let n = 8;` | sabit 8 |
| `let m = n + 1;` | sabit (n sabitse); `!`, tüm ikili işleçler |
| `let k = SOME_CONST;` | sabit — üst düzey `const` düz literalse |
| `dut.addr = n * 2` | kullanım yerinde de katlanır |
| `let x = dut.out;` | SABİT DEĞİL (port okuması) |
| `for i in 0..4 { }` | `i` SABİT DEĞİL (sayaç) |
| `let y = x + 1;` / `let n = i + 6;` | sabit değil (zehir yayılır) |
| `t[i]`, `len(t)` | sabit değil (bkz. Sınırlar) |
| `let z = 0; 4 / z` | sabit değil — sıfıra bölme koşuda testi düşürür (ADR-0059 ile aynı) |

Anlam `const_test_value` ile aynıdır (64 bitte sarar, ≥64 kaydırma 0);
o işlev artık boş ortamda `TestConsts::eval`dir.

### 2. Tek ortam, iki tüketici

Ortam `volt-hir`dedir ve iki yerde AYNI kurallarla yürür:

- **Denetim** (`sim.rs` `Checker` → `Scope.consts`): `check_set_port_value`
  ve `check_assert_compare` sabiti ortamdan alır → **E8512 derleme
  zamanında**, `volt check` ve LSP dahil. Sabit indeksle dizi sınırı
  (`let i = 9; t[i]`, E8511) da artık ortamdan değerlendirilir.
- **İndirgeme** (`sim_lower.rs` `Lowering.consts`): sabit + bilinen
  genişlik → `TbStep::SetPort` (desen olarak, korumasız — ADR-0059 §4
  kural 1). E8512 kararı ile betiğe yazılan desen aynı değerden çıkar;
  denetimden kaçmış taşan sabit indirgenmez (`None` → içsel hata), sessizce
  yazılmaz.

Görev tanımı ortamı `sim_lower.rs`de öngörüyordu; E8512 bir `Diagnostic`
olduğu ve `volt check`te de görünmesi gerektiği için ortam hir'e kondu,
`sim_lower.rs` onu kullanır. İki ayrı uygulama olsaydı "denetim geçti ama
indirgeme başka değer yazdı" sınıfı hatalar mümkün olurdu.

`let n = 7;` betikte yine C++ değişkeni olarak tanımlanır
(`TbStep::LetScalar`): `assert_eq(dut.echo, n)` onu okur. Yalnız porta
yazım literal desene iner. ADR-0060 özelliği kullanmayan testlerin
testbench'i bayt bayt aynıdır (golden kayıtları değişmedi).

### 3. Üst düzey `const` test bloğunda görünür

`let k = SOME_CONST;` eskiden E8506 ("tanımlı değil") idi. Artık
`sources`taki (test dosyası + kardeşi) üst düzey `const`lar görünür:

- **Düz literal** (`const LIMIT : u8 = 8`, `true`, eksi işaretli
  `const NEG : i8 = -1`): sabit; betiğe değeriyle (`TbValue::Lit`) yazılır
  — C++ tarafında böyle bir değişken yoktur. Negatif literal test
  değerleri gibi 64 bitte sarar (`0 - 1`), işaretli porta ADR-0059 §2
  kuralıyla sığar.
- **Hesaplanmış** (`const B : u8 = A + 1`) ya da 64 bite sığmayan: değeri
  burada çözülmez (seçenek 2'nin gerekçesi). SESSİZ KALMAZ: **E8506**
  `const 'B' is not a plain literal; its value is not visible in a test`
  + çözüm (`let B = ...;`). Yeni hata kodu açılmadı; E8506 zaten "bu ad
  bu testte kullanılamaz" sınıfıdır.

Yerel ad üst düzey sabiti gölgeler (hata değil; Rust ile aynı). Dizi
konumunda (`len(LIMIT)`, `LIMIT[0]`, `load` kaynağı) üst düzey sabit
E8511'dir ("sabit sayı, dizi değil"), E8506 değil.

**Kardeş dosya ve tek dosyalık analiz.** `m_test.volt`, `m.volt`deki
`const DEPTH`i kullanabilir; ama `compile()`, `analyze` ve LSP test
bloklarını kardeşi YÜKLEMEDEN denetler. O kipte (`assume_external_modules`
— dosyada hiç modül yok) bilinmeyen bir ad kardeşin sabiti olabilir:
E8506 verilmez, karar kardeşli tam denetime (`volt test`) kalır. Bu,
modül/port denetiminin aynı kipte zaten yaptığı gevşetmedir (ADR-0033);
bedeli de aynıdır: böyle bir dosyada `volt check` ad yazım hatasını
göremez, `volt test` görür. İlk uygulama bunu atlamıştı — tek dosyalık
ön denetim `DEPTH` için E8506, tam denetim aynı ad için E8512 veriyordu
(kod incelemesinde bulundu; `sibling_const_*` testleri nöbette).

Yinelenen `let` (E8506) sonrası ad ortamda "bilinmiyor"a iner: hangi
bağlamanın kastedildiği belirsizken eski değerle ikinci bir E8512
üretilmez.

### 4. Tanı

E8512 değişmedi; değer adlardan geliyorsa bir not satırı eklenir:

```
error[E8512]: value does not fit in port width
   ┌─ t_test.volt:14:16
   │
14 │     dut.addr = m;
   │         ----   ^ value 8
   │         │
   │         port 'addr' is u3 (max 7)
   │
   = note: known at compile time: m = 8
   = help: use a value in range 0..7
   = for more: volt explain E8512
```

(`let n = 7; let m = n + 1;` sonrası; not, ifadede DOĞRUDAN geçen adları
verir — `m`nin `n`den türediği kaynakta bir satır yukarıdadır.)

İfadede geçen her sabit ad (ilk görülme sırası, tekil) listelenir.
Bağlamanın konumu ikincil etiket olarak GÖSTERİLMEZ: üst düzey `const`
kardeş dosyada olabilir ve kardeş ayrı `SourceMap` ile derlenir
(`FileId` çakışır).

## Sınırlar

| Durum | Davranış | Sessiz mi? |
|---|---|---|
| Koşullu atama | Dilde `if` yok — durum oluşmaz | — |
| Yeniden atama | Dilde yok; ikinci `let` E8506 — durum oluşmaz | — |
| Döngü içinde `let` | Her yinelemede yeniden bağlanır; sağ taraf sayaca bağlı değilse SABİT (`for i … { let n = 8; dut.addr = n }` → E8512), bağlıysa çalışma zamanı | Hayır |
| Döngü sayacı, sınırları sabit olsa da (`for i in 0..4`) | Sabit değil; aralık analizi yok → koşuda denetlenir (`loop: i = 8`) | Hayır |
| Port okuması ve ona bağlı her `let` | Koşuda denetlenir (ADR-0059 §4) | Hayır |
| Dizi elemanı `t[0]` (literal dizi, sabit indeks) | Katlanmaz → koşuda denetlenir. İçerik derlemede bilinse de `Scope` yalnız uzunluk/en büyük değeri taşır | Hayır |
| `len(t)` | Katlanmaz → koşuda denetlenir | Hayır |
| Sabit sıfıra bölme | Sabit değil → koşuda test düşer | Hayır |
| Hesaplanmış üst düzey `const` | E8506 (derleme) | Hayır |
| Modülsüz test dosyasında bilinmeyen ad, tek dosyalık analiz (`volt check`, LSP) | Tanı yok — kardeşin sabiti olabilir; `volt test` tam denetimde E8506 | `check`te evet (ADR-0033 modül/port gevşetmesiyle aynı bedel), `test`te hayır |
| Genişliği çözülemeyen port (takma ad, generic) | Sabit bilinse de C++ depolama tipine göre koşuda (ADR-0059 §1) | Hayır |
| `step(n)`, `n` sabit 0 | Yalnız literal `step(0)` E8505; yayılmış 0 koşuya kalır | Hayır (0 adım) |

"Desteklenmeyen durum koşuya düşer" kuralı yapısaldır: `eval` `None`
dönünce indirgeme ADR-0059'un `SetPortChecked` yoluna girer; ayrı bir
"atla" dalı yoktur.

## Mevcut testler

- `computed_value_is_left_to_the_testbench` (hir) `let n = 8` örneğini
  eski davranış olarak sabitliyordu; örnek port okumasına çevrildi, eski
  örnek `constant_let_is_checked_at_compile_time` (E8512) oldu.
- Çalışma zamanı yolunu sabit `let` ile zorlayan testler
  (`sim_bounds_tests` ×4, `sim_lower` ×1) gerçek çalışma zamanı değerine
  (port okuması / döngü sayacı) çevrildi; hiçbir test silinmedi.

## Sonuçlar

- `volt check`, `volt test` ve LSP `let n = 8; dut.addr = n` için E8512
  verir; Verilator beklenmez.
- Sabit `let` ile yazımda çalışma zamanı koruması üretilmez (daha kısa
  testbench).
- Test dili bir gün `if` ya da yeniden atama kazanırsa bu ADR'nin dayanağı
  düşer: `TestConsts` o ADR'de birleşim noktalarında "bilinmiyor"a inen
  bir kafese genişletilmelidir. `rebinding_is_rejected_so_single_assignment_holds`
  testi bu varsayımı nöbette tutar.
- Doğrulama: Docker/Verilator'da `sim_bounds_tests` 21/21, 59 RISC-V
  testi, `tests/ui/pass/81` 2/2.
