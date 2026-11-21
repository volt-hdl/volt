# ADR-0129: Mutasyon Kapısının Yeniden Denemesi — Yalnız Test Dosyası Değişen Push'ta Yalnız Sonucu Değişebilecek Mutantlar Yeniden Sınanır

> Statü: Önerildi
> Önceki karar: ADR-0109 — "PR kapısı" eki §1 (her push değişen satırların bütün mutantlarını yeniden dener) ve "Kapının önceki sonucu" eki §3 ile §7 (önceki sonuç yalnız anahtarı aynı ve temiz geçmiş bir koşudan, bütünüyle alınır; mutant başına yeniden kullanım kapsam dışı); kalan her kararı yürürlükte
> İlgili: ADR-0128 (doğrulama yöntemi ve `mutants_compare.py`; tek temel koşu işi, zaman aşımı tabanı ve test sırası yeniden denemede de aynıdır), ADR-0107 (temel koşu ve parçalarda defterde sıfır SKIP)
> Issue: #364, #366 (planın PR 3'ü)
> Tarih: 2026-11-21
> Etkilenen: `scripts/ci/mutants_reuse.py`, `scripts/ci/mutants_summary.py`, `scripts/ci/mutants_compare.py`, `.github/workflows/mutants.yml`, `.github/workflows/README.md`, `AGENTS.md`, `CHANGELOG.md`

## Sorun

Kapı missed mutantlar yüzünden düşünce düzeltme çoğu zaman yalnız testtir:
her B mutantı için bir test, A mutantı için bir `exclude_re` kaydı. Sonraki
push yine bütün listeyi baştan dener. #318'in yeniden kullanımı yardım
etmez: yalnız *geçmiş* ve anahtarı *aynı* bir koşuyu kullanır, test
değiştiren bir push ise ağacı, dolayısıyla anahtarı değiştirir (#364'ün
ölçümü: #343'te 5, #360'ta 9 açık soru için yine 68 ve 30 mutant, 4 ve 3,5
saat).

Testten başka bir şey değişmediyse bir mutantın sonucu ancak onu sınayan
testler değiştiyse değişebilir. Bu ADR hangi mutantların yeniden
sınanacağını ve kalanların sonucunun nasıl taşınacağını belirler.

## Karar

### 1. Koşul

Kapı, bu PR'ın **son tam kapı koşusunu** taban alır: en yeni, tamamlanmış,
iptal edilmemiş, bütün listeyi kendisi sınamış koşu. #318'in yeniden
kullanımı da önceki bir yeniden denemesi de tam koşu sayılmaz; bunlar
atlanır. Tabandan daha eski bir koşuya hiç bakılmaz. Yeniden deneme şu
koşulların hepsi tutarsa yapılır:

