//! `volt new <ad>` / `volt init` — proje başlatma (ADR-0084 Bölüm 2).
//!
//! Şablon gömülüdür (`templates.rs`), yapı düzdür (`volt test` yalnız
//! çalışma dizinindeki `*_test.volt`'u tarar). Üzerine yazma yoktur,
//! `--force` da yoktur: `volt new` dolu dizine, `volt init` mevcut bir
//! şablon dosyasının üstüne yazmayı reddeder ve çakışanları listeler.
//!
//! Çıkış kodları (cli-contract.md §9b): 0 başarı, 2 geçersiz ad /
//! şablon / dolu dizin / çakışma, 3 yazma hatası. Durum satırları
//! stderr'e (§11), `--list` stdout'a (veri).

pub(crate) mod templates;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use volt_diagnostics::lstr;

use templates::Template;

/// `volt new` / `volt init` ortak seçenekleri.
pub(crate) struct NewOptions<'a> {
    pub template: &'a str,
}

/// `volt new --list`.
pub(crate) fn list() -> ExitCode {
    let title = lstr!(
        en: "Templates — 'volt new <name> --template <template>':";
        tr: "Şablonlar — 'volt new <ad> --template <şablon>':"
    );
    println!("{title}\n");
    for t in templates::TEMPLATES {
        let summary = match volt_diagnostics::lang() {
            volt_diagnostics::Lang::Tr => t.summary_tr,
            volt_diagnostics::Lang::En => t.summary_en,
        };
        let default = if t.name == templates::DEFAULT {
            lstr!(en: " (default)"; tr: " (varsayılan)")
        } else {
            String::new()
        };
        println!("  {:<9} {summary}{default}", t.name);
    }
    ExitCode::SUCCESS
}

/// `volt new <ad>`: `<ad>/` dizinini oluşturup şablonu yazar.
pub(crate) fn new(name: &str, opts: NewOptions) -> ExitCode {
    let template = match checked(name, opts.template) {
        Ok(t) => t,
        Err(code) => return code,
    };
    let dir = PathBuf::from(name);
    if dir.exists() && !is_empty_dir(&dir) {
        eprintln!(
            "{}",
            lstr!(
                en: "error: destination '{name}' already exists and is not empty\n\n  \
                     = reason: volt new never overwrites files\n  \
                     = help: pick another name, or run 'volt init' inside an existing directory";
                tr: "hata: hedef '{name}' zaten var ve boş değil\n\n  \
                     = neden: volt new hiçbir dosyanın üzerine yazmaz\n  \
                     = çözüm: başka bir ad seçin ya da mevcut bir dizinin içinde 'volt init' çalıştırın"
            )
        );
        return ExitCode::from(2);
    }
    if let Err(code) = write_files(&dir, template, name) {
        return code;
    }
    report(template, name, Some(name));
    ExitCode::SUCCESS
}

/// `volt init`: çalışma dizinine yazar; ad `--name` ya da dizin adı.
pub(crate) fn init(name: Option<&str>, opts: NewOptions) -> ExitCode {
    let dir = PathBuf::from(".");
    let dir_name = std::env::current_dir()
        .ok()
        .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()));
    let Some(name) = name.map(str::to_string).or(dir_name) else {
        eprintln!(
            "{}",
            lstr!(
                en: "error: cannot derive a project name from the current directory\n\n  \
                     = help: pass one with --name <name>";
                tr: "hata: çalışma dizininden proje adı çıkarılamadı\n\n  \
                     = çözüm: --name <ad> ile verin"
            )
        );
        return ExitCode::from(2);
    };
    let template = match checked(&name, opts.template) {
        Ok(t) => t,
        Err(code) => return code,
    };
    let clashes: Vec<&str> = template
        .files
        .iter()
        .map(|(p, _)| *p)
        .filter(|p| dir.join(p).exists())
        .collect();
    if !clashes.is_empty() {
        let list = clashes.join(", ");
        eprintln!(
            "{}",
            lstr!(
                en: "error: the current directory already has {list}\n\n  \
                     = reason: volt init never overwrites files\n  \
                     = help: move or delete them, or start in a new directory with 'volt new <name>'";
                tr: "hata: çalışma dizininde zaten {list} var\n\n  \
                     = neden: volt init hiçbir dosyanın üzerine yazmaz\n  \
                     = çözüm: onları taşıyın ya da silin, veya 'volt new <ad>' ile yeni bir dizinde başlayın"
            )
        );
        return ExitCode::from(2);
    }
    if let Err(code) = write_files(&dir, template, &name) {
        return code;
    }
    report(template, &name, None);
    ExitCode::SUCCESS
}

