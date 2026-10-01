# Mimari Karar Kayıtları (ADR) — İndeks

Bu dizin salt okunurdur; yeni karar yeni ADR açar (CLAUDE.md). Belge
öncelik sırasında ADR'ler UX Anayasası'ndan sonra, `docs/spec/`'ten önce
gelir (docs/README.md). Tek istisna başlık bloğundaki durum ve bağlantı
satırlarıdır (aşağıda): bir ADR önceki bir kararı değiştirdiğinde ya da
genişlettiğinde, eski ADR'nin yalnız bu satırları güncellenir; karar
metnine dokunulmaz.

## Buradan başla

Dili anlamak için şu sırayla okuyun (en fazla on kayıt; ayrıntı konu gruplarında):

1. [ADR-0013](ADR-0013-gramer-ve-parser-sozlesmesi.md) — Sözdiziminin sözleşmesi: dil neden LL(2), operatör öncelikleri ve hata kurtarma nasıl.
2. [ADR-0002](ADR-0002-domain-semantigi.md) — Volt'un ayırt edici fikri: saat, sıfırlama, güç ve güven tek domain; CDC tip hatasıdır.
3. [ADR-0041](ADR-0041-ayni-isaret-genisleme.md) — Genişlik ve işaret kurallarının bugünkü hâli (ADR-0025 ve ADR-0031 üzerine kurulu).
4. [ADR-0012](ADR-0012-sv-cikti-ongorulebilirlik.md) — Üretilen SV'nin neye benzeyeceği: modül ve ad eşlemesi, okunabilirlik garantisi.
5. [ADR-0027](ADR-0027-stdlib-mimarisi.md) — CDC neden kütüphane değil derleyici primitifi; stdlib bileşenlerinin temeli.
6. [ADR-0039](ADR-0039-bundle-port-gruplari.md) — Port grupları (`struct port`): arayüzlerin nasıl yazıldığı ve SV'ye nasıl açıldığı.
7. [ADR-0077](ADR-0077-struct-destegi.md) — Veri modellemenin bugünkü hâli: struct değerleri, bit düzeni ve enum ile ilişkisi.
8. [ADR-0011](ADR-0011-kontrat-sistemi.md) — Kontrat sistemi: tasarımın formal olarak nasıl doğrulandığı.
9. [ADR-0037](ADR-0037-l1-zamanlama.md) — L1 zamanlama: gecikmelerin tipte taşınması ve pipeline sözdiziminin temeli.
10. [ADR-0033](ADR-0033-test-bloklari-ve-simulasyon.md) — Test blokları ve Verilator köprüsü: tasarımın davranışının nasıl sınandığı (test dili ADR-0058, keşif ADR-0089).

## Numaralandırma

