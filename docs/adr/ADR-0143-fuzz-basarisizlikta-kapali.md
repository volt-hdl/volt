# ADR-0143: Fuzz Başarısızlıkta Kapalı Kalır — Gerçek Fuzz Süresi Ölçülür, Gece Derlemi Küçültülür, PR Tohumlardan Fuzz Eder

> Statü: Önerildi
> İlgili: ADR-0080 (§6: `-max_len=4096` aynı kalır; bu ADR'nin bayrakları ona eklenir), ADR-0123 (gecelik fuzz işinin saati ve yeri aynı; işin içi değişir), ADR-0134 (aynı ilke: bir kapı planlandığı gibi koşmadıysa yeşil vermez; bu ADR onu fuzz işlerine uygular)
> Issue: #305 (#410'un ilkesi)
> Tarih: 2026-11-26
> Etkilenen: `.github/workflows/fuzz-nightly.yml`, `.github/workflows/ci.yml` (fuzz işi), `scripts/ci/fuzz_health.py` (YENİ), `.github/workflows/README.md`, `GOVERNANCE.md`, `CHANGELOG.md`

## Sorun

libFuzzer önce derlemdeki her girdiyi bir kez koşar (yeniden oynatma);
`#N INITED` satırı onu bitirir. `-max_total_time` sürecin başından sayar,
yeniden oynatma dahil. Gece işi derlemi her gece büyütüp budamadan
kaydediyordu (115 692 dosya, 191 MB; libFuzzer'ın işe yarar bulduğu
14 158). Ölçüm, işlerin kendi loglarından:

| Koşu | Derlem (tohumlar dahil) | Yeniden oynatma | Gerçek fuzz (INITED → Done) | exec/s (INITED'den sonra) | Slowest unit |
|---|---|---:|---:|---:|---|
| gece 45739110046 (bütçe 1800 s) | 116 191 dosya, 192,3 MB | 1530 s | 370 s (%21) | 32 | 3 (25, 31, 35 s) |
| PR 45873059789, "Fuzz (60 s)" (bütçe 60 s) | 116 290 dosya, 192,5 MB | 924 s | 0 s (%0) | — | 0 |

İki iş de yeşildi. PR işi bütçesini yeniden oynatmada bitirip hiç fuzz
etmedi; gece işi bütçenin beşte birini fuzz etti. Varsayılan `-timeout`
1200 s olduğundan #446'nın 25-35 s'lik girdileri koşuyu kırmızı yapmadı;
`fuzz/artifacts` yalnız kırmızı koşuda yüklendiği için o girdiler
kayboldu ve önbellekten geri alınmak zorunda kalındı.

## Karar

1. **Başarısızlıkta kapalı.** Her iki fuzz işi libFuzzer'ın çıktısını
   `scripts/ci/fuzz_health.py stamp` ile zaman damgalı bir loga yazar.
   "Fuzz health (fail closed)" adımı, iptal dışında her zaman koşar ve şu
   durumlarda işi kırmızı yapar:
   - `INFO: Seed:` satırı yok (libFuzzer başlamadı);
   - `INITED` satırı yok (yeniden oynatma bitmedi);
   - `Done N runs in T second(s)` satırı yok (çökme, zaman aşımı ya da
     öldürülen adım);
   - gerçek fuzz süresi (`INITED` → `Done`) bütçenin yarısından az.

   Önceki adım düşse ya da atlansa da adım hükmünü verir: log yoksa
   kırmızıdır. Eşik yarıdır: yukarıdaki iki koşu (%21, %0) altında kalır,
   küçültülmüş derlemle gece ve tohumlarla PR üstünde kalmalıdır (Doğrulama).
2. **Görünürlük.** Adım her koşunun özetine yazar: derlem boyutu, yeniden
   oynatma süresi, gerçek fuzz süresi, `INITED`'den sonraki exec/s (çalışma
   sayacının `INITED`'den `Done`'a artışı bölü süre), `Slowest unit`
   satırlarının sayısı; gecede birleştirme ve kapsam (cov/ft).
3. **Bayraklar**, iki işte aynı: `-max_len=4096` (ADR-0080 §6),
   `-timeout=10`, `-report_slow_units=2`. #449'dan sonra 4 KB'lık en yavaş
   girdi release'te 0,21 s, fuzz derlemesinde ~1,6 s: 10 s'lik sınır ona
   altı kat pay bırakır ve #446'nın 25-35 s'lik girdilerini zaman aşımı
   olarak yakalar. `Slowest unit` satırı tam saniye sayar; 2 s bilinen en
   yavaştan yavaş her girdiyi `slow-unit-*` dosyası yapar.
4. **Gece derlemi küçültülür.** Gece işi en yeni
   `fuzz-corpus-parse_never_panics-*` girdisini geri yükler, 4 KB'tan uzun
   girdileri 4 KB'lık öneklerine keser (libFuzzer `-max_len=4096` ile
   yalnız o öneki okuyordu; derlemde 4 KB'tan uzun dosya kalmaz),
   derlemi `-merge=1` ile boş klasöre birleştirir, birleşmiş derlemin
   üstüne fuzz eder ve onu kaydeder. `tests/ui/` ve
   `tests/fuzz_regressions/` salt okunur tohum olarak kalır. Önbellek yolu
   ve anahtar öneki aynıdır: zincir kırılmaz, birleşmiş derlem eskisinin
   yerine geçer.
5. **Kayıt koşulu** (`corpus_ok`). Birleşmiş derlem şu durumda kaydedilir:
   birleştirme tek denemede bitti (`MERGE-OUTER: N new files with F new
   features added; C new coverage edges`), birleşmiş derlemin yeniden
   oynatması (`INITED` satırı) birleştirmenin aynı ikiliyle geri yüklenen
   derlemde ölçtüğü cov ve ft'ye %1 payla ulaştı, kaydedilecek dosya
   sayısı birleştirmenin yazdığından az değil ve 4 KB'tan uzun dosya yok.
   Pay, iki ayrı sürecin (birleştirme ve fuzz) `HashMap` tohumları farklı
   olduğu içindir; kaba bir kaybı (boş ya da yanlış klasör, düşen
   birleştirme) yakalar. Fuzz bir çökme bulsa da derlem bu koşulla
   kaydedilir; iş yine kırmızıdır. Koşul sağlanmazsa kayıt olmaz ve iş
   kırmızıdır: bir sonraki gece eski derlemi yeniden alır.
6. **PR işi tohumlardan fuzz eder.** "Fuzz (120 s, seeds)" gece derlemini
   okumaz; tohum `tests/ui/` ve `tests/fuzz_regressions/`'tır (526 dosya;
   release'te hepsi 1,95 s'de ayrışır, en yavaşı #446 regresyonları
   ~0,09 s). Bütçe 60 s'den 120 s'ye çıkar: fuzz derlemi release'ten ~8 kat
   yavaş olduğundan yeniden oynatma ~15 s beklenir, bütçenin çoğu fuzz'a
   kalır. Küçültülmüş gece derlemi (~14 000 dosya, ~16 MB) seçilmedi:
   yeniden oynatması dakikalar sürer ve önbellekteki derlemin o günkü
   hâline bağlıdır. Derlemin derinliği gece işindedir; PR işi değişikliğin
   ayrıştırıcıyı tohumlar ve mutasyonları üzerinde paniğe, takılmaya ya da
   yavaşlamaya götürmediğini denetler. Yeni girdiler checkout dışında
   geçici bir klasöre yazılır.
