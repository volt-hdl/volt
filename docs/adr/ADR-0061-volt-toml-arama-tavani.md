# ADR-0061: Volt.toml Aramasına Tavan

> Statü: KABUL EDİLDİ
> Tarih: 2026-09-21
> Etkilenen: volt-hir (`manifest_search.rs` YENİ — `find_manifest_dir`,
> `SearchStop`; `attrs.rs` `UnenforcedLint::discover`), volt-driver
> (`unit.rs` `Manifest::lookup`, E1011 notu; `sim_lower.rs` test veri kökü
> dolaylı), volt-lsp (dolaylı, `UnenforcedLint::discover` üzerinden).
> DOKUNULMADI: README.md, examples/, docs/spec/.
> İlgili: ADR-0042 (Volt.toml kök keşfi), ADR-0048 (`[lint]` politikası),
> ADR-0058 (test veri kökü — bu ADR onun "bilinen sınır"ını kapatır).

## Sorun

ADR-0058 "Bilinen sınırlar":

> `Volt.toml` araması yukarı doğru tavansızdır (ADR-0042 davranışı) —
> kendi manifest'i olmayan bir proje, üst dizindeki başıboş bir
> `Volt.toml`'u kök sayar ve okuma alanı genişler.

```
C:\Users\X\Volt.toml          (unutulmuş, eski)
C:\Users\X\projects\foo\      (Volt.toml yok)
volt build foo\main.volt      → C:\Users\X kök sayılır
```

Sonuç: yanlış `[package] src` ile `use` çözümlemesi (başka bir ağaçtan
dosya yüklenir), yanlış `[lint]` politikası (W0021 kazara susar) ve
`read_hex` okuma alanının ev dizininin tamamına genişlemesi.

## Tespit

Arama üç yerde, iki ayrı kopya olarak yapılıyordu; üçü de dosya sistemi
köküne kadar çıkıyordu:

| Yer | Kullanım |
|---|---|
| `volt-driver/src/unit.rs` `Manifest::discover` | `use` kökü (`<kök>/<src>`), `[lint]` |
| `volt-driver/src/sim_lower.rs` `FsTestFiles` | `read_hex`/`load` okuma kökü (ADR-0058) — `Manifest::discover` çağırır |
| `volt-hir/src/attrs.rs` `UnenforcedLint::discover` | LSP'nin W0021 politikası (ADR-0048) |

`main.rs` `manifest_lang` arama yapmaz — yalnız çalışma dizinindeki
`Volt.toml` `[ui] lang`'ı okur; bu ADR'nin kapsamı dışında.

Göreli bir başlangıç yolu (`volt build main.volt` → dizin `""`) sözcüksel
olarak çıkar ve çalışma dizininin üstüne geçmez; bu davranış korunur.

## Seçenekler

| | Tavan | Artı | Eksi |
|---|---|---|---|
| A | Git kökü (`.git` görülen dizin) | Proje sınırının en güvenilir işareti; CI checkout'larında hep var | Git dışı projede işe yaramaz |
| B | Ev dizini | Sorunun asıl senaryosunu (`~/Volt.toml`) kapatır | Ev dizini altındaki iç içe projeler arasında koruma yok |
| C | Dosya sistemi kökü (mevcut) | Değişiklik yok | Sorunun kendisi |
| D | A + B | İkisinin birleşimi: git projesinde repo sınırı, git dışında ev dizini | İki kural |

