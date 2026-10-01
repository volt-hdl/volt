# ADR-0097: Alt Örnek Yükümlülükleri — Örneğin `requires`/`assume`'u Üst Görevde `assert`, Saatsiz Kontrat E5005, Boş Doğrulama E5006

> Statü: Uygulandı
> Önceki karar: ADR-0011, ADR-0055 — ADR-0011'in `requires → assume property` eşlemesi artık yalnız modülün KENDİ görevinde geçerli; ADR-0055 §1'in görev kümesi ("kontratlı modüller") ve kontrat sayımı değişti
> İlgili: ADR-0064 (simülasyonda alt örneğin `requires`/`assume`'u zaten üst modülün hatasıdır; formal akış artık aynı anlamı taşır), ADR-0050 (Handshake tüketici tarafı otomatik `assume`), ADR-0040 (`prev()` yardımcı register'ları), ADR-0086 (cover kipinde ulaşılmayan otomatik cover), ADR-0075 (sby durumları ve çıkış kodları).
> Tarih: 2026-10-01
> Etkilenen: volt-sv-emit (`sva.rs`: `sub_instance_macro`, `is_obligation`,
> `UnclockedContracts`; `sby.rs`: `SbyTask.sub_defines`, görev-koşullu
> `read -define`; `reach.rs`: `instance_children`), volt-driver
> (`verify_plan.rs` YENİ: görev planı; `verify.rs`: E5005/E5006, sahibi
> modüle eşleme, örnek etiketi; `verify_report.rs`: `assumed`, `context`;
> `reach.rs`), volt-diagnostics (E5005, E5006; `explain` en/tr), testler:
> `crates/volt-driver/tests/verify_obligation_tests.rs` (YENİ, 13),
> `tests/fixtures/formal_obligations/` (YENİ, 8 fixture),
> `.github/workflows/ci.yml` (`verify` işi), `book/src/limitations.md`,
> CHANGELOG.

## Sorun

PR #71 envanterinde bulundu: `volt verify` bir alt örneğin `requires`
kontratını üst modülün görevinde de `assume` olarak bırakıyordu
(`crates/volt-sv-emit/src/sva.rs:447`, `ContractKind::Requires =>
("req", "assume")`). Üst görev `prep -top <Üst>` ile alt modülün gövdesini
de elaborate eder; alt modülün `assume`'u üst modülün sürdüğü sinyali
KISITLAR. Sonuç: üst modül alt modülü kontratına aykırı sürse bile kanıt
geçer. Kullanıcının en çok güvendiği çıktıda sessiz bir yanlış.

Ölçüm (2026-10-01, `main` = 218ca66, hdlc/formal Docker, `--depth 10`):

| Durum | Fixture | Önce | Bu ADR ile |
|---|---|---|---|
| Üst modül `x = 12` sürer, alt `requires: x < 10` | `violate.volt` | 2 property `ok`, çıkış 0 | `Child.req_0 E5001 (in Parent)`, çıkış 6, `parent_cex.vcd`, ikincil etiket `instance 'c'` |
| Aynısı, üst modülün kendi kontratı yok | (önce `nocontract_parent`) | Yalnız `Child` görevi, çıkış 0 | `Parent` görevi açılır, E5001, çıkış 6 |
| Üç seviye, ara modül girişi süzmeden geçirir | `three_level_violate.volt` | çıkış 0 | `Mid` ve `Top` görevlerinde E5001, çıkış 6 |
| İki örnek, biri `x = 3`, biri `x = 12` | `multi_instance.volt` | çıkış 0 | E5001, ikincil etiket yalnız `bad` |
| Saat portu olmayan modülde `requires` + yanlış `ensures` | `clockless.volt` | "Note no contracts found", çıkış 0 | E5005, çıkış 1 |
| Aynı saatsiz modül saatli üst modülde | — | 1 property `ok`, çıkış 0 | E5005, çıkış 1 |
| Tek kontrat tepe modülün `requires`'ı | `nothing_to_verify.volt` | "1 property ... ok", çıkış 0 (hiçbir iddia yok) | E5006, çıkış 1 |

