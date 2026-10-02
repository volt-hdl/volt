//! Üretilen SV adlarının çakışması (ADR-0090, E1003).
//!
//! Volt birçok SV adını kendisi kurar: örnek çıkış teli
//! (`<örnek>_<port>`), yerleşik primitifin iç sinyalleri (`<örnek>_<ad>`),
//! otomatik reset portu (`rst`/`rst_n`), reset bırakma zinciri
//! (`rst_sync_<saat>_stage<i>`), `sync()` köprüsü (`sync_<kaynak>_src`,
//! `sync_<kaynak>_stage<i>`). Kurulan ad kullanıcının bir adıyla ya da
//! başka bir kurulan adla aynı çıkarsa SV'de aynı ad iki kez bildirilir:
//! Verilator "Duplicate declaration" verir, Yosys ise bildirimleri SESSİZCE
//! birleştirir (ölçüm, ADR-0090 §1). Bu adlar yeniden adlandırılmaz —
//! port arayüzde, zincir/köprü/primitif register'ları SDC/XDC'de, örnek
//! çıkışı dalga biçiminde ve sv-mapping.md §9'da adıyla geçer — E1003.
//!
//! Denetim üretilen modül metni üzerindedir (gerçek ölçüt: SV'de iki
//! bildirim): yeni bir ad kaynağı eklendiğinde de çakışma yakalanır.
//! Adın kökeni tanınırsa ileti iki kaynağı adlandırır ve iki konum
//! gösterir; tanınmazsa modül adında genel ileti.

use std::collections::HashMap;

use volt_ast::reserved::is_sv_keyword;
use volt_ast::{ModuleDecl, StmtKind};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::Span;

use crate::{reset_port_set, reset_sync, Emitter};

/// Modül düzeyi bildirim satırının anahtar sözcükleri (emitter biçimi:
/// dört boşluk girinti; portlar başlıkta aynı girintide).
const DECL_KEYWORDS: &[&str] = &[
    "inout",
    "input",
    "int",
    "localparam",
    "logic",
    "longint",
    "output",
    "tri1",
    "wire",
];

/// `sync()` köprüsünün ürettiği adlar (ADR-0090 köken tablosu).
#[derive(Debug, Clone)]
pub(crate) struct SyncBridgeNames {
    /// `sync_<kaynak>`.
    pub(crate) base: String,
    /// Çağrının konumu.
    pub(crate) span: Span,
    /// Yakalama register'ı (`<base>_src`) var mı?
    pub(crate) capture: bool,
    pub(crate) stages: usize,
}

/// Bir SV adının kaynağı.
struct Origin {
    span: Span,
    /// İletide: "port 'x'", "output 'irq' of instance 'timer'" …
    what: String,
    /// Volt'un kurduğu ad mı (kullanıcının yazdığı değil)?
    generated: bool,
}

/// Modül metninde modül düzeyinde bildirilen adlar, metin sırasıyla:
/// portlar, `logic`/`wire`/`localparam`/… bildirimleri ve örnek adları.
/// Tutucudur: tanımadığı satırı atlar (yanlış alarm vermez; iki
/// bildirim her zaman geçersiz SV'dir).
pub(crate) fn declared_names(sv: &str) -> Vec<&str> {
    sv.lines().filter_map(declared_name).collect()
}

fn declared_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("    ")?;
    if rest.starts_with(' ') {
        return None;
    }
    let first = rest.split(|c: char| !is_ident_char(c)).next()?;
    if DECL_KEYWORDS.contains(&first) {
        return decl_line_name(rest);
    }
    instance_name(rest)
}

/// `logic [7:0] a;` / `input  logic b,` / `wire c = …;` /
/// `logic [7:0] mem [0:3];` → son tanımlayıcı (`=`, `;`, `,`, `//`
/// öncesi, köşeli parantez grupları atlanarak).
fn decl_line_name(rest: &str) -> Option<&str> {
    let end = rest.find(['=', ';', ',']).unwrap_or(rest.len());
    let end = rest[..end].find("//").unwrap_or(end);
    let head = &rest[..end];
    let mut depth = 0i32;
    let mut last = None;
    let mut start = None;
    for (i, c) in head.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ => {}
        }
        let ident = depth == 0 && is_ident_char(c);
        match (ident, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                last = Some(&head[s..i]);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        last = Some(&head[s..]);
    }
    last.filter(|n| n.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_'))
}

