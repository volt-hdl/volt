# ADR-0138: Çıktı Yazımı Kökün Altında Kalır — Bağlantının Üzerinden Yazılmaz, Tek Yazma Kapısı

> Statü: Uygulandı — bakımcı onayı (2026-11-25): Windows'ta yalnız yönlendiren (name surrogate) reparse point bağlantıdır
> İlgili: ADR-0021 (çıktılar `build/` altına; çıkış kodu 3 G/Ç hatasıdır), ADR-0125 (ret iletisi ne oldu, neden, ne yapabilirsin der; yeni tanı kodu yok, §7)
> Issue: #221, #427
> Tarih: 2026-11-25
> Etkilenen: `crates/volt-driver/src/output.rs` (YENİ, tek yazma kapısı), volt-driver'ın yazma noktaları (`main.rs`, `new/`, `sim/`, `verify.rs`, `verify_jobs.rs`, `waves.rs`, `extern_stage.rs`, `watch.rs`), `crates/volt-driver/tests/output_links_tests.rs` (YENİ), `crates/volt-driver/tests/output_writes_tests.rs` (YENİ, izin listesi), `SECURITY.md`, `CHANGELOG.md`

## Sorun

Giriş tarafı korunuyordu: `@source` ve `read_hex` yolları kanonikleştirilir,
proje dışına çıkan yol E1012 alır. Çıkış tarafında denetim yoktu
(#221, Linux'ta doğrulandı). Klonlanan bir depoda `build/rtl/Top.sv`
depo dışındaki bir dosyaya (ör. `~/.ssh/authorized_keys`) sembolik
bağlantıysa `volt build` o dosyanın üzerine yazıyordu; `build` dizininin
kendisi bağlantıysa bütün çıktılar onu izliyordu. Hedefin adı
(`<Modül>.sv`, `.h`, `.rs`, `.md`, `.sdc`) ve içeriğin bir kısmı
saldırganın elindedir; git bağlantıları korur.

volt-driver'da dosya yazan, dizin kuran, kopyalayan ya da silen yerler
on bir dosyaya dağılmıştı; ortak bir yazma fonksiyonu yoktu. Bir
denetimi her yere ayrı ayrı eklemek, giriş tarafındaki tek yerde
hesaplanan kuralın (E1012) tersine, ayrışmaya açık bir ikinci kopya
olurdu.

## Karar

### 1. Çıktı kökü

Her komutun bir çıktı kökü vardır; ilk kullanımda bir kez
kanonikleştirilir:

| Komut | Kök |
|---|---|
| `build`, `run`, `test`, `verify` (varsayılan `build/`) | çalışma dizini; `build/` kökün altındadır ve denetlenir |
| aynı komutlar `--target-dir <dizin>` ile | verilen dizinin kendisi |
| `volt new <ad>` | oluşturulan proje dizini |
| `volt init` | çalışma dizini |
| `volt run --vcd <yol>` | VCD dosyasının dizini |

Kökün kendisi ve üstündeki yol kullanıcının seçimidir, denetlenmez:
kullanıcının ana dizini ya da açıkça verdiği `--target-dir` bir bağlantı
olabilir, bu meşrudur.

### 2. Denetim

Kökün altındaki yolun var olan her parçası ve hedefin kendisi denetlenir
(`symlink_metadata`, bağlantıyı izlemeden):

- **Bağlantı:** std'nin `FileType::is_symlink`'inin bağlantı dediği
  girdi: Unix'te sembolik bağlantı; Windows'ta yazımı başka bir yola
  yönlendiren (name surrogate) reparse point, yani sembolik bağlantı ve
  junction (§5). Yazım reddedilir.
- **Kök dışı:** hedef kökün dışına çıkıyorsa (`..`) yazım reddedilir.

Kural yazmaya, dizin kurmaya, kopyalamaya, yeniden adlandırmaya ve
silmeye uygulanır. Silmede son parça bir bağlantı olabilir: silinen
bağlantının kendisidir, hedefi değil; üstündeki parçalar denetlenir.
Volt'un bir araca (Verilator, sby) verdiği dizin, araç başlamadan önce
bütünüyle denetlenir: araç var olan dosyaların üzerine yazar ve bağlantıyı
izlerdi. Verilator'un obj dizininde bağlantı varsa dizin silinir ve
derleme baştan kurulur (eski damgalı dizinin kuralıyla aynı); `build/formal`
altında bağlantı varsa `volt verify` reddeder. `volt run --vcd`'nin dosyasını
simülasyon yazar; Volt hedefi simülasyondan önce denetler.

### 3. Red

Red, var olan G/Ç hata yolundan bildirilir: çıkış kodu 3 (cli-contract
§2). İleti ADR-0125'in üç parçasını taşır: hangi yol yazılamadı (ne oldu),
hangi parça neden (bağlantı ya da kök dışı) ve ne yapılabilir
(bağlantıyı kaldır ya da başka bir çıktı dizini seç: `--target-dir`,
başka bir proje dizini, başka bir `--vcd` yolu). Yeni tanı kodu, seçenek
ya da JSON alanı yoktur (ADR-0125 §7). Bağlantısız yollarda iletiler ve
üretilen dosyalar değişmez.

### 4. Tek kapı ve izin listesi

volt-driver'ın bütün yazımları `crates/volt-driver/src/output.rs`'teki
`OutputRoot`'tan geçer. İzin listesi testi
(`crates/volt-driver/tests/output_writes_tests.rs`, ADR-0137 §4'ün
biçimi) `crates/volt-driver/src/**/*.rs`'i tarar; yorumları, dizgeleri ve
`#[cfg(test)]` öğelerini atlar. Yardımcı dışında `std::fs::write`,
`create_dir`, `copy`, `rename`, `remove_*`, `File::create`, `OpenOptions`
gibi bir çağrı ya da onları adıyla getiren bir içe aktarma testi düşürür;
gerçekten gereken bir istisna (dosya, fonksiyon, neden) listede durur.
Bugün liste boştur.

### 5. Windows

- Git for Windows'ta `core.symlinks` varsayılanı `false`'tur: depodaki
  bir sembolik bağlantı, hedef yolunu taşıyan sıradan bir dosya olarak
  çıkar; #221'in saldırısı varsayılan kurulumda oluşmaz. Kurulumda
  sembolik bağlantılar açıldıysa (`core.symlinks=true`, Geliştirici Modu
  ya da yönetici yetkisi) gerçek bağlantı çıkar ve kural onu reddeder.
- Git junction saklamaz; junction'ı kullanıcı, bir betik ya da arşiv
  aracı kurar.
- Bağlantı std'nin Windows tanımıdır (`library/std/src/sys/fs/windows.rs`,
  `FileType::new`): `is_symlink` = `FILE_ATTRIBUTE_REPARSE_POINT` **ve**
  reparse etiketinin name surrogate biti (`0x20000000`). Etiket
  `symlink_metadata`'da (`FILE_FLAG_OPEN_REPARSE_POINT` ile açılan
  tanıtıcıdan, `FileAttributeTagInfo`) okunur. Sembolik bağlantı
  (`IO_REPARSE_TAG_SYMLINK`) ve junction (`IO_REPARSE_TAG_MOUNT_POINT`)
  name surrogate'tır: adı başka bir yola yönlendirir, yazım oraya düşer;
  reddedilir.
