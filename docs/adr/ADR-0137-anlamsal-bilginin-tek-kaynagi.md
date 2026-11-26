# ADR-0137: Anlamsal Bilginin Tek Kaynağı — Çıktıyı Etkileyen Karar Tip Denetiminden ya da Ad Çözümlemeden Gelir, Sözdizimsel Tahmin İzin Listesinde Durur

> Statü: Uygulandı — kural ve denetimi (§4); §3'ün geçiş adımları açık
> İlgili: ADR-0136 (arka uçlar anlamsal bilgiyi tek yerden okur: bu ADR kuralı derleyicinin bütün geçitlerine genişletir, envanterin "Bilinen ayrışmalar" tablosunu ayıklar ve Aşama 1'in gölge biçimini saat/reset sınıflamasına uygular), ADR-0139 (kural kodlama bilgisine uygulanır; ikili varsayımların envanteri), ADR-0140 (`domain` bildiriminin tek okuyucusu ve yinelenen alanda E1003; §5'in 8. satırını kapatır), ADR-0141 (§1'in kuralı `prev()`'in genişliğine ve işaretine; üreticiye tip denetiminin cevabı)
> Issue: #412
> Tarih: 2026-11-24
> Etkilenen: `crates/volt-driver/tests/class_shadow_tests.rs` (YENİ), `crates/volt-driver/tests/signal_class_callers_tests.rs` (YENİ), `crates/volt-driver/tests/divergence_triage_tests.rs` (YENİ), `crates/volt-driver/tests/shadow_corpus/mod.rs` (YENİ, genişlik gölgesiyle ortak yükleyici), `crates/volt-driver/tests/width_shadow_tests.rs` (yükleyici ortak modüle taşındı), `docs/design/tek-yerde-bilgi-envanteri.md` ("Bilinen ayrışmalar" tablosu)

## Sorun

ADR-0136 arka uçların (SV, sürücüler, SDC, test simülasyonu) anlamsal
bilgiyi tip denetiminden okumasını hedefler. Aynı sorun arka uçların
önündeki geçitlerde de vardır: tip denetiminden önce koşan bir geçit bir
bilgiyi kendi kuralıyla, sözdiziminden tahmin eder.

#234'ün düzeltmesi (PR #416) bunun bir örneğini kapattı: portun saat,
reset ya da veri olduğu (`SignalClass`) iki yoldan okunur:

- anlamsal yol, `volt_hir::port_class::DefTypes`: tip denetiminin tanıma
  verdiği tip (`Ty::Clock`, `Ty::Reset`);
- sözdizimsel yol, `volt_ast::alias::signal_class`: takma ad zinciri, ada
  göre. Tip denetiminin sonucuna ulaşamayan yedi yer kullanır: ayrıştırıcının
  dört geçidi, W1001 süzgeci, test blokları, LSP'nin yarım kod yolu.

İki yol bugün aynı kuralı (takma ad hedef tipidir, ADR-0070 §6) okur ama
iki ayrı hesaptır. #415 ilk ayrışmalarıdır: kendisini kullanan öğeden
sonra bildirilen takma adı ayrıştırma sırasında koşan iki geçit görmez.
Bir hesabın ikinci kopyası olduğu her yerde aynı şey tekrar olabilir ve
bunu bugün hiçbir şey durdurmaz.

ADR-0136'nın envanteri (`docs/design/tek-yerde-bilgi-envanteri.md`) iki
hesabın bugün farklı cevap verdiği on bir yer listeler. ADR-0136 §2'ye
göre ayrışması kalan bilgi Aşama 2'de tek yere geçmez. Bu on bir yerin
hangisinin kullanıcıya ulaştığı bilinmiyordu.

## Karar

### 1. Kural