/// `Mod inst (` → `inst` (emitter örnek başlığı; parametreli örnek
/// üretilmez, monomorfizasyon somut modül adını yazar).
fn instance_name(rest: &str) -> Option<&str> {
    let body = rest.strip_suffix('(')?.trim_end();
    let mut words = body.split(' ');
    let (module, inst) = (words.next()?, words.next()?);
    let ident = |w: &str| {
        !w.is_empty()
            && w.chars().all(is_ident_char)
            && w.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
    };
    (words.next().is_none() && ident(module) && ident(inst) && !is_sv_keyword(module))
        .then_some(inst)
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// Metinde birden çok kez bildirilen adlar, ilk görünüş sırasıyla.
pub(crate) fn duplicate_decls(sv: &str) -> Vec<&str> {
    let mut count: HashMap<&str, usize> = HashMap::new();
    let mut order = Vec::new();
    for name in declared_names(sv) {
        let n = count.entry(name).or_insert(0);
        *n += 1;
        if *n == 2 {
            order.push(name);
        }
    }
    order
}

impl<'a> Emitter<'a> {
    /// ADR-0090: üretilen modül metninde iki kez bildirilen her ad için
    /// E1003. Aynı ad için başka bir E1003 (enum localparam'ı, fn
    /// açılımı) zaten verilmişse tekrar bildirilmez.
    pub(crate) fn audit_duplicate_decls(&mut self, module: &'a ModuleDecl, sv: &str) {
        for name in duplicate_decls(sv) {
            if self.collision_reported(name) {
                continue;
            }
            let origins = self.name_origins(module, name);
            let diag = collision_diag(module, name, &origins);
            self.diagnostics.push(diag);
        }
    }

    fn collision_reported(&self, name: &str) -> bool {
        let quoted = format!("'{name}'");
        self.diagnostics
            .iter()
            .any(|d| d.code == ErrorCode::E1003 && d.message.contains(&quoted))
    }

    /// `name` adını SV'ye koyan kaynaklar: önce kullanıcının yazdıkları
    /// (kaynak sırası), sonra Volt'un kurdukları.
    fn name_origins(&self, module: &ModuleDecl, name: &str) -> Vec<Origin> {
        let mut out = user_origins(self.ast, module, name);
        self.reset_origins(module, name, &mut out);
        self.sync_origins(name, &mut out);
        self.instance_origins(module, name, &mut out);
        out
    }

    /// Otomatik reset portu ve ham reset'in bırakma zinciri (ADR-0065).
    fn reset_origins(&self, module: &ModuleDecl, name: &str, out: &mut Vec<Origin>) {
        let clocks = self.collect_clock_ports(module);
        let clock_span = |clk: &str| port_span(module, clk);
        for cfg in reset_port_set(&clocks) {
            let clock = clocks
                .iter()
                .find(|c| c.info.reset.port_name() == cfg.port_name());
            if let (true, Some(c)) = (cfg.port_name() == name, clock) {
                let clk = &c.name;
                out.push(Origin {
                    span: clock_span(clk),
                    what: lstr!(
                        en: "the reset port Volt adds for clock '{clk}'";
                        tr: "Volt'un '{clk}' saati için eklediği reset portu"
                    ),
                    generated: true,
                });
            }
        }
        for c in clocks.iter().filter(|c| c.raw_reset.is_some()) {
            for i in 0..reset_sync::RESET_SYNC_STAGES {
                if reset_sync::stage_name(&c.name, i) == name {
                    let clk = &c.name;
                    out.push(Origin {
                        span: clock_span(clk),
                        what: lstr!(
                            en: "stage {i} of the reset synchronizer of clock '{clk}'";
                            tr: "'{clk}' saatinin reset senkronizörünün {i}. aşaması"
                        ),
                        generated: true,
                    });
                }
            }
        }
    }

    /// `sync()`/`sync3()` köprüsünün yakalama ve aşama register'ları.
    fn sync_origins(&self, name: &str, out: &mut Vec<Origin>) {
        for b in &self.sync_bridges {
            let Some(rest) = name
                .strip_prefix(b.base.as_str())
                .and_then(|r| r.strip_prefix('_'))
            else {
                continue;
            };
            let src = b.base.strip_prefix("sync_").unwrap_or(&b.base);
            let what = if rest == "src" && b.capture {
                lstr!(
                    en: "the capture register of the sync() of '{src}'";
                    tr: "'{src}' sync()'unun yakalama register'ı"
                )
            } else {
                match rest
                    .strip_prefix("stage")
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    Some(i) if i < b.stages => lstr!(
                        en: "stage {i} of the sync() of '{src}'";
                        tr: "'{src}' sync()'unun {i}. aşaması"
                    ),
                    _ => continue,
                }
            };
            out.push(Origin {
                span: b.span,
                what,
                generated: true,
            });
        }
    }

    /// Kullanıcı modülü örneklerinin çıkış telleri ve yerleşik primitif
    /// örneklerinin sinyalleri (`<örnek>_<ad>`).
    fn instance_origins(&self, module: &ModuleDecl, name: &str, out: &mut Vec<Origin>) {
        for &stmt in &module.body {
            let StmtKind::Instance(inst) = &self.ast.stmts[stmt].kind else {
                continue;
            };
            let i = &inst.name.text;
            let Some(rest) = name
                .strip_prefix(i.as_str())
                .and_then(|r| r.strip_prefix('_'))
            else {
                continue;
            };
            let what = if let Some(u) = self.user_insts.get(i) {
                if !u.outputs.iter().any(|(o, _)| o == rest) {
                    continue;
                }
                lstr!(
                    en: "output '{rest}' of instance '{i}'";
                    tr: "'{i}' örneğinin '{rest}' çıkışı"
                )
            } else if let Some(b) = self.builtin_insts.get(i) {
                let prim = b.prim.name();
                lstr!(
                    en: "signal '{rest}' of the built-in {prim} instance '{i}'";
                    tr: "yerleşik {prim} örneği '{i}'in '{rest}' sinyali"
                )
            } else {
                continue;
            };
            out.push(Origin {
                span: inst.name.span,
                what,
                generated: true,
            });
        }
    }
}

