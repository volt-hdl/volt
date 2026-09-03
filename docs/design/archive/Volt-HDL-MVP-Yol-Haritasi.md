> UYARI: Bu belge geçersizdir.
> Güncel sürüm: docs/design/ altında.

# Volt HDL — MVP Geliştirme Yol Haritası (6–12 Ay)

> **Amaç:** Spesifikasyonun **Core** uyumluluk seviyesini (L0/L1 dahil, L2 hariç)
> uçtan uca çalışır kılan bir MVP. Hedef: bir Volt kaynağını → tip-denetimli →
> CIRCT üzerinden sentezlenebilir SystemVerilog'a çeviren ve yerleşik simülatör +
> LSP ile gerçek bir FPGA tasarımını koşturabilen referans araç zinciri.

---

## 1. MVP Kapsamı (Net Sınırlar)

### 1.1 MVP'ye Dahil (In Scope)

- **Dil çekirdeği:** modül, port, `reg`/`wire`, `on D`, `if`/`match`, `let`,
  enum/struct/bundle, `newtype`, const-jenerikler, tip parametreleri.
- **Tip sistemi:** genişlik + işaretlilik + saat alanı + birim; genişleyen/wrapping/
  doygunluklu aritmetik; tek-sürücü, latch önleme, birleşimsel döngü kontrolü.
- **CDC:** `sync_2ff`, `gray_fifo`, `handshake` primitifleri + alan-tutarlılık denetimi.
- **Zamanlama:** L0 (pipeline hizalama) + L1 (`Delayed<τ,d>`).
- **Birinci sınıf yapılar:** `fsm`, `pipeline`, `Stream`/`Flow`.
- **Arka uç:** Rust önyüz → CIRCT (Calyx/Handshake/FIRRTL) → okunabilir SystemVerilog.
- **Doğrulama (asgari):** `assert`/`invariant`/`cover` → SVA + SymbiYosys köprüsü;
  yerleşik test (`tick`/`expect`/`forall`).
- **Simülasyon:** Verilator-tabanlı koşum + yerleşik hızlı olay simülatörü (faz 4).
- **Tooling:** `volt build/test/fmt`, LSP (anlık tanı, tamamlama, gezinme), paket
  manifesti (`volt.toml`), JSON tanılar.
- **Standart kütüphane (çekirdek):** FIFO, arbiter, sayaç, CDC primitifleri, temel
  AXI-Lite bundle.

### 1.2 MVP Dışı (Explicitly Out of Scope — v1+)

- L2 tam timeline tipleri (`#[timeline]`, II analizi).
- Python jeneratör API'si ve cocotb köprüsü (tasarımı hazır, uygulaması v1).
- K-framework resmî semantik kanıtı (entegrasyon noktası bırakılır, kanıt sonra).
- ASIC sign-off / hibrit broker akışı (yol haritasında ama MVP teslimatı değil).
- ML-tabanlı PPA tahmin motoru (SOG/MasterRTL — v1 araştırma izi).
- Görsel FSM izleyici, canlı PPA öngörüsü (LSP "nice-to-have", v1).
- Paket kayıt/registry sunucusu (yerel dosya bağımlılığı yeterli).

---

## 2. Fazlar ve Zaman Çizelgesi

Toplam: **52 hafta (12 ay)**, 6 faz. Aylık ritimle "yürüyen iskelet" (walking
skeleton) erken kurulur, sonra katman katman güçlendirilir.

| Faz | Süre | Aylar | Tema |
|---|---|---|---|
| **F0** Temel & İskelet | 4 hafta | 1 | Repo, CI, gramer, "merhaba modül" lowering |
| **F1** Önyüz & Parser | 8 hafta | 2–3 | Lexer, cstree CST, AST, error-recovery |
| **F2** Tip Sistemi | 12 hafta | 4–6 | Genişlik/işaret/alan, CDC, latch, L0/L1, tanılar |
| **F3** CIRCT Lowering & Kod Üretimi | 10 hafta | 6–8 | MLIR/CIRCT köprüsü → SystemVerilog |
| **F4** Simülatör & Doğrulama | 10 hafta | 9–11 | Yerleşik simülatör, Verilator, assert→SVA |
| **F5** Tooling & Sertleştirme | 8 hafta | 11–12 | LSP, fmt, stdlib, dokümantasyon, beta |

