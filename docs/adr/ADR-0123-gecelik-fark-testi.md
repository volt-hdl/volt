# ADR-0123: Fark Testinin Büyük Koşusu Her Gece; Gecelik İşler Akşam Saatinde, Tohum Gecenin Tarihinden (#255)

> Statü: Önerildi
> Önceki karar: ADR-0117 — §5'in gecelik tohumu: koşunun UTC tarihi yerine, koşunun başladığı andan 12 saat önceki UTC tarihi
> İlgili: ADR-0108 (§5'i genişletir: büyük koşu yalnız elle değil, her gece de koşar)
> Issue: #255
> Tarih: 2026-11-18
> Etkilenen: `.github/workflows/difftest.yml`, `.github/workflows/metamorphic-nightly.yml`, `.github/workflows/fuzz-nightly.yml`, `.github/workflows/known-failures.yml`, `.github/actions/ccache/action.yml` (`persist` girdisi), `scripts/ci/nightly_issue.py` (`--kind`), `scripts/ci/nightly_seed.py` (YENİ), `.github/workflows/README.md`, `CHANGELOG.md`

## Sorun

ADR-0108 §5 fark testinin büyük koşusunu (`difftest.yml`, dört parça,
parça başına 12 500 kabul edilmiş ifade) yalnız elle başlatılan bir iş
olarak tanımlar. Her PR araçlı işte tek bir sabit tohumla 2000 ifade
koşar; büyük koşu ise birinin hatırlamasına bağlıdır. #255'in katman
ilkesi ("PR'da hızlı, her gece geniş, her hafta tam koşu; her katmanın
kırmızısı kırmızıdır, uyarı değil") bu yüzden fark testinde gece
katmanından yoksundu. Büyük koşu #303'ün düzeltmesinden sonra ilk kez
yeşil (koşu 44112926993): geceye alınabilir.

Gece işlerinin saati de bir sorundur. GitHub zamanlanmış koşuları geç
başlatır: Kasım 2026'da 6 sa 35 dk ile 8 sa 38 dk arası (fuzz 03:00 →
09:35-11:17; install 05:17 → 13:30-13:55). 02:41 ve 03:00'e kurulan gece
işleri Türkiye saatiyle öğlen, PR'ların CI'ıyla aynı saatlerde koşuyordu.
Gecikme tarihi de değiştirebilir: ADR-0117 §5'in tohumu "koşunun UTC
tarihi"dir; gecikme bir koşuyu gece yarısının öte yanına iterse o gece
ertesi günün tohumunu koşar, ertesi gece aynı tohumu yeniden koşabilir.

## Karar

1. **Tetik.** Fark testinin büyük koşusu her gece main'de ve elle
   (`workflow_dispatch`) koşar. Elle koşunun girdileri kalır (`seed`,
   `count`, `div_operands`), bir girdi eklenir (`issue`).
2. **Saat.** Üç gece işi UTC 17:00 civarına alınır; aralarındaki sıra ve
   boşluklar korunur, :00 ve :30 kullanılmaz:

   | İş akışı | cron (UTC) | Gecikmesiz, TR (UTC+3) | 8 saat gecikmeyle, TR |
   |---|---|---|---|
   | `difftest.yml` | 17:03 | 20:03 | 04:03 |
   | `metamorphic-nightly.yml` | 18:21 | 21:21 | 05:21 |
   | `fuzz-nightly.yml` | 18:40 | 21:40 | 05:40 |

   Gecikme yoksa Türkiye akşamı, 8 saat gecikirse Türkiye gecesi
   koşarlar; iki durumda da PR'ların gündüz CI'ından ayrıdırlar. Fark
   testi metamorfik geceden 78, fuzz gecesinden 97 dakika önce başlar
   (eskiden 01:23, 02:41, 03:00).
3. **Gecenin tarihi.** Gecelik bir koşunun tohumu, koşunun başladığı
   andan 12 saat önceki UTC tarihinden türer (ADR-0117 §5'in "koşunun UTC
   tarihi" tanımının yerine). Metamorfik gece bu tarihi olduğu gibi
   kullanır (`2026-11-18`); fark testinin k. parçası `YYYYMMDD00 + k`
   koşar (2026111800 ... 2026111803). Tarihi `scripts/ci/nightly_seed.py`
   hesaplar; iki iş akışı da onu çağırır, self-test'i her PR'da koşar.
   Elle koşu aynı formülü kullanır: bir tohum verilmezse son başlayan
   gecenin tohumunu koşar.
4. **Yeniden üretme.** Tohum günlüğe bir not olarak, iş özetine yeniden
   üretme komutuyla yazılır. Fark testinde tohumu ayrı bir iş koşunun
   başında bir kez alır, parçalar ondan okur;
   `VOLT_DIFFTEST_SEED=<YYYYMMDD00 + k> VOLT_DIFFTEST_COUNT=12500` k.
   parçayı sabit hakemle (`scripts/ci/verilator.env`) yeniden üretir.
   Metamorfikte `VOLT_METAMORPHIC_SEED=<YYYY-MM-DD>` (değişmedi).
5. **Kırmızı gece.** Zamanlanmış koşu kırmızıysa (zamanlama yalnız
   main'de koşar) bir iş "Nightly differential test is red (ADR-0108)"
   başlıklı açık issue'yu bulur ve tohumla, parça komutlarıyla, dört
   parçanın raporuyla yorum ekler; açık issue yoksa açar. Yeşil gece bir
   şey yazmaz. Elle koşu issue'ya yalnız `issue=on` ile yazar. Betik
   metamorfik gecenininkidir (`scripts/ci/nightly_issue.py`), türüyle
   ayrılır (`--kind difftest`); self-test'i sahte bir `gh` ile her PR'da
   koşar.
6. **ccache kaydı kapalı.** Fark testinin parçaları Verilator
   derlemelerini ccache'ten geçirir, ama önbellek yalnız iş içinde
   yaşar: ne geri yüklenir ne kaydedilir (`.github/actions/ccache`,
   `persist: off`).
7. **Bütçe** 4 parça × 12 500 kabul edilmiş ifadedir, ADR-0108'in büyük
   koşusuyla aynı.
8. **Bir hafta sonra (2026-11-25)** yeni saatlerle gerçekleşen
   gecikmeler ölçülür (her gece işinin cron saati ile başlama anı); saat
   ve bütçe bu ölçümle yeniden değerlendirilir.

## Gerekçe

- **12 saat geri.** 17:03'e kurulan bir gece ölçülen en büyük gecikmeyle
  (8,6 saat) ertesi gün 01:39'da başlar; 12 saat geri gidince tarih
  kurulduğu gündür. Kural, gecikme yaklaşık -5 ile +19 saat arasında
  kaldıkça her geceye tek bir tarih verir: bir gece gecikip ertesi gece
  gecikmese de iki gece aynı tohumu koşmaz. Self-test bunu gecikmesiz,
  8 saat gecikmeli ve gece yarısını geçen durumlarda gösterir.
- **Tohum biçimi.** Fark testinin tohumu bir sayıdır (`u64`); tarih sayı
  olarak yazılır. `YYYYMMDD + k` olsaydı ertesi gecenin 0-2. parçaları
  bugünün 1-3. parçalarının tohumlarını yeniden koşardı. Sondaki iki hane
  bir gecede 100 parçaya yer bırakır.
- **Tohumu tek bir işin alması.** Parçalar ayrı koşucularda, ayrı
  zamanlarda başlar; her biri tarihi kendisi hesaplasaydı bir sınırı
  geçen parça başka gecenin tohumunu koşabilirdi.
- **Boşluklar.** Aynı gün kurulan işler yaklaşık aynı gecikmeyi alır:
  2026-11-17'de 02:41, 03:00 ve 03:17 → 11:04, 11:17 ve 11:51. Büyük
  koşu 7 dakika sürer; 78 dakikalık boşluk bu oynamayı karşılar.
- **Elle koşuda issue yalnız `issue=on` ile.** Büyük koşu dallarda bir
  düzeltmeyi denemek için de elle koşar (44112926993 #303'ün dalında);
  böyle bir koşunun kırmızısı onu başlatanın önündedir ve gece issue'sunu
  doldurmamalıdır.
- **ccache.** Rastgele tasarımlar koşudan koşuya tekrar etmez; isabetler
  Verilator'ın çalışma zamanı kitaplığının aynı iş içinde her toplu
  koşuda yeniden derlenmesidir. 44112926993'te soğuk başlayan parça 159,
  önbelleği geri yüklenen parçalar 162 isabet aldı; her parça 948-1114
  MiB kaydetti. Her gece ~4 GB olurdu; deponun önbellek kotası 10 GB'tır
  (o gün 10,43 GB dolu) ve aşılınca main'in Rust ve araç önbellekleri
  silinir. Kayıt kapalıyken test süresi değişmedi (44237991576: test
  258-351 s, isabet 159-162).
- **Bütçe.** Bir gece 7 dk 10 s sürer (dört parça paralel, 325-423 s),
  ≈ 24 iş-dakikası, en çok dört eşzamanlı iş (koşu 44237991576).

## Kanıt

- Elle koşu 44237991576: dört parça `diffs=0`, her parçada araç defteri
  `SKIP: 0`; ccache "nothing restored, nothing saved".
- Kırmızı gece yolu, 44246425921: bir kanıt dalında #303 düzeltmesinin
  yalnız üretici kısmı geri alındı, koşu `issue=on` ile tetiklendi. Dört
  parça kırmızı (`diffs=6-8`, `regressions failed=6`); ilk deneme #324'ü
  açtı, yalnız issue işinin yeniden koşusu ona yorum ekledi. #324 kanıt
  notuyla kapatıldı, dal silindi.
- Yeniden üretme: kanıt koşusu aynı tohumları koştu; her parçanın
  üretilen, kabul edilen ve karşılaştırılan değer sayıları yeşil
  koşununkilerle birebir aynı (parça 0: 13500 / 12693 / 1361074).
- Bu iki koşu tohumu henüz koşunun UTC tarihinden alıyordu; 12 saat
  geri kuralının koşusu PR #323'tedir.

## Sınırlar

- Zamanlanmış tetik bir PR'da sınanamaz: GitHub zamanlamayı yalnız
  main'deki iş akışıyla koşar. İlk gerçek gece birleştirmeden sonradır.
- Gecikmenin yeni saatte de aynı kalacağı varsayımdır; 8. madde onu
  ölçer.
- `known-failures.yml`'nin gece denetimi (03:17) ve `install.yml`'nin
  haftalık koşusu bu kararın dışındadır.
- #255'in kapsam matrisi, ikinci simülatör ve küçültülen bulguların
  derleme kendiliğinden eklenmesi bu kararın dışındadır.