Son satır ayrı bir yanlışın örneğiydi: yalnız `assume` taşıyan bir görev
hiçbir şey denetlemediği hâlde "N properties verified" diye sayılıyordu.

## Karar kaynağı

Belgeler alt örneğin ön koşulunu üst modülün yükümlülüğü sayar:

- ADR-0055 §Sınırlar (`ADR-0055-paralel-formal-dogrulama.md:185`):
  "Alt modüllerin kontratları üst modülün görevinde de denetlenir".
  `assume` denetlenmez, yalnız kısıtlar; kod bu cümleyi `requires` için
  karşılamıyordu. Aynı satırdaki "(bugünkü davranış korunur)" alt modül
  kontratlarının üst göreve GİRMESİNİ anlatır (kaldırılmadılar), `assume`
  olarak girmelerini bir karar olarak değil.
- cli-contract.md §8 "Kontratlar simülasyonda"
  (`docs/spec/cli-contract.md:720-721`): "Alt örneğin `requires`/`assume`'u
  uyarıcıya değil onu süren üst modüle yüklenir: `contract violated`."
- ADR-0064 §3 (`ADR-0064-simulasyonda-kontratlar.md:125-130`): alt örneğin
  varsayımını (Handshake tüketici kontratı dahil) "onu süren üst modül
  bozar; bu bir tasarım hatasıdır".
- ADR-0011 Gerekçe (`ADR-0011-kontrat-sistemi.md:37`): emsal "Rust
  `#[requires]`/Dafny tarzı sözleşmeler" — çağrılanın ön koşulu çağıranın
  kanıt yükümlülüğüdür. Tasarım belgesi
  (`docs/design/Volt-Butunlesik-Cozum-RDC-Dogrulama-Spec.md:89`): "Bu
  modülü kullanan herkes ön koşulun sağlandığından emin olmalı."
- ADR-0011 Karar (`:28`) ve tasarım belgesi (`:330`) `requires → assume
  property` der; bu, modülün KENDİ denetimi için doğrudur ve korunur.

Hiçbir belge üst modülün alt örneğin ön koşulunu varsayabileceğini
söylemez. Formal akış simülasyonla aynı anlamı alır.

## Karar

### 1. Bağlama göre `assume` / `assert`

Bir kontratın rolü, onu taşıyan modülün görevdeki yerine bağlıdır:

| Kontrat | Modülün KENDİ görevinde (`prep -top M`) | M bir üst görevin içinde örnekken | Neden |
|---|---|---|---|
| `requires` | `assume` | `assert` | Ön koşul: kendi görevinde ortam kısıtı, örnekken onu süren üst modülün yükümlülüğü |
| `assume` (Handshake tüketici otomatik kontratı dahil) | `assume` | `assert` | `requires` ile aynı (ADR-0064 §3 ayrımı) |
| `ensures`, `invariant`, `assert` | `assert` | `assert` | Modülün garantisi; üst görevde de yeniden denetlenir (ADR-0055 §Sınırlar, değişmedi) |
| `cover` | `cover` | `cover` | Değişmedi |
| Primitif kontratları (AsyncFifo, ...) | `assert` / `cover` | aynı | Primitifler `requires`/`assume` kontratı üretmez |
| Koşum: `initial assume (<reset>)` | `assume` | `assume` | Kontrat değil, BMC başlangıç koşumu; değişmedi (bkz. Sınırlar) |

`ensures` üst bağlamda VARSAYILMAZ: üst modülün kanıtında alt modülün
mantığı zaten vardır (ADR-0055) ve garantiyi yeniden kanıtlamak her
bağlamda sağlamdır. Garantileri mantığın yerine koymak (assume-guarantee)
ayrı bir yol haritası maddesidir (`docs/roadmap.md`).

### 2. Mekanizma: görev başına makro

