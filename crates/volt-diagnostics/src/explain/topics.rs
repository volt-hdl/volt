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
    "waveforms",
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
            ("stdlib", Lang::En) => "the built-in components",
            ("stdlib", Lang::Tr) => "yerleşik bileşenler",
            ("verify-setup", Lang::En) => "installing SymbiYosys for 'volt verify'",
            ("verify-setup", Lang::Tr) => "'volt verify' için SymbiYosys kurulumu",
            ("simulation-setup", Lang::En) => "installing Verilator for 'volt run'",
            ("simulation-setup", Lang::Tr) => "'volt run' için Verilator kurulumu",
            ("waveforms", Lang::En) => "recording waveforms; enum names in GTKWave",
            ("waveforms", Lang::Tr) => "dalga formu kaydı; GTKWave'de enum adları",
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
                     \x20 an SMT solver  boolector (default), bitwuzla, yices or z3",
                ),
                (
                    "SOLVERS",
                    "Pick one with --engine. Across every example and ui/pass design the solvers gave the same pass/fail/unknown verdicts and \
                     the same counterexample cycles; only the time differed (and, in prove \
                     mode, which non-inductive contract an 'unknown' report names first).\n\n\
                     \x20 boolector  default: fast, and in apt, hdlc/formal and OSS CAD Suite\n\
                     \x20 bitwuzla   often fastest on large designs (CPU cores, pipelines); OSS CAD Suite\n\
                     \x20 yices      fast on small designs; OSS CAD Suite, hdlc/formal\n\
                     \x20 z3         2-4x slower than the others here; kept for compatibility\n\n\
                     If sby reports that the solver is not installed, install it or pass \
                     --engine with one that is.",
                ),
                (
                    "INSTALL",
                    "Linux:\n\
                     \x20 apt install yosys boolector\n\
                     \x20 pip install symbiyosys\n\n\
                     Windows:\n\
                     \x20 install Docker Desktop: Volt runs sby in a container (DOCKER below), or use WSL\n\n\
                     Everything in one download — the YosysHQ OSS CAD Suite ships yosys, sby and all solvers:\n\
                     \x20 https://github.com/YosysHQ/oss-cad-suite-build",
                ),
                (
                    "DOCKER",
                    "If sby is not installed but Docker is running, 'volt verify' runs it in \
                     a container by itself and says so in one line:\n\n\
                     \x20 note: SymbiYosys not found locally; running it in Docker (hdlc/formal:all)\n\n\
                     The image is pinned by digest and holds Yosys, SBY, boolector, yices and \
                     z3 (no bitwuzla); it is downloaded once, on first use, and Volt prints its \
                     size before and the time after ('volt doctor' lists the versions). Volt still compiles on the host; only \
                     sby runs in the container, all -j jobs in one container. Every path Volt \
                     prints (counterexamples, logs) is a host path, and on Linux the container \
                     runs as your user, so build/ stays yours.\n\n\
                     \x20 VOLT_TOOL_BACKEND=local    never use Docker\n\
                     \x20 VOLT_TOOL_BACKEND=docker   use Docker even if sby is installed\n\
                     \x20 VOLT_DOCKER=<path>         the docker executable\n\n\
                     'volt doctor' shows which way 'volt verify' will run.",
                ),
                (
                    "NOTE",
                    "'volt build' and 'volt check' do not need SymbiYosys; only 'volt verify' \
                     calls it. If sby is installed somewhere unusual, point the VOLT_SBY \
                     environment variable at the executable.",
                ),
            ],
            more: &["https://volt-hdl.github.io/volt/tour/install.html"],
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
                     \x20 bir SMT çözücü boolector (varsayılan), bitwuzla, yices ya da z3",
                ),
                (
                    "ÇÖZÜCÜLER",
                    "--engine ile seçilir. Her örnekte ve ui/pass tasarımında \
                     çözücüler aynı geçti/başarısız/bilinmiyor kararını ve aynı karşı örnek \
                     döngüsünü verdi; yalnız süre farklıydı (bir de prove kipinde 'bilinmiyor' \
                     raporunun ilk adlandırdığı tümevarımsal olmayan kontrat).\n\n\
                     \x20 boolector  varsayılan: hızlı; apt, hdlc/formal ve OSS CAD Suite'te var\n\
                     \x20 bitwuzla   büyük tasarımlarda (işlemci çekirdeği, boru hattı) çoğu zaman en hızlısı; OSS CAD Suite\n\
                     \x20 yices      küçük tasarımlarda hızlı; OSS CAD Suite, hdlc/formal\n\
                     \x20 z3         burada diğerlerinden 2-4 kat yavaş; uyumluluk için duruyor\n\n\
                     sby çözücünün kurulu olmadığını söylerse kurun ya da kurulu olanı \
                     --engine ile verin.",
                ),
                (
                    "KURULUM",
                    "Linux:\n\
                     \x20 apt install yosys boolector\n\
                     \x20 pip install symbiyosys\n\n\
                     Windows:\n\
                     \x20 Docker Desktop kurun: Volt sby'yi konteynerde çalıştırır (aşağıda DOCKER) ya da WSL kullanın\n\n\
                     Tek indirmede her şey — YosysHQ OSS CAD Suite yosys, sby ve tüm çözücüleri içerir:\n\
                     \x20 https://github.com/YosysHQ/oss-cad-suite-build",
                ),
                (
                    "DOCKER",
                    "sby kurulu değil ama Docker çalışıyorsa 'volt verify' onu kendiliğinden \
                     konteynerde çalıştırır ve bunu tek satırla söyler:\n\n\
                     \x20 not: SymbiYosys yerelde bulunamadı; Docker'da çalıştırılıyor (hdlc/formal:all)\n\n\
                     İmaj özetiyle sabitlidir ve Yosys, SBY, boolector, yices ile z3 içerir \
                     (bitwuzla yok); yalnız ilk kullanımda bir kez indirilir, Volt boyutu önce, \
                     süreyi sonra basar (sürümleri 'volt doctor' listeler). Volt yine ana makinede derler; \
                     konteynerde yalnız sby koşar, tüm -j işleri tek konteynerde. Volt'un \
                     bastığı her yol (karşı örnek, günlük) ana makine yoludur; Linux'ta \
                     konteyner sizin kullanıcınızla koşar, build/ sizin kalır.\n\n\
                     \x20 VOLT_TOOL_BACKEND=local    Docker'ı hiç kullanma\n\
                     \x20 VOLT_TOOL_BACKEND=docker   sby kurulu olsa da Docker kullan\n\
                     \x20 VOLT_DOCKER=<yol>          docker yürütülebiliri\n\n\
                     'volt doctor' 'volt verify'ın hangi yolla çalışacağını gösterir.",
                ),
                (
                    "NOT",
                    "'volt build' ve 'volt check' SymbiYosys gerektirmez; yalnız 'volt verify' \
                     onu çağırır. sby alışılmadık bir yerdeyse VOLT_SBY ortam değişkenini \
                     çalıştırılabilir dosyaya yöneltin.",
                ),
            ],
            more: &["https://volt-hdl.github.io/volt/tour/install.html"],
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
                     Windows:\n\
                     \x20 install Docker Desktop: Volt runs Verilator in a container (DOCKER below), or use WSL",
                ),
                (
                    "DOCKER",
                    "If Verilator is not installed but Docker is running, 'volt run' and \
                     'volt test' run it in a container by themselves and say so in one line:\n\n\
                     \x20 note: Verilator not found locally; running it in Docker (verilator/verilator:v5.052)\n\n\
                     The image is pinned by digest and holds Verilator with g++ and make; it is \
                     downloaded once, on first use, and Volt prints its size before and the \
                     time after ('volt doctor' lists the versions). Volt still compiles and writes the testbench on the host; only \
                     Verilator and the simulation run in the container. Every path Volt prints \
                     (the Waveform line, Verilator errors) is a host path, and on Linux the \
                     container runs as your user, so build/ and the VCD stay yours.\n\n\
                     \x20 VOLT_TOOL_BACKEND=local    never use Docker\n\
                     \x20 VOLT_TOOL_BACKEND=docker   use Docker even if Verilator is installed\n\
                     \x20 VOLT_DOCKER=<path>         the docker executable\n\n\
                     'volt doctor' shows which way 'volt test' and 'volt run' will run. \
                     Ctrl-C in 'volt test --watch' removes the running container.",
                ),
                (
                    "TESTS",
                    "Write 'test \"name\" { ... }' blocks next to your modules, or put \
                     them in a sibling file: tests in X_test.volt can instantiate the \
                     modules of X.volt automatically. Inside a test, step(n) advances \
                     the clock, reset() pulses the implicit reset line, and \
                     assert_eq/assert_ne/assert_true/assert_false check output ports. \
                     'let' binds numbers and arrays ([1, 2, 3] or read_hex(\"file.hex\")), \
                     'for i in 0..n { ... }' repeats a block at run time, and \
                     load(dut.mem, data) writes an array into a memory of the design.\n\n\
                     \x20 volt test                  every *_test.volt of the project\n\
                     \x20 volt test my_design_test.volt\n\
                     \x20 volt test --watch          re-run on every save (Ctrl-C stops)\n\
                     \x20 volt run --cycles 20       the project's top module (Volt.toml top)\n\
                     \x20 volt run design.volt --vcd waves.vcd\n\n\
                     A failed test is re-run with tracing and its report ends with a \
                     'Waveform gtkwave ...' line ('volt explain waveforms').",
                ),
                (
                    "NOTE",
                    "'volt build', 'volt check' and 'volt verify' do not need Verilator; \
                     only 'volt run' and 'volt test' call it. If Verilator is installed \
                     somewhere unusual, point the VOLT_VERILATOR environment variable at \
                     the executable.",
                ),
            ],
            more: &["https://volt-hdl.github.io/volt/tour/install.html"],
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
                     Windows:\n\
                     \x20 Docker Desktop kurun: Volt Verilator'u konteynerde çalıştırır (aşağıda DOCKER) ya da WSL kullanın",
                ),
                (
                    "DOCKER",
                    "Verilator kurulu değil ama Docker çalışıyorsa 'volt run' ve 'volt test' \
                     onu kendiliğinden konteynerde çalıştırır ve bunu tek satırla söyler:\n\n\
                     \x20 not: Verilator yerelde bulunamadı; Docker'da çalıştırılıyor (verilator/verilator:v5.052)\n\n\
                     İmaj özetiyle sabitlidir ve g++ ile make'li Verilator içerir; yalnız ilk \
                     kullanımda bir kez indirilir, Volt boyutu önce, süreyi sonra basar \
                     (sürümleri 'volt doctor' listeler). Volt yine ana makinede derler ve testbench'i yazar; konteynerde \
                     yalnız Verilator ve simülasyon koşar. Volt'un bastığı her yol (Dalga formu \
                     satırı, Verilator hataları) ana makine yoludur; Linux'ta konteyner sizin \
                     kullanıcınızla koşar, build/ ve VCD sizin kalır.\n\n\
                     \x20 VOLT_TOOL_BACKEND=local    Docker'ı hiç kullanma\n\
                     \x20 VOLT_TOOL_BACKEND=docker   Verilator kurulu olsa da Docker kullan\n\
                     \x20 VOLT_DOCKER=<yol>          docker yürütülebiliri\n\n\
                     'volt doctor' 'volt test' ve 'volt run'ın hangi yolla çalışacağını gösterir. \
                     'volt test --watch' içinde Ctrl-C koşan konteyneri kaldırır.",
                ),
                (
                    "TESTLER",
                    "Modüllerinizin yanına 'test \"ad\" { ... }' blokları yazın ya da \
                     kardeş dosyaya koyun: X_test.volt içindeki testler X.volt'un \
                     modüllerini otomatik örnekleyebilir. Test içinde step(n) saati \
                     ilerletir, reset() örtük reset hattını atımlar; \
                     assert_eq/assert_ne/assert_true/assert_false çıkış portlarını \
                     denetler. 'let' sayı ve dizi bağlar ([1, 2, 3] ya da \
                     read_hex(\"dosya.hex\")), 'for i in 0..n { ... }' bloğu çalışma \
                     zamanında yineler, load(dut.mem, veri) diziyi tasarımın \
                     belleğine yazar.\n\n\
                     \x20 volt test                  projenin bütün *_test.volt dosyaları\n\
                     \x20 volt test tasarim_test.volt\n\
                     \x20 volt test --watch          her kayıtta yeniden koş (Ctrl-C durdurur)\n\
                     \x20 volt run --cycles 20       projenin üst modülü (Volt.toml top)\n\
                     \x20 volt run tasarim.volt --vcd dalga.vcd\n\n\
                     Düşen test izle yeniden koşar ve raporu 'Waveform gtkwave ...' \
                     satırıyla biter ('volt explain waveforms').",
                ),
                (
                    "NOT",
                    "'volt build', 'volt check' ve 'volt verify' Verilator gerektirmez; \
                     yalnız 'volt run' ve 'volt test' onu çağırır. Verilator alışılmadık \
                     bir yerdeyse VOLT_VERILATOR ortam değişkenini çalıştırılabilir \
                     dosyaya yöneltin.",
                ),
            ],
            more: &["https://volt-hdl.github.io/volt/tour/install.html"],
        }),
        ("waveforms", Lang::En) => Some(Topic {
            title: "Waveforms",
            summary: "How Volt records waveforms, where the files go, and how enum and \
                      Trit signals show their names instead of raw codes.",
            sections: &[
                (
                    "RECORD",
                    "  volt test                              a failed test (re-run with tracing)\n\
                     \x20 volt test --waves                      every test\n\
                     \x20 volt run --vcd waves.vcd design.volt   simulation (Verilator)\n\
                     \x20 volt verify design.volt                a failing contract's counterexample\n\n\
                     'volt test' runs the tests without tracing; when a test fails, only \
                     the failed tests run once more with tracing (the simulation is \
                     deterministic) into build/sim/<test file name without .volt>/waves/<Module>-<test>.vcd. \
                     A passing run costs nothing extra. --waves traces every test in the \
                     first run, --no-waves records nothing. 'volt run' writes the VCD \
                     where --vcd points. 'volt verify' copies a counterexample to \
                     build/formal/<task>_cex.vcd and, in --mode prove, the induction trace \
                     of an unproven contract to build/formal/<task>_induct.vcd.",
                ),
                (
                    "OPEN",
                    "Each command prints one line that opens the waveform; in the test \
                     report it ends the failed test's block:\n\n\
                     \x20 Waveform gtkwave waves.vcd waves.gtkw\n\n\
                     Any VCD viewer (gtkwave, surfer) opens the .vcd itself; the .gtkw \
                     session and its translate tables are GTKWave files. A test's session \
                     also lists the ports of the tested module, so its inputs and outputs \
                     are on screen at once.",
                ),
                (
                    "ENUM NAMES",
                    "A VCD has no enum type: a state register shows its code, 01 instead of \
                     Start. When the design has enum or Trit signals, Volt writes a GTKWave \
                     session next to the VCD (waves.gtkw) and one translate table per enum \
                     (waves.filters/<Enum>.txt). The session lists every enum- and Trit-typed \
                     signal of the top module and of every instance below it (ports, \
                     registers, wires, lets that become wires, struct fields such as req_kind, instance outputs \
                     such as u_state) and shows each through its table: Idle, Start, Data, \
                     Stop; a Trit as +1, 0, -1.\n\n\
                     The generated SystemVerilog does not change. For 'volt run' and \
                     'volt verify' a design without enums or Trits gets no extra files, and a \
                     waves.gtkw that Volt did not write is left unchanged.",
                ),
                (
                    "INVALID CODES",
                    "A code that belongs to no variant (3 in a three-state enum, 10 in a \
                     Trit) is shown in red as 'invalid 3' ('invalid -2'). Tables list every \
                     code up to 8 bits; the unused codes of a wider enum show as the raw \
                     binary value.",
                ),
                (
                    "LIMITS",
                    "Arrays of Trit are packed vectors: their elements have no names of their \
                     own, so they are not translated. The session names its files as \
                     Volt was given them; GTKWave resolves a relative path against its \
                     working directory, not the session's, so run the printed command from \
                     the directory you ran Volt in (inside Docker too). From elsewhere the \
                     signals load but show raw codes.",
                ),
            ],
            more: &["https://volt-hdl.github.io/volt/tour/tests-and-waveforms.html"],
        }),
        ("waveforms", Lang::Tr) => Some(Topic {
            title: "Dalga formları",
            summary: "Volt dalga formunu nasıl kaydeder, dosyalar nereye gider ve enum ile \
                      Trit sinyalleri ham kod yerine adlarını nasıl gösterir.",
            sections: &[
                (
                    "KAYIT",
                    "  volt test                               düşen test (izle yeniden koşar)\n\
                     \x20 volt test --waves                       her test\n\
                     \x20 volt run --vcd dalga.vcd tasarim.volt   simülasyon (Verilator)\n\
                     \x20 volt verify tasarim.volt                bozulan kontratın karşı örneği\n\n\
                     'volt test' testleri izsiz koşar; bir test düşünce yalnız düşen testler \
                     izle bir kez daha koşar (simülasyon belirlenimcidir) ve kayıt \
                     build/sim/<.volt uzantısız test dosyası adı>/waves/<Modül>-<test>.vcd dosyasına gider. \
                     Geçen koşunun ek maliyeti yoktur. --waves ilk koşuda her testi izler, \
                     --no-waves hiçbir şey kaydetmez. 'volt run' VCD'yi --vcd'nin gösterdiği \
                     yere yazar. 'volt verify' karşı örneği build/formal/<görev>_cex.vcd \
                     dosyasına, --mode prove'da kanıtlanamayan kontratın tümevarım izini \
                     build/formal/<görev>_induct.vcd dosyasına kopyalar.",
                ),
                (
                    "AÇMA",
                    "Her komut dalga formunu açan tek bir satır basar; test raporunda bu \
                     satır düşen testin bloğunu kapatır (rapor İngilizcedir):\n\n\
                     \x20 Dalga formu gtkwave dalga.vcd dalga.gtkw\n\
                     \x20 Waveform gtkwave build/sim/.../Counter-t.vcd build/sim/.../Counter-t.gtkw\n\n\
                     .vcd'yi her VCD görüntüleyici (gtkwave, surfer) açar; .gtkw oturumu ve \
                     çeviri tabloları GTKWave dosyalarıdır. Testin oturumu sınanan modülün \
                     portlarını da listeler: girişler ve çıkışlar hemen ekrandadır.",
                ),
                (
                    "ENUM ADLARI",
                    "VCD'de enum tipi yoktur: durum register'ı kodunu gösterir, Start yerine \
                     01. Tasarımda enum ya da Trit sinyali varsa Volt VCD'nin yanına bir \
                     GTKWave oturumu (dalga.gtkw) ve enum başına bir çeviri tablosu \
                     (dalga.filters/<Enum>.txt) yazar. Oturum üst modülün ve altındaki her \
                     örneğin enum ve Trit tipli bütün sinyallerini (portlar, register'lar, \
                     wire'lar, tel olan let'ler, req_kind gibi struct alanları, u_state gibi örnek \
                     çıkışları) listeler ve her birini tablosundan gösterir: Idle, Start, \
                     Data, Stop; Trit'i +1, 0, -1 olarak.\n\n\
                     Üretilen SystemVerilog değişmez. 'volt run' ve 'volt verify'da enum ya \
                     da Trit kullanmayan tasarımda ek dosya üretilmez; Volt'un yazmadığı bir \
                     dalga.gtkw'ye dokunulmaz.",
                ),
                (
                    "GEÇERSİZ KODLAR",
                    "Hiçbir varyanta ait olmayan kod (üç durumlu enum'da 3, Trit'te 10) \
                     kırmızı 'invalid 3' ('invalid -2') olarak görünür. Tablolar 8 bite kadar \
                     her kodu listeler; daha geniş bir enum'un kullanılmayan kodları ham \
                     ikili değer olarak görünür.",
                ),
                (
                    "SINIRLAR",
                    "Trit dizileri paketlenmiş vektördür: elemanlarının kendi adı olmadığı \
                     için çevrilmezler. Oturum dosyaları Volt'a verildiği gibi adlandırır; \
                     GTKWave göreli yolu oturumun değil kendi çalışma dizinine göre çözer, \
                     bu yüzden basılan komutu Volt'u koştuğunuz dizinden çalıştırın (Docker'da \
                     da). Başka bir dizinden sinyaller yüklenir ama ham kod gösterir.",
                ),
            ],
            more: &["https://volt-hdl.github.io/volt/tour/tests-and-waveforms.html"],
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
                     \x20 volt build blink.volt        writes build/rtl/Blink.sv\n\
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
            more: &["https://volt-hdl.github.io/volt/tour/new-check-build.html"],
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
                     \x20 volt build blink.volt        build/rtl/Blink.sv üretir\n\
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
            more: &["https://volt-hdl.github.io/volt/tour/new-check-build.html"],
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
            more: &["https://volt-hdl.github.io/volt/tour/cdc-error.html"],
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
            more: &["https://volt-hdl.github.io/volt/tour/cdc-error.html"],
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
                     the '->' operator:\n\n\
                     \x20 invariant: !busy -> tx        // when idle, the line is high\n\
                     \x20 ensures:   start -> busy      // SVA: start |-> busy\n\n\
                     'a -> b' means '!a || b'; both sides must be bool.",
                ),
                (
                    "INSTANCES",
                    "A module's 'requires' is assumed while the module itself is \
                     verified. Where the module is instantiated, the parent drives \
                     those inputs, so the parent must meet it: 'volt verify' checks it \
                     as an assertion in the parent's task. 'ensures' and \
                     'invariant' are checked in both places.",
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
            more: &[],
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
                    "En yaygın kontrat biçimi \"A ise B\"dir — '->' operatörüyle yazın:\n\n\
                     \x20 invariant: !busy -> tx        // boştayken hat yüksek\n\
                     \x20 ensures:   start -> busy      // SVA: start |-> busy\n\n\
                     'a -> b', '!a || b' demektir; iki taraf da bool olmalıdır.",
                ),
                (
                    "ÖRNEKLER",
                    "Bir modülün 'requires'ı modülün kendisi doğrulanırken varsayılır. \
                     Modülün örneklendiği yerde o girişleri üst modül sürer, ön koşulu \
                     da o karşılamalıdır: 'volt verify' onu üst modülün görevinde iddia \
                     olarak denetler. 'ensures' ve 'invariant' iki yerde de \
                     denetlenir.",
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
            more: &[],
        }),
        ("stdlib", Lang::En) => Some(Topic {
            title: "The built-in component library",
            summary: "These components are built into the compiler. \
                      Instantiate them like modules; the CDC bridges carry their own \
                      synchronizers.",
            sections: &[
                (
                    "CDC BRIDGES (two domains)",
                    "  AsyncFifo<T, DEPTH>     gray-pointer FIFO between two clocks\n\
                     \x20 HandshakeSync<T>        one word per 4-phase req/ack transfer\n\
                     \x20 PulseSync               carries a single-cycle pulse across\n\
                     \x20 AsyncDualPortRam<T, DEPTH>  write on one clock, read on the other",
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
                     DEPTH must be a power of two where noted.\n\n\
                     T may be a number, bool, enum or struct: a struct is \
                     stored as one packed word (first field in the high bits), so a \
                     memory still maps to block RAM. Ram, DualPortRam and \
                     AsyncDualPortRam reject a T with an enum or Trit field (E2009): a \
                     never-written address would read back raw bits.",
                ),
            ],
            more: &[],
        }),
        ("stdlib", Lang::Tr) => Some(Topic {
            title: "Yerleşik bileşen kütüphanesi",
            summary: "Bu bileşenler derleyicinin içindedir. \
                      Modül gibi örneklenir; CDC köprüleri kendi senkronizörlerini taşır.",
            sections: &[
                (
                    "CDC KÖPRÜLERİ (iki alan)",
                    "  AsyncFifo<T, DEPTH>     iki saat arasında gray-pointer FIFO\n\
                     \x20 HandshakeSync<T>        4-fazlı req/ack ile sözcük aktarımı\n\
                     \x20 PulseSync               tek döngülük darbeyi karşıya taşır\n\
                     \x20 AsyncDualPortRam<T, DEPTH>  bir saatte yaz, ötekinde oku",
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
                     belirtilen yerlerde DEPTH iki kuvveti olmalıdır.\n\n\
                     T sayı, bool, enum ya da struct olabilir: struct tek \
                     paketlenmiş sözcük olarak saklanır (ilk alan yüksek bitlerde), \
                     bellek yine blok RAM'e iner. Ram, DualPortRam ve AsyncDualPortRam \
                     enum ya da Trit alanlı T'yi reddeder (E2009): hiç yazılmamış adres \
                     ham bit döndürürdü.",
                ),
            ],
            more: &[],
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
    if topic.more.is_empty() {
        // Kitapta karşılığı olmayan konu: bölüm yazılmaz.
        out.pop();
        return Some(out);
    }
    out.push_str(&paint(HEADER_STYLE, more));
    out.push('\n');
    for doc in topic.more {
        out.push_str(&format!("  {doc}\n"));
    }
    Some(out)
}

/// Kurulum konusunun (`simulation-setup`, `verify-setup`) INSTALL /
/// KURULUM bölümünden tek işletim sisteminin komutları. `os_label`
/// bölümdeki paragraf başlığıdır ("Linux", "macOS", "Windows",
/// "Docker"). `volt doctor` eksik araç ipucunu buradan alır — metin tek
/// yerde yaşar (ADR-0084 §2). Konu, bölüm ya da paragraf yoksa boş.
pub fn install_steps(name: &str, lang: Lang, os_label: &str) -> Vec<&'static str> {
    let Some(topic) = lookup(name, lang) else {
        return Vec::new();
    };
    let Some((_, body)) = topic
        .sections
        .iter()
        .find(|(header, _)| matches!(*header, "INSTALL" | "KURULUM"))
    else {
        return Vec::new();
    };
    let heading = format!("{os_label}:");
    body.split("\n\n")
        .find_map(|paragraph| {
            let mut lines = paragraph.lines();
            (lines.next()?.trim() == heading)
                .then(|| lines.map(str::trim).filter(|l| !l.is_empty()).collect())
        })
        .unwrap_or_default()
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
