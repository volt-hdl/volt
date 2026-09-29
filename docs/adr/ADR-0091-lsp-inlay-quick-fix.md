# ADR-0091: LSP — Inlay İpuçları, Quick Fix ve Protokol Testleri

> Statü: Uygulandı — çok dosyalı ipuçları eki (2026-09-29) sonda
> İlgili: ADR-0070 (tanı paritesi: LSP = check), ADR-0037 (Delayed / gecikme), ADR-0068 (katlama), ADR-0088 (bildirimde alan açıklaması), cli-contract.md §5 (JSON `suggestions`).
> Tarih: 2026-09-27
> Etkilenen: volt-lsp (`inlay.rs`, `code_action.rs` — YENİ; `lib.rs`
> inlayHint/codeAction/didChangeConfiguration işleyicileri, `service()`,
> `hint_config_from`; `analysis.rs` `analyze_editor` ayrımı, birim
> tanısında öneri span'inin ana dosyaya taşınması), volt-hir
> (`timing.rs` `analyze_timing`/`TimingResult`, `pipeline.rs`
> `SemanticStages::delays`; `typeck` W2012 önerisi), volt-syntax
> (E0004, W0010, E4008 önerileri), volt-driver testleri
> (`quickfix_tests.rs` YENİ, `lsp_protocol_tests.rs` kapanış + yeni
> testler), tests/quickfix (YENİ), editors/vscode (ayarlar)
> Ek: Çok dosyalı ipuçları (2026-09-29, #67) — uygulama notu, yeni karar
> değil: ipuçları tanılarla aynı birim analizinden; fn gövdesi `let`'leri.

## Sorun

Editör yalnız tanı ve hover gösteriyordu. Volt'un çıkardığı genişlik,
saat alanı ve gecikme bilgisi yazarken görünmüyordu; tanıların çoğunun
"çözüm" satırı yalnız metindi. Kapsam ölçümünde volt-lsp en zayıf
crate'ti: `lib.rs` %0, `docs.rs` %40, `symbols.rs` %66, `hover.rs` %67.

## 1. Envanter (önce ölç)

### 1a. volt-lsp kapsamı ve `lib.rs` %0'ın kök nedeni

Ölçüm: `cargo llvm-cov -p volt-lsp` + `-p volt-driver --test
lsp_protocol_tests`, rapor volt-lsp dosyalarıyla sınırlı
(`build/lsp/cov.sh`).

`lib.rs` testsiz DEĞİLDİ: protokol testleri (`lsp_protocol_tests.rs`)
gerçek `volt lsp` alt sürecini stdio'dan sürüyor ve geçiyordu. Profil
verisi alt süreçte de toplanır ama **süreç çıkışında** yazılır.

1. Testlerin `Drop`'u alt süreci `kill` ediyordu → profil hiç yazılmadı.
2. Yalnız `shutdown` + `exit` göndermek YETMEDİ (ölçüm, `lib.rs` yine
   %0): tower-lsp 0.20'nin `Server::serve` döngüsü `exit`ten sonra ancak
   stdin EOF'unda ya da bir SONRAKİ mesajda döner. stdin açık kalınca
   süreç asılı kalır (`build/lsp/exit_probe.py`: stdin açık → 5 sn'de
   öldürüldü; kapalı → 0,02 sn'de çıkış 0). Çalışma zamanını
   `shutdown_background` ile kapatmak işe yaramadı (döngü dönmüyor);
   geri alındı.
3. Düzeltme testte: `shutdown` → `exit` → stdin'i kapat (EOF) → bekle
   (zaman aşımında `kill`). Yalnız bu değişiklikle `lib.rs` %0 → %64
   (satır) ve protokol testlerinin süresi 6,6 sn → 1,7 sn.

Kapanışta bir sunucu hatası daha bulundu (Karar 3.2): istemciye
yanıtsız kalan `workspace/inlayHint/refresh` isteği kapanışı 5 sn
bekletiyordu.

### 1b. Tanı önerileri: kesin olan / olmayan

335 tanı üretim noktası tarandı. Öneri (`Suggestion`) zaten yapısal bir
alandı (cli-contract §5 `suggestions` + `applicability`) ama yalnız 5
noktada doluydu: E0006/E0007 (`machine-applicable`) ve benzer ad
tahmini E1001/E1009 (`maybe-incorrect`).

**Kesin** = tek, tam belirli metin düzenlemesi; anlamı ya korur ya da
tek bir yazım yanlışını düzeltir; seçenek, yer tutucu ya da kullanıcı
değeri yok. Her biri Karar 2'deki uygulama testinden geçer.

| Kod | Öneri | Kesin mi | Neden |
|---|---|---|---|
| E0006 | `on` bloğunda `=` → `<=` | EVET (vardı) | Bağlam tek operatöre izin verir |
| E0007 | `comb` bloğunda `<=` → `=` | EVET (vardı) | Aynı |
| E0004 | `} module Blnk` → `} module Blink` | EVET (yeni) | Sonlandırıcı yalnız addır; modül adı zaten bellidir |
| W0010 | `a & b \| c` → `(a & b) \| c` | EVET (yeni) | Mevcut yorumu yazar, anlam değişmez |
| W2012 | `let k = 5` → `let k : i32 = 5` | EVET (yeni) | Derleyicinin varsaydığı tipi yazar, anlam değişmez |
| E4008 | `io.read` → `io.read()` | EVET (yeni) | Yöntemin tek biçimi |
| E1001/E1009 | benzer ad (`dta` → `data`) | HAYIR | Levenshtein tahmini; `maybe-incorrect` kalır |
| E1001 desen | `Idle` → `State::Idle` | HAYIR | Birden çok enum'da aynı varyant olabilir (ilki seçiliyor) |
| E0003 ayrılmış ad | `fsm` → `fsm_` | HAYIR | `fsm_` zaten var olabilir (E1003); tüm kullanımlar ayrı ayrı |
| E1013 | `name` → `name_` | HAYIR | Yalnız bildirimde raporlanır; kullanımlar da yeniden adlandırılmalı (çok noktalı) |
| E0014 | eksik kol iskeleti `_ => { }` | HAYIR | `comb`'da boş kol atanmamış sinyal/latch hatası üretir; ifade biçimi `<value>` yer tutucu |
| E3001/E3012 | `sync(...)` / AsyncFifo | HAYIR | Seçim (tek bit/çok bit, FIFO, el sıkışma) |
| E3010 | `y : <type> @Fast` | HAYIR | Yer tutucu; aday alanlardan ilki keyfi |
| W3002 | `sync(x)` → `x` | HAYIR | Senkronizörün iki çevrim gecikmesini de kaldırır — davranış değişir |
| W1001/W1004 | `_ad` öneki | HAYIR | Yazılan sinyal/port için çok noktalı; giriş portunda üst modül bağlaması kırılır |
| E2001 daraltma | `(x) as u4` | HAYIR | Bit atar; niyet belli değil |
| E2028 ters aralık | `for i in e..s` | HAYIR | Yarı açık aralıkta yinelenen değerler değişir |
| E5011 | `pipeline(N)` = aşama sayısı | HAYIR | Eksik aşama da olabilir (seçim) |
| E2030 | taban `u{gerekli}` | HAYIR | Yardım "ya da kaldırın" seçeneği sunar |
| E0013 | dosya sonuna `*/` | HAYIR | Kullanıcı büyük olasılıkla daha önce kapatmak istedi |
| E0002 | eksik `)`/`}` ekle | HAYIR | Ekleme konumu belirsiz (hata kurtarma konumu) |

Eklenmesi mümkün ama bu ADR'de yapılmayan, tek değerli seyrek
durumlar (E0009 `bus = AXI4Lite`, `@source`'un Volt modülünden
silinmesi, E0001 yinelenen `package`) ölçütü karşılar; kullanım
görülünce eklenir.

### 1c. HIR'de ipucu için hazır bilgi

| Bilgi | Kaynak | Açık mı |
|---|---|---|
| Çıkarılan tip/genişlik | `TypeckResult::def_types` + `TypeArena::display_named` | Evet (hover zaten kullanıyor) |
| Saat alanı | `DomainResult::signal_domains` (`DomainId::Explicit`) + `domains[i].name` | Evet |
| Gecikme | `timing.rs` `ModuleTiming::delays` | HAYIR — yerel; `analyze_timing` ile açıldı (davranış aynı) |

## 2. Karar — inlay ipuçları

Üç tür, her biri ayrı ayarla kapatılabilir:

| Tür | Nerede | Etiket |
|---|---|---|
| Tip | tipsiz `let` / `reg` (modül ve blok içi) | `: u9` |
| Saat alanı | yalnız ≥ 2 farklı açık alan taşıyan modülde; `@Alan` yazılmamış port/`reg`/`let`/`wire` | `@Fast` |
| Gecikme | yalnız `@strict_timing` modülde; `Delayed<…>` yazılmamış, sabit gecikmeye oturan tanım | `+2` |

Konum: yazılmış tipin sonu, yoksa adın sonu — `reg r : u8 @Fast +1 = 0`
sırası ADR-0088 yazımıyla aynıdır. (`wire` tipi zorunlu olduğundan tip
ipucu almaz.)

**İkinci çıkarım yok (ADR-0070).** Bilgi, `volt check`'in koştuğu
`run_semantic_stages` ürünlerinden okunur. Gecikme için `check_timing`
ince bir sarmalayıcıya dönüştü: `analyze_timing` aynı sabit nokta
hesabını yapar, tanılara ek olarak kesin (`Exact`) gecikmeleri döndürür;
`SemanticStages::delays` `domain` ile aynı kapıdan geçer.

**Güven kuralı — yanlış ipucu, ipucusuzluktan kötüdür:**

1. Editör boru hattında HATA bulunan modülde hiç ipucu yok (hatanın
   birincil span'i modülün içinde). Ayrıştırma ya da çözümleme hatası
   aşamaları zaten kapılar: dosyada hiç ipucu kalmaz. Modül içi tip
   hatası yalnız kendi modülünü susturur; komşu temiz modül ipucu alır.
   Hata modül DIŞINDAYSA (struct, enum, fn, const, domain ya da
   üretilmiş dosya) tüm dosya susar: her modül bunlara bağımlı olabilir
   (mutasyonla bulundu, §5).
2. Tipi `Error` ya da boyutsuz literal (`IntLit`) içeren tanıma tip
   ipucu yok.
3. Açılmış kopyalar (generic örneklemeler, `for`) aynı konuma farklı
   etiket üretirse o konumda ipucu yok (`Pass<4>` + `Pass<8>` → `s`
   `u4`/`u8` → ipucu yok; iki `Pass<4>` → tek `: u4`).
4. Kaynak metni bildirim adıyla uyuşmayan (desugar üretimi) tanıma
   ipucu yok.

**Görünür aralık:** yalnız istenen aralıkla kesişen modüller ve
bildirimler için ipucu kurulur. Analizin kendisi dosya bütünüdür (tip
ve alan çıkarımı modül sınırında yereldir ama `check` ile aynı aşamayı
koşmak için tüm dosya çözümlenir).

**Hafif analiz yolu:** ipucu (ve hover, tamamlama, tanıma git,
semboller) artık `analyze_editor`'ı çağırır: tek dosya boru hattı,
tanı yolu (birim yükleme + çıktısız SV emit + toplayıcı) KOŞMAZ. Bu
isteklerin hiçbiri yayımlanan tanıları okumuyordu; sonuçlar aynıdır.

**Ayarlar:** `initializationOptions.inlayHints` ya da
`workspace/didChangeConfiguration` `{"volt": {"inlayHints": {"types",
"clockDomains", "latency"}}}`; eksik/bool olmayan anahtar mevcut değeri
korur.

## 3. Karar — quick fix (code action)

1. **Veri tanıda yapısal olarak taşınır** — yeni alan yok: mevcut
   `Diagnostic::suggestions` (`span`, `replacement`, `applicability`).
   LSP metin ayrıştırmaz.
2. **"Kesin" filtresi LSP'de tek yerde** (`code_action::certain_fix`):
   yalnız `machine-applicable` ve tanı başına TEK böyle öneri →
   `quickfix` (`isPreferred`). `maybe-incorrect`, `has-placeholders`,
   `unspecified` ve iki kesin seçenek quick fix olmaz; veri tanıda ve
   JSON'da kalır.
3. Code action tanıları yayımlananla AYNI yoldan hesaplar (`analyze`,
   `volt check` yolu). Birim tanısı ana dosyaya taşınırken başka
   dosyaya düşen öneri atılır (önceden öneri span'i taşınmıyordu).
4. **JSON kararı: görünür, cli-contract DEĞİŞMEZ.** `suggestions`
   cli-contract §5 şemasında zaten vardır ve "machine-applicable →
   otomatik uygulanabilir (LSP quick-fix)" anlamı orada tanımlıdır; yeni
   öneriler bu alanda kendiliğinden görünür. İnsan çıktısı öneriyi
   basmaz (golden bayt bayt aynı); help metinleri değişmedi.
5. **Zorunlu uygulama testi** (`quickfix_tests.rs`, gerçek ikili): her
   `tests/quickfix/` fikstüründe `volt check --format=json` →
   tek `machine-applicable` önerinin bayt aralığına uygula → yeniden
   `volt check`: o kodun sayısı bir azalmalı ve (kod, önem, mesaj)
   kimliğiyle hiçbir YENİ hata çıkmamalı. Kesin liste testte sabittir.
   Uyarı açığa çıkabilir: ayrıştırma hatası sonraki aşamaları kapılar
   (ADR-0070); düzeltme kapıyı açınca o aşamaların uyarıları ilk kez
   görünür (E4008 → W3007 ölçüldü). W2012 düzeltmesi aynı satırdaki
   E2005'i de giderir.

## 4. Karar — sunucu kapanışı ve yenileme

1. Test kapanışı Karar 1a.3. tower-lsp'nin `exit` sonrası EOF beklemesi
   kütüphane davranışıdır; gerçek istemciler (VS Code) `exit`ten sonra
   boruyu kapatır. Sunucuya geçici çözüm eklenmedi.
2. `workspace/inlayHint/refresh` isteği yalnız `initialize`'da
   `workspace.inlayHint.refreshSupport` bildiren istemciye gönderilir.
   Önceki taslakta her istemciye gidiyordu; yanıt vermeyen istemcide
   `didChangeConfiguration` işleyicisi (ve kapanış) asılı kalıyordu
   (ölçüm: protokol testi 1,6 sn → 5,0 sn).

## 5. Doğrulama

- Testler: `volt-lsp/tests/inlay.rs` (14), `code_action.rs` (6),
  `server.rs` (9, işleyiciler süreç içi: initialize, didOpen/didChange/
  didSave/didClose, pull tanı, inlayHint, codeAction, hover,
  documentSymbol, tamamlama, tanım, yapılandırma), `open_paths.rs` (6,
  anahtar kelime belgeleri, tüm öğe sembolleri, tanım türü etiketleri,
  negedge/bildirim alanı, timeless, const genişlikli struct düzeni);
  `volt-driver/tests/quickfix_tests.rs` (3), `lsp_protocol_tests.rs`
  (+4: yetenekler, ipucu konumu/türü kablo üzerinde, ayarlar,
  code action); `volt-hir/tests/timing_tests.rs` (+2). Sayım 3442 →
  3478.
- Golden (`build/lsp/golden.py` + `golden_cmp.py`; main 987bf52 ile bu
  dal, release, `tests/ui` + `tests/fixtures` + `examples` + `templates`
  + `tests/quickfix`, 593 dosya): insan `check` çıktısı 0 fark, `build
  --emit=sva` çıktıları 0 fark; JSON'da `suggestions` dışında 0 fark,
  yeni öneriler W2012 ×2, E0004, E4008, W0010.
- Mutasyon (`build/lsp/mutate.py`, tek tek): 13/13 mutant öldü. İpucu kaynağı: tip (`def_types` yerine
  ilk tip), alan (hep ilk alan), gecikme (`n + 1`); quick fix
  uygulayıcı: splice, LSP `TextEdit` metni, W2012 ve E0004 üreticileri;
  "kesin değil" filtresi: applicability denetimi kalkınca, tek-öneri
  şartı kalkınca; bastırma: modül içi hata, modül dışı hata, güvenilmez
  tip filtresi, uyuşmayan kopyalar. İlk turda iki mutant YAŞADI ve
  testler güçlendirildi: W2012 `: i8` önerisi fikstürdeki değer i8'e
  sığdığı için geçiyordu (değer 100000 yapıldı: i8'de E2010); modül
  dışı hata kuralı yoktu — bozuk `struct`'a bağımlı modül `q : Bad`
  ipucu alıyordu (kural eklendi, Karar 2.1). `IntLit` filtresi bugün
  savunmadır: tip denetçisi tanıma `IntLit` yazmaz (W2012 i32'ye
  çevirir); `Error` dalı gerçektir (Leaf port genişliği hatası → Top'ta
  `leaf.b`).
- Yanıt süresi (`build/lsp/perf.py`, release, medyan): 

  | Dosya | Satır | Tanı (pull, `check` yolu) | inlayHint tüm dosya | inlayHint 60 satır | İpucu |
  |---|---|---|---|---|---|
  | `examples/soc/top.volt` | 176 | 11,1 ms | 1,3 ms | 1,0 ms | 0 (çok dosyalı, Sınırlar) |
  | `examples/riscv_core.volt` | 510 | 6,6 ms | 2,1 ms | 1,7 ms | 66 |

  N = 15, stdio üzerinden istek-yanıt (Windows, release). İpucu tanı
  yenilemesinden soc'ta 8,5×, riscv_core'da 3× hızlıdır: tanı yolu
  birim yükleme + çıktısız SV emit koşar, ipucu yolu koşmaz.
- volt-lsp kapsamı (satır): 

  | Dosya | Önce | Sonra |
  |---|---|---|
  | `lib.rs` | %0,0 | %99,0 |
  | `docs.rs` | %40,0 | %100 |
  | `symbols.rs` | %65,9 | %99,2 |
  | `hover.rs` | %66,9 | %87,5 |
  | `analysis.rs` | %81,6 | %92,2 |
  | `completion.rs` | %81,1 | %82,5 |
  | `definition.rs` | %72,0 | %72,0 |
  | `convert.rs` | %96,4 | %96,4 |
  | `inlay.rs` (yeni) | — | %86,3 |
  | `code_action.rs` (yeni) | — | %95,4 |
  | **volt-lsp** | **%68,7** (861/1253) | **%90,1** (1468/1630) |

## Sınırlar

- Çok dosyalı birimde (`use`) editör verisi tek dosya analizinden
  gelir (ADR-0070 §3): içe aktarılan ad çözümlenemeyince aşamalar
  kapılanır ve dosyada ipucu yoktur. Tanılar ve quick fix birim
  yolundandır, etkilenmez.
- Fonksiyon gövdesindeki `let`'ler ipucu almaz (yalnız modüller).
- Saat alanı ipucu yalnız açık alanı (`DomainId::Explicit`) gösterir;
  `timeless` ve çözülemeyen alan ipucu almaz.
- VS Code'da elle deneme bu çalışmada yapılamadı (grafik oturum yok);
  eklenti yalnız `tsc` ile derlendi. Protokol davranışı gerçek stdio
  testleriyle doğrulandı.

## Ek — Çok dosyalı ipuçları (2026-09-29, #67)

Uygulama notu; Karar 2'nin (bilgi `volt check`'in kendi analizinden,
güven kuralı) çok dosyalı birime uygulanmasıdır, yeni karar değildir.
Sınırlar'ın ilk iki maddesini kapatır.

**Kök neden.** `inlayHint` işleyicisi `analyze_editor` kullanıyordu: tek
dosya ayrıştırması + `resolve_file`. `use` hedefleri yüklenmediği için
içe aktarılan adlar çözülmüyordu; ADR-0070'ten beri tanılar ise birim
yolundan (`unit_load` + `resolve_unit`) geliyordu — iki yol ayrışmıştı.
Ölçülen etkiler (release, `main` 34fac38f):

- `examples/soc/top.volt`: birim tanıları 0; tek dosya analizinde
  `r.data.resp`'te (166:5, içe aktarılan `AxiRData` alanı) bir hata →
  SocTop modülü susuyordu.
- `examples/vga/vga_top.volt`: tek dosya analizinde hata YOK ama
  `@SysDomain`/`@PixDomain` (vga_timing.volt) açık alan olarak
  çözülmüyordu → modül çok saatli sayılmıyor, 15 alan ipucunun hiçbiri
  çıkmıyordu. Sessiz kayıp: hata olmadığı için güven kuralı da devreye
  girmiyordu.
- `examples/riscv_core.volt`: `riscv_imm`/`riscv_alu`'dan içe aktarılan
  fn çağrılarının tipi hata kurtarma tipiydi → 12 `let` ipucusuzdu
  (`is_reliable` doğru olarak eliyordu).

**Uygulama.** `analysis.rs`'te birim yolu tek fonksiyondur (`run_unit`):
birim yükleme → ön denetim → import → `@source` → ortak boru hattı →
(`validate_emit` ise) çıktısız emit. Tanılar (`unit_diagnostics`,
`validate_emit = true`) ve ipuçları (`analyze_hints`, `false`) bunu
paylaşır; ikinci bir birim yükleme yolu yoktur. İpucu yolu çıktısız emiti
koşmaz (Karar 2: emit bilgisi ipucuya girmez). `analyze_hints`'te
`ast`/`map` birimin tamamı, `file_id` ana dosyadır; birim yüklenemezse
(bağımlılık okunamadı) tek dosya analizi — tanı yolundaki geri dönüşle
aynı. Hover, tamamlama, semboller ve tanım `analyze_editor`'da kaldı (bu
ekin kapsamı dışı). volt-hir'de değişiklik gerekmedi (`LoadedUnit`,
`SemanticStages` zaten dışa açık).

**Güven kuralı birimde.** İpucu yalnız açık dosyanın kendi öğelerine
konur (`item.span.file == file_id`; ad ayrıca `written_as` ile ana
dosyada doğrulanır). Başka dosyadaki (`use` hedefi) hata "modül dışı
hata" sayılır ve dosyanın tamamını susturur — `in_some_module` hatanın
dosya kimliğine bakar; bayt ofseti açık dosyadaki bir modülün aralığına
düşse de. Birim yolunda ön denetim/import/`@source` hatası anlamsal
aşamaları kapılar (`volt check` ile aynı) → çözüm yoksa ipucu da yok;
tek dosya yolunda bu hatalar yalnız kendi modülünü susturuyordu.

**fn gövdesi (Sınırlar madde 2 kapandı).** Gövde tanımda bir kez tip
denetlenir ve `let` tipleri `def_types`'a modül `let`'iyle aynı yoldan
yazılır (`typeck/func.rs` `check_fn` → `handle_let`); ek analiz
gerekmedi. fn saf kombinasyonel olduğundan (ADR-0081 Karar 1) yalnız tip
ipucu; alan ve gecikme yok. Generic fn dilde hâlâ E0003 (ADR-0081 Karar
7) — açılmış kopya sorusu doğmuyor. fn modül dışı öğedir: gövdesindeki
hata dosyanın tamamını susturur (mevcut kural).

**Ölçüm.** Yanıt süresi `build/lsp/perf.py` (release, stdio, N = 15,
medyan, Windows); ipucu farkı `build/lsp-mf/hint_dump.py` (iki ikili,
satır + etiket):

| Dosya | Satır | inlayHint tüm (önce → sonra) | inlayHint 60 satır | Tanı (pull) | İpucu (önce → sonra) |
|---|---|---|---|---|---|
| `examples/soc/top.volt` | 176 | 1,3 → 8,8 ms | 1,1 → 8,8 ms | 11,3 → 11,6 ms | 0 → 0 |
| `examples/vga/vga_top.volt` | 134 | 0,7 → 2,0 ms | 0,6 → 1,9 ms | 2,6 → 2,8 ms | 0 → 15 |
| `examples/riscv_core.volt` | 510 | 2,2 → 3,7 ms | 1,7 → 3,2 ms | 6,7 → 6,4 ms | 66 → 78 |
| `examples/soc/uart.volt` | 101 | 0,9 → 3,3 ms | 0,8 → 2,8 ms | 4,6 → 4,2 ms | 0 → 0 |

Kalkan ipucu 0; eklenenlerin hepsi yukarıdaki kök nedenin satırları
(vga 15 alan, riscv_core 12 tip). İpucu artık birim yükleme + anlamsal
aşamaları koşar, çıktısız emit hariç: en kötü durumda (soc)
tanı yenilemesinin ~%76'sı. Önbellek eklenmedi: 8,8 ms bir tuş
vuruşunun bütçesinin altında; gerekirse tanı ve ipucu aynı `run_unit`
sonucunu paylaşabilir.

`soc/top.volt` diskteki hâliyle ipucu adayı taşımaz: tek saat (alan
ipucu yok), tipsiz `let`'lerin hepsi modül örneği (`StmtKind::Instance`,
Karar 2 kapsamı dışı), kalanlar tipli. Düzeltmenin soc'taki kanıtı
birimin hatasız analizi ve editör tamponunda örnek çıkışını okuyan
tipsiz `let`'in içe aktarılan tipi almasıdır (`resp : u2`, `rdata : u32`
— BusDecoder'dan; tek dosya yolunda 0).

**Test.** `crates/volt-lsp/tests/inlay.rs`: mevcut 14 test artık
`analyze_hints` yolundan (değişmeden geçti); yeni: vga alan ipuçları
(5 × SysDomain, 10 × PixDomain), soc hatasız + tampon tipi, ipucunun
açık dosyada kalması (geçici iki dosyalı birim, `lib.volt`'taki `let`
ipucu almaz), `use` hedefindeki hatanın dosyanın tamamını susturması
(hata ofseti Top'un aralığında; bağımsız Other modülü de susar), fn
gövdesi tip ipucu, fn gövdesi hatasının susturması.
`crates/volt-driver/tests/lsp_protocol_tests.rs`: gerçek stdio üzerinden
vga_top.volt ipuçları.

**Mutasyon** (`build/lsp-mf/mutate.py`, tek tek, dosya geri yüklenir):
8/9 öldü — işleyici tek dosya analizine döner, `analyze_hints` birimi
kullanmaz, birim hataları ipucu analizine taşınmaz, modül dışı hata
kuralında dosya kimliği kalkar, fn atlanır, fn `let`'i toplanmaz, tanı
yolu emit doğrulamasını atlar (parity testleri), ana dosya yerine ilk
dosya. YAŞAYAN: öğe dosya süzgeci (`item.span.file != file_id`) —
eşdeğer mutant: başka dosyadaki ad `written_as`'ta zaten elenir; süzgeç
diğer dosyaların modüllerinde bildirim toplamayı ve saat alanı sayımını
atlatan bir maliyet korumasıdır.

**Kalan sınırlar.** Hover, tanıma git, tamamlama ve semboller tek dosya
analizinde (içe aktarılan adın hover'ı yok). Modül örneği `let`'ine tip
ipucu yok (Karar 2). Saat alanı ipucu yalnız `DomainId::Explicit`
(değişmedi).
