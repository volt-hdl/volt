//! Volt'un yazdığı her dosyanın tek kapısı (#221).
//!
//! **Kök.** Çıktı kökü komut başına bir kez belirlenir ve ilk kullanımda
//! bir kez kanonikleştirilir:
//!
//! - `build`, `run`, `test`, `verify`: `--target-dir` verilmediyse çalışma
//!   dizini (proje kökü; varsayılan `build/` kökün altındadır ve
//!   denetlenir), verildiyse o dizinin kendisi;
//! - `volt new <ad>`: oluşturulan proje dizini; `volt init`: çalışma dizini;
//! - `volt run --vcd <yol>`: VCD dosyasının dizini.
//!
//! Kökün kendisi ve üstündeki yol kullanıcının seçimidir, denetlenmez:
//! kullanıcının ana dizini bir bağlantı olabilir, bu meşrudur.
//!
//! **Denetim.** Kökün altındaki yolun var olan her parçası ve hedefin
//! kendisi denetlenir. Sembolik bağlantı (Windows'ta junction ve başka her
//! reparse point) yazımı reddeder; kökün dışına çıkan hedef (`..`) de.
//! Red, G/Ç hatası olarak bildirilir: çıkış kodu 3 (cli-contract §2), ileti
//! ADR-0125'in biçiminde, yeni tanı kodu yok (ADR-0125 §7).
//!
//! **Denetim ile yazım arası (TOCTOU).** Denetim `symlink_metadata` ile
//! yapılır, yazım ardından yol üzerinden. Arada aynı kullanıcının yetkisiyle
//! çalışan başka bir süreç bir parçayı bağlantıyla değiştirebilir; std'nin
//! güvenli API'si dizin tanıtıcısına göre açmayı (`openat`, `O_NOFOLLOW`)
//! sunmadığından bu pencere ara dizinler için kapanmaz. Son parça için
//! daraltılır: hedef yoksa `create_new` ile açılır (`O_CREAT|O_EXCL`, arada
//! konan bir bağlantıyı izlemez), dizin `create_dir` ile kurulur (var olan
//! bir bağlantının içine değil, yerine kurulmaya çalışır ve başarısız
//! olur). #221'in tehdidi durağandır (klonlanan depodaki bağlantı); onu
//! denetim tamamen kapatır.
//!
//! **Silme.** Volt kendi eski çıktısını da siler (dalga formu, obj dizini).
//! Silmede son parça bir bağlantı olabilir (bağlantının kendisi silinir,
//! hedefi değil); üstündeki parçalar denetlenir.

use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;

use volt_diagnostics::lstr;

/// Kökü kim seçti: reddin `= help:` satırı başka bir dizini hangi yolla
/// verebileceğini söyler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    /// `build`, `run`, `test`, `verify`: `--target-dir`.
    TargetDir,
    /// `volt new` / `volt init`: projenin dizini.
    Project,
    /// `volt run --vcd`.
    Vcd,
}

/// Bir komutun çıktı kökü ve komutun yazdığı dizin.
#[derive(Debug)]
pub(crate) struct OutputRoot {
    /// Kökün kullanıcının gördüğü biçimi (boş: çalışma dizini).
    given: PathBuf,
    /// Komutun çıktı dizini (`build/`, `--target-dir`, proje dizini):
    /// kökün kendisi ya da altında.
    dir: PathBuf,
    origin: Origin,
    /// İlk kullanımda bir kez kanonikleştirilir.
    canonical: OnceLock<PathBuf>,
}

/// Neden reddedildi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RefusalKind {
    /// Sembolik bağlantı (Windows'ta junction da).
    Link,
    /// Windows: bağlantı olmayan bir reparse point.
    ReparsePoint,
    /// Hedef kökün dışında (`..`).
    OutsideRoot,
}

/// Reddedilen bir yazım.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Refusal {
    /// Nedeni taşıyan yol (bağlantı olan parça ya da hedef), kullanıcının
    /// gördüğü biçimde.
    pub culprit: PathBuf,
    pub kind: RefusalKind,
    /// Kök, kullanıcının gördüğü biçimde.
    pub root: PathBuf,
    pub origin: Origin,
}

/// Çıktı yazımının hatası: reddedildi ya da G/Ç hatası.
#[derive(Debug)]
pub(crate) enum OutputError {
    Refused(Refusal),
    Io(io::Error),
}

