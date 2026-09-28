# ADR-0092: Dalga Formunda Enum ve Trit Adları — GTKWave Oturumu ve Çeviri Tabloları

> Statü: Uygulandı
> İlgili: ADR-0074 ("Sınırlar": dalga formunda varyant adları — kapandı; RTL eşlemesi değişmedi), ADR-0062 (Trit kodlaması), ADR-0033 (`volt run`), ADR-0075 (tümevarım izi), ADR-0090 (örnek çıkış teli adları), ADR-0094 (Docker köprüsü: basılan yollar ana makine yolu), ADR-0095 (`volt test` düşen testin dalga formunu kaydeder; test oturumu DUT portlarını da listeler).
> Tarih: 2026-09-28
> Etkilenen: volt-sv-emit (`waves.rs` YENİ — `WaveInfo`, `filter_text`,
> `gtkw_text`; `EmitOutput::waves`; emit döngüsünde `record_waves`),
> volt-driver (`waves.rs` YENİ — oturum dosyalarının yazımı; `volt run
> --vcd` kapanış satırı; `volt verify` E5001 önerisi ve E5002 notu),
> volt-diagnostics (`volt explain waveforms`, iki dil), CI (Verilator
> işinde `wave_session_tests`).

## Sorun

ADR-0074 ölçümü: enum durum adları VCD'de hiç görünmüyor. GTKWave'de
`TxState::Idle` yerine `0`, `Stop` yerine `3` görünüyor. Hata ayıklamanın
en çok yapıldığı yer dalga formudur; FSM'lerde bu ciddi konfor kaybı.
Struct'ta sorun yok: alanlar zaten ayrı, adlı sinyallerdir (ADR-0077).

## 1. Tespit

### 1a. Dalga formu üreten komutlar

| Komut | Biçim | Yer | Kullanıcıya söylenen | Belge |
|---|---|---|---|---|
| `volt run --vcd <dosya>` | VCD (Verilator `--trace`) | kullanıcının verdiği yol (tb'ye mutlak gömülür) | `Waveform <dosya>` | cli-contract.md §7, `explain getting-started`, `simulation-setup` |
| `volt verify` (FAIL) | VCD (smtbmc) | `build/formal/<görev>_cex.vcd` | `= counterexample: <yol>`, öneri "open the counterexample with 'gtkwave' or 'surfer'" | cli-contract.md (verify çıktı ağacı), `explain E5001` |
| `volt verify --mode prove` (UNKNOWN) | VCD | `build/formal/<görev>_induct.vcd` | not "induction trace …: <yol>" | ADR-0075, `explain E5002` |
| `volt test` | **yok** — `VerilateJob { trace: false }` | — | — | — |

FST hiçbir komutta yok. Görüntüleyici tarifi yalnız E5001 açıklamasında
("Open it with 'gtkwave' or 'surfer'") geçiyordu.

### 1b. Enum tipli sinyallerin hiyerarşik adları

Sonda tasarımıyla (üst modül + alt örnek + enum portu + struct içi enum
alanı + taban tipli enum + Trit) iki araçta ölçüldü:

| Sinyal | Simülasyon VCD (Verilator) | Formal karşı örnek (sby) |
|---|---|---|
| Üst modül register'ı | `TOP.Probe.sub_ph_r [1:0]` | `Probe.sub_ph_r` (aralıksız) |
| Üst port (iki kopya) | `TOP.top_ph` ve `TOP.Probe.top_ph` | `Probe.top_ph` |
| Alt örnek içi | `TOP.Probe.u.st` | `Probe.u.st` |
| Örnek çıkış teli (ADR-0090) | `TOP.Probe.u_ph` | `Probe.u_ph` |
| Struct içi enum alanı (ADR-0077 yaprak) | `TOP.Probe.hold_kind`, `req_kind` | `Probe.hold_kind` |
| `localparam` varyantlar (ADR-0074 B) | `TOP.Probe.Phase_Idle [1:0]` … (gürültü) | yok |
| Enum dizisi | E0003 (`arrays of enum 'Phase'`) — uygulanmaz | — |

GTKWave ad kuralı headless GTKWave ile ölçüldü: kapsamlar `.` ile
birleşir; çok bitli değişken VCD'de aralık yazmasa da (sby izi)
`ad[W-1:0]` olur; aralıksız ad oturumda **sessizce düşer**; tek bit
aralıksızdır.

### 1c. Trit

Evet, aynı sorun: `Trit` → `logic signed [1:0]`, VCD'de
`$var wire 2 % u_tv [1:0]` — işaret bilgisi yok. `-1` GTKWave'de `11`
(ondalıkta 3) görünür.

## 2. Ölçüm — seçenekler

- **A — Çeviri tablosu + oturum:** enum başına GTKWave "translate filter
  file" + sinyalleri tablolara bağlayan `.gtkw`. RTL değişmez.
- **B — FST + `typedef enum`:** Verilator `--trace-fst` + modül içi
  `typedef enum logic [1:0] {…} Phase_t;` (paketsiz; ADR-0074'ün dosya
  sırası sorununa girmemek için).

B elle yazılmış SV ile Verilator 5.050'de ölçüldü (`build/wave/b/`):
Verilator FST'ye enum tablosu yazar (`declDTypeEnum(1, "Fsm.Phase_t", …)`),
ama **yalnız typedef tipli değişkenlere** bağlar: `r` (dtype 1) ve `st`
(dtype 2) adlı; port `ph`/`top_ph` ve örnek teli `u_ph` `dtypenum = -1`
(düz `logic`). Portu enum tipli yapmak için tip port listesinden önce
görünmeli → paket → ADR-0074'ün reddettiği dosya sırası sorunu. Aynı
typedef'li SV'nin `--trace` VCD'sinde enum bilgisi yok (`$attrbegin`
yazılmıyor — ADR-0074 ölçümüyle aynı).

