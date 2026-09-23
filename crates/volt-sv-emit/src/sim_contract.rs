//! Simülasyonda kontrat izleyicileri (ADR-0064, `SvaMode::Simulation`).
//!
//! `volt verify`'ın immediate kalıbı (`sva_immediate`) temel alınır —
//! aynı saat kenarı, aynı reset koruması, aynı `prev()` yardımcı reg
//! zinciri, aynı property adları — ama SV `assert`/`assume`/`cover`
//! yerine testbench'in DPI geri çağrıları üretilir:
//!
//! ```systemverilog
//! always @(posedge clk)
//!     if (!(rst)) if (!(count < 8'd5)) volt_contract_fail("Cnt.inv_0");
//! longint volt_hits_cov_0 = 0;
//! always @(posedge clk)
//!     if (!(rst)) if (count == 8'd3) volt_hits_cov_0 <= volt_hits_cov_0 + 1;
//! final volt_cover_report("Cnt.cov_0", volt_hits_cov_0);
//! ```
//!
//! Gerekçe (ADR-0064 ölçümü, Verilator 5.050): başarısız immediate
//! `assert` `$stop` ile TÜM test yürütülebilirini durdurur, `assume`
//! `assert` ile aynı iletiyi basar, `cover` hiçbir şey basmaz ve ileti
//! yalnız SV satırını gösterir. DPI çağrısı kimliği (`Modül.ad`), örnek
//! yolunu (`svGetScope`) ve testbench'in çevrim sayacını taşır; test
//! düşer ama yürütülebilir sonraki testlere devam eder. Cover sayacı
//! SV'dedir (çevrim başına tek artırma; DPI yalnız `final`'de — ölçüm:
//! çevrim başına DPI çağrısı VGA testlerini %24 yavaşlatıyordu).

use volt_ast::{ClockEdge, ModuleDecl};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};

use crate::sva::{kind_slot, sva_construct, ONE_BIT};
use crate::{ClockPort, Emitter};

/// İhlal geri çağrısı: `volt_contract_fail("Modül.ad")`.
pub const SIM_FAIL_FN: &str = "volt_contract_fail";
/// Cover raporu: örnek başına bir kez, `final` bloğunda (`dut.final()`)
/// toplam tetiklenme sayısıyla — hiç tetiklenmeyen cover da 0 ile gelir.
pub const SIM_COVER_REPORT_FN: &str = "volt_cover_report";