/// Kullanıcının yazdığı adlar: portlar (bundle alanı düzleştirilmiş
/// hâlde), register/wire/let bildirimleri ve örnek adları.
fn user_origins(ast: &volt_ast::SourceFile, module: &ModuleDecl, name: &str) -> Vec<Origin> {
    let mut out = Vec::new();
    for p in module.ports.iter().filter(|p| p.name.text == name) {
        let (span, what) = match &p.bundle {
            Some(b) => {
                let (port, field) = (&b.port.text, &b.path);
                (
                    b.port.span,
                    lstr!(
                        en: "field '{field}' of bundle port '{port}'";
                        tr: "'{port}' bundle portunun '{field}' alanı"
                    ),
                )
            }
            None => (
                p.name.span,
                lstr!(en: "port '{name}'"; tr: "'{name}' portu"),
            ),
        };
        out.push(Origin {
            span,
            what,
            generated: false,
        });
    }
    for &stmt in &module.body {
        let (n, what) = match &ast.stmts[stmt].kind {
            StmtKind::Reg(r) => (
                &r.name,
                lstr!(en: "register '{name}'"; tr: "'{name}' register'ı"),
            ),
            StmtKind::Wire(w) => (&w.name, lstr!(en: "wire '{name}'"; tr: "'{name}' teli")),
            StmtKind::Let(l) => (&l.name, lstr!(en: "let '{name}'"; tr: "'{name}' let'i")),
            StmtKind::Instance(i) => (
                &i.name,
                lstr!(en: "instance '{name}'"; tr: "'{name}' örneği"),
            ),
            // Ad bildirmeyen deyimler (modül düzeyi `for` parser'da açıldı, ADR-0056).
            StmtKind::On(_)
            | StmtKind::Comb(_)
            | StmtKind::Assign(_)
            | StmtKind::For(_)
            | StmtKind::Expr(_)
            | StmtKind::Error => continue,
        };
        if n.text == name {
            out.push(Origin {
                span: n.span,
                what,
                generated: false,
            });
        }
    }
    out
}

fn port_span(module: &ModuleDecl, name: &str) -> Span {
    module
        .ports
        .iter()
        .find(|p| p.name.text == name)
        .map_or(module.name.span, |p| p.name.span)
}