ADR-0001..0022 F0-F2 döneminde alınan ama dosyası açılmayan kararlardır;
2026-09-16'da geriye dönük belgelendi. Numaraları
`docs/design/Volt-Butunlesik-Mimari-v3.md` BÖLÜM VII'deki plan tablosundan
alınmıştır; kod ve sonraki ADR'ler bu numaralara zaten atıf yapıyordu
(ADR-0005 tutarlılık betiğinde, ADR-0007 ADR-0037'de, ADR-0012
`desugar.rs`'te, ADR-0013 dil spesifikasyonunda).

Numara boşlukları: **0030, 0043, 0045, 0046** hiç kullanılmadı — bu
numaralara atıf yapan dosya, commit ya da CHANGELOG girdisi yoktur.
Çift numara yoktur.

## Durum sözlüğü ve başlık bloğu

Her ADR'nin başlık bloğunda (başlıktan sonraki `>` satırları) tam olarak
bir **Statü** satırı bulunur. Değer şu altı sözcükten biridir; ardından
isteğe bağlı olarak ` — ` ve serbest açıklama gelir.

| Durum | Anlamı |
|---|---|
| Uygulandı | Karar yürürlükte ve kodda. ADR'nin kendi "ertelendi" dediği kapsam dışı işler bu durumu bozmaz. |
| Kabul edildi | Karar yürürlükte, ama ADR'nin kendi planının bir parçası (ör. "V1", "uygulanmadı") henüz kodda yok. |
| Rezerve | Yalnız ad, anahtar kelime ya da tanı kodu ayrıldı; mekanizma yok. |
| Kısmen yerini aldı: ADR-X | Kararın bir bölümünü sonraki ADR-X değiştirdi; açıklama hangi bölüm olduğunu söyler, kalanı yürürlükte. |
| Yerini aldı: ADR-X | Kararın tamamı ADR-X ile değişti; metin yalnız tarih kaydıdır. |
| Reddedildi | Önerilip kabul edilmeyen karar (bugün bu durumda ADR yok). |

Bağlantı satırları, her biri tek satır:

- `> Önceki karar: ADR-X — …` — bu ADR, ADR-X'in (bir bölümünün) yerini
  alır. İki yönlüdür: ADR-X'in Statü'sü `Yerini aldı` ya da `Kısmen yerini
  aldı` ile bu ADR'yi adlandırır, bu ADR de `Önceki karar` ile ADR-X'i.
- `> İlgili: ADR-X (…), ADR-Y (…)` — yerini almayan ilişkiler: uygulama,
  genişletme, "Sınırlar" maddesini kapatma, ek bölüm kaynağı. Eski ADR'de
  onu genişleten sonraki ADR'ler de listelenir; böylece bir kuralın bugünkü
  hâline eski kayıttan ulaşılır.

Önceki `Genişletir:`, `Kapatır:`, `Düzeltir:`, `Günceller:`, `Kaynak:`
satırları olduğu gibi kalır (ayrıntılı bölüm atıfları taşırlar).

Sözlüğün gerekçesi: eski dizindeki K / K/G / K/R kısaltmaları yalnız
"kabul edildi"yi ve belgeleme biçimini söylüyordu; bir kuralın bugün hâlâ
geçerli olup olmadığını söylemiyordu. "Kabul edildi" ile "Uygulandı"
ayrımı, planı yarım kalan geriye dönük kararları (ör. `volt.lock`, L2)
uygulanmış kararlardan ayırır; "Kısmen yerini aldı" en sık görülen
durumdur (bir ADR önceki bir ADR'nin tek bir bölümünü değiştirir), bu
yüzden tam yerini almadan ayrı tutulur. `scripts/check-consistency`
(kontrol 9-12) sözlüğü, bağlantı hedeflerini, iki yönlülüğü ve bu dizini
denetler.

## Süreç ve altyapı

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0001](ADR-0001-lisans-politikasi.md) | Lisans Politikası — Apache-2.0 OR MIT | Kod ve belgeler Apache-2.0 OR MIT çift lisansıyla dağıtılır; bağımlılıklar buna uymalıdır. | Uygulandı |
| [0005](ADR-0005-workspace-crate-sinirlari.md) | Workspace Crate Sınırları ve Araç Zinciri (Rust, logos, codespan-reporting) | Derleyici tek yönlü bağımlı crate'lere bölünür; CIRCT/melior yalnız volt-lower'da görünebilir. | Uygulandı |
| [0006](ADR-0006-arena-tabanli-ast-hir.md) | Arena Tabanlı AST/HIR — `Idx<T>` Handle Deseni, Tip Bilgisi HIR'da | AST ve HIR arenalarda tutulur, düğümlere `Idx<T>` ile erişilir; tip bilgisi HIR'da yaşar. | Uygulandı |
| [0009](ADR-0009-test-dosya-formati.md) | Test Dosya Formatı — `tests/ui/{pass,fail}`, `//~` Anotasyonları, Birebir Fixture | Testler `tests/ui/{pass,fail}` altında `//~` anotasyonlu fixture'lardır; beklenen çıktılar birebir karşılaştırılır. | Uygulandı |
| [0015](ADR-0015-determinizm-garantisi.md) | Determinizm Garantisi Kapsamı — Aynı Kaynak, Byte-Aynı Çıktı | Aynı kaynak ve aynı derleyici her zaman bayt bayt aynı çıktıyı üretir. | Kabul edildi |
| [0093](ADR-0093-hazir-ikililer-surum-is-akisi.md) | Hazır İkililer — Sürüm İş Akışı, Platformlar, Taşınabilirlik ve Kaynak Doğrulaması | `v*` tag'i dört platform için statik ikili arşivleri, `.vsix`, SHA256 özetleri ve derleme kaynağı kaydıyla taslak GitHub Release üretir. | Kısmen yerini aldı: ADR-0096 |
| [0094](ADR-0094-docker-koprusu.md) | Docker Köprüsü — Eksik Verilator/sby'yi Sabitlenmiş İmajda Otomatik Koşturmak | Verilator ya da sby yerelde yoksa `volt test`, `run` ve `verify` aracı özetle sabitlenmiş imajda koşturur; tek satır bildirir, yollar ana makine yolu kalır. | Uygulandı |
| [0096](ADR-0096-kurulum-betikleri.md) | Kurulum Betikleri — Tek Komutla Doğrulanmış, Yönetici Yetkisi İstemeyen, Geri Alınabilir Kurulum | `irm …/install.ps1 \| iex` ve `curl …/install.sh \| sh` sürümsüz varlık adlarıyla en yeni sürümü indirir, `SHA256SUMS` ile doğrular, kullanıcı klasörüne kurar ve PATH'e bir kez ekler; yeniden çalıştırma günceller, `VOLT_UNINSTALL=1` iz bırakmadan kaldırır. | Uygulandı |

## Sözdizimi ve dil yapıları

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0013](ADR-0013-gramer-ve-parser-sozlesmesi.md) | Gramer ve Parser Sözleşmesi — LL(2), Geri İzleme Yasağı, Hata Kurtarma, Öncelik Kararları | Gramer LL(2)'dir ve geri izleme yoktur; hata kurtarma ve operatör öncelikleri sabittir. | Uygulandı |
| [0019](ADR-0019-todo-semantigi.md) | `todo!` Semantiği — Kısmi Derleme | `todo!` ifadesi eksik tasarımın derlenmesine izin verir. | Kabul edildi |
| [0023](ADR-0023-baglamsal-anahtar-kelimeler.md) | Bağlamsal Anahtar Kelimeler — `sync` Çakışması | `sync` gibi sözcükler bağlama göre anahtar kelime ya da ad olur. | Uygulandı |
| [0028](ADR-0028-ayrilmis-kelime-revizyonu.md) | Ayrılmış Kelime Revizyonu — Stdlib'e Taşınanlar Serbest | Stdlib'e taşınan adlar ayrılmış kelime listesinden çıkarılır. | Uygulandı |
| [0032](ADR-0032-match-sirali-blokta.md) | Sıralı/Kombinasyonel Blokta `match` ve E0014 | `match` deyimi sıralı ve kombinasyonel bloklarda `case`'e iner; kapsayıcı olmayan match E0014. | Uygulandı |
| [0034](ADR-0034-implikasyon-operatoru.md) | İmplikasyon Operatörü `->` | `a -> b` implikasyon operatörü eklenir; SVA'da `\|->` olarak üretilir. | Uygulandı |
| [0056](ADR-0056-duzenli-yapilar.md) | Düzenli Yapılar — `for` İçinde Örnekleme, Bundle Dizileri, Paketlenmiş Dizi Portları | `for` içinde örnekleme, bundle dizileri ve paketlenmiş dizi portları desteklenir. | Uygulandı |
| [0081](ADR-0081-fonksiyon-destegi.md) | Fonksiyon Desteği — Saf Kombinasyonel `fn` Donanıma İner | Saf kombinasyonel `fn` çağrı yerinde açılarak donanıma iner; özyineleme E4013. | Uygulandı |
| [0083](ADR-0083-match-ifadesi-ve-blok-let.md) | `match` İfadesi ve Blok İçi `let` | `match` ifade olarak ve blok içi `let` kullanılabilir; kök konumda `case`, iç konumda üçlü zincir üretilir. | Uygulandı |
| [0085](ADR-0085-desen-adlari.md) | Desendeki Çıplak Ad Bir Değerdir — Bağlama Deseni Yok | Desendeki çıplak ad bir değerdir (sabit ya da varyant); bağlama deseni yoktur. | Uygulandı |