impl From<io::Error> for OutputError {
    fn from(e: io::Error) -> Self {
        OutputError::Io(e)
    }
}

/// Hata iletisinin ilk satırı: hangi işlem.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Action<'a> {
    /// "cannot write '<yol>'".
    Write,
    /// "cannot create '<yol>'" (dizin).
    Create,
    /// "cannot copy '<kaynak>' to '<yol>'".
    Copy { from: &'a Path },
}

impl OutputRoot {
    /// Kök ve çıktı dizini aynı: `dir`.
    pub(crate) fn new(dir: &Path, origin: Origin) -> Self {
        OutputRoot {
            given: dir.to_path_buf(),
            dir: dir.to_path_buf(),
            origin,
            canonical: OnceLock::new(),
        }
    }

    /// `--target-dir` kökü: değer varsayılansa (`build`) çalışma dizini —
    /// `build/` kökün altında kalır ve denetlenir; kullanıcı açıkça bir
    /// dizin verdiyse o dizinin kendisi.
    pub(crate) fn for_target(target_dir: &Path, explicit: bool) -> Self {
        let mut root = Self::new(target_dir, Origin::TargetDir);
        if !explicit {
            root.given = PathBuf::new();
        }
        root
    }

    /// Komutun çıktı dizini (`build/` ya da `--target-dir`'in değeri).
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// Kullanıcının gördüğü kök (çalışma dizini için `.`).
    fn shown_root(&self) -> PathBuf {
        if self.given.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            self.given.clone()
        }
    }

    /// Kanonik kök; yoksa `None`.
    fn canonical(&self) -> io::Result<Option<&Path>> {
        if self.canonical.get().is_none() {
            match fs::canonicalize(self.shown_root()) {
                Ok(c) => {
                    let _ = self.canonical.set(c);
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(e),
            }
        }
        Ok(self.canonical.get().map(PathBuf::as_path))
    }

    /// Kanonik kök; yoksa oluşturulur. Kök kullanıcının seçimidir:
    /// oluşturulması denetlenmez.
    fn canonical_created(&self) -> io::Result<&Path> {
        if self.canonical()?.is_none() {
            fs::create_dir_all(self.shown_root())?;
        }
        self.canonical()?
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn refusal(&self, culprit: PathBuf, kind: RefusalKind) -> OutputError {
        OutputError::Refused(Refusal {
            culprit,
            kind,
            root: self.shown_root(),
            origin: self.origin,
        })
    }

    /// `path`'in kökün altındaki parçaları. Kökün dışındaysa ya da `..`
    /// içeriyorsa red.
    fn parts(&self, path: &Path) -> Result<Vec<OsString>, OutputError> {
        let root = absolute(&self.given)?;
        let full = absolute(path)?;
        let outside = || self.refusal(path.to_path_buf(), RefusalKind::OutsideRoot);
        let rel = full.strip_prefix(&root).map_err(|_| outside())?;
        let mut parts = Vec::new();
        // `absolute` ve `components` `.` parçalarını düşürür; kalan her
        // Normal dışı parça (`..`, kök, önek) kökün dışına çıkar.
        for c in rel.components() {
            match c {
                Component::Normal(n) => parts.push(n.to_os_string()),
                _ => return Err(outside()),
            }
        }
        Ok(parts)
    }

    /// Var olan parçaları sırayla denetler (`check_last` yanlışsa son parça
    /// hariç) ve kanonik kökten kurulan gerçek yolu döndürür. Kök yoksa
    /// `None`: altında hiçbir şey yoktur.
    fn checked(&self, path: &Path, check_last: bool) -> Result<Option<PathBuf>, OutputError> {
        let parts = self.parts(path)?;
        let Some(base) = self.canonical()? else {
            return Ok(None);
        };
        let mut real = base.to_path_buf();
        let mut shown = self.given.clone();
        let mut exists = true;
        for (i, part) in parts.iter().enumerate() {
            real.push(part);
            shown.push(part);
            let last = i + 1 == parts.len();
            if !exists || (last && !check_last) {
                continue;
            }
            match fs::symlink_metadata(&real) {
                Ok(meta) => {
                    if let Some(kind) = link_kind(&meta) {
                        return Err(self.refusal(shown, kind));
                    }
                }
                // Bu parça yoksa altındakiler de yoktur.
                Err(e) if e.kind() == io::ErrorKind::NotFound => exists = false,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(Some(real))
    }

    /// Hedef yazılabilir mi: yolunda ve kendisinde bağlantı yok, kökün
    /// altında. Dosyayı Volt değil bir araç yazacaksa (ör. simülasyonun
    /// VCD'si) yazımdan önce çağrılır.
    pub(crate) fn check(&self, path: &Path) -> Result<(), OutputError> {
        self.checked(path, true).map(|_| ())
    }

    /// `std::fs::create_dir_all`'ın denetimli karşılığı: kökün altındaki
    /// her parça ya yoktur ve gerçek bir dizin olarak kurulur ya da vardır
    /// ve bağlantı değildir.
    pub(crate) fn create_dir_all(&self, dir: &Path) -> Result<(), OutputError> {
        let parts = self.parts(dir)?;
        let mut real = self.canonical_created()?.to_path_buf();
        let mut shown = self.given.clone();
        for part in &parts {
            real.push(part);
            shown.push(part);
            // `create_dir` var olan bir bağlantıyı izlemez, hata verir.
            // std'nin `create_dir_all`'ı gibi hatanın türüne bakılmaz:
            // parça var olan gerçek bir dizinse sürülür, yoksa hata döner.
            let err = match fs::create_dir(&real) {
                Ok(()) => continue,
                Err(e) => e,
            };
            let Ok(meta) = fs::symlink_metadata(&real) else {
                return Err(err.into());
            };
            if let Some(kind) = link_kind(&meta) {
                return Err(self.refusal(shown, kind));
            }
            if !meta.is_dir() {
                return Err(err.into());
            }
        }
        Ok(())
    }

    /// `std::fs::write`'ın denetimli karşılığı.
    pub(crate) fn write(&self, path: &Path, contents: impl AsRef<[u8]>) -> Result<(), OutputError> {
        let real = self.existing_root(path, true)?;
        open_for_write(&real)?.write_all(contents.as_ref())?;
        Ok(())
    }

    /// `std::fs::copy`'nin denetimli karşılığı: kaynak okunur, hedef
    /// [`OutputRoot::write`] gibi açılır (izinler kopyalanmaz).
    pub(crate) fn copy(&self, from: &Path, to: &Path) -> Result<(), OutputError> {
        let real = self.existing_root(to, true)?;
        let mut source = File::open(from)?;
        io::copy(&mut source, &mut open_for_write(&real)?)?;
        Ok(())
    }

    /// `std::fs::remove_file`'ın denetimli karşılığı: son parça bir
    /// bağlantıysa bağlantının kendisi silinir.
    pub(crate) fn remove_file(&self, path: &Path) -> Result<(), OutputError> {
        let real = self.existing_root(path, false)?;
        fs::remove_file(real)?;
        Ok(())
    }

    /// `std::fs::remove_dir_all`'ın denetimli karşılığı (std bağlantıyı
    /// izlemez: son parça bağlantıysa yalnız o silinir).
    pub(crate) fn remove_dir_all(&self, path: &Path) -> Result<(), OutputError> {
        let real = self.existing_root(path, false)?;
        fs::remove_dir_all(real)?;
        Ok(())
    }

    /// `std::fs::remove_dir`'ın denetimli karşılığı.
    pub(crate) fn remove_dir(&self, path: &Path) -> Result<(), OutputError> {
        let real = self.existing_root(path, false)?;
        fs::remove_dir(real)?;
        Ok(())
    }

    /// Bir araca (Verilator, sby) verilecek dizin: yolunda ve altında
    /// hiçbir bağlantı yok. Araç dizindeki var olan dosyaların üzerine
    /// yazabilir; Volt o yazımları göremez, dizini önceden denetler.
    pub(crate) fn check_tree(&self, dir: &Path) -> Result<(), OutputError> {
        let Some(real) = self.checked(dir, true)? else {
            return Ok(());
        };
        let mut stack = vec![(
            real,
            self.given
                .join(self.parts(dir)?.iter().collect::<PathBuf>()),
        )];
        while let Some((real, shown)) = stack.pop() {
            let entries = match fs::read_dir(&real) {
                Ok(e) => e,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            for entry in entries {
                let entry = entry?;
                let meta = fs::symlink_metadata(entry.path())?;
                let shown = shown.join(entry.file_name());
                if let Some(kind) = link_kind(&meta) {
                    return Err(self.refusal(shown, kind));
                }
                if meta.is_dir() {
                    stack.push((entry.path(), shown));
                }
            }
        }
        Ok(())
    }

    /// Denetlenmiş gerçek yol; yazım ve silme için kök var olmalıdır
    /// (yoksa std'nin vereceği hata: NotFound). Silmede (`check_last`
    /// yanlış) son parça bir bağlantı olabilir: silinen bağlantının
    /// kendisidir, ve eski bir bağlantının yerinde kalması aracın onun
    /// üzerinden yazmasına yol açardı.
    fn existing_root(&self, path: &Path, check_last: bool) -> Result<PathBuf, OutputError> {
        self.checked(path, check_last)
            .and_then(|real| real.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound).into()))
    }
}

/// Boş yol çalışma dizinidir; gerisi `std::path::absolute` (bağlantıları
/// çözmez; Windows'ta `..`'yu sözcüksel olarak çözer, Unix'te bırakır —
/// iki durumda da kök dışı hedef yakalanır).
fn absolute(path: &Path) -> io::Result<PathBuf> {
    if path.as_os_str().is_empty() {
        std::env::current_dir()
    } else {
        std::path::absolute(path)
    }
}

/// Bağlantı mı: sembolik bağlantı (Windows'ta std junction'ı da böyle
/// sayar) ya da Windows'ta başka bir reparse point.
fn link_kind(meta: &fs::Metadata) -> Option<RefusalKind> {
    if meta.file_type().is_symlink() {
        Some(RefusalKind::Link)
    } else if is_reparse_point(meta) {
        Some(RefusalKind::ReparsePoint)
    } else {
        None
    }
}

/// Windows: dosya öznitelikleri reparse point diyor mu. Başka platformda
/// reparse point yoktur. Bit denetimi [`has_reparse_bit`]'tedir: her
/// platformda sınanır.
fn is_reparse_point(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    let attributes = {
        use std::os::windows::fs::MetadataExt as _;
        meta.file_attributes()
    };
    #[cfg(not(windows))]
    let attributes = {
        let _ = meta;
        0
    };
    has_reparse_bit(attributes)
}

/// `FILE_ATTRIBUTE_REPARSE_POINT` (Windows SDK, `winnt.h`).
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

fn has_reparse_bit(attributes: u32) -> bool {
    attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Hedef yoksa yalnız yeni bir dosya olarak açılır: `create_new`
/// (`O_CREAT|O_EXCL`) denetimden sonra konan bir bağlantıyı izlemez. Varsa
/// (denetimde bağlantı değildi) kısaltılarak açılır. İkinci açış dosya
/// kurmaz; `create_new` başka bir nedenle düştüyse (üst dizin yok, izin)
/// o da aynı nedenle düşer, hatanın türüne bakmak gerekmez.
fn open_for_write(real: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(real)
        .or_else(|_| OpenOptions::new().write(true).truncate(true).open(real))
}

impl fmt::Display for OutputError {
    /// Tek satırlık neden (uyarı satırları için); hata için
    /// [`OutputError::report`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputError::Io(e) => write!(f, "{e}"),
            OutputError::Refused(r) => write!(f, "{}", r.reason()),
        }
    }
}

impl Refusal {
    /// `= reason:` satırının metni.
    fn reason(&self) -> String {
        let culprit = self.culprit.display();
        let root = self.root.display();
        match self.kind {
            RefusalKind::Link => {
                let what = if cfg!(windows) {
                    lstr!(en: "a symbolic link or junction"; tr: "bir sembolik bağlantı ya da junction")
                } else {
                    lstr!(en: "a symbolic link"; tr: "bir sembolik bağlantı")
                };
                lstr!(
                    en: "'{culprit}' is {what}; Volt writes only inside its output directory \
                         and never through a link, so that a link (for example one in a cloned \
                         repository) cannot make it overwrite a file elsewhere";
                    tr: "'{culprit}' {what}; Volt yalnız çıktı dizininin içine yazar, hiçbir \
                         bağlantının üzerinden yazmaz: böylece bir bağlantı (ör. klonlanan bir \
                         depodaki) başka bir yerdeki dosyanın üzerine yazdıramaz"
                )
            }
            RefusalKind::ReparsePoint => lstr!(
                en: "'{culprit}' is a reparse point; Volt cannot tell where a write through it \
                     lands, so it writes only to plain files and directories inside its output \
                     directory";
                tr: "'{culprit}' bir reparse point; Volt onun üzerinden yapılan bir yazımın \
                     nereye düştüğünü bilemez, bu yüzden çıktı dizininin içinde yalnız sıradan \
                     dosya ve dizinlere yazar"
            ),
            RefusalKind::OutsideRoot => lstr!(
                en: "'{culprit}' is outside the output directory '{root}'; Volt writes only \
                     inside it";
                tr: "'{culprit}' çıktı dizininin ('{root}') dışında; Volt yalnız onun içine yazar"
            ),
        }
    }

    /// `= help:` satırının metni.
    fn help(&self) -> String {
        let culprit = self.culprit.display();
        let elsewhere = match self.origin {
            Origin::TargetDir => lstr!(
                en: "write to another output directory with --target-dir <dir>";
                tr: "--target-dir <dizin> ile başka bir çıktı dizini seçin"
            ),
            Origin::Project => lstr!(
                en: "create the project in another directory";
                tr: "projeyi başka bir dizinde oluşturun"
            ),
            Origin::Vcd => lstr!(
                en: "pass another --vcd path";
                tr: "başka bir --vcd yolu verin"
            ),
        };
        match self.kind {
            RefusalKind::Link | RefusalKind::ReparsePoint => lstr!(
                en: "remove the link '{culprit}' if you did not create it, or {elsewhere}";
                tr: "'{culprit}' bağlantısını siz kurmadıysanız kaldırın ya da {elsewhere}"
            ),
            RefusalKind::OutsideRoot => elsewhere,
        }
    }
}

impl OutputError {
    /// Hatayı basar ve çıkış kodu 3'ü döndürür (cli-contract §2: G/Ç
    /// hatası). G/Ç hatasının iletisi eskisiyle aynıdır; red ADR-0125'in
    /// üç parçasını taşır.
    pub(crate) fn report(&self, action: Action<'_>, path: &Path) -> ExitCode {
        let path = path.display();
        let head = match (action, self) {
            (Action::Write, OutputError::Io(err)) => lstr!(
                en: "error: cannot write '{path}': {err}";
                tr: "hata: '{path}' yazılamadı: {err}"
            ),
            (Action::Create, OutputError::Io(err)) => lstr!(
                en: "error: cannot create '{path}': {err}";
                tr: "hata: '{path}' oluşturulamadı: {err}"
            ),
            (Action::Copy { from }, OutputError::Io(err)) => lstr!(
                en: "error: cannot copy '{}' to '{path}': {err}", from.display();
                tr: "hata: '{}', '{path}' konumuna kopyalanamadı: {err}", from.display()
            ),
            (Action::Write, OutputError::Refused(_)) => lstr!(
                en: "error: cannot write '{path}'";
                tr: "hata: '{path}' yazılamadı"
            ),
            (Action::Create, OutputError::Refused(_)) => lstr!(
                en: "error: cannot create '{path}'";
                tr: "hata: '{path}' oluşturulamadı"
            ),
            (Action::Copy { from }, OutputError::Refused(_)) => lstr!(
                en: "error: cannot copy '{}' to '{path}'", from.display();
                tr: "hata: '{}', '{path}' konumuna kopyalanamadı", from.display()
            ),
        };
        eprintln!("{head}");
        if let OutputError::Refused(r) = self {
            let (reason, help) = (r.reason(), r.help());
            eprintln!(
                "{}",
                lstr!(
                    en: "  = reason: {reason}\n  = help: {help}";
                    tr: "  = neden: {reason}\n  = çözüm: {help}"
                )
            );
        }
        ExitCode::from(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("volt-output-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dizini");
        dir
    }

    /// Kökün dışına `..` ile çıkan hedef reddedilir; dosya yazılmaz.
    #[test]
    fn a_target_that_leaves_the_root_is_refused() {
        let dir = temp("outside");
        let root = OutputRoot::new(&dir.join("out"), Origin::TargetDir);
        root.create_dir_all(&dir.join("out").join("rtl"))
            .expect("dizin");

        let target = dir
            .join("out")
            .join("rtl")
            .join("..")
            .join("..")
            .join("escaped.sv");
        let err = root.write(&target, "x").expect_err("kök dışı reddedilmeli");

        match err {
            OutputError::Refused(r) => assert_eq!(r.kind, RefusalKind::OutsideRoot),
            OutputError::Io(e) => panic!("red bekleniyordu, G/Ç hatası geldi: {e}"),
        }
        assert!(!dir.join("escaped.sv").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Kökün içinde kalan bir yol (`.` parçası dahil) yazılır; kökün
    /// kendisi yoksa oluşturulur.
    #[test]
    fn a_plain_path_inside_the_root_is_written() {
        let dir = temp("plain");
        let root = OutputRoot::new(&dir.join("out"), Origin::TargetDir);

        root.create_dir_all(&dir.join("out").join(".").join("a"))
            .expect("dizin");
        root.write(&dir.join("out").join("a").join("f.txt"), "bir")
            .expect("yazım");
        root.write(&dir.join("out").join("a").join("f.txt"), "iki")
            .expect("üzerine yazım");

        let text = fs::read_to_string(dir.join("out").join("a").join("f.txt")).expect("oku");
        assert_eq!(text, "iki");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Dizin bağlantısı: Unix'te sembolik bağlantı, Windows'ta yetki
    /// istemeyen junction.
    fn link_dir(target: &Path, link: &Path) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).expect("bağlantı");
        #[cfg(windows)]
        {
            let made = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .expect("cmd");
            assert!(made.status.success(), "junction kurulamadı: {made:?}");
        }
    }

    fn assert_link_refusal(result: Result<(), OutputError>, culprit: &Path) {
        match result {
            Err(OutputError::Refused(r)) => {
                assert_eq!(r.kind, RefusalKind::Link);
                assert_eq!(r.culprit, culprit);
            }
            other => panic!("bağlantı reddi bekleniyordu: {other:?}"),
        }
    }

    /// Yolun üstündeki bir bağlantı her işlemi reddeder; hedef dizine
    /// dokunulmaz.
    #[test]
    fn a_link_above_the_target_refuses_every_operation() {
        let dir = temp("above");
        let out = dir.join("out");
        let elsewhere = dir.join("elsewhere");
        fs::create_dir_all(&out).expect("dizin");
        fs::create_dir_all(&elsewhere).expect("dizin");
        fs::write(elsewhere.join("keep.txt"), "k").expect("yaz");
        fs::write(dir.join("src.txt"), "s").expect("yaz");
        link_dir(&elsewhere, &out.join("sim"));
        let root = OutputRoot::new(&out, Origin::TargetDir);
        let sim = out.join("sim");

        assert_link_refusal(root.write(&sim.join("x"), "x"), &sim);
        assert_link_refusal(root.create_dir_all(&sim.join("y")), &sim);
        assert_link_refusal(root.copy(&dir.join("src.txt"), &sim.join("z")), &sim);
        assert_link_refusal(root.remove_file(&sim.join("keep.txt")), &sim);
        assert_link_refusal(root.remove_dir_all(&sim.join("sub")), &sim);
        assert_link_refusal(root.check(&sim.join("w.vcd")), &sim);
        assert_link_refusal(root.check_tree(&out), &sim);

        let mut left: Vec<_> = fs::read_dir(&elsewhere)
            .expect("oku")
            .map(|e| e.expect("girdi").file_name())
            .collect();
        left.sort();
        assert_eq!(left, ["keep.txt"]);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Silmede son parça bir bağlantıysa silinen bağlantının kendisidir:
    /// eski bir bağlantı yerinde kalsaydı araç onun üzerinden yazardı.
    #[test]
    fn removing_a_link_removes_the_link_not_its_target() {
        let dir = temp("unlink");
        let out = dir.join("out");
        let elsewhere = dir.join("elsewhere");
        fs::create_dir_all(&out).expect("dizin");
        fs::create_dir_all(&elsewhere).expect("dizin");
        fs::write(elsewhere.join("keep.txt"), "k").expect("yaz");
        link_dir(&elsewhere, &out.join("obj"));
        let root = OutputRoot::new(&out, Origin::TargetDir);

        root.remove_dir_all(&out.join("obj"))
            .expect("bağlantı silinmeli");

        assert!(fs::symlink_metadata(out.join("obj")).is_err());
        assert_eq!(
            fs::read_to_string(elsewhere.join("keep.txt")).expect("oku"),
            "k"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ağacın derinindeki bir bağlantı `check_tree`'yi reddettirir; yol
    /// kullanıcının gördüğü biçimdedir.
    #[test]
    fn check_tree_finds_a_nested_link() {
        let dir = temp("tree");
        let out = dir.join("out");
        fs::create_dir_all(out.join("a").join("b")).expect("dizin");
        fs::write(out.join("a").join("f.txt"), "f").expect("yaz");
        link_dir(&dir, &out.join("a").join("b").join("up"));
        let root = OutputRoot::new(&out, Origin::TargetDir);

        assert!(root.check_tree(&out.join("a").join("b").join("c")).is_ok());
        assert_link_refusal(
            root.check_tree(&out.join("a")),
            &out.join("a").join("b").join("up"),
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Okunamayan bir yol (burada dizin değil, bir dosya) denetlenemez:
    /// `check_tree` hata döndürür. Yalnız var olmayan dizin boş sayılır.
    #[test]
    fn check_tree_fails_when_it_cannot_read_a_directory() {
        let dir = temp("unreadable");
        let out = dir.join("out");
        fs::create_dir_all(&out).expect("dizin");
        fs::write(out.join("f.txt"), "f").expect("yaz");
        let root = OutputRoot::new(&out, Origin::TargetDir);

        let result = root.check_tree(&out.join("f.txt"));

        assert!(matches!(result, Err(OutputError::Io(_))), "{result:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Reparse point biti öteki özniteliklerden bağımsız okunur (dizin
    /// 0x10, arşiv 0x20, salt okunur 0x1).
    #[test]
    fn the_reparse_bit_is_read_alone() {
        assert!(has_reparse_bit(0x400));
        assert!(has_reparse_bit(0x400 | 0x10 | 0x20));
        assert!(!has_reparse_bit(0x10 | 0x20 | 0x1));
        assert!(!has_reparse_bit(0));
    }

    /// Bağlantı olmayan sıradan dosya ve dizin reddedilmez; bağlantı
    /// `Link`'tir (Windows'ta junction da).
    #[test]
    fn plain_entries_are_not_links_and_links_are() {
        let dir = temp("kind");
        fs::write(dir.join("f.txt"), "f").expect("yaz");
        fs::create_dir_all(dir.join("d")).expect("dizin");
        link_dir(&dir.join("d"), &dir.join("l"));
        let kind = |p: &str| link_kind(&fs::symlink_metadata(dir.join(p)).expect("meta"));

        assert_eq!(kind("f.txt"), None);
        assert_eq!(kind("d"), None);
        assert_eq!(kind("l"), Some(RefusalKind::Link));
        let _ = fs::remove_dir_all(&dir);
    }

    /// `is_reparse_point` Windows'ta dosya özniteliğini okur (junction bir
    /// reparse point'tir); başka platformda reparse point yoktur, her girdi
    /// için `false` (ADR-0109 A2 girdisinin varsayımı).
    #[test]
    fn reparse_points_are_read_on_windows_and_absent_elsewhere() {
        let dir = temp("reparse");
        fs::write(dir.join("f.txt"), "f").expect("yaz");
        fs::create_dir_all(dir.join("d")).expect("dizin");
        link_dir(&dir.join("d"), &dir.join("l"));
        let reparse = |p: &str| is_reparse_point(&fs::symlink_metadata(dir.join(p)).expect("meta"));

        assert!(!reparse("f.txt"));
        assert!(!reparse("d"));
        assert_eq!(reparse("l"), cfg!(windows));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Tek satırlık metin (dalga formu uyarısı): red nedeni ya da G/Ç
    /// hatasının kendisi.
    #[test]
    fn display_is_the_reason_in_one_line() {
        let refused = OutputError::Refused(Refusal {
            culprit: PathBuf::from("build"),
            kind: RefusalKind::Link,
            root: PathBuf::from("."),
            origin: Origin::TargetDir,
        });
        let text = refused.to_string();
        assert!(text.starts_with("'build' is a symbolic link"), "{text}");
        assert!(!text.contains('\n'), "{text}");
        let io = OutputError::Io(io::Error::other("disk dolu"));
        assert_eq!(io.to_string(), "disk dolu");
    }

    /// Kök yokken denetim başarılıdır (altında hiçbir şey yok); yazım
    /// std'nin hatasını verir.
    #[test]
    fn a_missing_root_has_nothing_to_refuse() {
        let dir = temp("missing");
        let root = OutputRoot::new(&dir.join("absent"), Origin::Vcd);

        assert!(root.check(&dir.join("absent").join("w.vcd")).is_ok());
        assert!(matches!(
            root.write(&dir.join("absent").join("w.vcd"), "x"),
            Err(OutputError::Io(_))
        ));
        let _ = fs::remove_dir_all(&dir);
    }
}
