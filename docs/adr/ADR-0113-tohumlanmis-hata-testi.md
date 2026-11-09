# ADR-0113: Tohumlanmış Hata Testi — Güvenlik Denetimleri Güvensiz Tasarımı Kaçırıyor mu?

> Statü: Uygulandı
> İlgili: ADR-0108 (fark testi derleyicinin ürettiği SV'yi ölçer, bu test derleyicinin reddetmesi gereken tasarımı; beklenen değer ikisinde de derleyiciden bağımsızdır), ADR-0109 (mutasyon testi derleyiciyi değiştirip testleri ölçer, bu test tasarımı değiştirip denetimleri ölçer), ADR-0114 (#189 ve #193 satırları kapandı; bilinen açıklar listesi boş)
> Tarih: 2026-11-09
> Etkilenen: `crates/volt-driver/tests/seeded_bug_tests.rs` (YENİ),
> `crates/volt-driver/tests/seeded_bugs/` (YENİ: `mod.rs`, `scan.rs`,
> `kinds.rs`, `specials.rs`), `AGENTS.md`, `CHANGELOG.md`

## Sorun

Şimdiye kadar bulunan sessiz hataların hepsi "Volt yanlış SV üretiyor"
türündendi (#115, #117, #150, …): fark testi (ADR-0108) ve emitter
testleri bu türü arar. Öteki tür hiç sistemli aranmadı: bir güvenlik
denetiminin (CDC, RDC, reset, güç, güven ve "bu tasarım güvensiz" diyen
öteki denetimler) güvensiz bir tasarımı kabul etmesi. `tests/ui/fail/`
her kodu çoğunlukla tek bir yazılışla sınar; aynı tehlike başka bir
yazılışla (iç içe modül, `fn`, `comb` bloğu, extern, birkaç adım
uzaklıkta) geldiğinde denetimin onu görüp görmediği bilinmiyordu.

## Envanter (ADIM 0)

Güvenlik özelliği koruyan kodlar, `main` = c33b53d7. Sütunlar: o kodla
başlayan `tests/ui/fail/` dosyaları; onu üreten `tests/fixtures/parity/`
dosyaları; kodu anan crate test dosyaları (`crates/*/tests/`, birim test
dosyaları); `tests/quickfix/` ve `tests/suggestions/` fikstürleri;
"yazılış" = ui/fail + parity (her dosya bir sözdizimsel biçim); "derlem"
= bu ADR'nin bu kodu bekleyen mutasyon türü sayısı.

| Kod | Korur | Şartname | ui/fail | parity | crate testi | fikstür | yazılış | derlem |
|---|---|---|---|---|---|---|---|---|
| E3001 | saat alanı geçişi (CDC) | domain-inference.md K5–K9 | 10 | 3 | 16 | 5 | 13 | 16 |
| E3002 | tanımsız alan | K1, K8a | 4 | 0 | 7 | 1 | 4 | — (ad hatası) |
| E3003 | paylaşılan async reset, yakınsama, port/alan uyumsuzluğu | ADR-0065 R5, R6, §1 | 4 | 0 | 3 | 1 | 4 | 4 |
| E3004 | reset sırası | ADR-0065 (rezerve) | 0 | 0 | 0 | 0 | **0** | — (mekanizma yok) |
| E3005 | koşullu reset | ADR-0065 (rezerve) | 0 | 0 | 0 | 0 | **0** | — (gramerde yok) |
| E3006–E3008 | güç alanı: izolasyon, sıra, retention | domain-inference.md §5 [V1] | 0 | 0 | 1 (kod listesi) | 0 | **0** | — (anahtarlar E0003) |
| E3009 | güven seviyesi akışı | K11, ADR-0052 | 3 | 1 | 4 | 0 | 4 | 18 |
| E3010 | belirsiz alan | K3, §3 | 2 | 0 | 6 | 0 | 2 | 1 |
| E3011 | iki alandan yazılan register | K4 | 1 | 0 | 1 | 0 | **1** | 1 (E4001 ile ortak küme) |
| E3012 | `on` bloğunda yabancı alan okuması | K7, §5 | 0 | 0 | 2 | 1 | **0** | 7 |
| E3013 | bundle alanları farklı alanda | ADR-0039 | 1 | 0 | 2 | 0 | **1** | — |
| E3014 | sembolik alana iki saat | K8a | 1 | 0 | 2 | 0 | **1** | 1 |
| E3015 | `fn` gövdesinde `declassify` | K5, ADR-0081 | 1 | 1 | 2 | 0 | 2 | 1 |
| E3016 | kenarsız alanda flop | ADR-0098 Ek 2 | 6 | 0 | 2 | 0 | 6 | 1 |
| E0016 | gerekçesiz `declassify` | grammar-full.ebnf §18 | 1 | 0 | 2 | 0 | **1** | — (sözdizimi) |
| E4001 | çift sürücü | type-inference.md §11.2, ADR-0073 | 4 | 41 | 8 | 0 | 45 | 2 |
| E4003, E4004 | lineer port [V1] | type-inference.md §7 | 0 | 0 | 1–2 (kod listesi) | 0 | **0** | — (rezerve) |
| E4006 | bus'a ait MMIO alanına RTL yazması | ADR-0044 | 1 | 0 | 3 | 0 | **1** | 3 |
| E4007 | `valid` → `ready` kombinasyonel | ADR-0050 §4 | 1 | 0 | 2 | 0 | **1** | 5 |
| E4011 | örnek portu bağlantısı | ADR-0072 | 0 | 8 | 7 | 0 | 8 | — |
| E5010 | farklı aşama değerleri birleşiyor | ADR-0037 | 2 | 0 | 3 | 0 | 2 | 3 |
| W3003 | çok bitli `sync()` | K9 | 0 | 0 | 3 | 0 | **0** | 3 |
| W3005, W3006 | PulseSync aralığı, çift portlu bellek çakışması | ADR-0027, ADR-0049 | 0 | 0 | 3, 4 | 0 | **0** | — (her kullanımda çıkan hatırlatma) |
| W3007 | çift yönlü port senkronizasyonsuz okunuyor | K2a | 1 | 2 | 3 | 1 | 3 | 2 |
| W3008 | `declassify` iz kaydı | ADR-0052 §4 | 0 | 0 | 4 | 0 | **0** | yakın ıskalarda izinli |
| W3009 | kökte async reset bırakması | ADR-0065 R1 | 1 | 0 | 2 | 1 | **1** | 1 |
| W3010 | paylaşılan sync reset | ADR-0065 R5' | 2 | 4 | 4 | 1 | 6 | 1 |
| W3011 | ham reset extern'e | ADR-0103 | 1 | 0 | 3 | 0 | **1** | 1 |

Kalın: hiç sınanmayan ya da tek yazılışla sınanan kodlar.
E3012'nin `ui/fail` dosyası yok; yalnız `volt-hir` birim testleri ve bir
quickfix fikstürü onu üretir. Güç alanı kodları (E3006–E3008) tanımlıdır
ama `always_on`, `isolation`, `retention`, `voltage` ayrılmış sözcük
olarak E0003 alır: güç alanı yazılamadığından sessizce kabul de edilemez;
mutasyon kurulamaz.

## Karar

### 1. Derlem

`examples/`, `demo/`, `tests/ui/pass/`, `tests/fixtures/` altındaki her
`.volt` tasarımı (`*_test.volt` hariç) ve kitabın her ```` ```volt ````
bloğu. Kitap bloğu kendi dizininde, sayfanın `systemverilog,file=`
bloklarıyla birlikte denetlenir; `should_fail`, `output`, `from=` (depo
dosyasının kopyası) ve test blokları dışarıda kalır ve raporda yazılır.

Temiz = taban `volt check`'te hata yok. Uyarılar (W3006, W3008, …) taban
olarak kaydedilir; mutant ancak sayısı artan kodla değerlendirilir.
Hatası olan tasarım ya bir kurala (`tests/fixtures/parity/`: başarısız
olmak için yazılmış dosyalar) ya da gerekçeli bir listeye
(`EXCLUDED_DESIGNS`) uyar; uymayan bir hata testi düşürür.

### 2. Mutasyon türleri ve beklenen kod

Bir tür tek noktalı, gerçek bir tehlike yaratan değişikliktir ve uygun
olduğu her tasarıma uygulanır (`seeded_bugs/kinds.rs`): `sync()`'i
kaldırmak, `sync()`'i öteki saate çevirmek, bir `on` bloğunu öteki saate
taşımak, bir örneğin saat bağlamasını değiştirmek, bir portun alanını
değiştirmek ya da kaldırmak, bir register'ı ikinci bir saatten yazmak,
extern'in iki sembolik alanını birleştirmek, ham reset portunu kaldırmak,
ham reset'in polaritesini ya da türünü çevirmek, alanın saat kenarını
kaldırmak, `declassify`'ı kaldırmak, çıkış portunu daha düşük güven
seviyesine almak, çift yönlü okumadan `sync()`'i kaldırmak, `valid`'i
`ready`'ye bağlamak, MMIO register'ını bus'a ait yapmak, `delay<K>`'yı ya
da yeniden zamanlama açıklamasını kaldırmak, bir çıkışa ikinci sürücü
eklemek.

Bir kalıbın yazamadığı biçimler tasarıma özgü mutasyonlardır
(`seeded_bugs/specials.rs`): `if`/`match` ifadesi, `comb` bloğu, `fn`,
tel zinciri, saatsiz ve tek saatli alt modül, extern ve yerleşik
primitif üzerinden geçiş; çok bitli `sync()`'in struct ve enum biçimi;
çocukta ikinci reset senkronizörü; ham reset'in extern'e verilmesi.

**Beklenen kod şartnameden elle belirlenir; derleyicinin çıktısı
beklenen değer olarak yazılmaz.** Her tür kendi kuralını ve şartname
dayanağını taşır (`Kind::spec`). Cevabı tasarıma bağlı olan türün
(`expect: None`: ham reset'i kaldırmak, güven seviyesini düşürmek, …) her
mutantı `HAND` tablosunda gerekçesiyle sınıflandırılır; sınıflandırılmamış
mutant testi düşürür. Kuralın uymadığı bağlam da `HAND`'e yazılır
(örneğin yalnız `sync()` kaynağı olan bir girişin alanını değiştirmek
modülde geçiş bırakmaz; şartnamenin cevabı W3002'dir).

Şartnamenin iki kod verdiği yerde beklenen bir kümedir ve ikisi de
şartnameye dayanır:

- `on` bloğunda yabancı alan okuması: K7 örneği E3001, §5 tablosu E3012.
- İki saatten yazılan register: K4 E3011, ADR-0073 tablosu (d03b) E4001.
  Derleyici bugün her biçimde E4001 verir.

### 3. Yakın ıskalar

Tehlikeli görünen ama güvenli değişiklikler: `sync()` yerine `sync3()`,
bir register yazmasını aynı saatin ikinci `on` bloğuna taşımak, ham
reset'in spesifikasyonunu silmek (beslediği alandan alınır),
`posedge`↔`negedge`, tüketicinin `ready`'yi `valid`'den türetmesi,
sembolik alanı yeniden adlandırmak, sabit bir değerle alan geçmek,
yukarı güven akışı, kontratta sır okumak, her güven sızıntısının
`declassify` ile sarılmış ikizi. Yakın ıska hiçbir hata ve listede
olmayan hiçbir güvenlik kodu eklememelidir (`declassify` ikizi kendi
W3008'ini ekler).

### 4. Sonuçlar ve katı listeler

| Sonuç | Anlam | Liste |
|---|---|---|
| yakalandı | beklenen kodlardan biri yeni çıktı | — |
| kaçırıldı | yeni hata yok, beklenen kod yok | `KNOWN_MISSES` (issue numarasıyla) |
| farklı kod | beklenen kod yok ama başka bir hata ya da güvenlik kodu var | `KNOWN_DIFFERENT` (kodlarıyla) |
| sessiz | yakın ıska temiz | — |
| yanlış alarm | yakın ıskada hata ya da güvenlik kodu | `KNOWN_FALSE_ALARMS` (issue ve kodlarla) |

Listeler katıdır: listede olmayan bir kaçırma ya da alarm testi
düşürür; listedeki bir mutant yakalanır (ya da susar) hale gelince de
test düşer ve satırın silinmesini ister. Hiçbir mutantla eşleşmeyen
liste satırı da testi düşürür. Sessiz atlama yoktur. Her kök neden için
ayrı bir `silent-wrong` issue açılır (minimal örnek, beklenen kod,
şartname dayanağı); derleyici bu işte değiştirilmez.

### 5. Koşu

Test `cargo test --workspace` içinde koşar, hiçbir araç istemez
(`volt check` dışında süreç yok), deterministiktir (sabit sıra, rastgele
yok). Derlem en çok dört işçinin her birine bir kez geçici dizine
kopyalanır; mutant yazılır, `volt --lang en check --format json` koşar,
dosya geri yazılır. Çıkış: tür başına yakalanan/kaçırılan/farklı/
sessiz/alarm sayıları, yakalayan kodlar, dışarıda kalan tasarımlar.

### 6. Kural

Güvenlik denetimi ya da güvenlik özelliği koruyan bir tanı kodu ekleyen
PR derleme en az bir mutasyon türü ekler (`AGENTS.md`).

## Sonuçlar

İlk koşu (`main` = c33b53d7): 404 tasarım, 225'i temiz, 199'unda en az
bir mutasyon noktası; 194 tasarım dışarıda (178 parity, 1 demo, 15
kitap bloğu: `should_fail`, test, çıktı ve `from=` kopyaları). 77 tür
(61 tehlike, 16 yakın ıska), 987 mutant: 827 tehlike, 160 yakın ıska.
Koşu dört işçiyle yaklaşık 7 saniye.

- Kaçırılan: 1 — `cdc.leak-clockless-child` (#189). Saatsiz bir Volt
  modülü üzerinden `@Fast` → `@Slow` geçişi E3001 almıyor; `volt build`
  senkronizörsüz `assign` yazıyor. Kök neden: saatsiz hedef modülün
  portları `Timeless` sayılıyor, çıkış giriş alanını taşımıyor. Güven
  boyutunda aynı yapı K11 tutucu özetiyle yakalanıyor.
- Farklı kodla reddedilen: 0.
- Yanlış alarm veren yakın ıska: 0.

Geliştirme sırasında mutasyon yazımı üç kez düzeltildi; derleyici değil:
yalnız `sync()` kaynağı olan bir porta yapılan alan değişikliği (geçiş
bırakmaz, W3002), ebeveyninde o saatte flop olmayan çocuğa ham reset
vermek (tek zincir kalır; yakın ıskaya çevrildi ve R6 tehlikesi
`hybrid_top`'ta ebeveynin flop'u olan saatte kuruldu), SV eşlemesi
olmayan biçimler (`match` kol muhafızı ve bileşik `sync()` kaynağı
E0003 alır; ikizleri kaldırıldı ya da `let` ile yazıldı).

## Sınırlar

- Güç alanı (E3006–E3008), E3004, E3005, lineer portlar (E4003, E4004)
  rezervedir; mutasyon kurulamaz.
- E3013 ve E4011 derlemde değil: derlemde iki saatli bundle bağlaması
  yok; E4011 bağlantı biçimi denetimidir.
- W3005 ve W3006 primitif her kullanıldığında çıkar; tehlike/güvenli
  ayrımı yoktur.
- ADR-0050 örnek çıkışlarını opak sayar: `valid`'in bir alt modül
  üzerinden `ready`'ye bağlanması E4007 almaz; bu bir karar, kaçırma
  değil. K8a saatsiz extern portlarını açıkça `Timeless` sayar; #189 bu
  yolu karar olarak anar.
- Derlem yalnız depodaki tasarımlardır; bir türün kapsamı uygulanabildiği
  tasarımlar kadardır (iki saatli tasarım sayısı sınırlıdır).

## Reddedilen alternatifler

- **Beklenen kodu derleyicinin bugünkü çıktısından almak (golden).**
  Bir kaçırma bugünkü çıktıda da kaçırmadır; golden onu doğru sayardı.
- **Rastgele tasarım üretimi.** Deterministik olmaz ve beklenen kod
  elle belirlenemez; fark testi (ADR-0108) bu yolu ifade anlamı için
  zaten kullanır.
- **Mutantları ayrı bir iş ya da elle tetiklenen koşu yapmak.** Derlem
  birkaç saniyede koşar ve araç istemez; her PR'da koşması yeni bir
  denetimin ya da düzeltmenin etkisini hemen gösterir.
