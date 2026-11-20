# ADR-0128: Mutasyon Kapısının Süresi — Test Sırası, Koşu Başına Tek Temel Koşu, Sabit Zaman Aşımı Tabanı ve Parça Sayısı; Sonuçlar Birebir Aynı Kalır

> Statü: Önerildi
> Önceki karar: ADR-0109 — §1'in temel koşusu (her parçada) ve zaman aşımı (parçanın temel koşusunun iki katı), §1'in nextest gerekçesi (varsayılan sıra), §3 ve "PR kapısı" eki §1'in parça hesabı (S = 15 dk, t = 600 sn), "PR kapısı" eki §4'ün zaman aşımı; kalan her kararı yürürlükte
> İlgili: ADR-0104 (sürüm metni: `volt-sv-emit/build.rs` yeni commit'te yeniden koşmayı sürdürür), ADR-0107 (araç defteri nextest'in her sırasında aynı: SKIP: 0), ADR-0117 (metamorfik test kapının en yavaş ikilisidir)
> Issue: #366 (ölçüm ve plan), #364 (yalnız kaçanları yeniden sınamak), #300 (parçalarda ccache), #305 (fuzz derleminin yeniden oynatılması)
> Tarih: 2026-11-20
> Etkilenen: `.config/nextest.toml` (YENİ), `scripts/ci/mutants_nextest.py` (YENİ), `scripts/ci/mutants_compare.py` (YENİ), `.github/workflows/mutants.yml`, `crates/volt-sv-emit/build.rs`, `crates/volt-sv-emit/src/version_text.rs`, `AGENTS.md`, `.github/workflows/README.md`, `CHANGELOG.md`

## Sorun