- Name surrogate olmayan reparse point'ler, ör. bulut eşitleme yer
  tutucuları (`IO_REPARSE_TAG_CLOUD_*`, OneDrive'ın isteğe bağlı
  dosyaları), sıkıştırma (WOF) ve tekilleştirme, yazımı yerinde bırakır:
  bağlantı değildir, Volt onlara sıradan dosya gibi yazar. Belgeler
  klasörü OneDrive'a eşitlenmiş bir Windows kullanıcısında `volt build`
  bu yüzden çalışır (#427).
- Denetim yetki istemez, `unsafe` gerekmez.

### 6. Sürüm

Güvenlik düzeltmesidir (SECURITY.md: güvenlik düzeltmeleri son haneye
girer). Yeni seçenek, tanı kodu ve JSON alanı yoktur; üretilen dosyalar
bayt bayt aynıdır. Daha önce bağlantının üzerinden yazılan yollar artık
reddedilir: CHANGELOG'da "Davranış değişikliği" satırıdır.

## İlke denetimi

1. **Beyan:** istemez. Kök, var olan `--target-dir` seçeneğinden ya da
   komutun kendisinden gelir.
2. **Çıkarım:** kök komut satırından okunur (değer varsayılan mı, açıkça
   mı verildi); belirsizlik yoktur.
3. **Var olan anlam:** bir tasarımın anlamı değişmez, üretilen dosyalar
   aynıdır. Değişen, bağlantının üzerinden yapılan yazımın reddidir
   (§6).
4. **Niyet tahmini:** yok. Kural bir bağlantının kötü niyetli olup
   olmadığını tahmin etmez; kökün altındaki her bağlantıyı reddeder ve
   iletisi kullanıcıya iki yolu gösterir.
5. **Metamorfik yeniden yazım:** uygulanmaz (dilin kuralı değil).

## Sınırlar

- **Denetim ile yazım arası (TOCTOU).** Denetim yolu okur, yazım ardından
  yol üzerinden yapılır. Arada aynı kullanıcının yetkisiyle çalışan
  başka bir süreç bir ara dizini bağlantıyla değiştirebilir; std'nin
  güvenli API'si dizin tanıtıcısına göre açmayı (`openat`, `O_NOFOLLOW`)
  sunmaz, `unsafe` ve yeni bağımlılık kullanılmaz. Son parça için pencere
  daraltılır: hedef yoksa `create_new` (`O_CREAT|O_EXCL`) arada konan bir
  bağlantıyı izlemez; dizin `create_dir` ile kurulur. #221'in tehdidi
  durağandır (depoyla gelen bağlantı) ve denetim onu tamamen kapatır;
  eşzamanlı bir saldırgan zaten kullanıcının yetkisiyle çalışıyordur.
- **Araçların kendi yazımları.** Verilator, sby ve simülasyon dizinleri
  araç başlamadan önce denetlenir; aracın kendi kurduğu bir bağlantı
  (bugün hiçbiri kurmuyor) sonraki koşuda reddedilir ya da (obj dizini)
  silinir.
- **Windows'ta name surrogate olmayan reparse point'ler.** Kural onları
  kabul eder (§5); bu, std'nin tanımına dayanır. Test ortamında böyle bir
  reparse point yetkisiz ve `unsafe` olmadan kurulamaz: bulut yer
  tutucusu bir eşitleme sağlayıcısı ister, tekilleştirme sunucu rolü,
  WOF sıkıştırması (`compact /exe`) kullanıcı kipinde reparse point
  olarak görünmez. Gerçek bir OneDrive klasöründe sınama yapılmadı.
- **Name surrogate olan başka etiketler.** Etiketinde name surrogate biti
  olan her reparse point bağlantı sayılır ve reddedilir; std ayrım
  yapmaz, Volt da yapmaz.
- **Diğer crate'ler.** `volt-lsp` ve `volt-tools`'un `src/`'sinde dosya
  yazan üretim kodu yoktur (yalnız testleri yazar); `volt-difftest` bir
  test koşucusudur, kendi geçici dizinine yazar. İzin listesi bugün
  yalnız volt-driver'ı tarar.