/// E1003: iki kaynak tanınırsa ikisini adlandıran iki etiketli ileti
/// (birincil etiket Volt'un kurduğu adda), yoksa modül adında genel ileti.
fn collision_diag(module: &ModuleDecl, name: &str, origins: &[Origin]) -> Diagnostic {
    let help = lstr!(
        en: "rename one of them — Volt does not rename SystemVerilog names: ports are the module interface, and synchronizer and instance signals are named in timing constraints and waveforms (see volt explain E1003)";
        tr: "birini yeniden adlandırın — Volt SystemVerilog adlarını değiştirmez: portlar modül arayüzüdür, senkronizör ve örnek sinyallerinin adları zamanlama kısıtlarında ve dalga biçimlerinde geçer (bkz. volt explain E1003)"
    );
    let (first, second) = match pick_pair(origins) {
        Some(pair) => pair,
        None => {
            let m = &module.name.text;
            return Diagnostic::error(
                ErrorCode::E1003,
                lstr!(
                    en: "the SystemVerilog generated for module '{m}' declares '{name}' more than once";
                    tr: "'{m}' modülü için üretilen SystemVerilog '{name}' adını birden çok kez bildiriyor"
                ),
                LabeledSpan::primary(
                    module.name.span,
                    lstr!(en: "in this module"; tr: "bu modülde"),
                ),
                help,
            );
        }
    };
    let (a, b) = (&first.what, &second.what);
    // Birincil etiket Volt'un kurduğu adda: ikinci kaynak her zaman
    // kurulmuş addır (`pick_pair`).
    Diagnostic::error(
        ErrorCode::E1003,
        lstr!(
            en: "'{name}' is both {a} and {b}";
            tr: "'{name}' hem {a} hem {b}"
        ),
        LabeledSpan::primary(
            second.span,
            lstr!(en: "becomes '{name}'"; tr: "'{name}' adını alıyor"),
        ),
        help,
    )
    .with_secondary(first.span, lstr!(en: "also '{name}'"; tr: "bu da '{name}'"))
}

/// İletideki iki kaynak (ikincisi Volt'un kurduğu ad): kullanıcı adı ve
/// kurulan ad, yoksa iki kurulan ad. Yalnız kullanıcı adlarının
/// çakışması çözümlemenin E1003'üdür — burada genel ileti.
fn pick_pair(origins: &[Origin]) -> Option<(&Origin, &Origin)> {
    let mut generated = origins.iter().filter(|o| o.generated);
    let gen = generated.next()?;
    match origins.iter().find(|o| !o.generated) {
        Some(user) => Some((user, gen)),
        None => generated.next().map(|second| (gen, second)),
    }
}

/// `base`, `base_2`, `base_3`, …: `clash` olmayan ilk ad (ADR-0090 §2,
/// yalnız modül içinde kalan yardımcı adlar; fn açılımındaki `_2` ile
/// aynı kural).
pub(crate) fn fresh_name(base: &str, clash: impl Fn(&str) -> bool) -> String {
    if !clash(base) {
        return base.to_string();
    }
    (2..)
        .map(|k| format!("{base}_{k}"))
        .find(|n| !clash(n))
        .expect("sonsuz aday dizisi")
}