| Ölçüt | A | B |
|---|---|---|
| Simülasyon VCD'si (`volt run`'ın bugünkü çıktısı) | **çalışır** — 5 tasarımda her iz yüklendi, basılı dalga formunda adlar | çalışmaz: VCD enum taşımaz |
| Simülasyon FST'si | uygulanmaz (Volt FST yazmaz) | typedef'li değişkenlerde çalışır |
| Formal karşı örnek VCD'si | **çalışır** — 3 karşı örnek, her iz | çalışmaz: sby izi VCD'dir (typedef'li SV ile sby ölçülmedi) |
| Alt örnek içi sinyal | çalışır (`Probe.u.st`) | typedef'li değişkende çalışır |
| Enum portu / örnek çıkış teli | **çalışır** (`req_kind`, `u_ph`, `top_ph`) | çalışmaz (düz `logic`; paket gerekir) |
| Struct içi enum alanı | çalışır (`hold_kind`) | yaprağın typedef'li bildirimi gerekir |
| Üretilen RTL | **değişmez** (golden: 2436 dosya birebir) | her modüle typedef + sinyal tipleri |
| Kurulum | GTKWave | `verilator/verilator` imajında `--trace-fst` derlenmez: `zlib.h`, sonra `lz4.h` eksik (paketlerle çalıştı) |
| Surfer | ölçülmedi (GUI yok; `.gtkw` ve çeviri dosyası GTKWave biçimidir, Surfer VCD'yi yine açar) | ölçülmedi |

### Karar: A

RTL'yi değiştirmeyen yol iki dalga formu kaynağında da (simülasyon ve
formal) ve B'nin kapsayamadığı portlarda/örnek tellerinde çalışıyor. B
hem RTL değişikliği hem paket (ADR-0074'ün reddettiği) gerektirir ve
formal izde işe yaramaz. ADR-0074'ün kararı (seçenek B: `localparam` +
düz `logic`) değişmez; bu ADR yalnız onun "Sınırlar" maddesini kapatır
(İlgili bağlantısı, "Kısmen yerini aldı" değil).

## 3. Karar — uygulama

1. **Bilgi emit'ten gelir.** Emitter her modülden sonra enum tipli
   (`enum_sigs`: port, reg, wire, tipli ya da enum ifadeli `let`, struct
   yaprağı) ve tekil Trit sinyallerini (`trits`, diziler hariç), örnek
   çıkış tellerini (`<örnek>_<port>`, tip hedef porttan) ve kullanıcı
   modülü örneklerini kaydeder (`EmitOutput::waves`). Yalnız üretilen SV
   gövdesinde bildirilen adlar alınır. Tablolar `enum_layout`'tan: HIR,
   sv-emit ve otomatik kontratlarla aynı kodlama (ADR-0074).
2. **Dosyalar dalga formunun yanında, aynı adla:** `<ad>.vcd` →
   `<ad>.gtkw` + `<ad>.filters/<Enum>.txt` (Trit için `Trit.txt`).
   Formal: `build/formal/<görev>_cex.gtkw`, `…_induct.gtkw`.
3. **Hiyerarşi öneki araca göre:** simülasyon `TOP.<Üst>`, formal
   `<Üst>` (1b). Vektör adı `ad[W-1:0]`.
4. **İz biçimi** `@2028` = ikili + sağa yaslı + dosya çevirisi; her izden
   önce `^N <tablo>`. Tablo satırı `<W basamaklı ikili> <ad>`.
5. **Yollar Volt'a verildiği gibi** (göreli ise göreli). Ölçüm (GTKWave
   3.3.116): göreli yol `.gtkw`'nin dizinine göre DEĞİL çalışma dizinine
   göre çözülür; yanlış dizinden açınca sinyaller yüklenir ama ham kod
   gösterir. Mutlak yol, Volt Docker'da koşunca (`/work/...`) ana
   makinedeki GTKWave'de bulunmaz. Basılan komut aynı dizinden çalışır.