> Fazlar kasıtlı örtüşür (F2↔F3 ay 6'da, F4↔F5 ay 11'de): tip sistemi olgunlaşırken
> lowering başlar, böylece IR kararları tipe geri besleme yapar.

---

## 3. Faz Detayları

### F0 — Temel ve Yürüyen İskelet (Ay 1)

**Hedef:** En küçük uçtan uca akış — tek bir `Counter` modülünü gramerden parse edip
elle yazılmış basit bir lowering ile SystemVerilog üretmek.

**Teslimatlar**
- Monorepo (Rust workspace) + CI (GitHub Actions: build, test, clippy, fmt).
- EBNF gramerinin makine-okunur hâli; örnek `.volt` test korpusu (~20 dosya).
- `Counter`, `SatCounter`, basit FSM için "golden" SystemVerilog referansları.
- ADR (Architecture Decision Record) süreci; ilk ADR'ler: IR seçimi, hata modeli.

**Çıkış kriteri:** `volt build counter.volt` derleme hatasız çalışır; üretilen SV
Verilator'da simüle olur (lowering elle/şablon olabilir).

### F1 — Önyüz ve Parser (Ay 2–3)

**Hedef:** Endüstri-kalite, hata-toleranslı önyüz.

**Teslimatlar**
- Lexer (sized literal, time literal dahil).
- **cstree** tabanlı kayıpsız CST (yeşil/kırmızı düğüm); konum bilgisi her token'da.
- AST katmanı; `UnboundAst → BoundAst` fonksiyonel dönüşüm iskeleti.
- **Error recovery:** eksik `}`/`;` ve yarım ifadelerde parse devam eder (LSP için).
- Arena tahsisi + `u32` handle altyapısı.
- İsim çözünümü (scope, import, path); modül imza tablosu `Σ`.

**Çıkış kriteri:** Korpustaki tüm geçerli dosyalar AST üretir; bozuk dosyalar
makul kısmi AST + konumlu hata üretir; parser fuzz testinde panik yok.

### F2 — Tip Sistemi (Ay 4–6) — *en kritik faz*

**Hedef:** Spesifikasyon Bölüm 4'ün Core alt kümesini uygulamak.

**Teslimatlar (alt-aşamalar)**
- **F2a (Ay 4):** Skaler tipler, genişlik aritmetiği (`+`/`+%`/`+|`/`*`), işaretlilik
  yasağı, `truncate`/`resize`/`as`, atanabilirlik, karşılaştırma kuralları.
- **F2b (Ay 5):** Saat alanları, alan-tutarlılık, CDC primitif imzaları ve denetimi;
  register kuralları (`reg(D)`, reset zorunluluğu, `<=` bağlam kontrolü).
- **F2c (Ay 5–6):** Tek-sürücü, birleşimsel döngü, **kesin atama / latch önleme**,
  FSM iyi-tanımlılık, `pipeline` L0 hizalama, L1 `Delayed<τ,d>`.
- **F2d (Ay 6):** const-jenerik + tip parametresi monomorfizasyonu.
- **Tanı motoru:** Elm/Rust sınıfı hata mesajları (konum + neden + düzeltme);
  `E02xx/E03xx/E04xx` kataloğu; `--message-format=json`.

**Çıkış kriteri:** Spesifikasyondaki her kural için pozitif + negatif test;
"hata mesajı kalitesi" snapshot testleri; bilinen tüm latch/CDC/genişlik tuzakları
yakalanıyor.

### F3 — CIRCT Lowering ve Kod Üretimi (Ay 6–8)

**Hedef:** `BoundAst` → MLIR/CIRCT → okunabilir SystemVerilog.

**Teslimatlar**
- CIRCT/MLIR'e C-API veya FFI köprüsü (Rust ↔ MLIR); IR oluşturma katmanı.
- Lowering geçişleri: kontrol/FSM → **Calyx**, akış → **Handshake**, RTL → **FIRRTL/HW**.
- Kod üretimi: CIRCT `ExportVerilog` ile **okunabilir** SystemVerilog (kara kutu değil;
  sinyal isimleri korunur, yorumlar eşlenir).
- Hedef seçimi iskeleti: `--target=fpga:generic` (ASIC sonra).
- Lowering doğruluğu: F0'daki golden SV'lerle diferansiyel karşılaştırma.

