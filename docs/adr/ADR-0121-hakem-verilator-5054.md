# ADR-0121: Hakem Verilator 5.054 — Docker Arka Ucunun İmajı ve CI'ın Verilator'ı 5.054'e Geçer (#309)

> Statü: Uygulandı
> Önceki karar: ADR-0094 — §2.2'deki `test, run` imaj satırı (`verilator/verilator:v5.052@sha256:a5b73e2f…7352120`, Verilator 5.052) ve aynı bölümün "CI farkı" cümlesindeki 5.052
> İlgili: ADR-0107 (hakem sürümü: güncelleme kural 4'e göre ayrı PR, kanaryadan geçer; bu ADR iki kanarya ölçümü ekler), ADR-0120 (Sınırlar: "Volt'ta sabit olmayan, Verilator'da sabitleşen miktar", #309, bu ADR'yle kapanır; 5.054'te kalan dar hata bilinen sınır olur), ADR-0100 (sessiz yanlış istisnası: son hanede)
> Issue: #309
> Tarih: 2026-11-17
> Etkilenen: `scripts/ci/verilator.env`, `crates/volt-tools/src/docker.rs`
> (`SIMULATION_IMAGE`), `crates/volt-tools/src/lib.rs` (belge),
> `.github/actions/verilator/action.yml`, `crates/volt-diagnostics`
> (`simulation-setup` metni, en/tr), `crates/volt-tools/tests/verilator_pin_tests.rs`,
> `crates/volt-driver/tests/{verilator_canary_tests,signed_overshift_sim_tests,docker_backend_tests,docker_e2e_tests}.rs`,
> `crates/volt-driver/tests/verilator_canary/issue9256.sv` (YENİ),
> `tests/real_bugs/309_port_tied_signed_shift.volt` (YENİ), `.cargo/mutants.toml`
> (A2 kaydının gerekçesi), kitap (`limitations.md`, `volt test` ve `volt doctor`
> çıktıları), `README.md`, `examples/README.md`,
> `.github/workflows/README.md`, `CHANGELOG.md`

## Sorun

ADR-0120 işaretli `>>`'nin sabit K ≥ w miktarını SV'de `>>> (w-1)` yazar;
Verilator 5.050–5.052'nin hatası (sabit K, C++ kelime genişliğine (cppw)
eşit ya da büyükken işaret biti cppw-1'den alınır, negatif değere 0) bu
biçimde oluşmaz. Ama düzeltme yalnız Volt'un sabit gördüğü miktarları
kapsar. Miktar Volt'ta bir port, örnekte sabite bağlıysa Volt `a >>> k`
yazar; Verilator modülü açıp sabiti yayınca aynı hata oluşur (#309):

```volt
module G {
    in  a : i63
    in  k : u32
    out y : i63
    y = a >> k
}
module P {
    in  a : i63
    out y : i63
    let g = G { a: a, k: 64 }
    y = g.y
}
```

5.052'de `volt test` negatif `a` için 0 verir; ADR-0120'ye (ve IEEE
1800-2017 §11.4.10'a) göre değer -1'dir, Yosys ve `volt verify` -1 verir.
Volt bu sınıfı genel olarak göremez: miktarın sabitleşmesi Verilator'ın
eniyileyicisine bağlıdır. Hata Verilator 5.054'te düzeldi (Verilator
commit 84dec259c8).

## Karar

### 1. Hakem 5.054

Docker arka ucunun simülasyon imajı ve CI'ın Verilator'ı (ADR-0107,
"Hakem sürümü") Verilator 5.054'tür:

| Komut | Başvuru | İçerik | İndirme (sıkıştırılmış, amd64) |
|---|---|---|---|
| test, run | `verilator/verilator:v5.054@sha256:da7845c3…0e610448` (çok platformlu indeks: amd64 + arm64) | Verilator 5.054, g++ 13.3, make 4.3, Ubuntu 24.04 | 253 MB |

Tam özet `sha256:da7845c38c2ab84133661a1b1e2d717d028fe0bc629cc06c77bb80571e610448`;
tanım yine tek yerdedir (`scripts/ci/verilator.env`), `docker.rs`
`SIMULATION_IMAGE` onunla aynıdır (`verilator_pin_tests`). `volt doctor`'ın
`contents` metni "Verilator 5.054, g++ 13.3" olur; indirme bildirimi
"~250 MB" kalır.

`verilator_pin_tests` bir denetim daha alır: kitap, README'ler, `volt
explain` ve testler imajı adıyla anıyorsa sabitlenen sürümü anar (tarih
kaydı olan ADR'ler, CHANGELOG ve sürüm notları taranmaz). Böylece bir
sonraki güncelleme anılan her yeri bulur.

### 2. `-fno-dfg` kalır

Verilator #9060 5.054'te düzelmedi: DFG açıkken #145 ve #148 kanaryalarının
ikisi de yanlış basar (aşağıda, "Ölçüm"). ADR-0107'nin "Bayrağın
kaldırılma ölçütü" sağlanmadı; bayrak kalır. Kanarya testi DFG açık
sonucu sürüm başına ölçülen satırlarla karşılaştırır (`DFG_ON_MEASURED`):
#9060 bir sürümde düzelirse test bunu gösterir, kaldırma yine kendi
PR'ıdır.

### 3. Kanaryalar

- `ISSUE_302_MEASURED` (#302): 5.052 ve 5.054 satırları yeniden ölçüldü,
  değişmedi.
- `DFG_ON_MEASURED` (#9060, YENİ): #145 ve #148 tasarımları DFG açıkken,
  yanlış basılan alanlar.
- `verilator_canary/issue9256.sv` ve `ISSUE_9256_MEASURED` (YENİ):
  5.054'te kalan dar hata, #309'un biçiminde (miktar sabite bağlı port).
  Volt bu biçimden kaçınamaz; test doğru değeri değil, ölçülen sürümün
  ölçülen yanlışını sınar.
- Sabitlenen sürümün her tabloda satırı olmalıdır
  (`the_pinned_verilator_release_has_a_measured_row_in_every_table`).

### 4. Bilinen sınır: Verilator #9256

5.054'te kalan dar hata (https://github.com/verilator/verilator/issues/9256):
işaretli, 64 bitten geniş, w mod 32 = 1 (65, 97, 129) bir **bileşik**
işlenen (`(e + f)`, port ya da sinyal değil), K ≥ cppw (32·⌈w/32⌉) sabit
miktarla sağa kaydırılınca negatif değerde `…0001` verir. Volt'un sabit
gördüğü miktarlarda ADR-0120'nin `>>> (w-1)` biçimi bundan kaçınır (w - 1
< cppw). Kalan sınıf #309'unkiyle aynı biçimdir: miktar sabite bağlı bir
port. Dört koşul birlikte gerekir (geniş, w mod 32 = 1, bileşik, sabite
bağlı port ≥ cppw). Geçici çözüm: miktarı kaydıran modülün `const`'u ya da
generic parametresi yapmak. Kitabın "Known limitations" sayfası bunu
yazar; issue: #311.

### 5. Sürüm

#309 sessiz yanlıştır (`volt test` ile donanım ayrışır); ADR-0100'ün
istisnasıyla son hanede çıkar. Sürüm notunun "Behavior changes"
bölümüne girer: Docker arka ucunun ve önerilen Verilator'ın sürümü
5.054'tür; ilk `volt test` yeni imajı indirir (~250 MB); `volt doctor`
satırı "Verilator 5.054" der. Derleyicinin ürettiği SV değişmez.

### 6. Şartname

`docs/spec/cli-contract.md` §8b (imaj tablosu, bildirim satırları) ve §9a
(`volt doctor` örneği) imajı 5.054 diye anar: bakımcının a5429208
commit'i, altı satır. §8b'nin tablosunda `verilator/verilator:v5.054`,
"Verilator 5.054, g++ 13.3, make"; üç bildirim satırında ve §9a'nın
örneğinde imaj adı ve sürüm. Boyut (~250 MB) ve indirme süresi örneği
(8m 37s, ölçülmüş bir koşu) değişmedi.

## Ölçüm

Yerel Docker (Windows 11, Docker Desktop), iki imaj:
`verilator/verilator:v5.052@sha256:a5b73e2f…` ve
`verilator/verilator:v5.054@sha256:da7845c3…`. Kanaryalar Verilator'ın
kendisiyle, Volt'un komut satırıyla (`--binary --timing --assert -Wall
-Wno-fatal -Wno-DECLFILENAME --top-module tb -Mdir obj -o vtb -j 2
-CFLAGS -O0`, `-fno-dfg` ile ve onsuz).

#145 / #148 (`R 0 17` satırının ilk 11 hanesi; doğrusu `fc0040003f8`,
`o151k`, `o151e/d/b/a` yanlışken `fc00ffffff8`):

| Verilator | Seçenek | `issue145.sv` | `issue148.sv` |
|---|---|---|---|
| 5.052 | DFG açık | doğru | **yanlış** |
| 5.052 | `-fno-dfg` | doğru | doğru |
| 5.054 | DFG açık | **yanlış** | **yanlış** |
| 5.054 | `-fno-dfg` | doğru | doğru |

#302 (`R302` satırı: y64, y62, s130, s64; doğrusu y = `7fffffffffffffff`,
s = `1ffffffffffffffff`):

| Verilator | Seçenek | y64 (`a >>> 64`) | y62 (`a >>> 62`) | s130 (`(e + f) >>> 130`) | s64 (`(e + f) >>> 64`) |
|---|---|---|---|---|---|
| 5.052 | ikisi | `0000000000000000` | doğru | `00000000000000000` | doğru |
| 5.054 | `-fno-dfg` | doğru | doğru | `00000000000000001` | doğru |
| 5.054 | DFG açık | doğru | doğru | doğru | doğru |

#9256 (`R9256`: miktar sabite bağlı port, a + b = -1):

| Verilator | Seçenek | y65 (K 130) | y97 (K 130) | y129 (K 200) | y66 (K 132) |
|---|---|---|---|---|---|
| 5.052 | ikisi | 0 | 0 | 0 | 0 |
| 5.054 | ikisi | `…0001` | `…0001` | `…0001` | doğru |

## Kanıt

### Yerel (Docker, Verilator'ın iki imajı, `VOLT_REQUIRE_TOOLS=verilator`)

| Test | 5.052 | 5.054 |
|---|---|---|
| `signed_overshift_sim_tests::issue_309_…` (#309) | **düşer**: `left: 0`, `right: 9223372036854775807` | geçer |
| `signed_overshift_sim_tests` (kalan 8) | geçer | geçer |
| `metamorphic_tests::real_bugs` (`309_port_tied_signed_shift.volt`) | **düşer**: `1 passed; 1 failed` | geçer (45 test) |
| `verilator_canary_tests` (7) | geçer | geçer |

Volt düzeyinde #9256 (miktar sabite bağlı port, `(e + f) >> k`, `e, f :
i64`, k = 130; `[63:0]` dilimi): 5.052 `0`, 5.054 `1` (doğrusu
`0xFFFF_FFFF_FFFF_FFFF`). Aynı kaydırma generic parametreyle (`HG<130>`)
iki sürümde de doğru.

A2 mutantı (`.cargo/mutants.toml`, `clamped_signed_shift`'te `w - 1` →
`w / 1`), atılabilir bir worktree'de, 5.054'te: `signed_overshift_sim_tests`
9 (#309 dahil), `metamorphic_tests` 45, `verilator_canary_tests` 7,
`difftest_tests` 1 geçer; üretilen SV (`a >>> 63`, `65'(e) + 65'(f) >>>
65`) `verilator --lint-only -Wall` ile temiz. Eşdeğerlik sürer: K = w,
5.054'te kalan dar hatanın sınıfında da cppw'den küçüktür.

README'nin ve "Using Volt in an existing project" tarifinin ölçümü 5.054'te
yinelendi: `PressCounter.sv` ve `RisePulse.sv` `--lint-only -Wall` ile
temiz, elle yazılmış testbench iki basışı sayar (`count=2`).

Kitabın `volt test` ve `volt doctor` çıktıları (sekiz `volt test`, iki
`volt doctor` bloğu) Windows 11'de, Docker Desktop ile, bu dalın `volt`'uyla
yeniden alındı: bloklar yalnız imaj satırında değişti. İlk `volt test`'in
indirme satırı (`downloaded verilator/verilator:v5.054 in 2.7s`) gerçek
koşudandır, ama imajın katmanları Docker'ın içerik deposunda kaldığından
soğuk bir indirme değildir; kitabın indirme süresi cümlesi (45–57 s,
install.md) eski ölçümdür.

### CI

Önce düşen: kanıt PR'ı #310 (bu PR'ın iki test commit'i, hakem 5.052),
run 44074669626, "Verilator lint": `Verilator version check: OK (Verilator
5.052 2026-09-05 rev v5.052)`, `error: 2 targets failed`:
`metamorphic_tests` (`real_bugs`: `309_port_tied_signed_shift.volt`,
`test result: FAILED. 1 passed; 1 failed`) ve `signed_overshift_sim_tests`
(`issue_309_…`: `left: 0`, `right: 9223372036854775807`). Öteki işler
yeşil.

Bu PR (3d1674d9), run 44075582208, bütün işler yeşil; mutasyon kapısı (run 44075582548) 4 mutant: 3 yakalandı, 1 derlenemez, 0 kaçan. "Verilator lint":
`Verilator version check: OK (Verilator 5.054 2026-11-01 rev v5.054)`; 206
test özetinde 4139 geçti, 0 düştü; `issue_309_…`, `real_bugs` ve yedi
kanarya testi geçti; fark testi (tohum 20261104, 2000) `diffs=0`,
`regressions=39 failed=0 hand-computed=58`; `Tool ledger [integration]:
206 test(s) asked for a tool; 208 lookup(s) found the tool (208
required), SKIP: 0`; "Tool coverage" `209 ran with the tool required, 0
did not`. "Docker backend": `running it in Docker
(verilator/verilator:v5.054)`, `SKIP: 0`. Kitap (run 44075582571): `31
volt block(s) checked, 8 test file(s) run, 7 verify run(s), 0 failure(s)`.

5.052 ile karşılaştırma (aynı girdiler):

| Koşu | 5.052 (main 88f536ba) | 5.054 (bu dal) |
|---|---|---|
| CI "Verilator lint" | run 44066170905: 206 özet, 4135 geçti, 0 düştü; `SKIP: 0` (205 arama); difftest `diffs=0` | 206 özet, 4139 geçti (+4: #309 testi, iki yeni kanarya testi, imaj anma denetimi), 0 düştü; `SKIP: 0` (208) |
| Kitap | run 44066169719: 31 blok, 8 test dosyası, 7 verify, 0 hata | aynı |
| Fark testi büyük koşusu (tohum 20261105–20261108, parça başına 12500) | run 44075236280: 50.100 kabul, 5.384.798 değer karşılaştırması; parça 0/1/3 `diffs=1/3/1`, parça 2 `diffs=0` | run 44075586023: birebir aynı sayılar ve aynı beş bulgu |
| Metamorfik gecelik (tohum 2026-11-17) | run 44070588184: 491 tasarım, 178 donanım karşılaştırması; gerçek hata derlemi 25 örnek, 4 bilinen başarısızlık | run 44075592058: 492 tasarım, 179 karşılaştırma; 26 örnek (#309 dahil), 4 bilinen başarısızlık; `SKIP: 0` |

Fark testinin beş bulgusu iki sürümde de #303'tür (`as bits<N> as uN`
biçiminin `w` varyantında Verilator WIDTHEXPAND, `volt test` durur):
hakem değişikliği #303'ü ne düzeltir ne kötüleştirir; #303 açık kalır.
Metamorfik koşudaki tek fark yeni `309_port_tied_signed_shift.volt`
dosyasıdır.

## İlke denetimi

1. **Beyan:** uygulanmaz.
2. **Çıkarım:** uygulanmaz.
3. **Anlam değişikliği:** yok. Dilin anlamı ve üretilen SV aynıdır;
   değişen hakemin (Verilator) sürümüdür. 5.052'nin yanlış sonucu (#309)
   5.054'te doğru olur.
4. **Niyet tahmini:** uygulanmaz.
5. **Metamorfik yeniden yazım:** yok.

## Sınırlar

- **5.054'te kalan dar hata** (yukarıda, §4) bilinen sınırdır; Verilator
  düzeltince kanarya satırı değişir ve test düşer.
- **Yerelde 5.050–5.052 kullananlar** #309'u görmeye devam eder. `volt
  doctor` bu sürümleri bilinen hatalı aralık olarak bildirmez (aşağıda,
  "Reddedilenler"); kitap 5.054'ü ya da `VOLT_TOOL_BACKEND=docker`'ı
  önerir.
- `-fno-dfg` kalır (§2).

## Reddedilenler

- **Hakemi değiştirmemek** (#309'un 2. seçeneği). Sabite bağlı port
  miktarı CI'da ve Docker arka ucunda sessizce yanlış kalırdı.
- **Verilator master ya da 5.055 devel.** ADR-0107 kural 1: hakem bir
  sürüm etiketidir.
- **`volt doctor`'da bilinen hatalı sürüm aralığı (5.050–5.052).** Yararlı,
  ama bu ADR'de yapılmaz: araç durumu sözlüğüne yeni bir değer
  (`cli-contract.md` §9a, `ok | missing | unresponsive | broken |
  too_old`) ya da yeni bir metin satırı ister. ADR-0100 §1'e göre JSON
  değerinin değişmesi orta hanedir; şartname değişikliği de kendi ADR'sini
  ister. Öneri: v0.2.0'da ayrı bir ADR'yle `known_bug` durumu (yetenek
  `degraded`, `--strict` 1), satırında hatanın tek cümlesi ve
  `VOLT_TOOL_BACKEND=docker` önerisi; yeni tanı kodu gerekmez (doctor
  satırları tanı değildir).
