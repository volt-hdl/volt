# ADR-0095: Proje Kipi — Argümansız check/build/run/verify, Düşen Testin Dalga Formu ve `volt test --watch`

> Statü: Uygulandı
> İlgili: ADR-0084 (`volt new` şablonları: Volt.toml `top`, "Next:" satırları dosya adsız), ADR-0092 (dalga formu oturumu: `volt test` oturumu DUT portlarını da listeler), ADR-0094 (Docker köprüsü: "Ctrl-C yolu konteyneri kaldırmaz" sınırı `--watch`'ta kapandı), ADR-0061 (Volt.toml araması ve tavanı), ADR-0089 (test keşfinin atlama kuralları — proje kaynakları ve izleme aynı yürüyücüyü kullanır), ADR-0033 (`volt run` testbench'i: 64 bitten geniş portlar).
> Tarih: 2026-09-28
> Etkilenen: volt-driver (`project.rs`, `watch.rs`, `interrupt.rs` YENİ;
> `sim/test_waves.rs` YENİ; `main.rs`, `sim/test_cmd.rs`, `sim/verilator.rs`,
> `sim/discover.rs`, `sim/report.rs`, `sim/tb_output.rs`, `sim/run_cmd.rs`,
> `waves.rs`, `new/`), volt-sv-emit (`sim.rs`, `sim_width.rs` YENİ — yalnız
> testbench), volt-hir (`unit_load.rs` — yalnız manifest `top`),
> volt-diagnostics (`waveforms`, `simulation-setup` metinleri), templates/,
> cli-contract.md §1, §2, §4a (YENİ), §6, §7, §8, §8b, §9b, §10; CHANGELOG;
> yeni bağımlılık `ctrlc`

## Sorun

İngilizce öğretici yazılmadan önce kullanıcının yazacağı komutların son
hâli gerekiyor. Üç sürtünme var:

1. `volt new demo; cd demo` diyen kişi `volt check` yazamıyor: dosya adı
   zorunlu (clap, çıkış 2). `volt new`'in kendi "Next:" satırı da
   `volt check counter.volt` yazdırıyor.
2. `volt test` düşen testin dalga formunu üretmiyor. Dalga formu yalnız
   `volt run --vcd` ve `verify` karşı örneğinde var (ADR-0092); `volt run`
   ise testin girişlerini kullanmadığı için düşen testin kaydını almanın
   hiçbir yolu yok.
3. Konfor listesi madde 8: `volt test --watch`.

Ek (PR #63 yan bulgusu): `examples/hybrid_accel` HybridTop'ta `volt run`
64 bitten geniş portta testbench C++ derleme hatası veriyor.

## 1. Tespit (ölçüm, 2026-09-28, main = 85858217)

**a) Volt.toml bugün:** `[package] name`, `src` (varsayılan `src`),
`[lint] unenforced_attributes` (ADR-0048), `[test] paths` (ADR-0089),
`[ui] lang`. Üst modül ya da giriş dosyası bilgisi YOK. Ayrıştırıcı satır
tarayıcıdır (`volt_hir::unit_load::Manifest::parse`, TOML bağımlılığı yok).

**b) Argümansız `check/build/run/verify`:** dördü de clap hatası
`the following required arguments were not provided: <FILE>`, çıkış 2 —
Volt.toml'lu proje dizininde de (`examples/` ölçüldü).

**c) `volt run` üst modül seçimi:** `--top` yoksa BİRİMİN bütün modülleri
arasından "tek modül" aranır — `use` ile gelen kütüphane modülleri dahil.
`volt run examples/hybrid_accel/hybrid_top.volt`: "contains 7 modules —
pick one with --top" (dosyanın kendisinde 3 modül var).

**d) `volt test` ve iz:** testbench izsizdir (`VerilateJob.trace = false`,
`test_testbench_cpp` VCD başlığı üretmez). Maliyet ölçümü
`examples/riscv_core_test.volt` (RiscvCore 58 test, HelloSoc 1 test)
üzerinde, sabit imaj `verilator/verilator:v5.052`, Windows + Docker
Desktop, testbench'e elle iz eklenerek (`build/meas/patch_tb.py`):