6. **Tek satır:** `volt run`: `Waveform gtkwave <vcd> <gtkw>`;
   `volt verify` E5001 önerisi: `open the counterexample with enum
   names: gtkwave <vcd> <gtkw>`; E5002'de ayrı not. Oturum yoksa
   satırlar eskisi gibi (`Waveform <vcd>`; E5001 önerisi değişmez).
   JSON çıktısının `artifacts` listesine `.gtkw` eklenir.
7. **Enum/Trit yoksa hiçbir dosya üretilmez.** Volt'un yazdığı eski
   oturum (ilk satır `[*] Volt waveform session (ADR-0092)`) silinir;
   Volt'un yazmadığı aynı adlı `.gtkw`'ye **dokunulmaz** (kullanıcının
   kendi GTKWave kaydı olabilir) ve satır bunu söyler; tablolar yine
   tazelenir (GTKWave'de yeniden kaydedilen oturum işareti kaybeder ama
   tabloları göstermeye devam eder — enum değişince bayat ad olmasın).
   Enum'suz tasarımda kullanıcının `.gtkw`'si çıktıyı hiç etkilemez.
   Büyük/küçük harf duyarsız dosya sisteminde çakışan tablo adı
   (`State`/`STATE`) `<Enum>_<k>.txt` olur.
8. `volt test` dalga formu üretmez (1a); bayrak eklemek CLI sözleşmesi
   değişikliğidir (cli-contract.md) — bu ADR'nin kapsamı dışında.

### Trit kararı

1c olumlu → aynı mekanizma: `01` +1, `00` 0, `11` -1 (ADR-0062
kodlaması).

### Geçersiz kod kararı

Hiçbir varyanta ait olmayan kod (üç durumlu enum'da `11`, Trit'te `10`)
tabloda **kırmızı** `invalid <kod>` olur (`?red?invalid 3`; Trit'te
`invalid -2`). Neden: formal karşı örnekte serbest giriş ve `anyinit`
geçersiz kodu gerçekten üretir (sondada Opcode `invalid 0` göründü);
sessiz ham değer hata ayıklamada gözden kaçar. Tablolar 8 bite kadar
(256 satır) her kodu listeler; daha geniş taban tipli enum'da (ör. `u32`)
listeleme olanaksızdır, eşleşmeyen kod GTKWave'de ham ikili değer olarak
görünür. GTKWave'in `?renk?` önekini anladığı ölçüldü (basılı çıktıda
önek yok, metin `invalid 0`).