**Çıkış kriteri:** Korpustaki tüm modüller sentezlenebilir SV üretir; Yosys ile
sentez geçer; çıktı insan tarafından okunabilir.

### F4 — Simülatör ve Doğrulama (Ay 9–11)

**Hedef:** Tek komutla derle-simüle-doğrula döngüsü.

**Teslimatlar**
- **Verilator entegrasyonu:** üretilen SV'yi otomatik derleyip koşturan `volt test`.
- **Yerleşik hızlı simülatör:** IR'den native koda (cycle semantiği, Bölüm 6);
  küçük tasarımlarda kurulumsuz hız.
- Test çatısı: `tick`/`expect`/`forall` (constrained-random) çalıştırma + raporlama.
- **Doğrulama köprüsü:** `assert`/`invariant`/`cover` → SVA üretimi + **SymbiYosys**
  ile bounded model checking; ihlalde karşı-örnek + dalga formu (VCD/FST).
- Kapsama (coverage) raporu.

**Çıkış kriteri:** Sim = sentez tutarlılığı diferansiyel testlerle doğrulanır;
örnek tasarımlar (FIFO, AXI-Lite slave, basit RISC ALU) hem simüle hem formal
kontrol edilir.

### F5 — Tooling, Stdlib ve Sertleştirme (Ay 11–12)

**Hedef:** Dış kullanıcıların deneyebileceği tutarlı bir beta.

**Teslimatlar**
- **LSP sunucusu:** anlık tanı, otomatik tamamlama, tipte gezinme, yeniden adlandırma,
  hover (tip/genişlik/alan gösterimi). (Görsel FSM/canlı PPA → v1.)
- `volt fmt` biçimlendirici + `volt lint` (donanım anti-pattern uyarıları).
- **Standart kütüphane:** FIFO, skid-buffer, arbiter (round-robin/priority), CDC
  primitifleri, sayaç/shift-register, AXI-Lite bundle + adapter.
- `volt.toml` paket manifesti + yerel yol bağımlılıkları.
- Dokümantasyon: dil turu, "Verilog'dan Volt'a" geçiş rehberi, stdlib referansı,
  online playground (WASM derleyici hedefi).
- Sürüm `v0.1.0-beta`; davetli FPGA kullanıcılarıyla pilot.

**Çıkış kriteri:** Yeni bir kullanıcı, dokümandan başlayıp 30 dk içinde bir FPGA
tasarımını derleyip simüle edebiliyor; LSP VS Code'da çalışıyor.

---

## 4. Kilometre Taşları (Milestones)

| MS | Ay | Tanım |
|---|---|---|
| **M1** | 1 | Yürüyen iskelet: tek modül → SV → Verilator simülasyonu |
| **M2** | 3 | Hata-toleranslı parser + isim çözünümü tamam |
| **M3** | 6 | **Tip sistemi Core tamam** (CDC/latch/genişlik/L0/L1) — projenin omurgası |
| **M4** | 8 | CIRCT lowering: tüm korpus → okunabilir, sentezlenebilir SV |
| **M5** | 11 | Sim + formal doğrulama; örnek tasarımlar uçtan uca geçiyor |
| **M6** | 12 | `v0.1.0-beta`: LSP + stdlib + doküman + pilot kullanıcılar |

---

## 5. Ekip Listesi

### 5.1 Çekirdek Ekip (kademeli katılım)

| Rol | Sayı | Katılım | Sorumluluk |
|---|---|---|---|
| **Teknik Lider / Dil Tasarımcısı** | 1 | Ay 1 | Spesifikasyon sahipliği, mimari, ADR'ler, kod incelemesi |
| **Derleyici Mühendisi (Önyüz/Rust)** | 2 | Ay 1–2 | Lexer/parser/cstree, isim çözünümü, AST |
| **Tip Sistemi Mühendisi (PL)** | 1–2 | Ay 3 | Tip kuralları, çıkarım, tanı motoru (F2 lideri) |
| **Derleyici Mühendisi (Arka Uç/MLIR-CIRCT)** | 1–2 | Ay 5 | CIRCT köprüsü, lowering, kod üretimi |
| **Doğrulama/EDA Mühendisi** | 1 | Ay 8 | Simülatör, Verilator/SymbiYosys, test çatısı |
| **Tooling/DevEx Mühendisi** | 1 | Ay 9 | LSP, fmt, lint, playground, paketleme |
| **Donanım/FPGA Uzmanı (yarı zamanlı)** | 1 | Ay 1 | Gerçeklik kontrolü, stdlib tasarımı, örnek tasarımlar, pilot |
| **Teknik Yazar (yarı zamanlı)** | 1 | Ay 10 | Doküman, geçiş rehberi, tutorial |

