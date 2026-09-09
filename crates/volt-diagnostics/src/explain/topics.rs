//! Konu bazlı `volt explain <konu>` sayfaları (F4b).
//!
//! Hata kodları tek bir tanıyı açıklar; konular ise kurulum ve iş
//! akışı gibi koda bağlı olmayan bilgileri taşır (ör. `verify-setup`).
//! Kod çözümlemesi başarısız olduğunda driver burayı dener; eşleşme
//! yoksa bilinmeyen-kod yolu (çıkış 2) işler.

use crate::messages::Lang;

use super::{indent_code, wrap, HEADER_STYLE, MIN_WIDTH, RESET_STYLE, TITLE_STYLE};

/// Tek konu sayfası: başlık + özet + (bölüm başlığı, gövde) listesi.
///
/// Gövde satırlarından iki boşlukla başlayanlar kod/komut kabul edilir
/// ve SARILMAZ; kalan paragraflar `wrap` ile sarılır.
struct Topic {
    title: &'static str,
    summary: &'static str,
    sections: &'static [(&'static str, &'static str)],
    more: &'static [&'static str],
}

/// Tanımlı konu adları (küçük harf, tire ile).
pub const TOPIC_NAMES: &[&str] = &["verify-setup", "simulation-setup"];

fn lookup(name: &str, lang: Lang) -> Option<Topic> {
    match (name, lang) {
        ("verify-setup", Lang::En) => Some(Topic {
            title: "Setting up formal verification",
            summary: "'volt verify' proves the contracts in your design with SymbiYosys \
                      (sby), the open-source formal verification driver for Yosys. \
                      This page explains how to install it.",
            sections: &[
                (
                    "WHAT YOU NEED",
                    "  yosys          synthesis front-end\n\
                     \x20 sby            SymbiYosys verification driver\n\
                     \x20 an SMT solver  z3 (default), boolector or yices",
                ),
                (
                    "INSTALL",
                    "Linux:\n\
                     \x20 apt install yosys z3\n\
                     \x20 pip install symbiyosys\n\n\
                     Docker:\n\
                     \x20 docker pull hdlc/formal\n\
                     \x20 docker run --rm -v $PWD:/work -w /work hdlc/formal \\\n\
                     \x20     sby -f build/formal/<module>.sby\n\n\
                     Windows:\n\
                     \x20 use WSL or Docker (no native binaries are distributed)\n\n\
                     Everything in one download — the YosysHQ OSS CAD Suite ships yosys, sby and all solvers:\n\
                     \x20 https://github.com/YosysHQ/oss-cad-suite-build",
                ),
                (
                    "NOTE",
                    "'volt build' and 'volt check' do not need SymbiYosys; only 'volt verify' \
                     calls it. If sby is installed somewhere unusual, point the VOLT_SBY \
                     environment variable at the executable.",
                ),
            ],
            more: &["https://volthdl.org/guide/verify-setup"],
        }),
        ("verify-setup", Lang::Tr) => Some(Topic {
            title: "Formal doğrulama kurulumu",
            summary: "'volt verify', tasarımınızdaki kontratları Yosys'in açık kaynak \
                      formal doğrulama sürücüsü SymbiYosys (sby) ile kanıtlar. Bu sayfa \
                      kurulumu anlatır.",
            sections: &[
                (
                    "GEREKENLER",
                    "  yosys          sentez ön ucu\n\
                     \x20 sby            SymbiYosys doğrulama sürücüsü\n\
                     \x20 bir SMT çözücü z3 (varsayılan), boolector ya da yices",
                ),
                (
                    "KURULUM",
                    "Linux:\n\
                     \x20 apt install yosys z3\n\
                     \x20 pip install symbiyosys\n\n\
                     Docker:\n\
                     \x20 docker pull hdlc/formal\n\
                     \x20 docker run --rm -v $PWD:/work -w /work hdlc/formal \\\n\
                     \x20     sby -f build/formal/<modul>.sby\n\n\
                     Windows:\n\
                     \x20 WSL ya da Docker kullanın (yerel ikili dağıtılmıyor)\n\n\
                     Tek indirmede her şey — YosysHQ OSS CAD Suite yosys, sby ve tüm çözücüleri içerir:\n\
                     \x20 https://github.com/YosysHQ/oss-cad-suite-build",
                ),
                (
                    "NOT",
                    "'volt build' ve 'volt check' SymbiYosys gerektirmez; yalnız 'volt verify' \
                     onu çağırır. sby alışılmadık bir yerdeyse VOLT_SBY ortam değişkenini \
                     çalıştırılabilir dosyaya yöneltin.",
                ),
            ],
            more: &["https://volthdl.org/guide/verify-setup"],
        }),
        ("simulation-setup", Lang::En) => Some(Topic {
            title: "Setting up simulation",
            summary: "'volt run' and 'volt test' simulate your design with Verilator, \
                      the open-source SystemVerilog simulator. Volt compiles your design \
                      to SystemVerilog, generates a C++ testbench, builds both with \
                      Verilator and runs the result. This page explains the setup.",
            sections: &[
                (
                    "WHAT YOU NEED",
                    "  verilator      the simulator (version 5.x recommended)\n\
                     \x20 a C++ compiler and make (Verilator drives them)",
                ),
                (
                    "INSTALL",
                    "Linux:\n\
                     \x20 apt install verilator\n\n\
                     macOS:\n\
                     \x20 brew install verilator\n\n\
                     Docker:\n\
                     \x20 docker pull verilator/verilator\n\n\
                     Windows:\n\
                     \x20 use WSL or Docker (no native binaries are distributed)",
                ),
                (
                    "TESTS",
                    "Write 'test \"name\" { ... }' blocks next to your modules, or put \
                     them in a sibling file: tests in X_test.volt can instantiate the \
                     modules of X.volt automatically. Inside a test, step(n) advances \
                     the clock, reset() pulses the implicit reset line, and \
                     assert_eq/assert_ne/assert_true/assert_false check output ports.\n\n\
                     \x20 volt run design.volt --cycles 20 --vcd waves.vcd\n\
                     \x20 volt test my_design_test.volt",
                ),
                (
                    "NOTE",
                    "'volt build', 'volt check' and 'volt verify' do not need Verilator; \
                     only 'volt run' and 'volt test' call it. If Verilator is installed \
                     somewhere unusual, point the VOLT_VERILATOR environment variable at \
                     the executable.",
                ),
            ],
            more: &["https://volthdl.org/guide/simulation-setup"],
        }),
        ("simulation-setup", Lang::Tr) => Some(Topic {
            title: "Simülasyon kurulumu",
            summary: "'volt run' ve 'volt test', tasarımınızı açık kaynak SystemVerilog \
                      simülatörü Verilator ile simüle eder. Volt tasarımı SystemVerilog'a \
                      derler, bir C++ testbench üretir, ikisini Verilator ile derleyip \
                      sonucu koşturur. Bu sayfa kurulumu anlatır.",
            sections: &[
                (
                    "GEREKENLER",
                    "  verilator      simülatör (5.x sürümü önerilir)\n\
                     \x20 bir C++ derleyicisi ve make (Verilator kendisi çağırır)",
                ),
                (
                    "KURULUM",
                    "Linux:\n\
                     \x20 apt install verilator\n\n\
                     macOS:\n\
                     \x20 brew install verilator\n\n\
                     Docker:\n\
                     \x20 docker pull verilator/verilator\n\n\
                     Windows:\n\
                     \x20 WSL ya da Docker kullanın (yerel ikili dağıtılmıyor)",
                ),
                (
                    "TESTLER",
                    "Modüllerinizin yanına 'test \"ad\" { ... }' blokları yazın ya da \
                     kardeş dosyaya koyun: X_test.volt içindeki testler X.volt'un \
                     modüllerini otomatik örnekleyebilir. Test içinde step(n) saati \
                     ilerletir, reset() örtük reset hattını atımlar; \
                     assert_eq/assert_ne/assert_true/assert_false çıkış portlarını \
                     denetler.\n\n\
                     \x20 volt run tasarim.volt --cycles 20 --vcd dalga.vcd\n\
                     \x20 volt test tasarim_test.volt",
                ),
                (
                    "NOT",
                    "'volt build', 'volt check' ve 'volt verify' Verilator gerektirmez; \
                     yalnız 'volt run' ve 'volt test' onu çağırır. Verilator alışılmadık \
                     bir yerdeyse VOLT_VERILATOR ortam değişkenini çalıştırılabilir \
                     dosyaya yöneltin.",
                ),
            ],
            more: &["https://volthdl.org/guide/simulation-setup"],
        }),
        _ => None,
    }
}