## 4. Doğrulama

GUI açılamadığı için doğrulama yapısal ve headless:

- **Yol varlığı** (`build/wave/check.py`, VCD başlığını bağımsız
  ayrıştırır): simülasyon — uart_tx (`TOP.UartTx.state_r[1:0]`), i2c
  (`state` portu + `state_r`), riscv_core (`TOP.RiscvCore.u_tx.state_r`
  — alt örnek), sonda (12 iz: port, struct alanı, örnek teli, alt örnek,
  Trit, taban tipli enum), ternary_pe (Trit port); formal — uart_tx ve
  i2c kopyalarına eklenen bozuk invariant ile karşı örnek, sonda. 8
  oturum, eksik yol 0.
- **Gerçek GTKWave** (3.3.116, xvfb, Tcl `getDisplayedSignals` +
  `getTraceFlagsFromName` + `Print_To_File PS`), basılan komutla Volt'un
  koştuğu dizinden: 8/8 oturumda istenen iz = gösterilen iz, bayrak 8232
  (çevirili), PostScript çıktısında `Idle/Start/Data/Stop/Addr/Load/Alu/
  +1/invalid 0` metinleri. Ekran görüntüsü alınmadı.
- **Tablolar kodlamayla birebir** (`wave_emit_tests`): örtük (0,1,2),
  açık değerli + taban tipli (`u7`, bildirim sırası ≠ kod sırası),
  `match` son kolu `default` olan enum (tablo `case` biçiminden
  bağımsız), tek bitlik enum (aralıksız ad), Trit dizisi çevrilmez,
  enum'suz tasarımda oturum yok.
- **Uçtan uca** (`wave_session_tests`, CI Verilator işi): `volt run
  --vcd` → oturumdaki her yol aynı koşunun VCD başlığında; enum'suz
  tasarımda dizinde yalnız VCD.
- **Mutasyon** (`build/wave/mutate.py`, tek tek): tablo kodunu kaydırma
  → 3 test düşer; aralığı `[W:0]` yapma → birim testleri + gerçek VCD
  testi (`TOP.UartTx.state_r[2:0] VCD başlığında yok`); alt örnek
  kapsamını atlama → düşer; enum'suz tasarımda dosya yazma → düşer.
  4/4 öldürüldü.
- **Golden:** main ile bu dal, `examples/`, `tests/ui/pass`,
  `tests/fixtures` altındaki her `.volt` için RTL, SVA (ayrı + inline),
  SDC/XDC, `check` (insan + JSON) ve formal `.sby`/SV: 2436 dosya bayt
  bayt aynı (süre satırları normalize).

## Sınırlar

- `volt test` dalga formu üretmez (CLI sözleşmesi değişikliği gerekir).
- Trit dizileri paketlenmiş vektördür (ADR-0056): elemanın adı yok,
  çevrilmez. Enum dizisi zaten E0003.
- `extern` örneğin iç sinyalleri bilinmez; yalnız enum/Trit tipli çıkış
  teli oturuma girer.
- Farklı dosyalarda aynı adlı iki enum: tablo ilk bildirimden kurulur.
- `let` yalnız SV'de tel olarak bildirildiyse oturuma girer.
- Oturumu başka dizinden açmak: sinyaller yüklenir, ham kod görünür
  (GTKWave davranışı, bölüm 3 madde 5).
- Çok saatli formal akışı (`clk2fflogic`) ile karşı örnek yolları
  ölçülmedi.
- Surfer ölçülmedi.

## Yan bulgular (bu ADR kapsamında düzeltilmez)

1. `volt run` HybridTop'ta testbench derlenmiyor: 64 bitten geniş port
   `VlWide<4>` → `tb.cpp:50: no match for 'operator='`. Testbench üretimi
   bu değişiklikte değişmedi.
2. `verilator/verilator` imajında `--trace-fst` için `zlib1g-dev` ve
   `liblz4-dev` gerekiyor; Volt ileride FST yazarsa kurulum belgesine
   girmeli.