**Emsal — Cargo.** `cargo build` `Cargo.toml`'u çalışma dizininden dosya
sistemi köküne kadar arar; tavanı yoktur (`GlobalContext::search_stop_path`
varsayılan `None`, yalnız Cargo'nun kendi testleri ayarlar). Çalışma
alanı kökü aramasında (`find_root_iter`) tek sınır `CARGO_HOME`'dur:
`~/.cargo/registry` altındaki bir crate'in kullanıcının çalışma alanına
katılmaması için arama `CARGO_HOME`'u geçmez. Cargo başıboş bir üst
manifest'i sessizce kabul etmez ama bunu tavanla değil hatayla çözer
(üye listesinde olmayan paket için "current package believes it's in a
workspace when it's not"). Volt'ta kökün seçilmesi sessizce yanlış
dosya yükletir; hata üretecek bir üyelik listesi yoktur — bu yüzden
tavan gerekir. Git'in `GIT_CEILING_DIRECTORIES`'i aynı "yukarı aramayı
bir sınırda kes" fikrinin ikinci emsalidir; `VOLT_MANIFEST_DIR` ise
Cargo'nun `--manifest-path`'inin karşılığıdır.

## Karar

**D — git kökü + ev dizini; hangisi önce gelirse.**

1. Arama başlangıç dizininden yukarı çıkar; her dizinde sırayla:
   - Dizin ev dizini (`HOME` ya da `USERPROFILE`, kanonik karşılaştırma)
     VE başlangıç dizini değilse → dur (`SearchStop::Home`). Ev dizini
     **dışlayıcı** tavandır: `~/Volt.toml` alt dizinlerdeki projelerin
     kökü olamaz. Yalnız arama zaten ev dizininden başladıysa (kaynak
     dosya doğrudan `~` içinde) okunur.
   - `Volt.toml` varsa → bulundu.
   - `.git` varsa (dizin, ya da worktree/alt modüldeki `.git` dosyası)
     → dur (`SearchStop::GitRoot`). Git kökü **kapsayıcı** tavandır:
     repo kökündeki `Volt.toml` olağan yerdir, okunur; üstü okunmaz.
2. `VOLT_MANIFEST_DIR` (boş değilse) aramayı tümüyle geçersiz kılar: o
   dizindeki `Volt.toml` kullanılır, tavan uygulanmaz. Dizinde
   `Volt.toml` yoksa manifest yoktur — yukarı aranmaz
   (`SearchStop::Override`); kullanıcının açık isteği sessizce başka bir
   dosyaya dönüşmez.
3. Tek uygulama `volt_hir::manifest_search`; sürücü (`Manifest::lookup`),
   test veri kökü ve LSP (`UnenforcedLint::discover`) aynı fonksiyonu
   çağırır. LSP süreci de `VOLT_MANIFEST_DIR`'i kendi ortamından okur.

Alt modül (`.git` dosyası) olarak eklenmiş bir Volt paketi üst repodaki
`Volt.toml`'u artık görmez; bu kasıtlıdır (alt modül ayrı projedir) ve
gerekirse `VOLT_MANIFEST_DIR` ile aşılır.

## Tanı

Tavanda durup manifest bulamamak **sessizdir**: manifestsiz tek dosya
derleme olağan ve geçerlidir, her derlemede bir not gürültü olur.

Tek istisna, manifest'in fark yarattığı yerdir: bulunamayan paket
(E1011) zaten "yalnız dosyanın kendi dizini arandı" notunu taşıyordu. Not
artık aramanın nerede durduğunu söyler:

| Durma yeri | Not (en) |
|---|---|
| Git kökü | `no Volt.toml found up to the git root '<dir>' (the search stops there; set VOLT_MANIFEST_DIR to use another manifest) — …` |
| Ev dizini | `no Volt.toml found below the home directory '<dir>' (a Volt.toml in the home directory itself is not a project root; set VOLT_MANIFEST_DIR to use it) — …` |
| `VOLT_MANIFEST_DIR` | `VOLT_MANIFEST_DIR points to '<dir>', which has no Volt.toml — …` |
| Tavan görülmedi | Önceki metin, değişmedi |

Yeni tanı kodu yoktur; E1011'in açıklaması (`volt explain`) geçerliliğini
korur.

## Testler

- `volt-hir` `manifest_search` birim testleri (8): başlangıç dizini, git
  kökündeki manifest, git kökü üstündeki başıboş manifest (istenen ağaç:
  `tmp/Volt.toml`, `tmp/repo/.git/`, `tmp/repo/src/`), `.git` dosyası,
  ev dizini (dışlayıcı), ev dizininden başlayan arama, geçersiz kılma,
  manifest'siz geçersiz kılma. Ortam `SearchEnv` ile enjekte edilir;
  süreç ortamına dokunulmaz.
- `volt-driver` CLI testleri (5): git tavanı → E1011 + not, `HOME` /
  `USERPROFILE` alt süreçte geçici dizine çevrilerek ev dizini tavanı,
  tavan yokken ADR-0042 davranışının sürdüğü, `VOLT_MANIFEST_DIR`
  geçersiz kılması ve manifest'siz geçersiz kılmanın notu.
- `volt-lsp` (1): git kökü üstündeki `allow` politikası editörde
  uygulanmaz (W0021 görünür).
- Mutasyon: git tavanı kaldırılınca 4 test, ev tavanı dışlayıcılığı
  kaldırılınca 2 test düşer.

## Sonuçlar

- ADR-0058'in "Volt.toml araması tavansız" sınırı kapanır; `read_hex`
  okuma kökü de aynı tavanla sınırlanır.
- Git dışı ve ev dizini dışındaki bir ağaçta (ör. `/opt/proje`, CI
  `/tmp`) arama önceki gibi dosya sistemi köküne kadar çıkar.
- Monorepo içindeki alt dizin projeleri repo kökündeki `Volt.toml`'u
  görmeye devam eder (git kökü kapsayıcı).