## Tip sistemi ve veri tipleri

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0003](ADR-0003-trit-tipi.md) | Trit Tipi — Kısıtlı i2 ve Katmanlı Görünürlük | `Trit` üç değerli (−1/0/+1) kısıtlı bir i2 tipidir; görünürlüğü katmanlı planlandı. | Kabul edildi |
| [0018](ADR-0018-lineer-tipler-opt-in.md) | Lineer Tipler — Opt-in (`&inv`), Normal Portlarda Sürücü Analizi | Lineer (tek kullanımlık) port tipleri opt-in olacak; normal portlar sürücü analiziyle denetlenir. | Kısmen yerini aldı: ADR-0075 |
| [0025](ADR-0025-esnek-genislik-ve-w2013.md) | Aritmetik Sonuçlarda Esnek Genişlik Aralığı ve W2013 | Aritmetik sonuç genişliği esnek bir aralıktır; gereksiz genişleme W2013 ile uyarılır. | Uygulandı |
| [0031](ADR-0031-keyfi-bit-genisligi.md) | Keyfi Bit Genişlikli Tamsayı Tipleri (u1..u64 / i1..i64) | `u1..u64` / `i1..i64` keyfi genişlikli tamsayı tipleri desteklenir. | Uygulandı |
| [0035](ADR-0035-degisken-indeksleme.md) | Değişken Dizi İndeksi ve Indexed Part-Select | Değişken dizi indeksi ve `+:` / `-:` indexed part-select desteklenir. | Uygulandı |
| [0036](ADR-0036-isaretli-kaydirma-ve-cast.md) | İşaretli Kaydırma (`>>>`) ve `$signed`/`$unsigned` Üretimi | `>>>` işaretli kaydırma ve işaret dönüşümleri SV'de `$signed` / `$unsigned` ile üretilir. | Uygulandı |
| [0039](ADR-0039-bundle-port-gruplari.md) | Bundle (Port Grubu) Desteği — `struct port` | `struct port` port gruplarını tanımlar; bundle alanları yönleriyle porta düzleşir. | Uygulandı |
| [0041](ADR-0041-ayni-isaret-genisleme.md) | Aritmetik Ergonomisi — Aynı-İşaret Genişleme, Const Diziler, Generic Örnekleme | Aynı işaretli genişleme örtüktür; const diziler ve const generic modül örneklemesi desteklenir. | Uygulandı |
| [0051](ADR-0051-cift-yonlu-portlar.md) | Çift Yönlü Portlar — `inout` Yazma Desteği ve `opendrain` Tipi | `inout` portlara yazma ve `opendrain` tipi üç durumlu tamponlarla desteklenir. | Uygulandı |
| [0069](ADR-0069-ozyineli-tip-ve-generic-struct-port.md) | Özyineli Tiplerin Tek Tip Çizgesi Denetimi ve Generic Struct Port Reddi | Tüm özyineli tipler tek tip çizgesinde E4009 ile yakalanır; generic `struct port` E0003 alır. | Uygulandı |
| [0073](ADR-0073-coklu-surucu-denetimi.md) | Çoklu Sürücü Denetimi — Tek Tablo, Sürücü Türleri, Bit Aralıkları | Tüm sürücüler tek tabloda türleri ve bit aralıklarıyla denetlenir; çakışma E4001. | Uygulandı |
| [0074](ADR-0074-enum-destegi.md) | Enum Desteği — Birim Varyantlı Enum'lar Donanıma İner | Birim varyantlı enum'lar ikili kodlamayla donanıma iner; kapsayıcılık ve SV eşlemesi tanımlanır. | Uygulandı |
| [0077](ADR-0077-struct-destegi.md) | Struct Desteği — Düz Struct Değerleri Donanıma İner | Düz struct değerleri SV packed düzeninde donanıma iner; SV'de alanlar ayrı sinyallere açılır. | Uygulandı |

