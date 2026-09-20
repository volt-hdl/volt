//! Test betiği → C++ (ADR-0058): ifadeler, yerel değişkenler, çalışma
//! zamanı `for` döngüsü ve `load`.
//!
//! Betik değerleri C++'ta `unsigned long long`'tur. Tanımsız davranışa
//! düşebilecek işlemler (`/`, `%`, kaydırma, dizi indeksi) yardımcı
//! işlevlerden geçer; hata `volt_fault`'a yazılır ve deyimden sonra
//! `VOLT-ASSERT-FAIL` satırına çevrilir — test çökmez, düşer.
//! Üretilen kod İngilizcedir (ADR-0026).

use volt_ast::TestBinOp;

use super::sim::{TbAssertKind, TbPortCheck, TbStep, TbValue};

/// Betik yardımcıları; yalnız ihtiyaç duyan testbench'e eklenir.
pub(crate) const SCRIPT_PRELUDE: &str = "\
// Script runtime (ADR-0058): faults fail the test instead of crashing.
static int volt_fault = 0;
static unsigned long long volt_fault_a = 0;
static unsigned long long volt_fault_b = 0;
static const char* volt_fault_name() {
    switch (volt_fault) {
        case 1: return \"index_out_of_bounds\";
        case 2: return \"division_by_zero\";
        case 3: return \"load_too_long\";
        default: return \"load_value_too_wide\";
    }
}
static unsigned long long volt_set_fault(int kind, unsigned long long a, unsigned long long b) {
    if (!volt_fault) { volt_fault = kind; volt_fault_a = a; volt_fault_b = b; }
    return 0;
}
static unsigned long long volt_at(const unsigned long long* data, std::size_t n, unsigned long long i) {
    return i < n ? data[i] : volt_set_fault(1, i, n);
}
static unsigned long long volt_div(unsigned long long a, unsigned long long b) {
    return b ? a / b : volt_set_fault(2, a, b);
}
static unsigned long long volt_rem(unsigned long long a, unsigned long long b) {
    return b ? a % b : volt_set_fault(2, a, b);
}
static unsigned long long volt_shl(unsigned long long a, unsigned long long b) {
    return b < 64 ? a << b : 0;
}
static unsigned long long volt_shr(unsigned long long a, unsigned long long b) {
    return b < 64 ? a >> b : 0;
}
";

/// `load` yardımcısı: eleman tipi ve boyut Verilator modelinden gelir.
pub(crate) const LOAD_PRELUDE: &str = "\
template <typename T, std::size_t N>
static void volt_load(VlUnpacked<T, N>& dst, const unsigned long long* src, std::size_t n, unsigned bits) {
    if (n > N) { volt_set_fault(3, n, N); return; }
    for (std::size_t i = 0; i < n; ++i) {
        // bits == 0: element width unknown to the compiler, fall back to the C++ type.
        const bool fits = bits == 0 ? static_cast<unsigned long long>(static_cast<T>(src[i])) == src[i]
                                    : (bits >= 64 || (src[i] >> bits) == 0);
        if (!fits) { volt_set_fault(4, src[i], i); return; }
        dst[i] = static_cast<T>(src[i]);
    }
}
";

/// Port genişlik koruması (ADR-0059). Verilator girişleri maskelemez:
/// sığmayan değer modelde başıboş bit bırakır ve simülasyon donanımda
/// imkânsız bir durumu yürütür. Kural volt-hir `PortWidth::accepts` ile
/// aynıdır.
pub(crate) const PORT_PRELUDE: &str = "\
// Port width guard (ADR-0059): a value that does not fit fails the test.
static bool volt_port_fits(unsigned long long v, unsigned bits, bool is_signed) {
    if (bits >= 64) return true;
    const unsigned long long mask = (1ULL << bits) - 1;
    if ((v & ~mask) == 0) return true;
    // A negative number written as 0 - n: bits [63:bits-1] are all ones.
    return is_signed && (v | (mask >> 1)) == ~0ULL;
}
static unsigned long long volt_port_bits(unsigned long long v, unsigned bits) {
    return bits >= 64 ? v : v & ((1ULL << bits) - 1);
}
// Width unknown to the compiler: fall back to the C++ storage type.
template <typename T>
static bool volt_port_fits_type(const T&, unsigned long long v) {
    return static_cast<unsigned long long>(static_cast<T>(v)) == v;
}
";

/// Adımlarda (iç içe dahil) `pred`'i sağlayan var mı?
fn any_step(steps: &[TbStep], pred: &dyn Fn(&TbStep) -> bool) -> bool {
    steps.iter().any(|s| {
        pred(s)
            || match s {
                TbStep::For { body, .. } => any_step(body, pred),
                _ => false,
            }
    })
}

