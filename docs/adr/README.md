# Mimari Karar Kayıtları (ADR) — İndeks

Bu dizin salt okunurdur; yeni karar yeni ADR açar (CLAUDE.md). Belge
öncelik sırasında ADR'ler UX Anayasası'ndan sonra, `docs/spec/`'ten önce
gelir (docs/README.md).

ADR-0001..0022 F0-F2 döneminde alınan ama dosyası açılmayan kararlardır;
2026-09-16'da geriye dönük belgelendi. Numaraları
`docs/design/Volt-Butunlesik-Mimari-v3.md` BÖLÜM VII'deki plan tablosundan
alınmıştır; kod ve sonraki ADR'ler bu numaralara zaten atıf yapıyordu
(ADR-0005 tutarlılık betiğinde, ADR-0007 ADR-0037'de, ADR-0012
`desugar.rs`'te, ADR-0013 dil spesifikasyonunda).

Numara boşlukları: **0030, 0043, 0045, 0046** hiç kullanılmadı — bu
numaralara atıf yapan dosya, commit ya da CHANGELOG girdisi yoktur.

Statü kısaltmaları: **K** = Kabul edildi; **K/G** = Kabul edildi, geriye
dönük belgelendi (2026-09-16); **K/R** = Kabul edildi, yalnız rezervasyon /
kodlar tanımlı, mekanizma V1+.

## Dil tasarımı

| No | Başlık | Statü | Tarih |
|---|---|---|---|
| [0011](ADR-0011-kontrat-sistemi.md) | Kontrat sistemi — `requires` / `ensures` / `invariant` / `cover` | K/G | 2026-09-03 |
| [0013](ADR-0013-gramer-ve-parser-sozlesmesi.md) | Gramer ve parser sözleşmesi — LL(2), geri izleme yasağı, hata kurtarma, öncelik kararları | K/G | 2026-09-03 |
| [0014](ADR-0014-kontrat-kademeli-benimseme.md) | Kontrat sistemi kademeli benimseme — hiçbir kontrat zorunlu değil | K/G | 2026-09-03 |
| [0016](ADR-0016-hook-kontrat-uyumu.md) | Hook mekanizması — kontrat altında politika enjeksiyonu (`hook` rezerve) | K/R | 2026-09-03 |
| [0017](ADR-0017-modul-sistemi-ve-paket-semantigi.md) | Modül sistemi ve paket semantiği — `package` / `use` / `pub` | K/G | 2026-09-03 |
| [0019](ADR-0019-todo-semantigi.md) | `todo!` semantiği — kısmi derleme | K/G | 2026-09-03 |
| [0023](ADR-0023-baglamsal-anahtar-kelimeler.md) | Bağlamsal anahtar kelimeler — `sync` çakışması | K | 2026-09-03 |
| [0028](ADR-0028-ayrilmis-kelime-revizyonu.md) | Ayrılmış kelime revizyonu — stdlib'e taşınanlar serbest | K | 2026-09-08 |
| [0029](ADR-0029-stdlib-gunluk-yapi-taslari.md) | Stdlib genişletmesi — tek saatli günlük yapı taşları | K | 2026-09-08 |
| [0032](ADR-0032-match-sirali-blokta.md) | Sıralı/kombinasyonel blokta `match` ve E0014 | K | 2026-09-09 |
| [0034](ADR-0034-implikasyon-operatoru.md) | İmplikasyon operatörü `->` | K | 2026-09-09 |
| [0038](ADR-0038-pipeline-sozdizimi.md) | Pipeline sözdizimi — `pipeline`, `stage`, `stall`, `flush` | K | 2026-09-10 |
| [0039](ADR-0039-bundle-port-gruplari.md) | Bundle (port grubu) desteği — `struct port` | K | 2026-09-13 |
| [0040](ADR-0040-ardisik-kontratlar.md) | Ardışık kontratlar — `prev()` yerleşiği | K | 2026-09-13 |
| [0044](ADR-0044-mmio-register-haritasi.md) | `@mmio` register haritası — bellek eşlemeli register blokları | K | 2026-09-13 |
| [0050](ADR-0050-handshake-primitifi.md) | `Handshake<T>` — yerleşik tek saatli el sıkışma bundle'ı | K | 2026-09-14 |
| [0051](ADR-0051-cift-yonlu-portlar.md) | Çift yönlü portlar — `inout` yazma desteği ve `opendrain` tipi | K | 2026-09-15 |
| [0069](ADR-0069-ozyineli-tip-ve-generic-struct-port.md) | Özyineli tiplerin tek tip çizgesi denetimi — E4009 her adlandırılmış tipe genişler (struct, struct port, enum payload'ı/temel tipi, takma ad; dizi/demet/generic argüman üzerinden), bundle/Handshake açılımı sonucunu kullanır; generic `struct port` → E0003; issue #21 | K | 2026-09-24 |
| [0068](ADR-0068-tani-katlama-ve-ust-sinir.md) | Açılan kopyalarda özdeş tanı katlama (`for`/mono/bundle dizisi — "reported once; occurs in N …" notu), W0023 + `--max-diagnostics` (1000), açılım düğüm bütçesi `MAX_UNROLL_NODES`; fuzz'ın ikinci bulgusu, ADR-0067 §4'ü düzeltir | K | 2026-09-23 |
| [0067](ADR-0067-bundle-dongu-ve-duzlestirme-butcesi.md) | Özyineli bundle tespiti ve düzleştirme bütçesi — E4009 (kendini içeren `struct port` / Handshake payload'ı), E4010 (4096 düz port / 8 seviye), `tests/fuzz_regressions/`; fuzz'ın bulduğu ilk hata | K | 2026-09-23 |
| [0066](ADR-0066-otomatik-fsm-sayac-kontratlari.md) | Otomatik FSM ve sayaç kontratları — geçiş/durum cover'ı, sınırlı sayaç invariant'ı + sarma cover'ı, `@no_auto_contracts`, her otomatik kontratta "generated from" | K | 2026-09-23 |

## Tip sistemi

| No | Başlık | Statü | Tarih |
|---|---|---|---|
| [0003](ADR-0003-trit-tipi.md) | Trit tipi — kısıtlı i2 ve katmanlı görünürlük | K/G | 2026-09-03 |
| [0062](ADR-0062-trit-sv-eslemesi.md) | Trit SV eşlemesi — `logic signed [1:0]` (01/00/11), `Trit * x` çarpansız seçici, emit tanısı tekilleştirme | K | 2026-09-21 |
| [0007](ADR-0007-zamanlama-seviyeleri.md) | L0/L1/L2 zamanlama seviyeleri | K/G | 2026-09-03 |
| [0018](ADR-0018-lineer-tipler-opt-in.md) | Lineer tipler — opt-in (`&inv`), normal portlarda sürücü analizi | K/R | 2026-09-03 |
| [0025](ADR-0025-esnek-genislik-ve-w2013.md) | Aritmetik sonuçlarda esnek genişlik aralığı ve W2013 | K | 2026-09-03 |
| [0031](ADR-0031-keyfi-bit-genisligi.md) | Keyfi bit genişlikli tamsayı tipleri (u1..u64 / i1..i64) | K | 2026-09-09 |
| [0035](ADR-0035-degisken-indeksleme.md) | Değişken dizi indeksi ve indexed part-select | K | 2026-09-10 |
| [0036](ADR-0036-isaretli-kaydirma-ve-cast.md) | İşaretli kaydırma (`>>>`) ve `$signed`/`$unsigned` üretimi | K | 2026-09-10 |
| [0037](ADR-0037-l1-zamanlama.md) | L1 zamanlama seviyesi — `Delayed<T, N>`, `delay<K>` ve `@strict_timing` | K | 2026-09-10 |
| [0041](ADR-0041-ayni-isaret-genisleme.md) | Aritmetik ergonomisi — aynı-işaret genişleme, const diziler, generic örnekleme | K | 2026-09-13 |

## Domain ve CDC

| No | Başlık | Statü | Tarih |
|---|---|---|---|
| [0002](ADR-0002-domain-semantigi.md) | Domain semantiği — `@Domain` anotasyonu ve birleşik domain (saat + sıfırlama + güç + güven) | K/G | 2026-09-03 |
| [0020](ADR-0020-guvenlik-akis-denetimi.md) | Güvenlik akış denetimi — `trust_level` domain'in dördüncü boyutu | K/G | 2026-09-03 |
| [0027](ADR-0027-stdlib-mimarisi.md) | Stdlib mimarisi — CDC primitifleri derleyicide yerleşik | K | 2026-09-08 |
| [0047](ADR-0047-extern-domain-anotasyonu.md) | Extern modül sınırında domain anotasyonu — sembolik saat alanları | K | 2026-09-14 |
| [0049](ADR-0049-domain-aware-bellek.md) | Domain-aware bellek — `AsyncDualPortRam<T, DEPTH>` | K | 2026-09-14 |
| [0052](ADR-0052-guven-seviyeleri.md) | Güven seviyeleri — `trust_level`, bilgi akışı denetimi (E3009) ve `declassify` | K | 2026-09-15 |
| [0065](ADR-0065-rdc-ve-hedefli-sdc.md) | RDC denetimi ve hedefli SDC — ham `reset` portu + üretilen bırakma senkronizörü, E3003 etkin (R5/R6), W3xxx-A/B, `set_clock_groups` yerine hedefli kısıtlar, `--sdc-style` | K (tasarım; uygulama Aşama 2–4) | 2026-09-22 |

## Kod üretimi

| No | Başlık | Statü | Tarih |
|---|---|---|---|
| [0008](ADR-0008-sv-cikti-stili.md) | SV çıktı stili — `always_ff`, açık genişlik, yasak liste (x/z üretilmez) | K/G | 2026-09-03 |
| [0010](ADR-0010-arka-uc-circt-ertelendi.md) | Arka uç — CIRCT dialect seçimi ertelendi, doğrudan SystemVerilog üretimi | K/G | 2026-09-03 |
| [0012](ADR-0012-sv-cikti-ongorulebilirlik.md) | SV çıktı öngörülebilirlik garantisi — 1:1 modül, isim korunumu, ECO uyumu | K/G | 2026-09-03 |
| [0024](ADR-0024-sv-cikti-adlandirma.md) | SV çıktı dosyası adlandırması — DECLFILENAME çakışması | K | 2026-09-03 |
| [0026](ADR-0026-uretilen-kod-dili.md) | Üretilen kodun dili — her zaman İngilizce | K | 2026-09-08 |
| [0053](ADR-0053-hw-sw-koprusu.md) | HW-SW köprüsü — `@mmio` haritasından sürücü, başlık, `regmap.json` ve belge üretimi | K | 2026-09-15 |
| [0054](ADR-0054-sdc-uretimi.md) | SDC/XDC üretimi — `@timing` uygulanıyor, zamanlama kısıtları domain bilgisinden | K (ADR-0048 Bölüm 2'yi gerçekler; §2/§6 ADR-0065 ile güncellendi) | 2026-09-16 |
| [0057](ADR-0057-sv-operator-onceligi.md) | SV üretiminde operatör önceliği — parantez kararı IEEE 1800 tablosuyla (sessiz yanlış derleme düzeltmesi) | K | 2026-09-20 |

## Araç zinciri

| No | Başlık | Statü | Tarih |
|---|---|---|---|
| [0004](ADR-0004-tani-sozlesmesi.md) | Tanı sözleşmesi — hata kodu formatı, 5 parçalı mesaj, tanı dili | K/G | 2026-09-03 |
| [0009](ADR-0009-test-dosya-formati.md) | Test dosya formatı — `tests/ui/{pass,fail}`, `//~` anotasyonları, birebir fixture | K/G | 2026-09-03 |
| [0021](ADR-0021-artifact-uretim-ve-cli-sozlesmesi.md) | Artifact üretim mimarisi ve CLI sözleşmesi — tek kaynak, `build/` dizini, çıkış kodları | K/G | 2026-09-03 |
| [0022](ADR-0022-semver-donanim-kurallari.md) | SemVer donanım kuralları — `@version`, `@abi_version`, anlamsal diff (rezerve) | K/R | 2026-09-03 |
| [0033](ADR-0033-test-bloklari-ve-simulasyon.md) | Test blokları ve Verilator simülasyon köprüsü | K | 2026-09-09 |
| [0058](ADR-0058-test-dili-genisletme.md) | Test dili genişletme — yerel değişken, dizi, çalışma zamanı `for`, `read_hex`, `load` (E8507–E8511) | K | 2026-09-20 |
| [0059](ADR-0059-test-port-genislik-kontrolu.md) | Test bloğunda port genişlik denetimi — sabit değer E8512, hesaplanmış değer çalışma zamanında testi düşürür | K | 2026-09-20 |
| [0060](ADR-0060-test-sabit-yayilimi.md) | Test bloğunda sabit yayılımı — sabit `let` ve literal üst düzey `const` E8512'ye derleme zamanında ulaşır; port okuması/döngü sayacı koşuda denetlenir | K | 2026-09-20 |
| [0042](ADR-0042-coklu-dosya-derleme.md) | Çoklu dosya derleme ve import sistemi | K | 2026-09-13 |
| [0063](ADR-0063-regmap-tutarlilik-denetimi.md) | Register haritası tutarlılık denetimi — `volt check-regmap --against` (bayat/elle düzenlenmiş sürücü), `build --check-regmap` (sürücü ↔ SV), `regmap-hash` imzası, E9003/E9004 | K | 2026-09-21 |
| [0064](ADR-0064-simulasyonda-kontratlar.md) | Simülasyonda kontratlar — `volt test` izleyicileri (immediate kalıbı + DPI), assume ihlali testi düşürür (ayrı sınıf), cover özeti, `--no-contracts` / `volt run --contracts` | K | 2026-09-21 |
| [0061](ADR-0061-volt-toml-arama-tavani.md) | Volt.toml aramasına tavan — git kökü (kapsayıcı) + ev dizini (dışlayıcı), `VOLT_MANIFEST_DIR` geçersiz kılar | K | 2026-09-21 |
| [0048](ADR-0048-zamanlama-kisitlari.md) | Zamanlama kısıtları — uygulanmayan nitelikler W0021 üretir, SDC üretimi V1 planı | K (2. bölüm ADR-0054 ile gerçeklendi) | 2026-09-14 |

## Altyapı

| No | Başlık | Statü | Tarih |
|---|---|---|---|
| [0001](ADR-0001-lisans-politikasi.md) | Lisans politikası — Apache-2.0 OR MIT | K/G | 2026-09-03 |
| [0005](ADR-0005-workspace-crate-sinirlari.md) | Workspace crate sınırları ve araç zinciri (Rust, logos, codespan-reporting) | K/G | 2026-09-03 |
| [0006](ADR-0006-arena-tabanli-ast-hir.md) | Arena tabanlı AST/HIR — `Idx<T>` handle deseni, tip bilgisi HIR'da | K/G | 2026-09-03 |
| [0015](ADR-0015-determinizm-garantisi.md) | Determinizm garantisi kapsamı — aynı kaynak, byte-aynı çıktı | K/G | 2026-09-03 |