Tek `.sv` bütün görevlerce okunur (ADR-0055); rol görev başına değişmeli.
`SvaMode::Immediate` yükümlülük kontratını iki dallı üretir:

```systemverilog
// requires from violate.volt:12
always @(posedge clk)
`ifdef VOLT_SUB_Child
    if (!(rst)) assert (x < 8'd10); // volt:req_0
`else
    if (!(rst)) assume (x < 8'd10); // volt:req_0
`endif
```

`.sby` her görev için, görevin tepesi OLMAYAN ve yükümlülük taşıyan
modüllerin makrolarını `read -formal`'dan önce görev-koşullu tanımlar:

```
[script]
parent: read -define VOLT_SUB_Child
read -formal violate.sv
parent: prep -top Parent
```

Makro tanımsızken (elle `sby`, eski akış) `assume` kalır; `--emit=sva`
(ayrı/gömülü `property` blokları, ticari araçlar) bu ADR'den etkilenmez.
Yosys seçimiyle (`chformal -assume2assert`) dönüştürme reddedildi: koşum
`initial assume`'u ile kontrat `assume`'unu ayırt etmek hücre adı/öznitelik
taşınmasına dayanırdı; makro üretilen metinde görünür ve elle de koşulur.

### 3. Görev kümesi

Görev = bir şey DENETLEYEN modül. Modül M görev olur ⇔ M'nin kendi
iddiası/cover'ı varsa ya da M'nin altında (geçişli) yükümlülük taşıyan bir
örnek varsa. Sonuçlar:

- Kendi kontratı olmayan üst modül de görevdir: alt örneğin ön koşulunu
  kanıtlamak zorundadır (`violate.volt`).
- Yalnız `requires`/`assume` taşıyan modül görev OLMAZ: kendi görevinde
  denetleyecek bir şeyi yoktur.
- Üç seviyede `Top ⊃ Mid ⊃ Leaf`: `Leaf.requires` hem `Mid`'in hem
  `Top`'un görevinde `assert`'tür; `Mid.requires` `Mid`'de `assume`,
  `Top`'ta `assert`. `Mid` kendi ön koşulunu yazarak `Leaf`'inkini
  karşılar (`three_level_satisfy.volt`); bu, sağlam bir varsayım-garanti
  zinciridir: her yükümlülük, örneklendiği her bağlamda denetlenir.
- Aynı modülün birden çok örneği: her örnek kendi iddia hücresini taşır,
  her biri ayrı denetlenir; smtbmc'nin `Assert failed in Parent.bad:` yolu
  hangi örneğin bozulduğunu söyler, tanı yalnız onu etiketler.

### 4. Rapor ve sayım

- İlerleme satırı görevde DENETLENEN özellikleri sayar, varsayımları ayrı
  yazar: `[1/2] Child (1 property, 1 assumed) ... ok`.
- Özet yalnız denetlenenleri sayar. Hiçbir görevde denetlenmeyen
  yükümlülükler (birimin tepe modüllerinin `requires`'ı: ortam
  varsayımları) ayrı bir notta adlandırılır:
  `Note 2 assumption(s) about the environment, not verified: M.req_0, M.req_1`.
- JSON (cli-contract.md §8a zarfına geriye uyumlu ekler):
  `modules[].properties` denetlenen sayısı, `modules[].assumed` YENİ;
  `properties[].status` için YENİ değer `assumed`; alt örneğin üst görevde
  denetlenen yükümlülüğü ayrı satırdır, `module` sahibi modül, YENİ
  `context` alanı görevin tepesidir:
  `{ "module": "Child", "name": "req_0", "keyword": "requires", "status": "pass", "context": "Parent" }`.
- Karşı örnek kontratın SAHİBİNE eşlenir: satırın üstündeki `module <Ad>`
  başlığı + `// volt:<ad>` işareti, adaylar görevin kapsamındaki modüller
  (önce yalnız görevin tepesinde aranıyordu; alt örneğin `inv_0`'ı üst
  modülün `inv_0`'ına düşebiliyordu). Özet: `Child.req_0  E5001 contract
  violated at cycle 2 (in Parent)`. E5001 nedeni yükümlülük ihlalinde üst
  modülü adlandırır; ikincil etiket üst modüldeki örnektir.

### 5. Kontrat sessizce düşmez

- **E5005** — saat portu olmayan modülün kontratı: formal denetim saat
  kenarında örneklenir (ADR-0011, ADR-0040); kenar yoksa kontrat koşudan
  düşüyordu. `volt verify` artık ana dosyadan erişilebilir böyle her modül
  için E5005 verir (çıkış 1). Tanı erişilebilirlik süzmesinden SONRA
  üretilir (ADR-0042 Ek): `use` ile yüklenip örneklenmeyen kütüphane
  modülü sahte hata vermez. `volt check`/`build`/`test` etkilenmez.
- **E5006** — hiçbir görev bir şey denetlemiyor: kontrat yok ya da yalnız
  ortam varsayımları var. Önce "Note no contracts found", çıkış 0'dı;
  cli-contract.md §2 "uyarılar çıkış kodunu etkilemez" dediğinden başarı
  olmayan sonuç bir E-kodudur (çıkış 1). Not satırı E5006'nın nedenine
  taşındı; kontrat varsa adları listelenir.

## Tablo — hangi özellik, hangi görevde, ne olarak

`satisfy.volt` (Child: `requires x < 10`, `ensures y < 10`; Parent:
`invariant y < 10`, `Child { x: 3 }`):

| Özellik | `child` görevi | `parent` görevi | Neden |
|---|---|---|---|
| `Child.req_0` (`requires`) | assume | assert | Child'ın kendi görevinde ortam kısıtı; Parent'ta Parent'ın yükümlülüğü |
| `Child.ens_0` (`ensures`) | assert | assert | Child'ın garantisi, her bağlamda yeniden kanıtlanır |
| `Parent.inv_0` (`invariant`) | — | assert | Parent'ın kendi iddiası |
| Koşum `initial assume (rst)` | assume | assume (Parent'ınki ve Child'ınki) | BMC başlangıcı; kontrat değil |

JSON `properties`: `Child.ens_0 pass`, `Child.req_0 assumed`,
`Parent.inv_0 pass`, `Child.req_0 pass (context Parent)`.

## Örneklerin yeniden koşusu

Her örnek kök dosyası ve CI'ın formal fixture'ları tek tek, önce `main`
(218ca66) sonra bu ADR ile koşturuldu (`volt verify -j 2 --depth 12`,
hdlc/formal Docker, aynı anda tek konteyner). 28 dosyanın 24'ünde sonuç
aynı; değişen dördü:

| Dosya | Önce | Sonra | Sınıf | Açıklama |
|---|---|---|---|---|
| `hybrid_accel/ternary_array.volt` | 0 (9 özellik) | 6 | Eksik ön koşul | `TernaryPe`'nin `assume: (weight as i2) != -2` ön koşulu dizinin serbest `weight` girişinden gelir; `TernaryArray` bu ön koşulu arayüzünde söylemez, yani tek başına doğrulanırken ortamı geçersiz trit kodlaması (`2'b10`) sürebilir. Tasarım hatası değil: gerçek bağlamda (`HybridTop`, ağırlıklar `TernaryCtl`'den) aynı yükümlülük kanıtlanır. Örnek DEĞİŞTİRİLMEDİ: ön koşul 64 öğenin her biri içindir ve dil `for` içinde kontrat kabul etmez (`requires` orada E0001); 64 terimli tek bir ifade yazmak ya da `for` kontratı eklemek kullanıcının kararına bırakıldı. |
| `hybrid_accel/hybrid_top.volt` | 0 (32 özellik) | 6 | Aynı | Birim `TernaryArray`'ı ayrı görev olarak da içerir; düşen o görevdir. `HybridTop` görevi (alt örneklerin yükümlülükleriyle) geçer. |
| `riscv_alu.volt` | 0 ("no contracts found") | 1 (E5006) | Beklenen | Kontratsız fn/ALU dosyası: hiçbir şey denetlenmiyor. |
| `riscv_imm.volt` | 0 ("no contracts found") | 1 (E5006) | Beklenen | Aynı. |

Aynı kalanlardan biri öğretici: `soc/top.volt` önce "145 properties"
diyordu, bunların 45'i yalnız varsayımdı (beş modülün Handshake tüketici
`assume`'ları); gerçek denetim 100'dü. Şimdi 154 özellik denetlenir: aynı
100 + üst görevlerde `assert` edilen 54 alt örnek yükümlülüğü (SocTop 36,
Timer 9, UartCtrl 9). Hepsi geçer — SoC'nin üst modülleri alt örneklerinin
AXI el sıkışma protokolünü gerçekten koruyor; bu artık kanıtlı.
`binary_array.volt` aynı biçimde `BinaryPe`'nin `acc_in` aralığı
varsayımını üst görevde kanıtlar (bmc 12, 17 s → 48 s).

Hiçbir düşen durum `assume` eklenerek susturulmadı.

## Boşuna geçme (vacuity)

Çelişen varsayımlar kanıtı anlamsız biçimde geçirir. Ölçüm: `requires: x <
5` ve `requires: x > 10` taşıyan modülde yanlış `invariant: y == 99`
(`--depth 10`) çıkış 0 verir — hem reset'li hem reset'siz alanda.
sby'nin `--presat`'ı (smtbmc varsayılanı) yalnız varsayımların BİR ADIMDA
sağlanamadığını yakalar; reset serbest bir giriş olduğundan çözücü reset'i
sonsuza dek basılı tutar, koruma (`if (!rst)`) her iddiayı ve varsayımı
kapatır, varsayımlar sağlanabilir kalır.

Bu ADR, bu sınıfın en sık kaynağını kapatır: alt örneğin ön koşulu artık
üst bağlamda varsayım değildir, üst modülün sabit sürdüğü değerle
çelişip üst görevi boşa çıkaramaz. Geriye kalan — kullanıcının kendi
çelişen varsayımları — için bir mekanizma YOKTUR. Gerekli olan, görev
başına "varsayımlar reset bırakıldıktan sonra `--depth` boyunca
sağlanabilir mi" cover denetimidir (ayrı bir cover-kipi görevi ya da
koşum sayacı); bu, koşum anlamını (reset ortada yeniden basılabilir mi),
CI süresini (görev sayısı ikiye katlanır) ve raporu değiştirir. Küçük
değil; ayrı bir karar olarak bırakıldı ve `book/src/limitations.md`'de
yazılı.