| | RiscvCore (58 test) | HelloSoc (1 test, 913 çevrim) |
|---|---|---|
| derleme, taze konteyner, izsiz | 10,06 s / 9,06 s | 8,52 s / 8,37 s |
| derleme, taze konteyner, `--trace` | 11,37 s / 11,05 s | 10,59 s / 10,44 s |
| koşu, izsiz | 26–42 ms | 6–9 ms |
| koşu, `--trace` derlenmiş, iz kapalı | 27 ms | 6–9 ms |
| koşu, her test izli | 120–132 ms | 23–31 ms |
| VCD | 58 dosya, 1,29 MB | 1 dosya, 2,81 MB |

Aynı konteynerde ikinci derleme ccache nedeniyle 0,6 s'ye iner — ürün
her derlemeyi taze konteynerde yaptığı için anlamlı sayı soğuk derlemedir.
Tüm `volt test riscv_core_test.volt` (Docker) 20,2 s. Sonuç: iz maliyeti
koşuda ihmal edilebilir, derlemede grup başına **+1,3–2,1 s (%15–25)**.

**e) HybridTop hatası main'de var.** Kök neden testbench üretiminde
(volt-sv-emit `sim.rs`): Verilator 64 bitten geniş portu `VlWide<N>`
(32 bitlik sözcük dizisi) olarak üretir; `volt run` testbench'i her portu
skaler sayar — `dut.t_weight = 1;` ("no match for operator=") ve
`(unsigned long long)dut.t_weight` ("invalid cast"). `t_weight` ve
`b_weight` `[Trit; 64]` = 128 bit. Aynı hata `examples/crypto/key_store.volt`
(`bits<128>`) için de ölçüldü.

## 2. Karar: proje kipi

Dosya argümanı verilmeyen `check`, `build`, `run`, `verify` çalışma
dizininden yukarı ilk Volt.toml'u (ADR-0061 araması ve tavanı) bulur.

- **Kaynaklar:** `[package] src` altındaki `*.volt`, `*_test.volt`
  hariç; atlama kuralları test keşfiyle aynı yürüyücü (ADR-0089: gizli
  dizinler, `build`, `target`, iç içe proje, `.gitignore` dizinleri).
- **`volt check`:** her kaynak BİR kez. Başka bir kaynağın `use` ile
  yüklediği dosya o birimde denetlenir (kütüphane dosyası tamamen
  denetlenir, ADR-0070) — tanı iki kez basılmaz. Yalnız birbirini yükleyen
  döngü kalırsa o dosyalar da kök olur. Tek "Finished/Result" satırı;
  `--format json` dosya başına bir zarf (ardışık JSON değerleri).
- **Üst modül — ikisi birden:** `[package] top = "Soc"` (ya da liste
  `top = ["A", "B"]`) yazılmışsa o; yoksa çıkarım: projenin kaynaklarında
  HİÇ örneklenmemiş, generic olmayan modül (örnekleme kuralı emitter'ınki:
  modül gövdesinin üst düzey `Instance` deyimi, hedef yolun son parçası).
  - Çıkarımda **tek aday şarttır.** Birden fazla aday kullanım hatasıdır
    (çıkış 2): adaylar dosyalarıyla listelenir, `top = "..."` ve dosya
    argümanı önerilir. **Sessiz seçim yok.** Aday yoksa aynı biçimde hata.
  - Açık `top` adı projede tanımlı değilse ya da iki dosyada tanımlıysa
    kullanım hatası; projedeki modüller listelenir.
  - Neden ikisi birden: çıkarım şablonlar ve küçük projelerde yazmadan
    çalışır; kütüphane modülü taşıyan projede (iki bağımsız kök) açık alan
    kararı kullanıcıya bırakır. Yalnız açık alan her projeye bir satır
    yazdırırdı; yalnız çıkarım test sarmalayıcısı ya da kullanılmayan
    modül eklenince sessizce kırılırdı (hata olarak görünür, ama çözüm yolu
    yoktu).