7. **Artifact her koşuda.** `fuzz/artifacts/parse_never_panics/`
   (slow-unit, crash, timeout, oom dosyaları) koşu yeşil olsa da yüklenir
   (`if: !cancelled()`, `if-no-files-found: ignore`; gece 30 gün, PR 14 gün).
8. **Dal koşuları main'in zincirine yazmaz.** Bir dalda elle koşulan gece
   işinin kaydettiği önbellek GitHub'ın kuralıyla yalnız o dala görünür.

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; karar dilin bir yazımını istemez, depo
   aracının kararını belirler.
2. **Çıkarım (İ2):** uygulanmaz. Denetim fuzz'un koştuğunu tahmin etmez;
   libFuzzer'ın kendi satırlarını okur, eksik satırı başarısızlık sayar.
3. **Anlam değişikliği (İ3):** dilin anlamı, üretilen SV ve tanılar aynı.
   Fuzz hedefi (`parse_never_panics`) aynı. Değişen: derlemin içeriği
   (kapsamı korunarak küçülür), PR işinin tohumu ve bütçesi, zaman aşımı,
   ve fuzz etmeyen koşunun kırmızı olması.
4. **Niyet tahmini (İ4):** uygulanmaz. Kırmızı ileti eksik olanı ve
   ölçülen süreleri adlandırır, çökme ya da zaman aşımında artifact'ı
   yeniden üretme komutunu verir.
