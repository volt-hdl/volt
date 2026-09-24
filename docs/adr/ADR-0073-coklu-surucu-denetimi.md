# ADR-0073: Çoklu Sürücü Denetimi — Tek Tablo, Sürücü Türleri, Bit Aralıkları

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-24
> Etkilenen: volt-hir (`drivers.rs` — `DriverKind`, bit aralığı, çakışma
> kuralı, E4001 etiket/öneri seçimi, birim testleri; `typeck/stmt.rs` —
> giriş portu ve `let` başlangıç kaydı, lvalue bit aralığı, blok içi `for`
> sınırları; `typeck/instance.rs` — inout/opendrain hattı kaydı;
> `typeck/mod.rs` — `loop_bounds`, kaynak adı), `tests/fixtures/parity/`
> (57 `d*` sondası), `volt-driver/tests/double_driver_tests.rs` (YENİ),
> `tests/ui/pass/93`, `tests/ui/fail/84`.
> DOKUNULMADI: README.md, docs/spec/, docs/research/, examples/.
> Genişletir: `docs/spec/type-inference.md` §11 (spec salt okunur; bu ADR
> §11.1-§11.2'nin güncel hâlidir).

## 1. Sorun

PR #27 sırasında bulundu:

```volt
let v = a
v = 3
```

hiçbir tanı vermiyor, SV `wire v = a; assign v = 3;` — tek ağda iki
sürücü. Sessizce yanlış donanım.

### 1.1 Mevcut analiz (ADIM 1)

Sürücü tablosu (`crates/volt-hir/src/drivers.rs`, §11) tip denetimiyle
aynı geçişte doldurulur ve **yalnız atamaları** (`=`/`<=`) kaydediyordu:
hedefi port, register, wire veya `let` olan her atama; aynı `on`/`comb`
bloğundakiler tek sürücü. Kaydedilmeyen ya da sayılmayan yollar:

| yapı | kayıt | sonuç |
|---|---|---|
| `let` bildirimi (başlangıç değeri) | yok | sonraki atama tek sürücü görünür |
| giriş portu | yok | içeride atanabilir (`assign a = b`) |
| alt modülün inout/opendrain portuna bağlı hat | yok | aynı hatta push-pull atama sessiz |
| bit/aralık/eleman/alan hedefi | kaydedilir, `partial` | **hiç çakışmaz** (E4001 dışı) |
| `out`, `reg`, `wire`, bundle alanı, `on`/`comb`, `for` açılımı (tam hedef) | kaydedilir | E4001 doğru |
| alt modül çıkışı (`inst.q = …`) | — | E4011 (ADR-0072) |
| inout/opendrain porta doğrudan atama | — | E4008 (ADR-0051) |

Bundle portları parser'da `<port>_<alan>` portlarına düzleşir; modül
seviyesi `for` ve boru hattı aşamaları parser'da açılır. Tablo bu yüzden
açılmış AST'yi görür — ayrı bir yol gerekmez, eksik olan sürücü
**türleri** ve **kısmi hedeflerin karşılaştırılmasıydı**.

## 2. Sınıf taraması (ADIM 2)

Her satır `tests/fixtures/parity/d*.volt` sondasıdır. "Önce" PR #27
ikilisi (6f8a2a3), "sonra" bu ADR. SV sütunu önceki ikilinin ürettiği
SV'nin bağımsız denetimidir: Yosys `check` ("multiple conflicting
drivers") ya da Verilator `-Wall` (`MULTIDRIVEN`, `ASSIGNIN`).

| # | sınıf | önce check/build | önceki SV | sonra |
|---|---|---|---|---|
| d01 | `let v = a; v = b` | **temiz** | çift sürücü (Yosys) | E4001 |
| d01b | tipli `let` + atama | **temiz** | çift sürücü (Yosys) | E4001 |
| d01c | `let` + `comb { v = b }` | **temiz** | çift sürücü (Yosys) | E4001 |
| d01d | `let` + `on clk { v <= b }` | **temiz** | çift sürücü (Yosys) | E4001 |
| d02 | `out y` iki sürekli atama | E4001 | — | E4001 |
| d02b | `out y` sürekli + `comb` | E4001 | — | E4001 |
| d02c | `out y` iki `comb` | E4001 | — | E4001 |
| d02d | `out y` `on` + sürekli | E4001 | — | E4001 |
| d03 | `reg` iki `on` bloğu | E4001 | — | E4001 |
| d03b | `reg` iki farklı saat | E4001 | — | E4001 |
| d04 | `reg` `on` + `comb` | E4001 | — | E4001 |
| d04b | `reg` `on` + sürekli | E4001 | — | E4001 |
| d05 | `wire` `=` + `comb` | E4001 | — | E4001 |
| d05b | `wire` iki `=` | E4001 | — | E4001 |
| d06 | bundle alanı iki kez | E4001 | — | E4001 |
| d06b | bundle alanı sürekli + `comb` | E4001 | — | E4001 |
| d06c | struct alanı iki kez | E0003 | — | E4001 (struct sinyali zaten E0003) |
| d06d | struct ayrı alanlar | E0003 | — | E0003 (E4001 yok) |
| d07 | `arr[0] = x` + `for i { arr[i] = y }` | **temiz** | çift sürücü (Yosys) | E4001 |
| d07b | `arr[0]` iki kez | **temiz** | çift sürücü (Yosys) | E4001 |
| d07c | `arr[0]` `comb` + sürekli | **temiz** | çift sürücü (Yosys) | E4001 |
| d07d | `comb { for … arr[i] }` + `arr[1]` | **temiz** | çift sürücü (Yosys) | E4001 |
| d07e | `arr = a` + `arr[1] = b` | **temiz** | çift sürücü (Yosys) | E4001 |
| d07f | `comb` `for` 0..2 + `arr[2]`, `arr[3]` | temiz | temiz | temiz |
| d07g | `arr[0][3:0]` + `arr[0][7:4]` + `arr[1]` | temiz | temiz | temiz |
| d07h | `arr[0] = a` + `arr[0][2] = …` | **temiz** | `MULTIDRIVEN` | E4001 |
| d08 | `y = a` + `y[0] = …` | **temiz** | `MULTIDRIVEN` | E4001 |
| d08b | `y[7:4]` + `y[5:0]` | **temiz** | çift sürücü (Yosys) | E4001 |
| d08c | `y[7:4]` + `y[3:0]` | temiz | temiz | temiz |
| d08d | `y[0]` iki kez | **temiz** | `MULTIDRIVEN` | E4001 |
| d08e | `y[0 +: 4]` + `y[5 -: 4]` | **temiz** | çift sürücü (Yosys) | E4001 |
| d08f | `y[0 +: 4]` + `y[7 -: 4]` | temiz | temiz | temiz |
| d09 | `let w = s.q; w = b` (alt modül çıkışı) | **temiz** | çift sürücü (Yosys) | E4001 |
| d09b | `wire w; w = s.q; w = b` | E4001 | — | E4001 |
| d09c | `s.q = b` | E4011 | — | E4011 |
| d10 | inout hattı + `bus = v` | **temiz** | çift sürücü (Yosys) | E4001 |
| d10b | inout porta doğrudan atama | E4008 | — | E4008 |
| d10c | `dq.drive()` + `dq_out = v` | E4001 | — | E4001 |
| d10d | opendrain hattı + `bus = p` | **temiz** | push-pull ↔ wired-AND (`tri1`; Yosys ayrıştıramaz) | E4001 |
| d10e | iki inout aynı hatta | temiz | üç durumlu veri yolu (geçerli) | temiz |
| d10f | inout hattı okunmuyor | temiz | — | temiz, W4001 yok |
| d11 | boru hattı: iki aşamada aynı ad | E5016 | — | E5016 |
| d11b | boru hattı: bir aşamada aynı ad iki kez | E5016 | — | E5016 |
| d11c | boru hattı aşama `let`ine atama | **temiz** | çift sürücü (Yosys) | E4001 |
| d11d | boru hattı çıkışı iki kez | E4001 | — | E4001 |
| d12 | `for` gövdesinde tam hedef | E4001 | — | E4001 (+ yineleme notu) |
| d12b | `for` gövdesinde sabit eleman | **temiz** | `MULTIDRIVEN` | E4001 |
| d12c | `for` gövdesinde `let t = …; t = b` | **temiz** | çift sürücü (Yosys) | E4001 (ad `t`, tek tanı) |
| d12d | `for` ile ayrık elemanlar | temiz | temiz | temiz |
| d13 | `comb` içi `let` yeniden atama | E0003 | — | E0003 |
| d13b | `comb` içinde if/else | temiz | temiz | temiz |
| d13c | `comb` içinde varsayılan + geçersiz kılma | temiz | temiz | temiz |
| d14 | giriş portuna atama | **temiz** | çift sürücü (Yosys) | E4001 |
| d14b | giriş portuna kısmi atama | **temiz** | `ASSIGNIN` (Verilator hata) | E4001 |
| d15 | `reg` iki sürekli atama | E4001 | — | E4001 |
| d16 | `[Trit; 2]` ayrı elemanlar | temiz | temiz | temiz |
| d17 | okunmayan `let` | W1001 | — | W1001 (W4001 yok) |

**22 sessiz yol** (kalın): `let` başlangıcı (6), kısmi hedef çakışması
(12), giriş portu (2), paylaşılan hat (2). Hepsi artık E4001; geçerli
tasarım satırları (`ok`) temiz kalır.

## 3. Karar

### 3.1 Tek tablo, sürücü türleri

Denetim **yalnız** `drivers.rs`'tedir; tip denetimi kayıt yapar,
karar vermez. Her kayıt bir **tür** taşır:

| tür | kaynak | kayıt yeri |
|---|---|---|
| `Assign` | `=`/`<=` (sürekli, `comb`, `on`) | `stmt.rs::check_assign` |
| `LetInit` | `let v = e` başlangıcı (modül seviyesi ya da blok) | `stmt.rs::handle_let` |
| `ParentInput` | giriş portu — üst modül sürer | `stmt.rs::check_module` |
| `SharedLine` | alt modülün inout/opendrain portuna bağlı wire/port | `instance.rs::record_shared_line` |

E4002 (sürücüsüz çıkış) ve W4001/W4002 (yazılıp okunmayan) yalnız
`Assign` kayıtlarına bakar: `let` başlangıcı ya da üç durumlu hat
"yazma" sayılmaz, böylece geçerli tasarımlarda yeni uyarı çıkmaz.

### 3.2 Çakışma kuralı

İki kayıt **çakışır** ⇔ farklı gruptadır (§11.2 aynı blok istisnası
korunur) **ve** ikisi birden `SharedLine` değildir (üç durumlu sürücüler
birbiriyle uyumludur — veri yolu) **ve** bit aralıkları kesişir.

Bit aralığı `[lo, hi)`, lvalue soneklerinden hesaplanır: sabit indeks
(dizi elemanı = eleman genişliği × indeks; tam sayıda tek bit), sabit
aralık `[h:l]`, sabit parça seçimi `+:`/`-:`, struct alanı (bildirim
sırasıyla soyut düzen — yalnız ayrıklık için, SV düzeni değil). Tam hedef
ve derleme zamanında bilinmeyen aralık bütün sinyaldir (**muhafazakâr**).
Blok içi `for` döngüsünün çıplak değişkeniyle indeks (`arr[i]`, sınırlar
sabit) döngü aralığındaki elemanların birleşimidir — `comb` içindeki
döngü açılır, SV'de her eleman ayrı bir sabit parça seçimidir.

### 3.3 Mesaj

Sinyal başına ilk çakışan çift raporlanır: sonraki sürücü birincil,
önceki ikincil etiket; ikisinin konumu da görünür. Etiket ve öneri
türlere göre seçilir:

| çift | ikincil etiket | öneri |
|---|---|---|
| `let` | "the 'let' initializer already drives it" | `wire v : T` bildir, tek atama; ya da seçimi başlangıç ifadesine kat |
| giriş portu | "input port: the instantiating module drives it" (port bildirimi) | iç bir `wire` sür |
| paylaşılan hat | "shared with the tri-state port 'p.dq' here" (örnekleme) | hat yalnız o port üzerinden (drive/release) sürülür |
| kısmi | "first assignment here" + not "bits L..=H of 'y' are driven by both" | ayrık bitler ya da tek blok |
| tam | "first assignment here" | tek atama ya da koşullu ifade (değişmedi) |

Gerekçe notu spec/ADR atfı taşır ("… (type-inference.md §11,
ADR-0073)"); beş parça kuralı. `for` açılımında ad kaynak adıdır
(ADR-0072 `source_name`: `t_0` değil `t`) — kopyalar ADR-0068
katlamasıyla tek tanı olur. İki sürücü aynı kaynak konumundaysa (tam
hedef ya da sabit eleman `for` gövdesinde) not: "each iteration of the
unrolled 'for' loop drives 'y' again; index the target with the loop
variable".

### 3.4 Parite (ADR-0070)

Kayıt ve karar `run_semantic_stages` içindeki tip denetimindedir; `check`,
LSP ve `build` aynı tanıyı üretir. `parity_tests.rs` 54 yeni sondanın
beklenen hata kümesini ve LSP = check eşliğini denetler (57 yeni sonda);
`double_driver_tests.rs` her E4001 sondasında birincil/ikincil satırı ve
`build`'in SV üretmeden durduğunu, her `ok` sondasında E4001/W4001/W4002
olmadığını doğrular.

## 4. Mevcut kod (ADIM 4)

`examples/` (tüm `.volt`), `tests/ui/pass/` (82), `tests/fixtures/`
yeni denetimle: **gerçek çift sürücü yok, yanlış alarm yok** (E4001
yalnız `tests/ui/fail/03`, `15`, `84`'te). Golden (PR #27 ikilisi ↔ bu
ADR, `tests/ui` + `tests/fixtures` + `examples` içindeki her dosyanın
check insan/JSON çıktısı ve build dosya içerikleri): önceden var olan 273
dosyada SV ve geçerli tasarım çıktısı **aynı**; fark yalnız iki E4001 fail fixture'ının
gerekçe notundaki ADR atfı.

## 5. Doğrulama

- Mutasyon (17 mutasyon; sonda testleri
  `double_driver_tests` + `parity_tests`): giriş portu kaydı, `let`
  kaydı, paylaşılan hat kaydı, kesişim (komşu sayma / hiç kesişmeme),
  üç durumlu uyumluluk, grup istisnası, sabit indeks, döngü sınırı,
  aralık, azalan parça seçimi, struct alan ofseti, Trit genişliği,
  `is_driven` türü, denetimin tamamı, kaynak adı, dizi eleman genişliği —
  sonuç bölüm 7'de.
- `tests/ui/pass/93_disjoint_partial_drivers.volt` (ayrık aralık, parça
  seçimi, `comb` `for` + sürekli eleman, iki inout aynı hatta) build'den
  geçer ve Verilator `-Wall` temizdir; `tests/ui/fail/84_let_double_driver.volt`.

## 6. Sınırlar

- Derleme zamanında bilinmeyen indeks (`arr[sel] = x`) bütün sinyal
  sayılır: başka bir bloktaki ayrık eleman ataması da E4001 verir. SV'de
  en uzun statik önek de bütün değişkendir; muhafazakâr kalınır.
- Blok içi döngü değişkeni yalnız çıplak indekste aralık verir
  (`arr[i + 1]` bütün sinyal).
- Hata iletisi bundle alanı için düzleşmiş adı gösterir (`hs_data`);
  ADR-0072 kaynak adı tablosu yalnız `for` açılımını kapsar.
- Aynı üst modülde iki farklı hattı birbirine bağlayan yapı yoktur
  (Volt'ta `assign a = b` takma adı yok), bu yüzden ağ birleşimi analizi
  gerekmez; eklenirse bu tablo ağ düzeyine taşınmalıdır.

## 7. Mutasyon sonucu

Her mutasyon tek başına uygulanıp `double_driver_tests` + `parity_tests`
koşuldu; **17/17 yakalandı**. İlk turda iki mutasyon kaçtı ve sonda
eklenerek kapatıldı:

- `is_driven` türe bakmıyor → okunmayan `let` W4001 alır: `d17`.
- dizi eleman genişliği 1 → tam eleman ölçekle birlikte kayar, yalnız
  eleman içi bit aralığı fark ettirir: `d07g` (yanlış alarm), `d07h`.

| mutasyon | yakalayan sonda (örnek) |
|---|---|
| giriş portu kaydı kapalı | d14, d14b |
| `let` kaydı kapalı | d01*, d09, d11c, d12c |
| paylaşılan hat kaydı kapalı | d10, d10d |
| komşu aralıklar kesişir | d08c, d07f (yanlış alarm) |
| aralıklar hiç kesişmez | d07b, d08b |
| üç durumlu uyumluluk kapalı | d10e (yanlış alarm) |
| grup istisnası kapalı | d13c (yanlış alarm) |
| sabit indeks bilinmiyor | d12d, d16 (yanlış alarm) |
| döngü sınırı bilinmiyor | d07f (yanlış alarm) |
| aralık tabandan başlar | d08c (yanlış alarm) |
| azalan parça seçimi yanlış | d08e |
| struct alan ofseti 0 | d06d (yanlış alarm) |
| Trit genişliği bilinmiyor | d16 (yanlış alarm) |
| `is_driven` türsüz | d17 (W4001) |
| denetim kapalı | tüm E4001 sondaları |
| kaynak adı kapalı | d12c (`t_0` sızar) |
| dizi eleman genişliği 1 | d07g, d07h |