- **`volt build`:** üst modül(ler)in dosyaları, ilk görülme sırasıyla;
  her dosya bugünkü `volt build <dosya>` ile aynı (çıktı kümesi ADR-0042
  eki). En kötü çıkış kodu döner.
- **`volt run`:** tek üst modül ister; dosyası ve `--top`'u Volt.toml'dan.
  `volt run --top X` projede X'i tanımlayan dosyayı bulur. Liste birden
  fazlaysa hata ve `volt run --top <ad>` önerisi.
- **`volt verify`:** tek üst modülün dosyası; liste birden fazlaysa hata ve
  dosya önerisi (bir dosyanın bütün modülleri doğrulanır — bugünkü kural).
- **Volt.toml yok:** kullanım hatası korunur (çıkış 2), ileti yol
  gösterir: `volt <komut> design.volt` ya da `volt new <name>`.
- **Dosya verilen her kullanım bayt bayt eskisi gibidir** (§6 golden).
  "Next:" satırları yalnız proje kipinde dosya adsızdır.
- **Şablonlar:** dördünün Volt.toml'u `top = "<Modül>"` taşır; README'ler,
  `counter.volt`/`blinker.volt` başlık yorumları ve `volt new`'in "Next:"
  satırları argümansız komutları gösterir.

## 3. Karar: `volt test` dalga formu — B

Seçenekler (§1d ölçümüyle):

- **A — her testte iz:** başarılı yolda da grup başına +1,3–2,1 s derleme
  ve her test için VCD; `--watch` döngüsünde her kayıtta ödenir.