- **Mutantlar aynı.** Anahtarın (#318) `mutants` kısmı alan alan aynıdır:
  hariç tutmalardan önceki liste (`file:line:col` ile adlar) ve
  değiştirdikleri dosyaların blob'ları.
- **Ortam aynı.** Anahtarın `environment` kısmı aynıdır:
  - `exclude_re`'siz `.cargo/mutants.toml`;
  - iş akışı ve araç sabitlemeleri;
  - rustc, cargo-mutants ve nextest sürümleri;
  - imajın haftalık sürümü ve araç zinciri.
- **Değişen her yol bir test dosyasıdır.** Anahtarın `tree` kısmında
  tabandan bu yana eklenen, silinen ya da değişen her yol bir çalışma
  alanı paketinin `tests/` dizinindedir (`crates/<paket>/tests/…`).
  Etkisiz yollar (#318) ağaç kısmında hiç yoktur.
  - Kaynak dosyadaki `#[cfg(test)] mod tests` bir kaynak değişikliğidir.
  - Deponun kökündeki `tests/` (ui ve gerçek hata derlemleri) birçok
    crate'in testlerince okunur.
  - `Cargo.toml` ve `Cargo.lock` da test dosyası sayılmaz.
  - Bunlardan birinin değişmesi kapıyı tam koşturur.
- **Taban eksiksiz.** Temel koşu işi geçmiş: artifact'ı, damgası,
  `--min-ran 60`. Her parça tamamlanmış: listedeki her mutantın sonucu
  var. Defterlerde SKIP 0. Damgalar anahtarın araçlarını ve imajını
  taşıyor. Taban kapıyı geçmiş olmak zorunda değildir: missed'i olan bir
  koşu tam da yeniden denemenin konusudur.

Koşullardan biri tutmazsa kapı tam koşar ve günlük nedenini yazar: hangi
yol, hangi alan, hangi eksik.

### 2. Yeniden sınananlar

Bugünkü listenin (bugünkü `exclude_re`'den sonra) her mutantı için:

| Tabandaki sonuç | Karar |
|---|---|
| missed, timeout, bellek sınırı | yeniden sınanır |
| sonucu yok (o gün hariç tutulmuş, bugün değil) | yeniden sınanır |
| caught; yakalayan test ikilisi bu push'ta değişti | yeniden sınanır |
| caught; günlüğü yakalayan ikiliyi adlandırmıyor | yeniden sınanır |
| caught; yakalayan ikili değişmedi | sonucu taşınır |
| unviable | sonucu taşınır |

- **Yakalayan ikili** tabanın parça artifact'ındaki mutant günlüğünden
  okunur (`mutants.out/outcomes.json` günlüğün yolunu verir). Okunan,
  nextest'in hata satırıdır (`FAIL`, `SIG…`, `TIMEOUT` …): ikili kimliği
  ve test adı. `Cancelling due to test failure` satırından sonraki satırlar
  nextest'in durdurduğu testlerdir, yakalayan değildir; sayılmaz. Hata
  satırı yoksa eşleme kurulamamıştır ve mutant yeniden sınanır.
- **Değişen ikililer.**
  - `cargo metadata`'ya göre bir bütünleşme testi hedefinin kök dosyası o
    ikilidir (`<paket>::<hedef>`).
  - Bir paketin `tests/`'indeki her başka yol o paketin bütün ikilileri
    sayılır: bir modül, bir veri dosyası ya da silinmiş bir hedef. Kütüphane
    birim testleri de buna dahildir.
- **Neden doğru.** Kaynak aynı, ortam aynı, yakalayan ikilinin kaynağı
  da aynıysa o ikilinin düşen testi aynı mutantla yine düşer; mutant yine
  caught olur. Bu kez onu daha önce başlayan başka bir test yakalayabilir,
  ama sonuç değişmez. Unviable bir mutantın derlemesi mutasyonun kendisinde
  kırılır; testler onu değiştiremez.

### 3. Yürütme

- **Liste.** Yeniden sınanacak adlar hedefli koşu gibi daraltılır (ADR-0109
  "Hedefli koşu"):
  - Adlardan tam eşleşen bir `--re` yazılır; Rust ve Python regex'inde
    aynı anlama gelir.
  - `mutants_target.py diff` o adların satırlarını `--in-diff` için yazar.
  - cargo-mutants 27.1.0 alan silme mutantlarını `--re` ile süzmez. Yeniden
    sınanan bir satırdaki böyle bir mutant da yeniden sınanır.
  - `mutants_reuse.py retest-check` şunu kanıtlar: son liste yeniden
    sınanacak her mutantı tutar ve kapının listesi dışında hiçbir şey
    tutmaz. Kanıtlanamazsa kapı tam koşar.
- **Parça sayısı.** Yeniden sınanan mutant sayısından, kapının bugünkü
  hesabıyla bulunur. Değiştirilmesi ADR-0128 §4'ün işidir.
- **Temel koşu işi** (ADR-0128 §2) ve zaman aşımı tabanı (§3) aynen
  geçerlidir. Yeniden sınanacak mutant yoksa parça ve temel koşu yoktur;
  karar taşınan sonuçlardan verilir.

### 4. Özet ve karar

- Özetin başında `retest: run <id> (SHA <sha>): k of N retested, N-k
  carried over` yazar. Altında şunlar gelir:
  - değişen yollar ve değişen ikililer;
  - yeniden sınanan her mutant ve nedeni;
  - taşınan her mutant, tabandaki sonucu ve yakalayan ikilisiyle.
- Kapının kararı birleşik sonuçlardan verilir. Taşınan sonuç yalnız caught
  ya da unviable olabileceğinden, kapıyı düşüren missed'ler bu koşuda
  sınanmış mutantlardır.
- Kapının listesinde ne bu koşuda sınanmış ne taşınmış bir mutant varsa
  özet düşer. Taşınan bir sonuç caught ya da unviable değilse de düşer.
- `mutants_compare.py` bir yeniden denemeyi bütün kapı olarak okur: liste
  `gate-all.txt`, taşınan sonuçlar koşunun kendi sonuçlarıdır. Böylece bir
  yeniden deneme aynı commit'teki tam koşuyla mutant mutant
  karşılaştırılır.
- #318'in yeniden kullanımı bir yeniden denemeyi hiç yeniden kullanmaz.
  Yeniden deneme de yalnız tam koşuyu taban alır: taşınan bir sonuç her
  zaman onu ölçmüş koşudan gelir.

### 5. Elle

`gh workflow run mutants.yml --ref <dal> -f gate_base=<revizyon> -f
retest_from=<koşu>`: elle bir kapı koşusu, adlandırılan koşuyu taban alır.
Koşullar §1'inkilerdir; PR ve dal kapsamı yerine koşu açıkça verilir (aynı
depo, aynı iş akışı). `retest_from` yalnız `gate_base` ile geçerlidir. Bir
PR koşusu `retest_from` kullanmaz.

## Doğrulama yöntemi

ADR-0128'in yöntemi. #363'ün ağacında (ikinci referans: 3 missed) bir
taban commit'i A ve iki test commit'i kurulur:

- **B, test ekler.** Yeni bir test dosyası missed bir mutantı öldürür.
- **C, test zayıflatır.** Bir testin iddiası gevşetilir; yakaladığı bir
  mutant missed'e döner.

A'da tam koşu tabandır. B ve C'de hem yeniden deneme (`retest_from`) hem
tam koşu yapılır. `mutants_compare.py` iki çiftte de fark bulmamalıdır.
C'de, yakalanmış bir mutantın yeniden sınanıp missed çıktığı
gösterilmelidir.

## Kanıt

Koşular bitince bu bölüme yazılır.

## Reddedilenler

- **`#[cfg(test)]` öğelerini kesip kaynak dosyadaki testleri de "test"
  saymak** (#364'ün önerisi). Bunun için Rust'ı ayrıştırıp öğe sınırı
  bulmak gerekir. Aynı dosyadaki bir değişiklik mutantların satırlarını da
  kaydırabilir. Kaynak dosya değişince kapı tam koşar.
- **`cargo mutants --iterate`** (#364'ün (a) seçeneği). Önceki
  `mutants.out`'u çıktı dizinine geri koymak ve atlama kuralını
  cargo-mutants'a bırakmak gerekirdi. Atlama ölçütü (caught/unviable)
  burada yeterli değil: yakalayan ikilisi değişen bir caught mutant
  yeniden sınanmalıdır. Liste açık adlarla yazılır ve denetlenir.
- **Yalnız test eklemeye izin vermek.** #364 iki yol önermişti: test silen
  ya da zayıflatan push'u tam koşturmak, ya da riski kabul etmek. Bu ADR
  ikisini de seçmez. Yakalayan ikilisi değişen her caught mutant yeniden
  sınanır; zayıflatılmış bir test böylece görülür (Kanıt'taki C
  senaryosu).
- **Tabanı yeniden denemeye de taşımak.** Taşınan sonuç ölçüldüğü koşudan
  uzaklaşırdı. Taban her zaman son tam koşudur.
- **Yeniden denemede parça = k.** Parça sayısı ADR-0128 §4'ündür.

## Sınırlar

- Testin okuduğu ama `tests/` dışında duran bir dosya (kök `tests/`,
  `examples/`, kitap) değişince kapı tam koşar. Böyle bir değişikliği ikili
  başına eşlemek bu ADR'nin dışındadır.
- Yakalayan ikili tabanın günlüğünden okunur. Günlük artifact'ın süresi
  dolmuşsa (90 gün) taban okunamaz; kapı tam koşar.
- Taban, yakalanan bir mutantı ilk düşen testle kaydeder. Bu push başka
  bir ikilide o mutantı da yakalayan bir testi siler ya da zayıflatırsa
  sonuç yine caught'tır: yakalayan ikili değişmemiştir. Yeniden denemenin
  sonucu tam koşununkiyle aynıdır, ama o test artık o mutantı
  yakalamıyordur. Bunu kapı ölçmez; testin kendisi gözden geçirilir.
