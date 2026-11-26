# ADR-0134: Mutasyon Kapısının Özeti Başarısızlıkta Kapalı Kalır — Önceki İşlerden Biri Planlandığı Gibi Bitmezse Özet Kırmızıdır

> Statü: Uygulandı
> İlgili: ADR-0109 ("PR kapısı" eki §1 "Karar": özetin düştüğü durumlara bir tane eklenir), ADR-0128 (§2: temel koşu geçmezse özet mutant kararı vermez; bu ADR aynı kuralı özetten önceki bütün işlere genişletir), ADR-0125 (kırmızı ileti ne olduğunu, neden kırmızı olduğunu ve ne yapılabileceğini söyler), ADR-0143 (aynı ilke fuzz işlerinde: fuzz etmeyen koşu kırmızıdır)
> Issue: #398 (zorunlu denetim kararı #366'da, belge #397)
> Tarih: 2026-11-23
> Etkilenen: `.github/workflows/mutants.yml`, `scripts/ci/mutants_summary.py`, `.github/workflows/README.md`, `CHANGELOG.md`

## Sorun

2026-11-23'ten beri "Mutation test summary" ve "Mutation exclusions match
their mutants" main için zorunlu denetimdir ("main protection" ruleset'i,
#366, #397). Özet işinin koşulu şuydu:

```yaml
needs: [tools, baseline, mutants]
if: ${{ !cancelled() && needs.tools.result == 'success' }}
```

"Tools and the mutants in scope" düşünce özet koşmaz ve check run'ı
`skipped` biter. GitHub koşulu yüzünden atlanan bir işi geçmiş sayar,
zorunlu denetimde de. Tools işi ruleset'te değildir. Böylece mutant
denemeden düşen bir kapı zorunlu denetimi karşılıyordu. Kanıt: koşu
43730310580 (2026-11-15, `feat/mutants-pr-gate`): tools `failure`, özet
`skipped`, parçalar `skipped`.