/// Konu sayfasını §9 açıklama biçiminde üretir; konu tanımsızsa `None`.
///
/// Bölüm gövdelerinde iki boşlukla başlayan satırlar (komutlar) aynen
/// korunur, düz paragraflar `width` sütununa sarılır.
pub fn render_topic(name: &str, lang: Lang, width: usize, color: bool) -> Option<String> {
    let topic = lookup(&name.trim().to_ascii_lowercase(), lang)?;
    let width = width.max(MIN_WIDTH);
    let paint = |style: &str, text: &str| {
        if color {
            format!("{style}{text}{RESET_STYLE}")
        } else {
            text.to_string()
        }
    };

    let mut out = String::new();
    out.push_str(&paint(TITLE_STYLE, &format!("{name}: {}", topic.title)));
    out.push_str("\n\n");
    out.push_str(&wrap(topic.summary, width));
    out.push_str("\n\n");

    for (header, body) in topic.sections {
        out.push_str(&paint(HEADER_STYLE, header));
        out.push_str("\n\n");
        out.push_str(&render_body(body, width));
        out.push_str("\n\n");
    }

    let more = match lang {
        Lang::En => "FOR MORE",
        Lang::Tr => "DAHA FAZLA",
    };
    out.push_str(&paint(HEADER_STYLE, more));
    out.push('\n');
    for doc in topic.more {
        out.push_str(&format!("  {doc}\n"));
    }
    Some(out)
}

/// Gövdeyi paragraf paragraf işler: kod satırı içeren paragraflar
/// `indent_code` disipliniyle aynen kalır, düz paragraflar sarılır.
fn render_body(body: &str, width: usize) -> String {
    body.split("\n\n")
        .map(|paragraph| {
            if paragraph.lines().any(|l| l.starts_with(' ')) {
                indent_code(paragraph.trim_end())
            } else {
                wrap(paragraph, width)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