PR kapısı (ADR-0109 eki "PR kapısı") dört ölçülen koşuda 27 dakika ile
5 sa 20 dk arasında sürdü (#366): #343'ün 68 mutantı 4 parçada 320 dk,
#360'ın 29 mutantı 2 parçada 190 dk, #363'ün 65 mutantı 3 parçada 240 dk,
#359'un 2 mutantı 27 dk. Ölçüm (iş ve adım süreleri, her parçanın
günlüğü, her mutantın `outcomes.json`'u, temel koşudaki her testin süresi)
#366'dadır. Özeti:

- **Parça süresinin %96'sı test.** Mutant başına derleme medyanda 23 sn,
  bütün mutant süresinin %3,3'ü.
- **Yakalanan mutant, yakalayan testi bekler.** 164 mutantın 139'u
  yakalandı. Yakalanan bir mutantın test süresi medyanda 960 sn, onu
  yakalayan testinki ~0,01 sn. nextest testleri ikili sırasıyla 4 yuvada
  başlatır; Verilator'lı `volt-driver::*` ikilileri `volt-hir`'in birim ve
  bütünleşme testlerinden öncedir; yakalayan test 4222 testin
  800–2400. sırasındadır. Yakalanan 139 mutantın 109'unu ilk yakalayan
  ikiliye PR'ın kendisi dokunmuştu. ADR-0109 §1 nextest'i "birim testleri
  bütün takımı beklemesin" diye seçmişti; varsayılan sırada beklerler.
- **Her parçanın temel koşusu 18–31 dk** (derleme 33–55 sn, testler
  1074–1805 sn). Aynı takımın süresi koşucudan koşucuya 1,68 kat
  değişir. Kurulum (araçlar, önbellekler) parça başına 28–55 sn.
- **Hiçbir mutant zaman aşımına gitmedi** (0/164; sınır 2148–3610 sn).
- **Her mutantın test aşaması `volt-sv-emit`, `volt-lsp` ve `volt-driver`'ı
  yeniden derler** (142/142, medyan 13,7 sn): `volt-sv-emit/build.rs`
  CI'ın sığ checkout'unda var olmayan `.git/packed-refs`'i
  `rerun-if-changed` ile izler; cargo var olmayan yolu her çağrıda değişmiş
  sayar (`Dirty volt-sv-emit …: the file .git/packed-refs is missing`).
- Planlanan t = 600 sn, ölçülen parça ortalamaları 408–1013 sn; #343'ün
  0. parçası 240 dakikalık hedefi aşıp 318 dk sürdü.

## Kalite ölçütü

Her değişiklik **sonucu değiştirmeden** süreyi kısaltır: aynı mutantlar,
aynı testlerle sınanır ve her mutantın sonucu (caught, missed, unviable,
timeout, bellek sınırı) aynı kalır. Mutant örneklemek, mutant başına
test kümesini daraltmak, araçlı testleri kapıdan çıkarmak ve fuzz süresini
kısaltmak bu ADR'nin dışındadır.

## Karar

### 1. Test sırası: nextest profili `mutants` (c′, uygulandı)

Parçalar testleri nextest'in `mutants` profiliyle koşar
(`.config/nextest.toml`, `NEXTEST_PROFILE=mutants`; temel koşu da aynı
sırayla). Sıra bir testin ne zaman başladığını belirler, koşup
koşmadığını değil: yakalanan mutant ilk düşen testte durur
(`--max-fail=1:immediate`), missed mutant bütün takımı koşar.

- **Öncelik 100: PR'ın dokunduğu test ikilileri.** Parça, temel koşudan
  önce kapının değişen dosyalarından (`mutants-list/changed.txt`)
  yazar (`scripts/ci/mutants_nextest.py pr`): bir paketin bütünleşme testi
  dosyası o ikiliyi, bir paketin `src/`'indeki dosya o paketin birim
  testlerini öne alır. Dosya depodaki kopyaya yazılmaz.
- **Öncelik −100: yavaş ikililer.** Temel koşuda testlerinin toplamı
  40 sn ve üstü olan `volt-driver` ikilileri (#366'nın 10 temel
  koşusunun medyanı; 22 ikili, takımın test süresinin ~%90'ı).
- **Öncelik 50: `volt-driver` dışındaki her crate'in testleri.**
- Her test için ilk eşleşen kural geçer; aynı öncelikteki testler
  nextest'in sırasını korur.
- `mutants_nextest.py check` profilin adlandırdığı her ikilinin
  çalışma alanında var olduğunu denetler (yeniden adlandırılan bir test
  dosyası sessizce listeden düşmez); parça bunu her koşuda yapar.
- Benzetim (#366; nextest'in kuyruk kuralları, bugünkü sırayı ölçüme göre
  doğrulanmış): #343 318 → ~58 dk, #360 189 → ~44 dk, #363 238 → ~104 dk,
  #359 26 → ~21 dk (en yavaş parça; temel koşu parçada kalırken).
- **Geçici girdi.** `workflow_dispatch`'in `nextest_order` girdisi
  (`priority` | `default`) aynı commit'i iki sırayla koşturur; bu ADR'nin
  referans karşılaştırmaları içindir, karar verilince kaldırılır.

### 2. Koşu başına tek temel koşu (a, sonraki PR)

Temel koşu parçadan çıkar, koşu başına tek bir iş olur: bugünkü adımın
aynısı (nextest, aynı profil, etkisiz yolların silinmesi, bellek bekçisi,
araçlar, `VOLT_REQUIRE_TOOLS=all`), parçalarla paralel. Özet işi o iş
geçmedikçe kapıyı düşürür ve mutant kararı vermez. Bugün parçanın temel
koşusundan gelen dört şey oraya taşınır: zaman aşımı (yerine §3'ün
tabanı), bellek tepe uyarısı (#253), yeniden kullanım damgası (#318),
defterin `--min-ran 60` denetimi (parçalar `--require-zero-skips`'i
korur). Aynı SHA'daki CI test işinin yeşil olması tek başına yetmez:
`cargo test` ile, etkisiz yollar dururken, bekçisiz ve ccache ile koşar.

### 3. Sabit zaman aşımı tabanı (b, 2 ile birlikte)

`--timeout = max(2 × koşunun temel koşusu, F)`. F, kapının kullandığı
sırayla ölçülen en uzun geçen tam takımın en az iki katıdır ve bugünkü en
büyük değerin (3610 sn) altına inmez: hiçbir mutant bugünkünden az süre
almaz. Koşucular 1,68 kat farklı hızdadır; tek bir koşucunun temel
koşusundan türeyen sınır başka bir parçanın koşucusuna uymaz, taban bu
yüzden sabittir. Aynı taban #300'ün çekincesini (ccache'le ısınmış temel
koşu sınırı kısaltır) de kaldırır.

### 4. Parça sayısı (f, 2'den sonra)

t son kapı koşularının en yavaş parçasından yeniden ölçülür (1 ve 2
sonrası). N, ⌈M / N⌉ · t + kurulum ≤ hedef olan en küçük N'dir, bir
tavanla (koşucu havuzu 20 eşzamanlı iş, CI'ın her push'ta 9 işiyle
paylaşılır). Hedef ve tavan bakımcının kararıdır. `--shard k/N` aynı
listeyi böler; kapsam işi parça listelerinin toplamını denetler.

### 5. Test aşamasında gereksiz yeniden derleme yok (h, uygulandı)

`volt-sv-emit/build.rs` yalnız var olan git dosyalarını izler
(`git_rerun_paths`: HEAD, HEAD'in dalı, `packed-refs`). Dal sonradan
`packed-refs`'e taşınırsa izlenen gevşek dosya silinir; cargo bunu
değişiklik sayar ve build script yeniden koşar, sürüm metni yeni commit'i
alır (ADR-0104).

## Doğrulama yöntemi

Her ayar sabit bir referansla karşılaştırılır; mutant listesi ve her
mutantın sonucu birebir aynı olmalıdır. Değişen her mutant raporlanır ve
o ayar geri alınır.

- **Referans:** aynı ağaç ve aynı fark tabanı. #343'ün birleştirme ağacı
  main'deki `1268ffa2`'nin ağacıdır (`2f3d4c9e` tabanı `62373205`'i
  zaten içeriyordu); referans kapı koşusu 44615986697. Kanıt dalı bu
  commit ve değişikliktir; `gate_base=62373205`. #363'ün ilk koşusu
  (44597750883) ikinci referanstır: missed mutantı olan tek koşu (missed
  ile timeout sınırı).
- **Önce A/A, sonra A/B, aynı gün:** rustc `@stable` ile gelir, koşucu
  imajı parçadan parçaya değişir. Aynı daldan ayar kapalı ve açık iki koşu
  (`nextest_order=default` ve `priority`), parça sayısı sabit
  (`-f shards=<n>`). Kapalı koşu ile kaydedilmiş referans arasındaki fark
  ortamın ya da kararsız bir testin işaretidir; o mutant raporlanır.
- **Karşılaştırma:** `scripts/ci/mutants_compare.py A B` iki koşunun
  `mutants-list`'ini ve `mutants-out-*`'larını okur; listeler farklıysa,
  bir mutantın sonucu yoksa ya da farklıysa ("missed -> timeout: …")
  1 ile çıkar. Her iki koşunun parça ve mutant başına derleme ve test
  sürelerini, temel koşu sürelerini ve zaman aşımlarını yazar.
- **Sıra:** her ayar tek başına, kendi A/A çiftiyle: h ve c′ (PR 1),
  a + b (PR 2), e (#364'ün planıyla, PR 3), f (PR 4). Derleme profili
  ve bağlayıcı (#366 "d") ayrı bir ölçümle ve bu ADR'ye ek olarak gelir.

## Kanıt

### PR 1 (h, c′): referans karşılaştırması

Kanıt dalı `evidence/366-order-343`: `1268ffa2` (+ bu PR'ın değişikliği),
`gate_base=62373205`, 4 parça. Sonuçlar koşular bitince bu bölüme yazılır.

## Reddedilenler

- **Temel koşu yerine aynı SHA'daki CI test işi.** Kapının temel
  koşusunun denetlediğini denetlemez (nextest, etkisiz yollar, bekçi,
  ccache); ek koşul olarak kalabilir, yerine geçmez.
- **Test düzeyinde yavaş liste.** Benzetimde ikili düzeyindeki listeden
  kötü (#343: ~79 ve ~58 dk) ve her yeni yavaş test için bakım ister.
- **Yavaş testleri en öne almak (en uzun iş önce).** Tam takım kısalır
  ama 4 yuvanın dördü de yavaş testlerle dolar, yakalayan hızlı testler
  beklemeye devam eder.
- **Mutant örneklemek, mutant başına test süzmek, araçlı testleri
  kapıdan çıkarmak.** Kalite ölçütünü bozar.

## Sınırlar

- Sıra sonucu değiştirmemelidir; değiştirirse sıraya ya da yanındaki
  teste bağlı bir test vardır. Bu bir hatadır (AGENTS.md: kararsız test),
  karşılaştırma onu adıyla gösterir.
- Missed mutantlar bütün takımı koşmaya devam eder; sıra yavaş ikilileri
  sona koyduğundan takımın süresi değişebilir. Zaman aşımı aynı sırayla
  ölçülen temel koşudan türer (bugün) ya da §3'ün tabanından (2'den
  sonra).
- Yavaş listesi ölçümdür: yeni bir yavaş ikili listeye girmezse sonuç
  değişmez, yalnız sıra kötüleşir.