/// Testbench tarafı: DPI geri çağrılarının tanımları. İhlal (kimlik,
/// örnek) başına yalnız İLK çevrim kaydedilir; `volt test` her adımdan
/// sonra listeye bakar, `volt run` koşu sonunda hepsini basar. Cover
/// sayaçları tüm testler boyunca toplanır ve `main` sonunda basılır.
/// `extern "C"` bağlaması Verilator'un `__Dpi.h` bildirimleriyle aynıdır.
pub(crate) const CONTRACT_PRELUDE: &str = "\
// Contract monitors (ADR-0064): the design calls these DPI functions.
struct VoltViolation {
    std::string id;
    std::string inst;
    unsigned long long cycle;
};
static unsigned long long volt_cycle = 0;
static std::vector<VoltViolation> volt_violations;
static std::map<std::string, unsigned long long> volt_cover_hits;
extern \"C\" void volt_contract_fail(const char* id) {
    const char* scope = svGetNameFromScope(svGetScope());
    const std::string inst = scope ? scope : \"?\";
    for (const auto& v : volt_violations) {
        if (v.id == id && v.inst == inst) return;
    }
    volt_violations.push_back({id, inst, volt_cycle});
}
extern \"C\" void volt_cover_report(const char* id, long long hits) {
    volt_cover_hits[id] += static_cast<unsigned long long>(hits);
}
static void volt_contracts_begin() {
    volt_violations.clear();
    volt_cycle = 0;
}
static void volt_contracts_dump() {
    for (const auto& v : volt_violations) {
        std::printf(\"VOLT-CONTRACT-FAIL %s cycle=%llu inst=%s\\n\", v.id.c_str(), v.cycle, v.inst.c_str());
    }
}
static void volt_cover_summary() {
    for (const auto& c : volt_cover_hits) {
        std::printf(\"VOLT-COVER %s %llu\\n\", c.first.c_str(), c.second);
    }
}
";

/// İzleyicili testbench'in ek başlıkları (`svdpi.h` Verilator'la gelir).
pub(crate) const CONTRACT_INCLUDES: &str =
    "#include \"svdpi.h\"\n#include <map>\n#include <string>\n#include <vector>\n";

/// Üretilen SV'deki DPI bildirimlerinin ortak öneki.
const SIM_DPI_MARKER: &str = "import \"DPI-C\" context function void volt_";

/// Üretilen SV simülasyon izleyicisi içeriyor mu? Testbench DPI
/// tanımlarını (ve `svdpi.h`'yi) yalnız o zaman ekler — DPI'sız bir
/// tasarımda Verilator `svGetScope`'u bağlamaz.
pub fn uses_sim_contracts(sv: &str) -> bool {
    sv.contains(SIM_DPI_MARKER)
}

/// Modülün izleyicilerinin kullandığı DPI geri çağrıları.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SimDpiUse {
    pub fail: bool,
    pub cover: bool,
}

impl<'a> Emitter<'a> {
    /// Modül kontratlarının izleyici bloğu; kontrat ya da saat yoksa None.
    /// Property adları ve `SvaProp` kayıtları `sva_immediate` ile aynıdır.
    pub(crate) fn sva_simulation(
        &mut self,
        module: &'a ModuleDecl,
        clocks: &[ClockPort],
        indent: usize,
    ) -> Option<String> {
        if module.contracts.is_empty() {
            return None;
        }
        let clock = clocks.first()?.clone();
        let ind = " ".repeat(indent);

        let mut counters = [0u32; 6];
        let mut blocks = Vec::new();
        // ADR-0040: prev() yardımcı reg zinciri — reset sonrası ilk
        // döngüde prev(x) == 0, formal ile AYNI anlam.
        if let Some(block) = self.past_reg_block(module, &clock, indent) {
            blocks.push(block);
        }
        for c in &module.contracts {
            let (prefix, verb) = sva_construct(c.kind);
            let slot = kind_slot(c.kind);
            let name = format!("{prefix}_{}", counters[slot]);
            counters[slot] += 1;
            let span = self.ast.exprs[c.expr].span;
            let prop = self.contract_prop(module, &name, c);
            self.sva_props.push(prop);
            let comment = self.contract_comment(c, &ind);
            let Some(expr) = self.monitor_expr(c.expr, span) else {
                continue;
            };
            let id = format!("{}.{name}", module.name.text);
            blocks.push(format!(
                "{comment}\n{}",
                self.sim_monitor(&clock, verb, &expr, &id, indent),
            ));
        }
        Some(blocks.join("\n\n"))
    }

    /// Kontrat ifadesinin SV metni. İfade SV'ye inemiyorsa (E0003) izleyici
    /// ÜRETİLMEZ: hata W5001'e indirgenir — kontratlar `volt test`'te
    /// varsayılan açıktır ve eskiden derlenen bir test derlenmeye devam
    /// etmelidir (ADR-0064). Adlandırma sayacı ilerlemiştir: diğer
    /// kontratların kimlikleri verify'dakiyle aynı kalır.
    fn monitor_expr(
        &mut self,
        expr: volt_ast::Idx<volt_ast::Expr>,
        span: volt_span::Span,
    ) -> Option<String> {
        let before = self.diagnostics.len();
        let text = self.emit_expr(expr, ONE_BIT);
        if !self.diagnostics[before..]
            .iter()
            .any(|d| !d.code.is_warning())
        {
            return Some(text);
        }
        self.diagnostics.truncate(before);
        self.diagnostics.push(Diagnostic::warning(
            ErrorCode::W5001,
            lstr!(
                en: "this contract has no SystemVerilog form yet; it is not monitored in simulation";
                tr: "bu kontratın henüz SystemVerilog karşılığı yok; simülasyonda izlenmiyor"
            ),
            LabeledSpan::primary(span, ""),
            lstr!(
                en: "rewrite it with operators or if-expressions (volt verify rejects it too), or run 'volt test --no-contracts'";
                tr: "operatör ya da if-ifadesiyle yeniden yazın (volt verify de reddeder) ya da 'volt test --no-contracts' kullanın"
            ),
        ));
        None
    }

    /// Tek izleyici. `verb` SVA fiilidir (`assert`/`assume`/`cover`);
    /// assert ve assume aynı ihlal geri çağrısını kullanır — sınıflandırma
    /// sürücüde kimlikten (`SvaProp::keyword`) yapılır.
    pub(crate) fn sim_monitor(
        &mut self,
        clock: &ClockPort,
        verb: &str,
        expr: &str,
        id: &str,
        indent: usize,
    ) -> String {
        let ind = " ".repeat(indent);
        let edge = match clock.info.edge {
            ClockEdge::Negedge => "negedge",
            _ => "posedge",
        };
        let is_cover = verb == "cover";
        // Sayaç adı kimliğin modül içi kısmından: `volt_hits_cov_0`.
        let counter = format!("volt_hits_{}", id.rsplit('.').next().unwrap_or(id));
        let check = if is_cover {
            self.sim_dpi.cover = true;
            format!("if ({expr}) {counter} <= {counter} + 1;")
        } else {
            self.sim_dpi.fail = true;
            format!("if (!({expr})) {SIM_FAIL_FN}(\"{id}\");")
        };
        // Reset süresince denetim kapalı (formal'deki disable iff / reset
        // koruması ile aynı).
        let stmt = if clock.info.reset.is_none() {
            check
        } else {
            format!("if (!({})) {check}", clock.info.reset.condition())
        };
        let mut out = String::new();
        if is_cover {
            out.push_str(&format!("{ind}longint {counter} = 0;\n"));
        }
        out.push_str(&format!(
            "{ind}always @({edge} {})\n{ind}    {stmt}",
            clock.name
        ));
        if is_cover {
            out.push_str(&format!(
                "\n{ind}final {SIM_COVER_REPORT_FN}(\"{id}\", {counter});"
            ));
        }
        out
    }

    /// Modülde kullanılan DPI geri çağrılarının `import` bildirimleri;
    /// hiçbiri kullanılmadıysa None.
    pub(crate) fn sim_dpi_imports(&self) -> Option<String> {
        let SimDpiUse { fail, cover } = self.sim_dpi;
        if !fail && !cover {
            return None;
        }
        let mut lines =
            vec!["    // simulation contract monitors (ADR-0064): volt test callbacks".to_string()];
        // Her ad `volt_` ile başlar: bildirim SIM_DPI_MARKER'ı içerir.
        let import = |name: &str, args: &str| {
            format!("    import \"DPI-C\" context function void {name}({args});")
        };
        if fail {
            lines.push(import(SIM_FAIL_FN, "input string id"));
        }
        if cover {
            lines.push(import(
                SIM_COVER_REPORT_FN,
                "input string id, input longint hits",
            ));
        }
        Some(lines.join("\n"))
    }
}
