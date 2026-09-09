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
pub const TOPIC_NAMES: &[&str] = &[
    "getting-started",
    "domains",
    "contracts",
    "stdlib",
    "verify-setup",
    "simulation-setup",
];

/// `volt explain --topics` — konu adı + tek satır özet listesi.
pub fn render_topic_list(lang: Lang) -> String {
    let (title, footer) = match lang {
        Lang::En => (
            "Topics — 'volt explain <topic>':",
            "Diagnostic codes: 'volt explain E3001' or 'volt explain --list'.",
        ),
        Lang::Tr => (
            "Konular — 'volt explain <konu>':",
            "Tanı kodları: 'volt explain E3001' ya da 'volt explain --list'.",
        ),
    };
    let mut out = String::new();
    out.push_str(title);
    out.push_str("\n\n");
    for name in TOPIC_NAMES {
        let summary = match (*name, lang) {
            ("getting-started", Lang::En) => "your first design, from file to waveform",
            ("getting-started", Lang::Tr) => "ilk tasarımınız: dosyadan dalga formuna",
            ("domains", Lang::En) => "clock domains, resets and CDC safety",
            ("domains", Lang::Tr) => "saat alanları, reset'ler ve CDC güvenliği",
            ("contracts", Lang::En) => "requires/ensures/invariant/cover",
            ("contracts", Lang::Tr) => "requires/ensures/invariant/cover",
            ("stdlib", Lang::En) => "the 11 built-in components",
            ("stdlib", Lang::Tr) => "11 yerleşik bileşen",
            ("verify-setup", Lang::En) => "installing SymbiYosys for 'volt verify'",
            ("verify-setup", Lang::Tr) => "'volt verify' için SymbiYosys kurulumu",
            ("simulation-setup", Lang::En) => "installing Verilator for 'volt run'",
            ("simulation-setup", Lang::Tr) => "'volt run' için Verilator kurulumu",
            _ => "",
        };
        out.push_str(&format!("  {name:<18} {summary}\n"));
    }
    out.push('\n');
    out.push_str(footer);
    out.push('\n');
    out
}

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
        ("getting-started", Lang::En) => Some(Topic {
            title: "Your first Volt design",
            summary: "Volt compiles Rust-like source to SystemVerilog and checks clock \
                      domain safety in the type system. This page walks through one \
                      design from file to waveform.",
            sections: &[
                (
                    "WRITE",
                    "Save this as blink.volt:\n\n\
                     \x20 module Blink {\n\
                     \x20     in  clk : clock\n\
                     \x20     out led : bool\n\n\
                     \x20     reg count_r : u8 = 0\n\n\
                     \x20     on clk {\n\
                     \x20         count_r <= count_r + 1\n\
                     \x20     }\n\n\
                     \x20     led = count_r[7]\n\
                     \x20 }",
                ),
                (
                    "COMPILE AND RUN",
                    "  volt check blink.volt        errors only, no output files\n\
                     \x20 volt build blink.volt        writes build/rtl/blink.sv\n\
                     \x20 volt run blink.volt --cycles 300 --vcd waves.vcd\n\n\
                     Open waves.vcd with any viewer (e.g. gtkwave) to see the led toggle.",
                ),
                (
                    "NEXT STEPS",
                    "Add a contract ('invariant: count_r <= 255') and prove it with \
                     'volt verify blink.volt'. Write a 'test \"name\" { ... }' block and \
                     run it with 'volt test'. Related topics: 'volt explain contracts', \
                     'volt explain domains', 'volt explain stdlib'.",
                ),
            ],
            more: &["https://volthdl.org/guide/getting-started"],
        }),
        ("getting-started", Lang::Tr) => Some(Topic {
            title: "İlk Volt tasarımınız",
            summary: "Volt, Rust benzeri kaynağı SystemVerilog'a derler ve saat alanı \
                      güvenliğini tip sisteminde denetler. Bu sayfa bir tasarımı dosyadan \
                      dalga formuna kadar adım adım gösterir.",
            sections: &[
                (
                    "YAZIN",
                    "Bunu blink.volt olarak kaydedin:\n\n\
                     \x20 module Blink {\n\
                     \x20     in  clk : clock\n\
                     \x20     out led : bool\n\n\
                     \x20     reg count_r : u8 = 0\n\n\
                     \x20     on clk {\n\
                     \x20         count_r <= count_r + 1\n\
                     \x20     }\n\n\
                     \x20     led = count_r[7]\n\
                     \x20 }",
                ),
                (
                    "DERLEYİN VE KOŞTURUN",
                    "  volt check blink.volt        yalnız hatalar, çıktı dosyası yok\n\
                     \x20 volt build blink.volt        build/rtl/blink.sv üretir\n\
                     \x20 volt run blink.volt --cycles 300 --vcd dalga.vcd\n\n\
                     dalga.vcd'yi bir görüntüleyiciyle (ör. gtkwave) açıp led'in \
                     değişimini izleyin.",
                ),
                (
                    "SONRAKİ ADIMLAR",
                    "Bir kontrat ekleyin ('invariant: count_r <= 255') ve 'volt verify \
                     blink.volt' ile kanıtlayın. Bir 'test \"ad\" { ... }' bloğu yazıp \
                     'volt test' ile koşturun. İlgili konular: 'volt explain contracts', \
                     'volt explain domains', 'volt explain stdlib'.",
                ),
            ],
            more: &["https://volthdl.org/guide/getting-started"],
        }),
        ("domains", Lang::En) => Some(Topic {
            title: "Clock domains and CDC safety",
            summary: "Every signal in Volt lives in a clock domain. Mixing two domains \
                      without an explicit synchronizer is a compile error (E3001) — this \
                      is Volt's core promise: no silent clock-domain-crossing bugs.",
            sections: &[
                (
                    "SINGLE CLOCK",
                    "With one clock you never write the word 'domain'; everything is \
                     inferred:\n\n\
                     \x20 module M {\n\
                     \x20     in  clk : clock\n\
                     \x20     ...\n\
                     \x20 }",
                ),
                (
                    "TWO OR MORE CLOCKS",
                    "Declare a domain per clock and tag the ports:\n\n\
                     \x20 domain Fast { clock = posedge, reset = sync active_high }\n\
                     \x20 domain Slow { clock = posedge, reset = sync active_high }\n\n\
                     \x20 module Bridge {\n\
                     \x20     in  fast_clk : clock @Fast\n\
                     \x20     in  slow_clk : clock @Slow\n\
                     \x20     in  a : bool @Fast\n\
                     \x20     ...\n\
                     \x20 }\n\n\
                     A domain fixes the clock edge (posedge/negedge) and the reset \
                     style (sync/async, active_high/active_low, or none).",
                ),
                (
                    "CROSSING DOMAINS",
                    "Reading an @Fast signal under the Slow clock is E3001. Cross with \
                     sync() (two-flop synchronizer, single bits) or a stdlib bridge \
                     (AsyncFifo, HandshakeSync, PulseSync) for words and pulses — see \
                     'volt explain stdlib'.",
                ),
            ],
            more: &["https://volthdl.org/guide/domains"],
        }),
        ("domains", Lang::Tr) => Some(Topic {
            title: "Saat alanları ve CDC güvenliği",
            summary: "Volt'ta her sinyal bir saat alanında yaşar. İki alanı açık bir \
                      senkronizör olmadan karıştırmak derleme hatasıdır (E3001) — Volt'un \
                      temel vaadi: sessiz saat alanı geçişi hatası yok.",
            sections: &[
                (
                    "TEK SAAT",
                    "Tek saatte 'domain' sözcüğünü hiç yazmazsınız; her şey çıkarsanır:\n\n\
                     \x20 module M {\n\
                     \x20     in  clk : clock\n\
                     \x20     ...\n\
                     \x20 }",
                ),
                (
                    "İKİ VEYA DAHA FAZLA SAAT",
                    "Saat başına bir alan bildirin ve portları etiketleyin:\n\n\
                     \x20 domain Fast { clock = posedge, reset = sync active_high }\n\
                     \x20 domain Slow { clock = posedge, reset = sync active_high }\n\n\
                     \x20 module Bridge {\n\
                     \x20     in  fast_clk : clock @Fast\n\
                     \x20     in  slow_clk : clock @Slow\n\
                     \x20     in  a : bool @Fast\n\
                     \x20     ...\n\
                     \x20 }\n\n\
                     Alan; saat kenarını (posedge/negedge) ve reset biçimini \
                     (sync/async, active_high/active_low ya da none) sabitler.",
                ),
                (
                    "ALANLAR ARASI GEÇİŞ",
                    "@Fast bir sinyali Slow saati altında okumak E3001'dir. Tek bitler \
                     için sync() (çift flip-flop senkronizörü), sözcük ve darbeler için \
                     stdlib köprüleri (AsyncFifo, HandshakeSync, PulseSync) kullanın — \
                     bkz. 'volt explain stdlib'.",
                ),
            ],
            more: &["https://volthdl.org/guide/domains"],
        }),
        ("contracts", Lang::En) => Some(Topic {
            title: "Behavioral contracts",
            summary: "Contracts state what must hold in every reachable cycle. \
                      'volt verify' proves them formally with SymbiYosys; 'volt build \
                      --emit=sva' turns them into SystemVerilog assertions.",
            sections: &[
                (
                    "THE FOUR KEYWORDS",
                    "  requires:  ...   assumption about the inputs (ports only)\n\
                     \x20 ensures:   ...   guarantee about the outputs (ports only)\n\
                     \x20 invariant: ...   always true inside (ports + registers)\n\
                     \x20 cover:     ...   reachability target (sees everything)",
                ),
                (
                    "IMPLICATION",
                    "The most common contract shape is \"if A then B\" — write it with \
                     the '->' operator (ADR-0034):\n\n\
                     \x20 invariant: !busy -> tx        // when idle, the line is high\n\
                     \x20 ensures:   start -> busy      // SVA: start |-> busy\n\n\
                     'a -> b' means '!a || b'; both sides must be bool.",
                ),
                (
                    "PROVING",
                    "  volt verify design.volt                 bounded check (bmc)\n\
                     \x20 volt verify design.volt --mode prove    k-induction proof\n\
                     \x20 volt verify design.volt --mode cover    reach the cover targets\n\n\
                     A counterexample exits with code 6 and writes a .vcd trace; \
                     'volt explain E5001' explains how to read it. Installation: \
                     'volt explain verify-setup'.",
                ),
            ],
            more: &["https://volthdl.org/guide/contracts"],
        }),
        ("contracts", Lang::Tr) => Some(Topic {
            title: "Davranışsal kontratlar",
            summary: "Kontratlar, erişilebilir her döngüde neyin doğru kalması \
                      gerektiğini söyler. 'volt verify' bunları SymbiYosys ile formal \
                      kanıtlar; 'volt build --emit=sva' SystemVerilog assertion'larına \
                      çevirir.",
            sections: &[
                (
                    "DÖRT ANAHTAR KELİME",
                    "  requires:  ...   girişler hakkında varsayım (yalnız portlar)\n\
                     \x20 ensures:   ...   çıkışlar hakkında güvence (yalnız portlar)\n\
                     \x20 invariant: ...   içeride hep doğru (portlar + register'lar)\n\
                     \x20 cover:     ...   erişilebilirlik hedefi (her şeyi görür)",
                ),
                (
                    "İMPLİKASYON",
                    "En yaygın kontrat biçimi \"A ise B\"dir — '->' operatörüyle yazın \
                     (ADR-0034):\n\n\
                     \x20 invariant: !busy -> tx        // boştayken hat yüksek\n\
                     \x20 ensures:   start -> busy      // SVA: start |-> busy\n\n\
                     'a -> b', '!a || b' demektir; iki taraf da bool olmalıdır.",
                ),
                (
                    "KANITLAMA",
                    "  volt verify tasarim.volt                 sınırlı denetim (bmc)\n\
                     \x20 volt verify tasarim.volt --mode prove    k-endüksiyon kanıtı\n\
                     \x20 volt verify tasarim.volt --mode cover    cover hedeflerine ulaş\n\n\
                     Karşı örnek 6 koduyla çıkar ve bir .vcd izi yazar; nasıl okunacağını \
                     'volt explain E5001' anlatır. Kurulum: 'volt explain verify-setup'.",
                ),
            ],
            more: &["https://volthdl.org/guide/contracts"],
        }),
        ("stdlib", Lang::En) => Some(Topic {
            title: "The built-in component library",
            summary: "Eleven components are built into the compiler (ADR-0027/0029). \
                      Instantiate them like modules; the generated RTL is battle-tested \
                      and CDC-correct.",
            sections: &[
                (
                    "CDC BRIDGES (two domains)",
                    "  AsyncFifo<T, DEPTH>     gray-pointer FIFO between two clocks\n\
                     \x20 HandshakeSync<T>        one word per 4-phase req/ack transfer\n\
                     \x20 PulseSync               carries a single-cycle pulse across",
                ),
                (
                    "SINGLE-CLOCK BUILDING BLOCKS",
                    "  SyncFifo<T, DEPTH>      buffer between producer and consumer\n\
                     \x20 Ram<T, DEPTH>           synchronous single-port memory\n\
                     \x20 DualPortRam<T, DEPTH>   one write port, one read port\n\
                     \x20 Counter<WIDTH>          up-counter with enable and wrap\n\
                     \x20 ShiftRegister<T, LEN>   fixed-length delay line\n\
                     \x20 RoundRobinArbiter<N>    fair grant among N requesters\n\
                     \x20 PriorityArbiter<N>      lowest index wins\n\
                     \x20 EdgeDetect              rising/falling edge pulses",
                ),
                (
                    "USAGE",
                    "  let f = SyncFifo<u8, 16> { clk: clk, ... }\n\
                     \x20 ... f.rd_data ...\n\n\
                     Wrong generic arguments produce E2003 with the expected shape; \
                     DEPTH must be a power of two where noted.",
                ),
            ],
            more: &["https://volthdl.org/guide/stdlib"],
        }),
        ("stdlib", Lang::Tr) => Some(Topic {
            title: "Yerleşik bileşen kütüphanesi",
            summary: "Derleyicide on bir yerleşik bileşen vardır (ADR-0027/0029). \
                      Modül gibi örneklenir; üretilen RTL denenmiş ve CDC-doğrudur.",
            sections: &[
                (
                    "CDC KÖPRÜLERİ (iki alan)",
                    "  AsyncFifo<T, DEPTH>     iki saat arasında gray-pointer FIFO\n\
                     \x20 HandshakeSync<T>        4-fazlı req/ack ile sözcük aktarımı\n\
                     \x20 PulseSync               tek döngülük darbeyi karşıya taşır",
                ),
                (
                    "TEK SAATLİ YAPI TAŞLARI",
                    "  SyncFifo<T, DEPTH>      üretici ile tüketici arasında tampon\n\
                     \x20 Ram<T, DEPTH>           senkron tek portlu bellek\n\
                     \x20 DualPortRam<T, DEPTH>   bir yazma, bir okuma portu\n\
                     \x20 Counter<WIDTH>          enable ve sarmalı yukarı sayaç\n\
                     \x20 ShiftRegister<T, LEN>   sabit uzunluklu gecikme hattı\n\
                     \x20 RoundRobinArbiter<N>    N istekçi arasında adil tahsis\n\
                     \x20 PriorityArbiter<N>      en düşük indeks kazanır\n\
                     \x20 EdgeDetect              yükselen/düşen kenar darbeleri",
                ),
                (
                    "KULLANIM",
                    "  let f = SyncFifo<u8, 16> { clk: clk, ... }\n\
                     \x20 ... f.rd_data ...\n\n\
                     Yanlış generic argüman, beklenen kalıbı gösteren E2003 üretir; \
                     belirtilen yerlerde DEPTH iki kuvveti olmalıdır.",
                ),
            ],
            more: &["https://volthdl.org/guide/stdlib"],
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