/// Betik ADR-0058 özelliklerinden birini kullanıyor mu? Kullanmıyorsa
/// testbench ADR-0033 çıktısıyla bayt bayt aynı kalır.
pub(crate) fn uses_script_runtime(steps: &[TbStep]) -> bool {
    any_step(steps, &|s| {
        !matches!(
            s,
            TbStep::SetPort {
                value: TbValue::Lit(_) | TbValue::Port(_),
                ..
            } | TbStep::Step(_)
                | TbStep::Reset
                | TbStep::Loc(_)
                | TbStep::Assert {
                    left: TbValue::Lit(_) | TbValue::Port(_),
                    right: TbValue::Lit(_) | TbValue::Port(_),
                    ..
                }
        )
    })
}

pub(crate) fn uses_port_check(steps: &[TbStep]) -> bool {
    any_step(steps, &|s| matches!(s, TbStep::SetPortChecked { .. }))
}

pub(crate) fn uses_load(steps: &[TbStep]) -> bool {
    any_step(steps, &|s| matches!(s, TbStep::Load { .. }))
}

/// TbValue → C++ ifadesi (tam parantezli: C++ önceliğine güvenilmez).
pub(crate) fn cpp_value(v: &TbValue) -> String {
    match v {
        TbValue::Lit(n) => format!("{n}ULL"),
        TbValue::Port(p) => format!("(unsigned long long)dut.{p}"),
        TbValue::Var(name) => format!("v_{name}"),
        TbValue::Len(name) => format!("(unsigned long long)n_{name}"),
        TbValue::Index { array, index } => {
            format!("volt_at(v_{array}, n_{array}, {})", cpp_value(index))
        }
        TbValue::Not(inner) => format!("(unsigned long long)(({}) == 0)", cpp_value(inner)),
        TbValue::Binary { op, lhs, rhs } => cpp_binary(*op, &cpp_value(lhs), &cpp_value(rhs)),
    }
}

fn cpp_binary(op: TestBinOp, l: &str, r: &str) -> String {
    let infix = |sym: &str| format!("(({l}) {sym} ({r}))");
    let truth = |sym: &str| format!("(unsigned long long)(({l}) {sym} ({r}))");
    match op {
        TestBinOp::Add => infix("+"),
        TestBinOp::Sub => infix("-"),
        TestBinOp::Mul => infix("*"),
        TestBinOp::And => infix("&"),
        TestBinOp::Or => infix("|"),
        TestBinOp::Xor => infix("^"),
        TestBinOp::Div => format!("volt_div({l}, {r})"),
        TestBinOp::Rem => format!("volt_rem({l}, {r})"),
        TestBinOp::Shl => format!("volt_shl({l}, {r})"),
        TestBinOp::Shr => format!("volt_shr({l}, {r})"),
        TestBinOp::Eq => truth("=="),
        TestBinOp::Ne => truth("!="),
        TestBinOp::Lt => truth("<"),
        TestBinOp::Le => truth("<="),
        TestBinOp::Gt => truth(">"),
        TestBinOp::Ge => truth(">="),
        TestBinOp::LogAnd => format!("(unsigned long long)((({l}) != 0) && (({r}) != 0))"),
        TestBinOp::LogOr => format!("(unsigned long long)((({l}) != 0) || (({r}) != 0))"),
    }
}

