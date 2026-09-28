//! Gömülü proje şablonları (ADR-0084 §5).
//!
//! Kaynak `templates/<ad>/` altındaki gerçek dosyalardır; `include_str!`
//! ikiliye gömer — kurulum dizininden bağımsız, repo'daki dosyayla bayt
//! bayt aynı. `.volt` dosyaları aynen yazılır (repo'da doğrudan derlenir,
//! çıktı doğrulama ağının korpusundadır); yer tutucu `{{name}}` yalnız
//! `Volt.toml` ve `README.md`'de.

/// Tek şablon.
pub(crate) struct Template {
    pub name: &'static str,
    pub summary_en: &'static str,
    pub summary_tr: &'static str,
    /// (yol, içerik); yazım sırası `Next:` listesinin sırasıdır.
    pub files: &'static [(&'static str, &'static str)],
}

/// Yer tutucu — proje adı.
pub(crate) const NAME_PLACEHOLDER: &str = "{{name}}";

macro_rules! template_files {
    ($dir:literal: $($file:literal),+ $(,)?) => {
        &[$(($file, include_str!(concat!("../../../../templates/", $dir, "/", $file)))),+]
    };
}

/// Varsayılan şablon.
pub(crate) const DEFAULT: &str = "minimal";

pub(crate) const TEMPLATES: &[Template] = &[
    Template {
        name: "minimal",
        summary_en: "counter with a test and two contracts",
        summary_tr: "test ve iki kontratlı sayaç",
        files: template_files!("minimal":
            "Volt.toml", "counter.volt", "counter_test.volt", "README.md", ".gitignore"),
    },
    Template {
        name: "cdc",
        summary_en: "two clock domains, a sync() crossing, tests",
        summary_tr: "iki saat alanı, sync() geçişi, testler",
        files: template_files!("cdc":
            "Volt.toml", "event_counter.volt", "event_counter_test.volt", "README.md", ".gitignore"),
    },
    Template {
        name: "fifo",
        summary_en: "packet buffer on the built-in SyncFifo, struct values",
        summary_tr: "yerleşik SyncFifo üstünde paket tamponu, struct değerleri",
        files: template_files!("fifo":
            "Volt.toml", "packet_buffer.volt", "packet_buffer_test.volt", "README.md", ".gitignore"),
    },
    Template {
        name: "mmio",
        summary_en: "@mmio register map, AXI4-Lite slave, C/Rust drivers",
        summary_tr: "@mmio register haritası, AXI4-Lite slave, C/Rust sürücüleri",
        files: template_files!("mmio":
            "Volt.toml", "blinker.volt", "blinker_test.volt", "README.md", ".gitignore"),
    },
];

pub(crate) fn find(name: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.name == name)
}

/// Dosya içeriği, yer tutucu proje adıyla değiştirilmiş.
pub(crate) fn render(content: &str, project: &str) -> String {
    content.replace(NAME_PLACEHOLDER, project)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_has_the_flat_layout() {
        for t in TEMPLATES {
            let names: Vec<&str> = t.files.iter().map(|(p, _)| *p).collect();
            for required in ["Volt.toml", "README.md", ".gitignore"] {
                assert!(names.contains(&required), "{}: {required} yok", t.name);
            }
            // Bir tasarım dosyası ve onun `_test.volt` kardeşi.
            let designs: Vec<&&str> = names
                .iter()
                .filter(|n| n.ends_with(".volt") && !n.ends_with("_test.volt"))
                .collect();
            assert_eq!(designs.len(), 1, "{}: {designs:?}", t.name);
            let test = designs[0].replace(".volt", "_test.volt");
            assert!(names.contains(&test.as_str()), "{}: {test} yok", t.name);
            assert!(
                names.iter().all(|p| !p.contains('/')),
                "{}: düz değil",
                t.name
            );
        }
    }

    #[test]
    fn placeholder_only_in_manifest_and_readme() {
        for t in TEMPLATES {
            for (path, content) in t.files {
                let has = content.contains(NAME_PLACEHOLDER);
                let expected = matches!(*path, "Volt.toml" | "README.md");
                assert_eq!(has, expected, "{}/{path}", t.name);
            }
        }
    }

    #[test]
    fn default_template_exists() {
        assert!(find(DEFAULT).is_some());
        assert!(find("no-such").is_none());
    }
}