5. **Metamorfik yeniden yazım:** uygulanmaz.

## Doğrulama

- **Öz denetim** (`fuzz_health.py --self-test`, iki işte de koşar):
  45739110046'nın gecesi ve 45873059789'un PR işi (GitHub'ın log
  biçiminde) kırmızı; sağlıklı bir koşu yeşil; bütçenin tam yarısı geçer,
  bir saniye eksiği düşer; `INITED`'den önce ve `Done`'dan önce kesilmiş
  log, zaman aşımı, boş log ve zaman damgasız satırlar kırmızı; son
  satırı olmayan, iki denemede biten ve kapsam kaybeden birleştirme
  kaydedilmez; `stamp` baytları değiştirmeden yansıtır.
- **Gerçek loglar.** `check` aynı zamanda `gh run view --log` dosyalarını
  okur: yukarıdaki "önce" tablosu bu betiğin iki koşunun loglarından
  çıkardığı değerlerdir.
- **Gerçek CI.** Bu kararı getiren PR'ın kendi "Fuzz (120 s, seeds)" işi
  yeni PR işini sınar; gece işi dalında `workflow_dispatch` ile iki kez
  koşar: ilki eski büyük derlemi birleştirir (birleştirme süresi ve
  kapsamın korunduğu), ikincisi dalın kaydettiği küçük derlemle sürekli
  durumu ölçer. "Sonra" değerleri ve önbellek kayıtlarının `ref`'i PR
  açıklamasındadır.

## Sınırlar

- Eski derlemin ilk birleştirmesi, eski yeniden oynatma kadar sürer
  (#449'dan sonra ~15 dk); sonraki geceler küçük derlemi birleştirir.
  Adımın sınırı 35 dk, işin 90 dk.
- Kapsam karşılaştırması aynı gecenin ikilisiyle yapılır; gecelerin
  arasında kod değiştiği için önceki gecenin cov/ft'si eşik değildir.
- Bir zaman aşımı ya da çökme, birleştirmede eski derlemden gelirse
  birleştirme ikinci denemeye geçer; iş kırmızıdır ve derlem kaydedilmez:
  girdi artifact'tadır, düzeltilene kadar her gece kırmızı kalır.
- `-report_slow_units` tam saniye sayar; 2 s'nin altındaki yavaşlama
  görünmez, yalnız exec/s'ye yansır.

## Reddedilenler

- **Yeniden oynatmaya adım zaman sınırı** (#305 seçenek 3): fuzz yine
  başlamayabilirdi; sınır yalnız işi kısaltırdı.
- **PR'da küçültülmüş gece derlemi:** yeniden oynatması bütçenin yarısını
  aşabilir ve önbelleğin o günkü hâline bağlıdır (karar 6).
- **4 KB'tan uzun girdileri silmek:** libFuzzer'ın gördüğü önekin kapsamı
  kaybolabilirdi; kesmek aynı girdiyi saklar.
- **Önbellek anahtarını değiştirmek** (yeni önek): zincir kırılırdı ve
  ilk gece boş derlemle başlardı.