/// Ad ve şablon doğrulaması; hata iletisini basar (çıkış 2).
fn checked(name: &str, template: &str) -> Result<&'static Template, ExitCode> {
    if let Err(problem) = validate_name(name) {
        let reason = problem.reason();
        let hint = problem.hint(name);
        eprintln!(
            "{}",
            lstr!(
                en: "error: '{name}' is not a valid project name\n\n  \
                     = reason: {reason}\n  \
                     = help: {hint}";
                tr: "hata: '{name}' geçerli bir proje adı değil\n\n  \
                     = neden: {reason}\n  \
                     = çözüm: {hint}"
            )
        );
        return Err(ExitCode::from(2));
    }
    templates::find(template).ok_or_else(|| {
        let known: Vec<&str> = templates::TEMPLATES.iter().map(|t| t.name).collect();
        let known = known.join(", ");
        eprintln!(
            "{}",
            lstr!(
                en: "error: unknown template '{template}'\n\n  \
                     = help: available templates: {known} (volt new --list)";
                tr: "hata: bilinmeyen şablon '{template}'\n\n  \
                     = çözüm: mevcut şablonlar: {known} (volt new --list)"
            )
        );
        ExitCode::from(2)
    })
}

/// Ad neden geçersiz.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NameProblem {
    Empty,
    /// Geçersiz karakter ya da rakamla başlama.
    NotIdentifier,
    /// Volt anahtar sözcüğü (lexer `Ident` saymıyor).
    VoltKeyword,
    /// Hedef dilde ayrılmış (ADR-0078 tablosu).
    Reserved(&'static str),
}

impl NameProblem {
    fn reason(&self) -> String {
        match self {
            NameProblem::Empty => lstr!(en: "the name is empty"; tr: "ad boş"),
            NameProblem::NotIdentifier => lstr!(
                en: "a project name is a Volt identifier: letters, digits and '_', not starting with a digit";
                tr: "proje adı bir Volt tanımlayıcısıdır: harf, rakam ve '_'; rakamla başlamaz"
            ),
            NameProblem::VoltKeyword => lstr!(
                en: "it is a Volt keyword";
                tr: "bir Volt anahtar sözcüğü"
            ),
            NameProblem::Reserved(lang) => lstr!(
                en: "it is a reserved word in {lang}, which Volt generates";
                tr: "Volt'un ürettiği {lang} dilinde ayrılmış bir sözcük"
            ),
        }
    }

    fn hint(&self, name: &str) -> String {
        let fixed: String = name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let fixed = if fixed.starts_with(|c: char| c.is_ascii_digit()) {
            format!("_{fixed}")
        } else {
            fixed
        };
        match self {
            NameProblem::NotIdentifier if !fixed.is_empty() && validate_name(&fixed).is_ok() => {
                lstr!(en: "try '{fixed}'"; tr: "'{fixed}' deneyin")
            }
            _ => lstr!(
                en: "choose another name, e.g. 'my_design'";
                tr: "başka bir ad seçin, ör. 'my_design'"
            ),
        }
    }
}