**Zirve eş-zamanlı kadro:** ~6–7 tam zamanlı eşdeğer (FTE), Ay 5–8 arası.
**Toplam ~12 ay-FTE'lik efor** ölçeğinde (6–8 kişilik bir ekiple 12 ayda
tamamlanabilir).

### 5.2 Yetkinlik Notları

- **Rust derinliği** önyüz/arka uç için zorunlu (sahiplik, FFI, performans).
- **PL/tip teorisi** F2 için kritik — bu faz projenin başarı/başarısızlık eşiğidir.
- **MLIR/CIRCT deneyimi** nadir; bulunamazsa CIRCT topluluğuyla (LLVM Discourse)
  erken temas ve 1–2 aylık öğrenme tamponu planlanmalı.
- **Donanım gerçekliği** baştan masada olmalı; aksi halde "şık ama sentezlenmeyen"
  soyutlama riski doğar.

---

## 6. Araç ve Teknoloji Yığını

| Katman | Araç/Teknoloji | Gerekçe |
|---|---|---|
| Önyüz dili | **Rust** | Bellek/eşzamanlılık güvenliği, performans, FFI |
| CST kütüphanesi | **cstree** (rowan ailesi) | Kayıpsız, error-recovery'li ağaçlar |
| Lexer/parser yardımcıları | `logos` / elle-yazım | Hız + kontrol |
| Arka uç IR | **MLIR + CIRCT** (Calyx, Handshake, FIRRTL, HW, SV) | Olgun, modüler, SV üretir |
| FFI köprüsü | MLIR C-API + `bindgen` veya `melior` benzeri | Rust↔MLIR |
| Mantıksal sentez | **Yosys** | Açık kaynak, SV→netlist, doğrulama |
| Simülasyon | **Verilator** + yerleşik native simülatör | Hız + kurulumsuz iç döngü |
| Formal | **SymbiYosys** (+ yosys-smtbmc) | assert/cover BMC |
| Dalga formu | **GTKWave** uyumu (VCD/FST) | Standart hata ayıklama |
| LSP | `tower-lsp` (Rust) | VS Code/Neovim entegrasyonu |
| Formal semantik (v1) | **K framework** | CIRCT semantik açığını kapatma noktası |
| Playground (F5) | Rust→**WASM** | Tarayıcıda dene |
| CI/CD | GitHub Actions + `cargo nextest` | Hızlı, paralel test |
| Test | snapshot (`insta`), fuzz (`cargo-fuzz`), diferansiyel (golden SV) | Tanı/parser/lowering güvencesi |
| Sürümleme | SemVer + ADR + RFC süreci | Şeffaf dil evrimi |

> **Bağımlılık riski:** CIRCT hızlı evrilen bir proje; sürüm sabitleme (pinning) ve
> haftalık upstream takip rutini şart. FFI sınırı net tutulup CIRCT API'si tek bir
> Rust modülünde izole edilmeli (değişim maliyetini sınırlamak için).

---

## 7. Holistik Çapraz-Kesit Çalışmalar

Tek faza sığmayan, baştan sona süren işler:

- **Tanı kalitesi:** Her tip kuralı eklendiğinde hata mesajı + düzeltme önerisi
  birlikte yazılır (sonradan eklenmez). "Hata mesajı incelemesi" PR şartı.
- **Test korpusu:** Her özellik pozitif/negatif örnekle gelir; korpus M1'den
  itibaren büyür ve regresyon kalkanıdır.
- **Performans bütçesi:** Orta ölçekli tasarımda (≈50k satır) derleme < birkaç
  saniye hedefi; F2'den itibaren ölçülür.
- **Dogfooding:** stdlib MVP derleyiciyle yazılır; ekip kendi aracını kullanır.
- **Topluluk:** Açık geliştirme (GitHub), RFC'ler, erken pilot kullanıcılar; "kötü
  tooling iyi dilleri öldürür" ilkesiyle DevEx baştan önceliklenir.

---

## 8. Risk Kaydı (Risk Register)