## Zamanlama seviyeleri ve pipeline

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0007](ADR-0007-zamanlama-seviyeleri.md) | L0/L1/L2 Zamanlama Seviyeleri | Zamanlama güvenliği üç seviyelidir: L0 denetimsiz, L1 gecikme tipleri, L2 timeline tipleri. | Kabul edildi |
| [0037](ADR-0037-l1-zamanlama.md) | L1 Zamanlama Seviyesi — `Delayed<T, N>`, `delay<K>` ve `@strict_timing` | `Delayed<T, N>` ve `delay<K>` gecikmeyi tipte taşır; `@strict_timing` uyuşmazlığı E5010 yapar. | Uygulandı |
| [0038](ADR-0038-pipeline-sozdizimi.md) | Pipeline Sözdizimi — `pipeline`, `stage`, `stall`, `flush` | `pipeline` / `stage` / `stall` / `flush` aşamalı tasarımı L1 zamanlamasıyla ifade eder. | Uygulandı |

## Saat alanı, CDC ve RDC

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0002](ADR-0002-domain-semantigi.md) | Domain Semantiği — `@Domain` Anotasyonu ve Birleşik Domain (Saat + Sıfırlama + Güç + Güven) | Saat, sıfırlama, güç ve güven tek bir `@Domain` anotasyonunda birleşir; saat alanı uyuşmazlığı tip denetiminde yakalanır. | Kabul edildi |
| [0047](ADR-0047-extern-domain-anotasyonu.md) | Extern Modül Sınırında Domain Anotasyonu — Sembolik Saat Alanları | Extern modül sınırındaki portlar sembolik saat alanlarıyla anotasyonlanır. | Uygulandı |
| [0065](ADR-0065-rdc-ve-hedefli-sdc.md) | RDC Denetimi ve Hedefli SDC Kısıtları — İki Güvenlik Ağı, İki Ayrı Yırtık | RDC denetimi (E3003) ve üretilen reset bırakma senkronizörü gelir; SDC'de hedefli kısıtlar varsayılan olur. | Uygulandı |
| [0088](ADR-0088-bildirimde-alan-aciklamasi.md) | Sinyal Bildirimlerinde Saat Alanı Açıklaması | `wire` / `let` / `reg` bildirimleri denetlenen bir `@Alan` saat alanı açıklaması taşıyabilir. | Uygulandı |