/// Geçerli Volt tanımlayıcısı, Volt anahtar sözcüğü değil, ADR-0078
/// tablosunda (SV, Rust, C/C++) ayrılmış değil.
pub(crate) fn validate_name(name: &str) -> Result<(), NameProblem> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(NameProblem::Empty);
    };
    let ident = (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !ident {
        return Err(NameProblem::NotIdentifier);
    }
    let lexed = volt_syntax::tokenize(volt_span::FileId(0), name);
    let is_ident = matches!(
        lexed.tokens.as_slice(),
        [tok] if tok.kind == volt_syntax::TokenKind::Ident
    );
    if !is_ident {
        return Err(NameProblem::VoltKeyword);
    }
    if volt_ast::reserved::is_sv_keyword(name) {
        return Err(NameProblem::Reserved("SystemVerilog"));
    }
    if let Some(lang) = volt_ast::reserved::sw_keyword_language(name) {
        return Err(NameProblem::Reserved(lang));
    }
    Ok(())
}

fn is_empty_dir(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none())
}

/// Şablonu yazar; ilk hatada durur (çıkış 3).
fn write_files(dir: &Path, template: &Template, name: &str) -> Result<(), ExitCode> {
    let fail = |path: &Path, e: std::io::Error| {
        let path = path.display();
        eprintln!(
            "{}",
            lstr!(
                en: "error: cannot write '{path}': {e}";
                tr: "hata: '{path}' yazılamadı: {e}"
            )
        );
        ExitCode::from(3)
    };
    std::fs::create_dir_all(dir).map_err(|e| fail(dir, e))?;
    for (file, content) in template.files {
        let path = dir.join(file);
        std::fs::write(&path, templates::render(content, name)).map_err(|e| fail(&path, e))?;
    }
    Ok(())
}

/// Başarı raporu ve `Next:` satırları (stderr).
fn report(template: &Template, name: &str, cd: Option<&str>) {
    let kind = template.name;
    let files: Vec<&str> = template.files.iter().map(|(p, _)| *p).collect();
    let files = files.join(", ");
    let created = match cd {
        Some(dir) => lstr!(
            en: "    Created {kind} project '{name}' in {dir}";
            tr: " Oluşturuldu {kind} projesi '{name}', dizin: {dir}"
        ),
        None => lstr!(
            en: "    Created {kind} project '{name}' in the current directory";
            tr: " Oluşturuldu {kind} projesi '{name}': çalışma dizini"
        ),
    };
    eprintln!("{created}");
    eprintln!("             {files}");
    let mut steps: Vec<String> = Vec::new();
    if let Some(dir) = cd {
        steps.push(format!("cd {dir}"));
    }
    // Proje kipi (ADR-0095): komutlar dosya adı almaz.
    steps.push("volt check".to_string());
    steps.push("volt test".to_string());
    let label = lstr!(en: "       Next:"; tr: "   Sıradaki:");
    for (i, step) in steps.iter().enumerate() {
        let lead = if i == 0 {
            label.as_str()
        } else {
            "            "
        };
        eprintln!("{lead} {step}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_valid_names() {
        for ok in ["blinky", "my_design", "_x", "Counter2", "uart_rx"] {
            assert_eq!(validate_name(ok), Ok(()), "{ok}");
        }
    }

    #[test]
    fn non_identifiers_are_rejected() {
        assert_eq!(validate_name(""), Err(NameProblem::Empty));
        for bad in ["my-design", "2fast", "a b", "x.y", "ç", "../up"] {
            assert_eq!(validate_name(bad), Err(NameProblem::NotIdentifier), "{bad}");
        }
    }

    #[test]
    fn keywords_are_rejected_from_the_shared_tables() {
        for kw in ["module", "domain", "impl", "reg", "match"] {
            assert_eq!(validate_name(kw), Err(NameProblem::VoltKeyword), "{kw}");
        }
        assert_eq!(
            validate_name("always"),
            Err(NameProblem::Reserved("SystemVerilog"))
        );
        assert_eq!(validate_name("crate"), Err(NameProblem::Reserved("Rust")));
        assert_eq!(
            validate_name("volatile"),
            Err(NameProblem::Reserved("C/C++"))
        );
    }

    #[test]
    fn hint_fixes_hyphens_and_leading_digits() {
        assert!(NameProblem::NotIdentifier
            .hint("my-design")
            .contains("my_design"));
        assert!(NameProblem::NotIdentifier.hint("2fast").contains("_2fast"));
    }
}