## Sınırlar

- Ortak satırlar: koşum `initial assume (<reset>)` alt modülde de
  kalır. Alt modülün reset'i üst modülün senkronize reset'idir (ADR-0065),
  iki varsayım aynı sinyali bağlar; farklı bir reset türetilirse bu
  varsayım üst görevin başlangıç durumunu kısıtlar. Kontrat değildir, bu
  ADR'nin kapsamı dışında.
- `--emit=sva` (ayrı `.sva` + `bind`, `--sva=inline`) ticari araçlar için
  `assume property` üretmeye devam eder; orada bağlam ayrımı yoktur.
- Simülasyonda saatsiz modülün kontratları hâlâ izlenmez (ADR-0064
  Sınırlar); E5005 yalnız `volt verify`'dadır.
- Vacuity mekanizması yok (yukarıda).

## Ölçütler

- `violate.volt` önce çıkış 0, şimdi çıkış 6 + `parent_cex.vcd`; tanı
  `Parent` içindeki `c` örneğini gösterir.
- `verify_obligation_tests.rs` (13 test: 7 araçsız, 6 gerçek sby; CI
  `verify` işinde `VOLT_REQUIRE_TOOLS=sby`), sv-emit +10, driver birim +7;
  baseline 3614 → 3644, tanı kodu 151 → 153.
- `just check`, `just consistency` temiz.