| Risk | Olasılık | Etki | Azaltma |
|---|---|---|---|
| **MLIR/CIRCT öğrenme eğrisi/kararsızlığı** | Yüksek | Yüksek | Erken spike (F0'da mini lowering); CIRCT topluluğu teması; FFI izolasyonu; sürüm pinning |
| **Tip sistemi karmaşıklık patlaması** (F2) | Orta | Yüksek | Core alt küme net; L2'yi MVP dışında tut; her kurala test; kademeli teslim (F2a–d) |
| **Sim/sentez tutarlılığı kanıtlanamazsa** | Orta | Yüksek | Diferansiyel test (yerleşik sim ↔ Verilator ↔ golden); tek cycle semantiği; K-framework noktası bırak |
| **CIRCT'nin okunabilir SV üretmemesi** | Orta | Orta | İsim koruma + yorum eşleme geçişi; gerekirse özel ExportVerilog ayarı |
| **MLIR-CIRCT yetkinliği bulunamaması** | Orta | Yüksek | 1–2 ay öğrenme tamponu; danışman; alternatif olarak doğrudan FIRRTL üretimi planı B |
| **Kapsam kayması (scope creep)** | Yüksek | Orta | MVP sınırları (Bölüm 1) sözleşme gibi; v1 kovası; faz çıkış kriterleri katı |
| **Performans hedefinin kaçması** | Düşük | Orta | Arena/handle altyapısı baştan; profil F2'den itibaren |
| **Benimseme yokluğu** | Orta | Yüksek | FPGA topluluğu önce; geçiş rehberi; playground; gerçek örnek tasarımlar |

---

## 9. Başarı Ölçütleri (KPIs)

**Teknik (MVP sonu):**
- Spesifikasyon Core kurallarının ≥ %95'i uygulanmış ve test edilmiş.
- Korpustaki tasarımların %100'ü → sentezlenebilir SV (Yosys geçer).
- Yerleşik sim ↔ Verilator diferansiyel tutarlılık: 0 uyumsuzluk.
- Orta ölçekli tasarımda derleme süresi < birkaç saniye.
- Bilinen latch/CDC/genişlik tuzaklarının %100'ü derleme zamanında yakalanıyor.

**Deneyim:**
- Yeni kullanıcı "ilk çalışan tasarım"a < 30 dakika (doküman + playground ile).
- LSP VS Code'da anlık tanı + tamamlama sunuyor.
- Hata mesajı incelemesi: örnek tuzakların düzeltme önerileri net (kullanıcı testi).

**Topluluk (pilot):**
- ≥ 3–5 pilot kullanıcı/ekip gerçek bir FPGA tasarımını Volt'ta tamamlıyor.
- İlk dış katkıların (PR/issue) gelmesi.

---

## 10. MVP Sonrası (v1 Önizleme — Yol Haritasının Devamı)

MVP'yi takiben öncelik sırası: **(1)** L2 timeline tipleri ve II analizi;
**(2)** Python jeneratör API'si + cocotb köprüsü; **(3)** ML-tabanlı PPA tahmin
motoru (SOG/MasterRTL) ve LSP canlı PPA öngörüsü; **(4)** ASIC hedefi + hibrit
broker sign-off akışı (Yosys/OpenROAD entegrasyonu); **(5)** K-framework ile
lowering doğruluk kanıtı; **(6)** paket registry ve IP ekosistemi.

---

## Özet Gantt (Çeyreklik Görünüm)

```
Ay:        1   2   3   4   5   6   7   8   9  10  11  12
F0 İskelet [██]
F1 Önyüz       [████████]
F2 Tip Sist.            [████████████]
F3 Lowering                      [██████████]
F4 Sim/Doğr.                              [██████████]
F5 Tooling                                        [████████]
Milestone  M1      M2          M3      M4      M5      M6
```

> **Kritik yol:** F2 (tip sistemi) → M3. Proje başarısı bu fazın zamanında ve
> kaliteli bitmesine bağlıdır; ekip ve zaman tamponu öncelikle buraya ayrılmalıdır.

---

*Bu yol haritası bir MVP planıdır; gerçek hız, MLIR/CIRCT yetkinliğinin
edinilme süresine ve tip sistemi fazının seyrine göre ±2 ay oynayabilir. Faz çıkış
kriterleri ve risk azaltmaları, kapsam kaymasına karşı birincil koruma mekanizmasıdır.*