Özet, tools işi geçtiğinde de parça işinin kendi sonucunu okumuyordu:
bir parçayı her sonucu varsa tamamlanmış sayıyordu. Bir parça işi missed
mutant yüzünden düşmez (cargo-mutants'ın 2 ve 3 çıkışı normal biter);
liste uyuşmazlığında, 300 dakikada durunca ya da araç defterinde
SKIP > 0 olunca düşer. Son durumda sonuçlar tam olabilir ve özet yeşil
verebilirdi.

## Karar

1. **Özet, koşu iptal edilmedikçe koşar:**
   `if: ${{ !cancelled() && github.event_name != 'push' }}`. Önceki
   işlerden biri düşse, iptal edilse ya da atlansa da koşar. main'e
   push'ta yalnız `cache-warm` koşar; tools işi orada bilerek atlanır,
   özet de.
2. **Özet önce önceki işlerin sonucunu denetler**
   (`mutants_summary.py --needs`, iş akışının `toJSON(needs)`'i). Planlandığı
   gibi biten iki durum vardır:
   - (a) tools işi geçti ve N > 0 parça planladı; temel koşu ve her parça
     geçti. Ardından özetin bugünkü kararı verilir (kapsanmayan missed
     mutant, tamamlanmayan parça, temel koşu, zaman aşımı: ADR-0109,
     ADR-0128).
   - (b) tools işi geçti ve parça planlamadı: test edilecek mutant yok
     (nedenini yazar), anahtarı aynı önceki koşu yeniden kullanılıyor
     (#318; karar o koşunun sonuçlarından) ya da yeniden denenecek mutant
     yok (#364; taşınan sonuçlar). Temel koşu ve parçalar `if`'leriyle
     atlanmıştır.

   Başka her bileşim kırmızıdır:

   | tools | `shards` çıktısı | temel koşu | parçalar | Denetim |
   |---|---|---|---|---|
   | success | `0` | skipped | skipped | geçer, karar (b) |
   | success | N > 0 | success | success | geçer, karar (a) |
   | failure, cancelled, skipped, yok | — | — | — | kırmızı |
   | success | boş ya da sayı değil | — | — | kırmızı |
   | success | N > 0 | failure, cancelled ya da skipped | — | kırmızı |
   | success | N > 0 | — | failure, cancelled ya da skipped | kırmızı |
   | success | `0` | skipped dışında | — | kırmızı |
   | success | `0` | — | skipped dışında | kırmızı |

3. **Kırmızı ileti ADR-0125'in üç parçasını taşır** (iş özeti ve
   `::error` satırı): ne oldu (hangi iş, hangi sonuç), neden kırmızı (çalışmayan
   ya da bitmeyen bir kapı geçmiş sayılamaz), ne yapılabilir (o işin
   günlüğündeki ilk hata; neden PR'ın dışındaysa `gh run rerun <koşu>
   --failed` ya da "Re-run failed jobs", PR'ın içindeyse düzeltip push).
4. **Tools işi geçtiyse birleştirme adımı yine koşar**: düşen parça ve
   temel koşu, test edilmeyen mutantlarıyla bugünkü gibi adlandırılır.
5. **İş adları değişmez**: "Mutation test summary" ve "Mutation exclusions
   match their mutants" ruleset'te bu adlarla durur.

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; karar dilin bir yazımını istemez, depo
   aracının kararını belirler.
2. **Çıkarım (İ2):** uygulanmaz. Denetim bir işin sonucunu tahmin etmez;
   GitHub'ın bildirdiği sonucu okur, yoksa `missing` sayar ve düşer.
3. **Anlam değişikliği (İ3):** dilin anlamı, üretilen SV ve tanılar aynı.
   Kapının denediği mutant listesi ve her mutantın sonucu da aynıdır:
   tools, temel koşu ve parça işleri değişmedi. Değişen, önceki işlerden
   biri planlandığı gibi bitmediğinde özetin kırmızı olmasıdır: tools
   düşünce atlanmak yerine, bir parça işi sonuçları tam olduğu hâlde
   düşünce yeşil vermek yerine.
4. **Niyet tahmini (İ4):** uygulanmaz. Özet, düşen bir işin nedenini
   tahmin etmez; işi ve sonucunu adlandırır, günlüğe yönlendirir.
5. **Metamorfik yeniden yazım:** uygulanmaz.

## Doğrulama

- **Öz denetim** (`mutants_summary.py --self-test`, özet işinin "Script
  self-test" adımı): tablonun her satırı için bir durum, ayrıca tools
  sonucunun hiç olmadığı ve `shards` çıktısının sayı olmadığı durumlar
  (16 durum), iki sorunun birlikte adlandırılması ve koşu numarası
  bilinmeyen ileti. Her kırmızı durumda iletinin üç parçası aranır.
  43730310580'un bileşimi (tools `failure`, ötekiler `skipped`) bunlardan
  biridir.
- **Mutant listesi ve sonuçları aynı.** `git diff` yalnız `summary` işini
  ve başlık yorumunu değiştirir; tools, temel koşu ve parça işleri bayt
  bayt aynıdır. Kapının denediği liste ve sonuçlar bu işlerden geldiği
  için ADR-0128'in referans koşusu gerekmez (ADR-0133'teki gerekçe).
- **Gerçek CI.** Bu kararı getiren PR'da Rust değişmez: kendi kapı koşusu
  durum (b)'yi gerçek `needs` ile sınar. Kırmızı durumlar gerçek CI'da
  bilerek üretilmedi; öz denetim onları `toJSON(needs)` biçiminde sınar.
- **İptal.** İptal edilen bir koşuda özet `skipped` değil `cancelled`
  biter (koşular 44913806181, 44918028714); `cancelled` zorunlu denetimi
  karşılamaz. Yeni push eski koşuyu iptal eder; yeni SHA'nın kendi özeti
  vardır.

## Sınırlar

- Yeniden kullanım anahtarı (#318) iş akışını içerir: bu değişiklikten
  sonra açık her PR'ın ilk kapı koşusu yeniden kullanmaz, tam koşar.
- "Mutation exclusions match their mutants" işinin `needs`'i ve `if`'i
  yoktur; her tetikte koşar, bu kural ona gerekmez.
- Tools işi ruleset'e eklenmedi: özetin kendisi düşer.

## Reddedilenler

- **Tools işini ruleset'e eklemek** (#398'deki seçenek 1): temel koşu ya
  da parça `skipped` biterse açık kalırdı ve kural depoda değil ayarda
  dururdu.
- **Olduğu gibi bırakmak** (seçenek 3): zorunlu denetim kapının
  çalıştığını kanıtlamazdı.
- **`always()`**: iptal edilen koşuda da özet koşardı; `!cancelled()` ile
  özet `cancelled` biter ve bu da zorunlu denetimi karşılamaz.