Derleyici çıktısını etkileyen her anlamsal karar tip denetiminden ya da ad
çözümlemeden gelir. Çıktı: üretilen dosyalar (SV, SVA, SDC, XDC,
sürücüler, regmap), tanılar ve `volt test`'in sonuçları. Anlamsal karar:
bir ifadenin ya da sinyalin genişliği, işareti, sabit değeri, takma adın
çözüldüğü tip, saat alanı, saat/reset sınıfı, struct sinyalinin yaprakları
(ADR-0136 §1'in listesi).

Sözdizimsel tahmin (AST'den, ada göre, tip denetimi olmadan yapılan
hesap) kalıcı olarak yalnız editör yardımcılarında kalabilir: LSP
tamamlaması, öneri metni gibi kullanıcının kararını değiştirmeyen ve
yanlışsa yalnız bir öneriyi zayıflatan yerler. Başka her sözdizimsel
tahmin geçicidir: izin listesinde, nedeni ve onu kaldıracak geçiş adımıyla
durur (§3, §4).

### 2. Uygulama alanı

Kural yeni kodu bağlar: tip denetiminden sonra koşan bir geçit bilgiyi
tip denetiminin sonucundan okur (saat/reset sınıfı için `DefTypes`'ın
`class`, `port`, `is_clock_port`, `is_raw_reset` yöntemleri). Bugünkü
sözdizimsel tahminler §3'ün sırasıyla kalkar. ADR-0136'nın arka uç
geçişi (Aşama 2-3) aynı kuralın arka uçtaki uygulamasıdır; bu ADR onun
kararlarını değiştirmez.

### 3. Geçiş sırası

1. **Kısa vade: W1001 süzgeci ve test blokları anlamsal yola geçer.**
   W1001 süzgeci (`rdc::without_raw_reset_unused`) tip denetimi koştuysa
   onun sonucunu okur; çözümleme hatalı olduğu için tip denetimi
   koşmadığında ne yapacağı o adımın PR'ında, çıktısı ölçülerek
   belirlenir (kullanıcı o durumda zaten bir hata görür). Test blokları
   (`sim::check_set_port`, E8503) birimin tip denetimini okur: `volt test`
   birimi zaten derler.
2. **Orta vade: ayrıştırıcının geçitleri ad çözümlemenin sonucunu okur.**
   Pipeline saati (E5011), E0019 önerisi, `@mmio` açılımı ve otomatik
   kontratlar ayrıştırma bittikten ve ad çözümleme koştuktan sonra
   portun sınıfını çözülmüş tanımdan alır. Bu #415'i kapatır: bildirim
   sırası önemini yitirir (ADR-0070 §6, öğeler sıralanmaz).
3. **Açılımlar ara temsile taşınır** (ADR-0136 Aşama 3): generic, `for`,
   bundle ve fn açılımı ara temsilin kurulduğu yerde yapılır; ayrıştırıcıda
   anlamsal bilgi gerektiren geçit kalmaz.

LSP'nin yarım kod yolu (`analysis::clock_ports`, `on ` tamamlaması) kalıcı
olarak izin listesinde kalabilir: tip denetimi koştuysa zaten onun sonucunu
okur, koşmadıysa yalnız bir tamamlama önerisi etkilenir.

### 4. Denetim

Kuralın iki mekanik denetimi vardır; ikisi de yalnız testte koşar,
kullanıcıya yeni seçenek, bayrak ya da ortam değişkeni girmez.

**İzin listesi** (`crates/volt-driver/tests/signal_class_callers_tests.rs`).
Test her `crates/*/src/**/*.rs` dosyasını tarar ve
`volt_ast::alias::signal_class`'a her başvuruyu içinde bulunduğu
fonksiyonla eşler. Listede (dosya, fonksiyon, neden, onu kaldıracak adım)
olmayan bir çağıran testi düşürür; listede olup artık çağırmayan girdi de
düşürür, böylece liste geçişle birlikte küçülür. Satır numarası eşlenmez
(ADR-0133'ün gerekçesiyle: kod taşınınca liste değişmez). Çağrıyı gizleyen
içe aktarma (`alias::*`, başka adla içe aktarma) da testi düşürür. Bugünkü
liste:

| Çağıran | Neden tip denetimini okuyamıyor | Kaldıracak adım |
|---|---|---|
| `volt-syntax/src/parser/desugar.rs` `single_clock` | pipeline saati (E5011), ayrıştırma sırasında | §3.2 (#415) |
| `volt-syntax/src/parser/register_assign.rs` `reg_clocks` | E0019 önerisi, ayrıştırma sırasında | §3.2 (#415) |
| `volt-syntax/src/parser/mmio.rs` `module_clock` | `@mmio` açılımı, ad çözümlemeden önce | §3.2 |
| `volt-syntax/src/parser/auto_contract/scan.rs` `scan_module` | otomatik kontratlar, ad çözümlemeden önce | §3.2 |
| `volt-hir/src/domain/rdc/mod.rs` `without_raw_reset_unused` | W1001 süzgeci, tip denetiminden önce ve çözümleme hatalıyken | §3.1 |
| `volt-hir/src/sim.rs` `check_set_port` | test blokları (E8503), birim düzeyinde tip denetimi yok | §3.1 |
| `volt-lsp/src/analysis.rs` `clock_ports` | `on ` tamamlaması, yarım kodda tip denetimi yok | kalıcı (§1) |

**Sınıflama gölgesi** (`crates/volt-driver/tests/class_shadow_tests.rs`,
ADR-0136 Aşama 1'in biçimi). Genişlik gölgesinin girdisiyle (fixtures,
`ui/pass`, `ui/multifile`, `real_bugs`, örnekler, şablonlar, demo, kitap
blokları; ortak yükleyici `shadow_corpus`) derlenen her birimin her modül
ve extern portu için tip denetiminin sınıfı takma ad zincirininkiyle
karşılaştırılır. Zincir iki görünümde okunur, çünkü çağıranlar onu iki
farklı anda çağırır:

- **birim**: birimin bütün AST'si (W1001 süzgeci, test blokları, LSP,
  `@mmio`, otomatik kontratlar);
- **ayrıştırma sırası**: öğe ayrıştırılırken AST'de bulunanlar, yani
  önceki dosyaların bütün öğeleri ve kendi dosyasında ondan önce gelenler
  (pipeline saati, E0019 önerisi; `volt_syntax::parse_unit`).

Bir fark ya bilinen bir nedene (issue numarasıyla) uyar ya da test
ADR-0125'in biçiminde (ne oldu, neden, ne yapabilirsin) düşer. Bugün tek
bilinen neden #415'tir: ayrıştırma sırası görünümünde fark var, birim
görünümünde yok. Nedeni çözen PR girdiyi (ve son çağıranı kalkan görünümü)
kaldırır. Bilinen nedeni olmayan bir fark testte susturulmaz.

### 5. Ayıklamanın sonucu

Envanterin "Bilinen ayrışmalar" tablosunun on bir satırının her biri için
en küçük örnek `volt check`, `volt build` (SV, SVA, SDC, XDC, sürücüler)
ve uygulanıyorsa `volt test` ile çalıştırıldı. Sınıflar:

- **sessiz yanlış**: çıktı ya da test sonucu spec'ten farklı ve tanı yok;
- **reddedilen**: bir taraf tanı veriyor, kullanıcı ayrışmaya ulaşamıyor;
- **gizli**: bugün kullanıcının görebileceği bir fark yok.

| # | Bilgi | Sınıf | Görülen |
|---|---|---|---|
| 1 | Genişlik üst sınırı | sessiz yanlış | `bits<65536>` E2025'in aralığında (1..=65536), tip denetimi (`resolve_sized`, 1..=65535) onu sessizce hata tipine çevirir: `bits<8>`'e atama E2001'siz geçer, SV `8'(x)` ile keser. |
| 2 | Sabitte `as` | reddedilen | `as`'lı sabit üreticide E0003 alır; dizi indeksinde tip denetimi kesilmemiş değerle yanlış E2006 verir. Satır içi `x << (9 as u3)` derlenir, SV doğru (1 bit), ama W2013 kesilmemiş değerle "hep 0" der (açık bulgu). |
| 3 | `clog2` ve `~` | reddedilen | genişlikte E2005, değerde E0003. |
| 4 | Yerleşik DEPTH/N | reddedilen | adlı sabit E2008; generic parametre monomorflamada literal olur, iki taraf aynı. |
| 5 | Bool/Trit genişliği | sessiz yanlış | `ShiftRegister<Trit, 4>`'ün `taps`'ı tip denetiminde `bits<4>` (`signal_width` Trit'e `None`, `unwrap_or(1)`), SV'de 8 bit; `bits<4>`'e atama SV'de kesilir, doğru `bits<8>` E2001 alır. Trit `sync()`'i W3003 almaz. |
| 6 | Test dilinde tasarım sabiti | reddedilen | hesaplanan const E8506. |
| 7 | Takma adlı saat/reset | kapandı (#416) | #234'ün örnekleri doğrudan yazılanla aynı tanıyı ve aynı dosyaları verir; kalan #415 reddedilen (E5011, E0019 önerisinde `on <clock>`). |
| 8 | `domain` bildirimi | sessiz yanlış | `reset = none` ardından `reset = sync active_high`: SV ve RDC son alanı alır, senkronizör üretilir; kısıt modeli `any` ile alanı reset'siz sayar, XDC senkronizörün `ASYNC_REG` satırını yazmaz. |
| 9 | Sürücü saati | sessiz yanlış | `@a_clk` (saat portu adı) açıklamalı sinyalden `let` üzerinden `sync()`: XDC `set_max_delay -datapath_only` yerine `set_false_path` yazar; aynı alan `@A` ile yazılınca doğru. |
| 10 | Struct yaprak adı | gizli | SV, SDC ve test dili `p.i.x`'i `p_i_x` yazar; çakışma E1003. |
| 11 | Değer aralığı | gizli | yedi "sığar mı" kopyası 1..=64 bitte aynı sınırı verir; test dilinin bit deseni kabulü ADR-0059'un kuralıdır. |

Dört sessiz yanlış satırın her biri bir arka ucun ya da tip denetiminin
bilgiyi ikinci kez, kendi kuralıyla hesaplamasından gelir; bu ADR'nin
kuralı ve ADR-0136'nın geçişi onların ortak çaresidir. Her biri için issue
metni bu ADR'yi getiren PR'ın açıklamasındadır; issue'ları bakımcı açar.
Reddedilen ve gizli satırların bugünkü davranışı
`crates/volt-driver/tests/divergence_triage_tests.rs`'te sabittir.

## İlke denetimi

1. **Beyan (İ1):** uygulanmaz; kullanıcıdan yeni bir yazım istenmez.
2. **Çıkarım (İ2):** çıkarımın kendisi değişmez; kural, her anlamsal
   bilginin tek cevaplı tek bir çıkarımı olmasını ister. Bugün iki hesabın
   ayrışabildiği yerler (§5) İ2'nin "tek cevaplı" koşulunu bozuyor; gölge
   ve izin listesi bunu ölçer.
3. **Anlam değişikliği (İ3):** yok. Bu ADR'yi getiren PR derleyicinin
   kaynağını değiştirmez (yalnız test ve belge). §3'ün her adımı ayrı bir
   PR'dır ve ADR-0136 §1'deki gibi çıktının aynı kalmasıyla kabul edilir;
   #415'i kapatan adım reddedilen bir girdiyi kabul eder (davranış
   değişikliği, CHANGELOG'a girer). Sessiz yanlış satırların düzeltmesi
   kendi issue'su ve İ3'ün koşullarıyla ele alınır.
4. **Niyet tahmini (İ4):** kural sözdizimsel tahmini kalıcı olarak yalnız
   editör yardımcılarına bırakır; orada bir tahmin yanlışsa yalnız bir
   öneri zayıflar.
5. **Metamorfik yeniden yazım:** uygulanmaz; gölge bir yeniden yazım
   değil, iki hesabın aynı portta karşılaştırılmasıdır.

## Doğrulama

- Derleyici değişmez: PR `crates/*/src/` altında dosya değiştirmez. Main'in
  ve dalın `volt` ikilisi gölgenin girdi dosyalarının her biri için
  `volt build --single-file --emit=sva,sdc,xdc,rust,c,regmap` ile
  çalıştırıldı (ADR-0136'nın yöntemi): çıkış kodları ve üretilen dosyalar
  sürüm başlık satırı dışında aynı (sayılar PR açıklamasında).
- Genişlik gölgesinin yükleyicisi ortak modüle taşındı; testin yazdırdığı
  tablo taşımadan önce ve sonra satır satır aynı.
- Sınıflama gölgesinin yerel koşusu (2026-11-24): 488 dosya ve 25 kitap
  bloğu; derlenen birim 367; portlu öğe 479; port 2736 (saat 397, reset
  36, veri 2303). İki görünümde de 2736 port aynı sınıfı aldı; bilinen #415
  farkı derlemde 0 porttadır. #415'in biçimi (takma ad modülden sonra)
  ayrı bir testte bilinen neden olarak tanınır; yeni farkın ADR-0125
  metni ayrı bir testte sınanır.
- İzin listesi testi bugünkü yedi çağıranı bulur; listede olmayan
  çağıranın, eskimiş girdinin ve gizleyen içe aktarmanın testi düşürdüğü
  ayrı bir testte sınanır.

## Sınırlar

- Ayrıştırma sırası görünümü ayrıştırıcının davranışının bir modelidir,
  ölçümü değildir: geçitlerin AST'de neyi gördüğünü `parse_unit`'in
  sırasından türetir. #415 kapanınca model geçitlerle birlikte kalkar;
  kalkmazsa gölge yanlış olarak #415'in farkını görmeye devam eder ve
  izin listesinde çağıranı kalmamış bir görünüm kalır.
- Gölge yalnız saat/reset sınıflamasını karşılaştırır. Envanterin öteki
  bilgileri (genişlik dışında) için gölge yoktur; §5'in ayıklaması onları
  elle sınadı.
- İzin listesi bir metin taramasıdır: `//` yorumları atılır, çağrı
  içinde bulunduğu en yakın `fn` satırına bağlanır. Makro içinden
  üretilen bir çağrıyı göremez; depoda böyle bir kullanım yoktur.
- Gölge derlenen birimlerin portlarını görür. Derlenmeyen bir birimin
  (ör. #415'in pipeline örneği E5011 ile) portu karşılaştırılmaz.

## Reddedilenler

- **Sözdizimsel sınıflamayı hemen kaldırmak.** Ayrıştırıcının geçitleri
  bugün ad çözümlemeden önce koşar; onları taşımak #415'in seçeneklerinden
  birinin seçilmesini ister (bakımcının kararı) ve çıktısı değişen bir
  iştir. Bu ADR kuralı ve denetimini koyar, taşımayı sıraya bağlar.
- **İzin listesini tutarlılık betiğinde (sh ve ps1) tutmak.** İki betikte
  aynı kuralı iki kez yazmak bu ADR'nin önlemek istediği şeydir; Rust
  testi tek yerdedir ve CI'ın test koşan işlerinde koşar.
- **Gölgeyi kullanıcıya açık bir bayrakla (`--shadow`) sunmak.** Kural
  karşılaştırmanın yalnız testte koşmasıdır (ADR-0136 §4).
- **Bilinen farkları dosya:satır listesi olarak tutmak.** Aynı nedenin her
  yeni örneği listeyi değiştirirdi; neden yapısıyla tanınır, örnek sayısı
  tabloda yazdırılır.