## Yerleşik primitifler (stdlib)

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0027](ADR-0027-stdlib-mimarisi.md) | Stdlib Mimarisi — CDC Primitifleri Derleyicide Yerleşik | CDC primitifleri (AsyncFifo, HandshakeSync, …) derleyicide yerleşik stdlib bileşenleridir. | Uygulandı |
| [0029](ADR-0029-stdlib-gunluk-yapi-taslari.md) | Stdlib Genişletmesi — Tek Saatli Günlük Yapı Taşları | Tek saatli günlük yapı taşları (FIFO, RAM, kaydırma yazmacı, arbiter …) yerleşik primitif olur. | Uygulandı |
| [0049](ADR-0049-domain-aware-bellek.md) | Domain-Aware Bellek — `AsyncDualPortRam<T, DEPTH>` | `AsyncDualPortRam<T, DEPTH>` iki saat alanlı yerleşik bellektir. | Uygulandı |
| [0050](ADR-0050-handshake-primitifi.md) | `Handshake<T>` — Yerleşik Tek Saatli El Sıkışma Bundle'ı | `Handshake<T>` valid/ready el sıkışmasını otomatik protokol kontratlarıyla yerleşik bundle olarak sunar. | Uygulandı |
| [0087](ADR-0087-primitif-struct-ogeleri.md) | Yerleşik Primitiflerde Struct / Enum Öğe Tipi | Yerleşik primitifler struct/enum öğe tipini tek paketlenmiş sözcük olarak saklar. | Uygulandı |

## Güven ve bilgi akışı

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0020](ADR-0020-guvenlik-akis-denetimi.md) | Güvenlik Akış Denetimi — `trust_level` Domain'in Dördüncü Boyutu | `trust_level` domain'in dördüncü boyutudur; güvenlik akışı domain denetimiyle yapılır. | Uygulandı |
| [0052](ADR-0052-guven-seviyeleri.md) | Güven Seviyeleri — `trust_level`, Bilgi Akışı Denetimi (E3009) ve `declassify` | `trust_level` kafesinde bilgi akışı denetlenir (E3009); `declassify` iz bırakarak seviyeyi düşürür. | Uygulandı |

## Kontratlar ve formal doğrulama

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0011](ADR-0011-kontrat-sistemi.md) | Kontrat Sistemi — `requires` / `ensures` / `invariant` / `cover` | Modüller `requires` / `ensures` / `invariant` / `cover` kontratları taşır; SVA ve SymbiYosys ile doğrulanır. | Kısmen yerini aldı: ADR-0097 |
| [0014](ADR-0014-kontrat-kademeli-benimseme.md) | Kontrat Sistemi Kademeli Benimseme — Hiçbir Kontrat Zorunlu Değil | Hiçbir kontrat zorunlu değildir; kontratsız tasarım uyarısız derlenir. | Uygulandı |
| [0016](ADR-0016-hook-kontrat-uyumu.md) | Hook Mekanizması — Kontrat Altında Politika Enjeksiyonu (`hook` Rezerve) | `hook` anahtar kelimesi kontrat altında politika enjeksiyonu için ayrıldı; mekanizma yok. | Rezerve |
| [0040](ADR-0040-ardisik-kontratlar.md) | Ardışık Kontratlar — `prev()` Yerleşiği | `prev()` yerleşiği bir önceki çevrime bakan ardışık kontratları yazdırır. | Uygulandı |
| [0055](ADR-0055-paralel-formal-dogrulama.md) | Paralel Formal Doğrulama — `volt verify -j`, Modül Başına sby Görevi, Kaynak Sıralı Rapor | `volt verify -j` modül başına sby görevlerini paralel koşar, raporu kaynak sırasında verir. | Kısmen yerini aldı: ADR-0097 |
| [0066](ADR-0066-otomatik-fsm-sayac-kontratlari.md) | Otomatik FSM ve Sayaç Kontratları | FSM ve sayaçlar için geçiş/durum cover'ları ve sınır invariant'ları otomatik üretilir. | Kısmen yerini aldı: ADR-0074, ADR-0086 |
| [0082](ADR-0082-varsayilan-formal-motoru.md) | Varsayılan Formal Motoru — boolector; bitwuzla Seçeneği, Portföy ve Otomatik Seçim Reddedildi | Varsayılan formal motoru boolector olur; `--engine bitwuzla` seçeneği eklenir. | Uygulandı |
| [0086](ADR-0086-otomatik-cover-erisilebilirligi.md) | Otomatik Cover'ların Yapısal Erişilebilirliği — Cover Kipinde Yanlış E5001 Yok | Otomatik cover yapısal olarak ölü değilse cover kipinde E5001 sayılmaz. | Uygulandı |
| [0097](ADR-0097-alt-ornek-yukumlulukleri.md) | Alt Örnek Yükümlülükleri — Örneğin `requires`/`assume`'u Üst Görevde `assert`, Saatsiz Kontrat E5005, Boş Doğrulama E5006 | Bir örneğin ön koşulu onu süren üst modülün görevinde denetlenir; saatsiz modülün kontratı ve hiçbir şey denetlemeyen koşu başarı sayılmaz. | Uygulandı |