/// Metni printf biçim dizgesine + C++ string literaline gömülebilir
/// kılar: dosya ya da test adındaki `%`, `"` ve `\` yorumlanmamalı.
pub(crate) fn printf_literal(text: &str) -> String {
    text.replace('%', "%%")
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

/// Değer çalışma zamanı hatası üretebilir mi (indeks, bölme)?
fn can_fault(v: &TbValue) -> bool {
    match v {
        TbValue::Lit(_) | TbValue::Port(_) | TbValue::Var(_) | TbValue::Len(_) => false,
        TbValue::Index { .. } => true,
        TbValue::Not(inner) => can_fault(inner),
        TbValue::Binary { op, lhs, rhs } => {
            matches!(op, TestBinOp::Div | TestBinOp::Rem) || can_fault(lhs) || can_fault(rhs)
        }
    }
}

/// Adım üreticisi: girinti, geçerli kaynak konumu ve çevreleyen döngü
/// değişkenleri (hata satırına `loop=i=3,j=1` olarak eklenir).
pub(crate) struct ScriptEmitter {
    out: String,
    indent: usize,
    loc: String,
    loop_vars: Vec<String>,
    has_loads: bool,
}

impl ScriptEmitter {
    pub(crate) fn new(has_loads: bool) -> Self {
        Self {
            out: String::new(),
            indent: 1,
            loc: "?".to_string(),
            loop_vars: Vec::new(),
            has_loads,
        }
    }

    pub(crate) fn finish(self) -> String {
        self.out
    }

    fn line(&mut self, text: &str) {
        self.out.push_str(&"    ".repeat(self.indent));
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// `VOLT-ASSERT-FAIL` satırının döngü bağlamı: (biçim eki, argüman eki).
    fn loop_context(&self) -> (String, String) {
        if self.loop_vars.is_empty() {
            return (String::new(), String::new());
        }
        let fmt: Vec<String> = self.loop_vars.iter().map(|v| format!("{v}=%llu")).collect();
        let args: String = self.loop_vars.iter().map(|v| format!(", v_{v}")).collect();
        (format!(" loop={}", fmt.join(",")), args)
    }

    /// Başarısızlık gövdesi: rapor satırı + temizlik + `return false`.
    fn fail_block(&mut self, kind_fmt: &str, kind_arg: &str, left: &str, right: &str) {
        self.fail_block_with(kind_fmt, kind_arg, left, right, "");
    }

    /// `extra`: satır sonuna eklenen sabit belirteç (` port=addr:u3`).
    fn fail_block_with(
        &mut self,
        kind_fmt: &str,
        kind_arg: &str,
        left: &str,
        right: &str,
        extra: &str,
    ) {
        let (ctx_fmt, ctx_args) = self.loop_context();
        let loc = printf_literal(&self.loc);
        let extra = printf_literal(extra);
        self.indent += 1;
        self.line(&format!(
            "std::printf(\"VOLT-ASSERT-FAIL {kind_fmt} {loc} left=%llu right=%llu{ctx_fmt}{extra}\\n\"{kind_arg}, {left}, {right}{ctx_args});"
        ));
        self.line("dut.final();");
        self.line("return false;");
        self.indent -= 1;
    }

    fn fault_check(&mut self) {
        self.line("if (volt_fault) {");
        self.fail_block("%s", ", volt_fault_name()", "volt_fault_a", "volt_fault_b");
        self.line("}");
    }

    fn fault_check_if(&mut self, values: &[&TbValue]) {
        if values.iter().any(|v| can_fault(v)) {
            self.fault_check();
        }
    }

    pub(crate) fn emit_steps(&mut self, steps: &[TbStep]) {
        for step in steps {
            self.emit_step(step);
        }
    }

    fn emit_step(&mut self, step: &TbStep) {
        match step {
            TbStep::Loc(loc) => self.loc.clone_from(loc),
            TbStep::SetPort { port, value } => {
                self.line(&format!("dut.{port} = {};", cpp_value(value)));
                self.fault_check_if(&[value]);
            }
            TbStep::SetPortChecked { port, value, check } => {
                self.emit_checked_port(port, value, check);
            }
            TbStep::Step(n) => self.line(&format!(
                "for (unsigned long long s = 0; s < {n}ULL; ++s) run_cycle(&dut, ctx);"
            )),
            TbStep::StepBy(count) => {
                self.line(&format!(
                    "{{ const unsigned long long volt_n = {}; for (unsigned long long s = 0; s < volt_n; ++s) run_cycle(&dut, ctx); }}",
                    cpp_value(count)
                ));
                self.fault_check_if(&[count]);
            }
            TbStep::Reset => {
                self.line("apply_reset(&dut, ctx);");
                if self.has_loads {
                    // Loaded memories model initialised storage: they
                    // survive reset() (ADR-0058).
                    self.line("for (auto& volt_l : volt_loads) volt_l();");
                    self.line("dut.eval();");
                }
            }
            TbStep::Assert {
                kind,
                left,
                right,
                loc,
            } => self.emit_assert(*kind, left, right, loc),
            TbStep::LetScalar { name, value } => {
                self.line(&format!(
                    "const unsigned long long v_{name} = {};",
                    cpp_value(value)
                ));
                self.fault_check_if(&[value]);
            }
            TbStep::LetArray { name, data } => self.emit_array(name, data),
            TbStep::For {
                var,
                start,
                end,
                body,
            } => self.emit_for(var, start, end, body),
            TbStep::Load {
                target,
                source,
                elem_bits,
            } => self.emit_load(target, source, *elem_bits),
        }
    }

    /// Değer bir kez hesaplanır, denetlenir, sonra yazılır: sığmayan
    /// değer porta HİÇ ulaşmaz. İşaretli portta aralıktaki negatif sayı
    /// bit desenine indirgenir (kayıpsız — kırpma değil).
    fn emit_checked_port(&mut self, port: &str, value: &TbValue, check: &TbPortCheck) {
        self.line(&format!(
            "{{ const unsigned long long volt_pv = {};",
            cpp_value(value)
        ));
        self.fault_check_if(&[value]);
        let (fits, stored) = match check.bits {
            Some(bits) => (
                format!("volt_port_fits(volt_pv, {bits}U, {})", check.signed),
                format!("volt_port_bits(volt_pv, {bits}U)"),
            ),
            None => (
                format!("volt_port_fits_type(dut.{port}, volt_pv)"),
                "volt_pv".to_string(),
            ),
        };
        self.line(&format!("if (!{fits}) {{"));
        let bits = check.bits.unwrap_or(0);
        self.fail_block_with(
            "port_overflow",
            "",
            "volt_pv",
            &format!("{bits}ULL"),
            &format!(" port={port}:{}", check.type_name),
        );
        self.line("}");
        self.line(&format!("dut.{port} = {stored}; }}"));
    }

    /// Sınırlar döngüden ÖNCE bir kez hesaplanır: gövde sınırı okuyan
    /// portu değiştirse de yineleme sayısı sabittir.
    fn emit_for(&mut self, var: &str, start: &TbValue, end: &TbValue, body: &[TbStep]) {
        self.line(&format!(
            "{{ const unsigned long long volt_lo_{var} = {}, volt_hi_{var} = {};",
            cpp_value(start),
            cpp_value(end)
        ));
        self.fault_check_if(&[start, end]);
        self.line(&format!(
            "for (unsigned long long v_{var} = volt_lo_{var}; v_{var} < volt_hi_{var}; ++v_{var}) {{"
        ));
        self.indent += 1;
        self.loop_vars.push(var.to_string());
        self.emit_steps(body);
        self.loop_vars.pop();
        self.indent -= 1;
        self.line("} }");
    }

    /// Yükleme bir lambda olarak saklanır: `reset()` sonrası yeniden
    /// uygulanır. `elem_bits` bilinmiyorsa 0 — denetim C++ tipine düşer.
    fn emit_load(&mut self, target: &str, source: &str, elem_bits: Option<u32>) {
        let bits = elem_bits.unwrap_or(0);
        self.line(&format!(
            "{{ auto volt_l = [&dut]() {{ volt_load(dut.rootp->{target}, v_{source}, n_{source}, {bits}U); }}; volt_l(); volt_loads.push_back(volt_l); }}"
        ));
        self.line("dut.eval();");
        self.fault_check();
    }

    fn emit_assert(&mut self, kind: TbAssertKind, left: &TbValue, right: &TbValue, loc: &str) {
        self.loc = loc.to_string();
        let l = cpp_value(left);
        let r = cpp_value(right);
        let (cond, name) = match kind {
            TbAssertKind::Eq => (format!("({l}) == ({r})"), "assert_eq"),
            TbAssertKind::Ne => (format!("({l}) != ({r})"), "assert_ne"),
            TbAssertKind::True => (format!("({l}) != 0"), "assert_true"),
            TbAssertKind::False => (format!("({l}) == 0"), "assert_false"),
        };
        if can_fault(left) || can_fault(right) {
            // Evaluate once, report a fault before judging the values.
            self.line(&format!(
                "{{ const unsigned long long volt_al = {l}, volt_ar = {r};"
            ));
            self.fault_check();
            let cond = match kind {
                TbAssertKind::Eq => "volt_al == volt_ar",
                TbAssertKind::Ne => "volt_al != volt_ar",
                TbAssertKind::True => "volt_al != 0",
                TbAssertKind::False => "volt_al == 0",
            };
            self.line(&format!("if (!({cond})) {{"));
            self.fail_block(name, "", "volt_al", "volt_ar");
            self.line("} }");
            return;
        }
        self.line(&format!("if (!({cond})) {{"));
        self.fail_block(name, "", &l, &r);
        self.line("}");
    }

    /// Dizi statik depolamadadır: `load` lambda'ları yakalamadan erişir
    /// ve `for` gövdesinde tanımlansa bile ömrü test boyunca sürer.
    fn emit_array(&mut self, name: &str, data: &[u64]) {
        const PER_LINE: usize = 8;
        self.line(&format!("static const unsigned long long v_{name}[] = {{"));
        self.indent += 1;
        for chunk in data.chunks(PER_LINE) {
            let row: Vec<String> = chunk.iter().map(|w| format!("0x{w:X}ULL")).collect();
            self.line(&format!("{},", row.join(", ")));
        }
        self.indent -= 1;
        self.line("};");
        self.line(&format!(
            "static const std::size_t n_{name} = {};",
            data.len()
        ));
    }
}