impl Emitter<'_> {
    /// Yardımcı ad seçiminin gördüğü modül adları: gövde üretilmeden
    /// önce kullanıcının yazdıkları (portlar, bildirimler, örnekler).
    pub(crate) fn seed_helper_taken(&mut self, module: &ModuleDecl) {
        let ast = self.ast;
        self.helper_taken.clear();
        self.helper_taken
            .extend(module.ports.iter().map(|p| p.name.text.clone()));
        for &stmt in &module.body {
            let name = match &ast.stmts[stmt].kind {
                StmtKind::Reg(r) => &r.name,
                StmtKind::Wire(w) => &w.name,
                StmtKind::Let(l) => &l.name,
                StmtKind::Instance(i) => &i.name,
                // Ad bildirmeyen deyimler (modül düzeyi `for` parser'da açıldı, ADR-0056).
                StmtKind::On(_)
                | StmtKind::Comb(_)
                | StmtKind::Assign(_)
                | StmtKind::For(_)
                | StmtKind::Expr(_)
                | StmtKind::Error => continue,
            };
            self.helper_taken.insert(name.text.clone());
        }
    }

    /// Gövde üretildikten sonra: metinde bildirilen her ad (üretilenler
    /// dahil) ve ön bildirilen örnek çıkış telleri.
    pub(crate) fn extend_helper_taken(&mut self, chunks: &[&str]) {
        let names: Vec<String> = chunks
            .iter()
            .flat_map(|c| declared_names(c))
            .map(str::to_owned)
            .chain(self.pre_decls.iter().map(|(w, _)| w.clone()))
            .collect();
        self.helper_taken.extend(names);
    }

    /// Yardımcı ad (`volt_hits_*`) modülde kullanılıyor mu?
    pub(crate) fn helper_name_taken(&self, name: &str) -> bool {
        self.helper_taken.contains(name) || self.module_name_taken(name)
    }

    /// `prev()` zinciri tabanı: `<taban>_<k>` biçiminde hiçbir modül adı
    /// yok ve başka zincirin tabanı değil.
    pub(crate) fn chain_base_taken(&self, base: &str, other_bases: &[&str]) -> bool {
        other_bases.contains(&base)
            || self.helper_taken.iter().any(|n| {
                n.strip_prefix(base)
                    .and_then(|r| r.strip_prefix('_'))
                    .is_some_and(|k| !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_names_cover_emitter_declaration_shapes() {
        let sv = "module M (\n    input  logic [7:0] a,\n    input  logic       clk,\n    \
                  output logic y  // yorum\n);\n\n    logic [7:0] r;\n    logic signed [3:0] s;\n    \
                  logic [7:0] mem [0:3];\n    wire [3:0] w = a[3:0];\n    wire v = a == 8'd3;\n    \
                  localparam logic [1:0] S_Idle  = 2'd0;\n    longint volt_hits_cov_0 = 0;\n    \
                  tri1 sda_bus;\n    Timer timer (\n        .clk(clk)\n    );\n    \
                  assign y = r[0];\n    always_ff @(posedge clk) begin\n        logic z;\n    end\n\
                  endmodule\n";
        assert_eq!(
            declared_names(sv),
            [
                "a",
                "clk",
                "y",
                "r",
                "s",
                "mem",
                "w",
                "v",
                "S_Idle",
                "volt_hits_cov_0",
                "sda_bus",
                "timer"
            ]
        );
    }

    #[test]
    fn non_declarations_are_not_names() {
        let sv = "    assert property (\n    assign a = b;\n    assert property (inv_0);\n    property cov_0;\n    \
                  always_comb begin\n    function automatic logic [7:0] T_at(input logic [1:0] i);\n    \
                  initial assume (rst);\n    );\n    // logic x;\n";
        assert!(declared_names(sv).is_empty(), "{:?}", declared_names(sv));
    }

    fn module_of(src: &str) -> ModuleDecl {
        let parsed = volt_syntax::parser::parse(volt_span::FileId(0), src);
        parsed
            .ast
            .items
            .iter()
            .find_map(|&i| {
                if let volt_ast::ItemKind::Module(m) = &parsed.ast.items_arena[i].kind {
                    Some(m.clone())
                } else {
                    None
                }
            })
            .expect("modül")
    }

    fn origin(generated: bool, what: &str, at: u32) -> Origin {
        Origin {
            span: Span::new(volt_span::FileId(0), at, at + 1),
            what: what.to_string(),
            generated,
        }
    }

    #[test]
    fn unknown_origin_falls_back_to_module_message() {
        let m = module_of("module M {\n    out y : bool\n    y = true\n}\n");
        let d = collision_diag(&m, "x", &[]);
        assert_eq!(d.code, ErrorCode::E1003);
        assert!(
            d.message.contains("declares 'x' more than once"),
            "{}",
            d.message
        );
        assert_eq!(d.spans.len(), 1);
        // Yalnız kullanıcı adları: çözümlemenin işi, burada genel ileti.
        let users = [origin(false, "port 'x'", 1), origin(false, "let 'x'", 5)];
        assert!(collision_diag(&m, "x", &users)
            .message
            .contains("more than once"));
    }

    #[test]
    fn user_name_first_generated_name_primary() {
        let m = module_of("module M {\n    out y : bool\n    y = true\n}\n");
        let o = [
            origin(true, "output 'irq' of instance 'timer'", 9),
            origin(false, "port 'timer_irq'", 2),
        ];
        let d = collision_diag(&m, "timer_irq", &o);
        assert!(
            d.message.contains(
                "'timer_irq' is both port 'timer_irq' and output 'irq' of instance 'timer'"
            ),
            "{}",
            d.message
        );
        assert_eq!(d.primary_span().map(|s| s.span.start), Some(9));
    }

    #[test]
    fn duplicates_in_first_seen_order() {
        let sv = "    logic b;\n    logic a;\n    wire a = b;\n    Timer b (\n";
        assert_eq!(duplicate_decls(sv), ["a", "b"]);
    }
}