## Simülasyon ve test dili

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0033](ADR-0033-test-bloklari-ve-simulasyon.md) | Test blokları ve Verilator simülasyon köprüsü | Modüle `test` blokları yazılır; `volt test` bunları Verilator tezgâhına çevirip koşar. | Kısmen yerini aldı: ADR-0089 |
| [0058](ADR-0058-test-dili-genisletme.md) | Test Dili Genişletme — Yerel Değişken, Dizi, `for`, `read_hex`, `load` | Test dili yerel değişken, dizi, çalışma zamanı `for`, `read_hex` ve `load` kazanır. | Kısmen yerini aldı: ADR-0080 |
| [0059](ADR-0059-test-port-genislik-kontrolu.md) | Test Bloğunda Port Genişlik Denetimi (E8512) | Test bloğunda porta sığmayan sabit E8512 olur, hesaplanmış değer koşuda testi düşürür. | Uygulandı |
| [0060](ADR-0060-test-sabit-yayilimi.md) | Test Bloğunda Sabit Yayılımı | Sabit `let` ve literal `const` değerleri E8512 denetimine derleme zamanında ulaşır. | Uygulandı |
| [0064](ADR-0064-simulasyonda-kontratlar.md) | Simülasyonda Kontratlar — İzleyici Olarak `volt test` | `volt test` kontratları izleyici olarak çalıştırır; assume ihlali testi düşürür, cover özeti basılır. | Uygulandı |
| [0089](ADR-0089-volt-test-proje-kesfi.md) | `volt test` — Proje Kökünden Özyinelemeli Test Keşfi | `volt test` testleri proje kökünden özyinelemeli olarak keşfeder. | Uygulandı |
| [0092](ADR-0092-dalga-formunda-enum-adlari.md) | Dalga Formunda Enum ve Trit Adları — GTKWave Oturumu ve Çeviri Tabloları | `volt run --vcd` ve `volt verify` karşı örneği enum/Trit sinyallerini adlarıyla gösteren bir GTKWave oturumu yazar; RTL değişmez. | Uygulandı |