- **B — düşen testi izle yeniden koş:** testler izsiz koşar; bir grupta
  test düşerse YALNIZ düşen testler `--trace` ile ayrı bir yürütülebilirde
  (`tb_<M>_waves.cpp`, `obj_<m>_waves/`) bir kez daha koşar. Simülasyon
  deterministiktir (Verilator, sabit uyarıcı). Başarılı yolda maliyet ve
  çıktı SIFIR değişir; düşüşte bir derleme (~10 s Docker'da) eklenir.

**Karar: B varsayılan.** A `--waves` ile istenir, `--no-waves` kaydı
kapatır (`conflicts_with`). Ayrıntılar:

- Testbench: `test_testbench_cpp_traced` — `run_cycle(dut, ctx)` imzası
  DEĞİŞMEZ (betik adımları aynı), döküm global `volt_tfp` üzerinden;
  `VoltTrace` testin her çıkışında (erken `return false` dahil) VCD'yi
  kapatır; `traceEverOn` model kurulmadan önce. İzsiz testbench metni bayt
  bayt aynı (golden `tb_Table.cpp` değişmedi).
- Yol: `build/sim/<test dosyası>/waves/<Modül>-<test>.vcd`; testbench
  yolu çalışma dizinine (sim dizini) göre açar — yerelde ve Docker'da
  aynı, konteyner yolu hiç görünmez (ADR-0094). Grubun eski kayıtları
  koşu başında silinir.
- Rapor: düşen testin bloğunun son satırı
  `  Waveform gtkwave <vcd> <gtkw>` (ADR-0092 biçimi, ana makine yolu).
  Geçen testlerde çıktı değişmez. Yeniden koşuda test geçerse (belirlenimci
  olmayan tasarım) uyarı basılır, satır yazılmaz; iz derlemesi düşerse
  uyarı basılır, test sonucu değişmez.
- Oturum: ADR-0092'nin enum/Trit çevirilerine ek olarak sınanan modülün
  portları çevirisiz izler olarak listelenir (`@28` tek bit, `@24` vektör
  ondalık — raporla aynı taban); enum portu bir kez, çevirisiyle. Bu
  yüzden `volt test` oturumu enum'suz tasarımda da yazılır; `volt run` ve
  `verify` davranışı değişmedi.

## 4. Karar: `volt test --watch`

- İzlenen küme: proje kökünden (yoksa çalışma dizini, özyinelemesiz)
  gizli olmayan bütün dosyalar — `.volt`, test veri dosyaları, Volt.toml.
  Test keşfinin atlama kuralları geçerli; `--target-dir` proje içindeyse o
  da izlenmez; editör yedekleri (`x~`) ve gizli dosyalar izlenmez; süzgeç
  bir dosyaysa o da izlenir.
- Değişiklik **yoklamayla** bulunur (300 ms; liste + mtime + boyut): yeni
  bağımlılık yok, üç platformda aynı. İlk değişiklikten sonra küme 250 ms
  sabit kalana dek beklenir — art arda kayıtlar tek koşu olur (§6 testi).
- Her koşu `── volt test --watch · run N · SS:DD:ss UTC ──` ayırıcısıyla
  başlar, `Watched <sonuç> in N.Ns; waiting for changes in K file(s)`
  özetiyle biter. Saat UTC'dir: yerel saat dilimi standart kitaplıkta yok.
- **Ctrl-C:** `ctrlc` yakalayıcısı YALNIZ `--watch`'ta kurulur. Bu kipte
  Docker konteynerleri `volt-watch-<pid>-<sıra>` diye adlandırılır; adlar
  koşu boyunca kayıtta kalır ve her koşu sonunda ve kesmede tek bir
  `docker rm -f <adlar…>` ile süpürülür. Kesme sürerken yeni konteyner
  başlatılmaz ve öldürülen konteynerin 137 çıkışı "bellek bitti" diye
  raporlanmaz. Çıkış kodu 130, son satır `Stopped watching`.
- **Yeni bağımlılık `ctrlc` 3.5** (workspace): konsol kesmesini yakalamak
  Windows'ta `SetConsoleCtrlHandler`, Unix'te sinyal işleyicisi ister;
  ikisi de `unsafe` FFI'dır ve proje `unsafe`'i yasaklar. `ctrlc` bunu
  güvenli API arkasında yapan yaygın crate'tir; ek geçişli bağımlılıklar
  Linux'ta `nix`, macOS'ta `dispatch2`/`objc2`, Windows'ta zaten kilitli
  `windows-sys`. Alternatif `tokio::signal` async çalışma zamanı isterdi.

## 5. HybridTop: > 64 bit port

Düzeltme testbench üretiminde (§1e): `SimPort.bits` (derleme zamanında
çözülen genişlik; çözülemezse `None`). Genişliği 64'ü aşan ya da bilinmeyen
port `volt_drive`/`volt_show` C++ şablonlarıyla sürülür ve basılır:
skaler tipte düz atama ve ondalık, `VlWide<N>`'de sözcük sözcük atama ve
`0x` + onaltılık yazım (sütun genişliği sözcük sayısından). Yardımcı
yalnız böyle bir port varsa eklenir — dar portlu testbench'ler bayt bayt
aynı (golden: 186 komutta tek fark `key_store` testbench'i). HybridTop ve
key_store gerçek Verilator'da (Docker) simüle edildi.

## 6. Doğrulama

- Testler: `project_mode_tests` (15), `test_waves_tests` (4; Unix'te gerçek
  SIGINT → 130), `sim_waves_tests` (6), birim testleri (`project`,
  `watch`, `interrupt`, `test_waves`, `sim_width`, manifest `top`),
  `docker_e2e_tests`'e proje kipi + düşen test kaydı adımı (VOLT_DOCKER_E2E).
- Windows + Docker (`volt new demo; cd demo; volt check; volt build;
  volt test`): kasıtlı yanlış iddia → çıkış 5, `Waveform gtkwave
  build\sim\counter_test\waves\Counter-counts_while_enabled.vcd …gtkw`,
  dosyalar ana makinede, `.gtkw`'nin 4 izi de VCD başlığında
  (`build/meas/gtkw_check.py`).
- `--watch` (Windows + Docker, `build/meas/watch_e2e.py`): art arda iki
  kayıt → tek yeniden koşu; konteyner koşarken CTRL_BREAK → çıkış 130,
  `docker ps -a`'da `volt-watch-*` yok. **Ölçüm:** normal kesmede
  `docker.exe` sinyali konteynere iletir ve konteyner `--rm` ile kendiliğinden
  gider; ADR-0094'ün sınırı istemci sinyali iletmeden öldüğünde görünür
  ("sert" kip: `docker.exe` zorla öldürülür, konteyner koşmaya devam eder).
  İlk sürüm bu kipte kalıyordu (ad `docker run` dönünce kayıttan
  düşüyordu; ayrıca süren süpürmenin `rm`'i de kesmeyi alıp ölüyordu) —
  §4'teki kayıt/süpürme kuralı bu ölçümün sonucudur.
- Mutasyon (tek tek, hepsi öldürüldü): üst modül çıkarımında ilk adayı
  sessizce seç → `project_mode_tests`; izleme anlık görüntüsü mtime/boyutu
  yok say → `test_waves_tests` (watch); VCD yolunu testbench'e ana makinenin
  mutlak yolu olarak göm → `docker_e2e_tests`; `docker rm -f` yok →
  sert kip `watch_e2e.py` (konteyner kaldı).
- Golden: main ikilisi ile dal ikilisi, `examples/` altındaki her tasarım
  için `check` (human/json/short), `build` (human/json + dosyalar), `run`,
  `verify`, her test dosyası için `test` (sahte araçla; üretilen dosyalar
  dahil), `volt`, `explain E3001` — 186 komut, 185'i bayt bayt aynı; tek fark
  §5'in amaçlanan `key_store` testbench'i (`build/meas/golden.py`).

## Reddedilen seçenekler

- **A (her testte iz) varsayılan:** §1d — başarılı yolda her koşuda
  derleme maliyeti; `--waves` ile isteğe bağlı kaldı.
- **`notify` crate'i ile dosya olayları:** yeni bağımlılık ve platform
  farkları (Windows'ta olay birleştirme, ağ sürücüleri); proje boyutunda
  300 ms yoklama yeterli ve ölçülebilir.
- **Birden fazla adayda ilkini seçmek:** sessiz seçim; hangi modülün
  simüle/doğrulandığı kullanıcıdan gizlenirdi.
- **`volt check` her kaynağı ayrı ayrı:** kütüphane dosyasının tanıları
  her yükleyende tekrar basılırdı.

## Sınırlar

- `volt test` test dilinde 64 bitten geniş port değerini yazamaz
  (`bits<100>` portuna `5` → E2003, ölçüldü); geniş portlu modül bugünkü
  gibi sarmalayıcı modülle sınanır (`hybrid_test.volt`). §5 yalnız
  `volt run` testbench'ini kapsar.
- `volt verify` ve `volt run` proje kipinde tek üst modülde çalışır; liste
  için dosya ya da `--top` verilir. `check-regmap` dosya ister (değişmedi).
- `--watch` saati UTC gösterir. Yoklama çok büyük ağaçlarda (binlerce
  dosya) her 300 ms'de dizin yürür.
- Windows'ta güvenli kodla konsol kesmesi gönderilemediği için otomatik
  testlerde Ctrl-C yolu yalnız Unix'te (SIGINT) sınanır; Windows yolu
  §6'daki betikle elle doğrulandı.
- Yeniden koşu, testbench'in deterministik olduğunu varsayar; olmayan
  tasarımda uyarı basılır, kayıt yazılmaz.
- `volt verify` ve `volt run` konteynerleri `--watch` dışında adsızdır;
  Ctrl-C davranışları ADR-0094'teki gibidir.