## SV üretimi

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0008](ADR-0008-sv-cikti-stili.md) | SV Çıktı Stili — `always_ff`, Açık Genişlik, Yasak Liste (x/z Üretilmez) | Üretilen SV `always_ff` ve açık genişlik kullanır; x/z ve yasak listedeki yapılar üretilmez. | Uygulandı |
| [0010](ADR-0010-arka-uc-circt-ertelendi.md) | Arka Uç — CIRCT Dialect Seçimi Ertelendi, Doğrudan SystemVerilog Üretimi | CIRCT arka ucu ertelendi; SystemVerilog doğrudan metin şablonlarıyla üretilir. | Uygulandı |
| [0012](ADR-0012-sv-cikti-ongorulebilirlik.md) | SV Çıktı Öngörülebilirlik Garantisi — 1:1 Modül, İsim Korunumu, ECO Uyumu | Her Volt modülü tek SV modülüne iner ve adlar korunur; çıktı okunabilir ve ECO'ya uygundur. | Uygulandı |
| [0024](ADR-0024-sv-cikti-adlandirma.md) | SV Çıktı Dosyası Adlandırması — DECLFILENAME Çakışması | Her modül kendi adını taşıyan SV dosyasına yazılır (Verilator DECLFILENAME). | Uygulandı |
| [0026](ADR-0026-uretilen-kod-dili.md) | Üretilen Kodun Dili — Her Zaman İngilizce | Üretilen kodun başlıkları ve yorumları her zaman İngilizcedir. | Uygulandı |
| [0057](ADR-0057-sv-operator-onceligi.md) | SV Üretiminde Operatör Önceliği — Parantez Kararı Hedef Dilin Tablosuyla Verilir | SV'de parantez kararı IEEE 1800 öncelik tablosuyla verilir (sessiz yanlış derleme düzeltmesi). | Uygulandı |
| [0062](ADR-0062-trit-sv-eslemesi.md) | Trit SV Eşlemesi — İşaretli 2 Bit ve Çarpansız Ternary MAC | `Trit` SV'de `logic signed [1:0]` olarak eşlenir; `Trit * x` çarpansız seçiciye iner. | Uygulandı |
| [0071](ADR-0071-extern-sv-eslemesi.md) | `extern module` Örneklemesinin SV Eşlemesi ve ui/pass Build Denetimi | `extern module` örnekleri SV'ye bildirilen portlarıyla iner; ui/pass fixture'ları build'den de geçer. | Uygulandı |
| [0076](ADR-0076-extern-kaynaklari.md) | extern Modül Kaynakları — `@source`, Extern İçi CDC ve Flop'suz Modülün Reset Portu | `@source` extern modülün SV kaynağını bağlar (E1012); extern içi CDC gizlenmez, raporlanır. | Uygulandı |
| [0078](ADR-0078-hedef-dil-ayrilmis-sozcukleri.md) | Hedef Dillerin Ayrılmış Sözcükleri — SV Anahtar Sözcüğü Ad Olamaz | Hedef dillerin (SV, Rust, C/C++) ayrılmış sözcükleri ad olamaz (E1013); sessiz yeniden adlandırma yok. | Uygulandı |
| [0090](ADR-0090-uretilen-ad-carpismalari.md) | Üretilen Ad Çarpışmaları ve `let x = sync(...)` | Üretilen SV adı başka bir adla çakışırsa E1003 olur; `let x = sync(...)` desteklenir. | Uygulandı |

## Zamanlama kısıtları (SDC/XDC)

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0048](ADR-0048-zamanlama-kisitlari.md) | Zamanlama Kısıtları — Uygulanmayan Nitelikler W0021 Üretir, SDC Üretimi V1 Planı | Uygulanmayan nitelikler W0021 üretir; SDC üretimi için V1 planı çizildi. | Kısmen yerini aldı: ADR-0054 |
| [0054](ADR-0054-sdc-uretimi.md) | SDC/XDC Üretimi — `@timing` Uygulanıyor, Zamanlama Kısıtları Domain Bilgisinden Türetiliyor | `@timing` uygulanır; SDC/XDC kısıtları domain bilgisinden türetilir. | Kısmen yerini aldı: ADR-0065 |

## HW-SW köprüsü (`@mmio`)

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0044](ADR-0044-mmio-register-haritasi.md) | `@mmio` Register Haritası — Bellek Eşlemeli Register Blokları | `@mmio` bellek eşlemeli register bloklarını tanımlar ve RTL'e açar. | Uygulandı |
| [0053](ADR-0053-hw-sw-koprusu.md) | HW-SW Köprüsü — `@mmio` Haritasından Sürücü, Başlık, `regmap.json` ve Belge Üretimi | `@mmio` haritasından Rust/C sürücüsü, başlık, `regmap.json` ve belge üretilir. | Uygulandı |
| [0063](ADR-0063-regmap-tutarlilik-denetimi.md) | Register Haritası Tutarlılık Denetimi — `check-regmap` ve `--check-regmap` | `check-regmap` bayat sürücüyü, `build --check-regmap` sürücü ile SV uyuşmazlığını yakalar (E9003/E9004). | Uygulandı |

## Modüller, paketler ve proje

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0017](ADR-0017-modul-sistemi-ve-paket-semantigi.md) | Modül Sistemi ve Paket Semantiği — `package` / `use` / `pub` | `package` / `use` / `pub` ile modül sistemi ve görünürlük kuralları tanımlanır. | Uygulandı |
| [0022](ADR-0022-semver-donanim-kurallari.md) | SemVer Donanım Kuralları — `@version`, `@abi_version`, Anlamsal Diff (Rezerve) | `@version` / `@abi_version` ile donanım için SemVer ve anlamsal diff planlandı. | Rezerve |
| [0042](ADR-0042-coklu-dosya-derleme.md) | Çoklu Dosya Derleme ve Import Sistemi | Birden çok dosya `use` ile tek derleme birimi olur; her modül ayrı SV dosyasına yazılır. | Uygulandı |
| [0061](ADR-0061-volt-toml-arama-tavani.md) | Volt.toml Aramasına Tavan | Volt.toml araması git kökünde ve ev dizininde durur; `VOLT_MANIFEST_DIR` geçersiz kılar. | Uygulandı |

## Araç zinciri, CLI ve LSP

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0021](ADR-0021-artifact-uretim-ve-cli-sozlesmesi.md) | Artifact Üretim Mimarisi ve CLI Sözleşmesi — Tek Kaynak, `build/` Dizini, Çıkış Kodları | Tüm çıktılar tek kaynaktan `build/` altına üretilir; çıkış kodları ve stdout/stderr ayrımı sözleşmedir. | Kabul edildi |
| [0084](ADR-0084-doctor-ve-new.md) | `volt doctor` ve `volt new` / `volt init` — Kurulum Teşhisi, Tek Kaynaklı Araç Keşfi, Doğrulanmış Şablonlar | `volt doctor` kurulumu teşhis eder; `volt new` / `volt init` doğrulanmış şablonlardan proje kurar. | Uygulandı |
| [0091](ADR-0091-lsp-inlay-quick-fix.md) | LSP — Inlay İpuçları, Quick Fix ve Protokol Testleri | LSP tip, saat alanı ve gecikme inlay ipuçları ile tek öneriden quick fix sunar. | Uygulandı |
| [0095](ADR-0095-proje-kipi-test-dalga-formu-watch.md) | Proje Kipi — Argümansız check/build/run/verify, Düşen Testin Dalga Formu ve `volt test --watch` | Dosyasız komutlar Volt.toml projesinde çalışır, üst modül `top` ya da tek adaydan gelir; düşen test izle yeniden koşup `Waveform` satırı verir; `--watch` kayıtta yeniden koşar. | Uygulandı |

## Tanılar ve sağlamlık (fuzz, sınırlar)

| No | Başlık | Özet | Durum |
|---|---|---|---|
| [0004](ADR-0004-tani-sozlesmesi.md) | Tanı Sözleşmesi — Hata Kodu Formatı, 5 Parçalı Mesaj, Tanı Dili | Her tanı kod, konum, açıklama, öneri ve spec referansından oluşan beş parçayı taşır; varsayılan dil İngilizcedir. | Uygulandı |
| [0067](ADR-0067-bundle-dongu-ve-duzlestirme-butcesi.md) | Özyineli Bundle Tespiti ve Düzleştirme Bütçesi | Kendini içeren bundle E4009, düzleştirme bütçesi aşımı E4010 ile reddedilir. | Kısmen yerini aldı: ADR-0068 |
| [0068](ADR-0068-tani-katlama-ve-ust-sinir.md) | Açılan Kopyalarda Özdeş Tanı Katlama, Tanı Üst Sınırı ve Açılım Düğüm Bütçesi | Açılan kopyalardaki özdeş tanılar katlanır, tanı sayısı sınırlanır (W0023) ve açılım düğüm bütçesi konur. | Uygulandı |
| [0070](ADR-0070-tani-tutarliligi.md) | Tanı Tutarlılığı — `check`, LSP ve `build` Aynı Şeyi Söyler | `check`, LSP ve `build` tek ortak boru hattından geçer ve aynı tanıları üretir. | Uygulandı |
| [0072](ADR-0072-kucuk-tani-duzeltmeleri.md) | Küçük Tanı Düzeltmeleri — Kaynak Adı, E4011, Yerleşik Argüman Sayısı, Ölü Reset Zinciri | Açılmış kopyalarda kaynak adı, yeni E4011, yerleşik argüman sayısı ve ölü reset zinciri düzeltilir. | Uygulandı |
| [0075](ADR-0075-yaniltici-rapor-ve-tani-temizligi.md) | Yanıltıcı Raporlar ve Tanı Kalitesi — verify Durumları, Tek Kod Tek Bulgu, Kapı Arkasında E0014 | `verify` sby'nin beş durumunu ayrı raporlar (E5002, zaman aşımı); bir bulgu tek kodla bildirilir. | Uygulandı |
| [0079](ADR-0079-cikti-dogrulama-agi.md) | Çıktı Doğrulama Ağı — Her Çıktı Gerçek Tüketicisiyle Denetlenir | Her çıktı testte gerçek tüketicisiyle (Verilator, Yosys, OpenSTA, C/Rust derleyicileri) denetlenir. | Uygulandı |
| [0080](ADR-0080-ozyineleme-derinligi.md) | Özyineleme Derinliği — Parser Ağacı Sınırlar, Derleyici Bilinen Yığında Koşar | Parser ağaç derinliğini 256 ile sınırlar (E0018); derleyici 64 MB'lık yığında koşar. | Uygulandı |
