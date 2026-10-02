//! Dalga formunda enum ve Trit adları (ADR-0092).
//!
//! VCD enum tipi taşımaz (ADR-0074 ölçümü): `state_r` GTKWave'de `01`
//! görünür, `Start` değil. Üretilen RTL'ye dokunmadan görüntüleyici
//! tarafında çözülür: her enum için bir GTKWave çeviri tablosu ("translate
//! filter file") ve sinyalleri bu tablolara bağlayan bir oturum dosyası
//! (`.gtkw`). Aynı dosyalar simülasyon VCD'sinde ve formal karşı örnek
//! VCD'sinde çalışır; ikisi yalnız hiyerarşi önekiyle ayrılır.
//!
//! Emitter modül başına enum/Trit sinyallerini ve örnekleri kaydeder
//! ([`WaveInfo`]); hiyerarşi yürüyüşü ve metin üretimi saf işlevlerdir,
//! dosya yazmak sürücünün işidir.

use std::collections::HashSet;
use std::fmt::Write as _;

use volt_ast::{ItemKind, ModuleDecl, PortDir, StmtKind, TypeRefKind};

use crate::Emitter;

/// Trit tablosunun adı (ADR-0062: işaretli 2 bit).
pub const TRIT_TABLE: &str = "Trit";

/// Geçersiz kodların tek tek yazıldığı en büyük genişlik: 8 bitte tablo
/// en çok 256 satırdır. Daha genişte eşleşmeyen kod GTKWave'de ham ikili
/// değer olarak görünür.
pub const MAX_LISTED_INVALID_WIDTH: u32 = 8;

/// Oturum dosyasının ilk satırı: sürücü yalnız bu satırla başlayan
/// dosyaların üzerine yazar (kullanıcının kendi `.gtkw`'si korunur).
pub const GTKW_MARKER: &str = "[*] Volt waveform session (ADR-0092)";

/// Tek sinyal: SV adı, genişlik, çeviri tablosunun adı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveSignal {
    pub name: String,
    pub width: u32,
    pub table: String,
}

/// Modülün enum/Trit sinyalleri (bildirim sırası) ve kullanıcı modülü
/// örnekleri (örnek adı, hedef modül).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WaveModule {
    pub name: String,
    pub signals: Vec<WaveSignal>,
    pub instances: Vec<(String, String)>,
}

/// Çeviri tablosu: kod → ad, kod sırasıyla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveTable {
    pub name: String,
    pub width: u32,
    pub entries: Vec<(u128, String)>,
}

/// Birimin dalga formu bilgisi.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WaveInfo {
    pub modules: Vec<WaveModule>,
    pub tables: Vec<WaveTable>,
}

/// Oturumdaki tek iz: görüntüleyicideki tam ad ve tablo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveTrace {
    pub path: String,
    pub table: String,
}

/// Bir üst modülden kurulan oturum; yalnız izlerin kullandığı tablolar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveSession<'a> {
    pub traces: Vec<WaveTrace>,
    pub tables: Vec<&'a WaveTable>,
}

/// Hiyerarşi derinliği sınırı (özyinelemeli örnekleme HIR'da reddedilir;
/// bu yalnız güvenlik ağı).
const MAX_DEPTH: usize = 64;

impl WaveInfo {
    /// `top` modülünden aşağı bütün enum/Trit izleri. `scope` VCD'deki
    /// üst kapsamın tam adıdır: Verilator simülasyonunda `TOP.<Modül>`,
    /// sby karşı örneğinde `<Modül>`. Hiç iz yoksa `None` (dosya
    /// üretilmez).
    pub fn session(&self, top: &str, scope: &str) -> Option<WaveSession<'_>> {
        let mut traces = Vec::new();
        self.walk(top, scope, 0, &mut traces);
        if traces.is_empty() {
            return None;
        }
        let used: HashSet<&str> = traces.iter().map(|t| t.table.as_str()).collect();
        let tables = self
            .tables
            .iter()
            .filter(|t| used.contains(t.name.as_str()))
            .collect();
        Some(WaveSession { traces, tables })
    }

    fn walk(&self, module: &str, scope: &str, depth: usize, out: &mut Vec<WaveTrace>) {
        if depth > MAX_DEPTH {
            return;
        }
        let Some(m) = self.modules.iter().find(|m| m.name == module) else {
            return;
        };
        for s in &m.signals {
            out.push(WaveTrace {
                path: format!("{scope}.{}", gtkwave_name(&s.name, s.width)),
                table: s.table.clone(),
            });
        }
        for (inst, target) in &m.instances {
            self.walk(target, &format!("{scope}.{inst}"), depth + 1, out);
        }
    }
}

/// GTKWave'in vektör adı: çok bitli sinyal `ad[W-1:0]` — VCD'de aralık
/// yazılmamış olsa da (sby izi) GTKWave aralığı ekler; aralıksız ad
/// oturumda sessizce düşer (ölçüm ADR-0092).
fn gtkwave_name(name: &str, width: u32) -> String {
    if width > 1 {
        format!("{name}[{}:0]", width - 1)
    } else {
        name.to_string()
    }
}

/// Kodun `width` basamaklı ikili yazımı (oturum izi ikili gösterimdedir).
fn binary(code: u128, width: u32) -> String {
    format!("{code:0w$b}", w = width as usize)
}

/// Çeviri tablosu metni. Geçersiz kodlar (hiçbir varyanta ait olmayan)
/// 8 bite kadar kırmızı `invalid <kod>` olarak listelenir.
pub fn filter_text(table: &WaveTable) -> String {
    let mut out = format!(
        "# Volt: {} ({} bit) - GTKWave translate filter (ADR-0092)\n",
        table.name, table.width
    );
    let listed = table.width <= MAX_LISTED_INVALID_WIDTH;
    let mut codes: Vec<(u128, String)> = table.entries.clone();
    if listed {
        let known: HashSet<u128> = table.entries.iter().map(|(c, _)| *c).collect();
        for code in 0..(1u128 << table.width) {
            if !known.contains(&code) {
                codes.push((code, invalid_label(table, code)));
            }
        }
    }
    codes.sort_by_key(|(c, _)| *c);
    for (code, label) in codes {
        let _ = writeln!(out, "{} {label}", binary(code, table.width));
    }
    out
}

/// Geçersiz kodun etiketi: Trit'te işaretli değer (`10` = -2), enum'da
/// kodun kendisi.
fn invalid_label(table: &WaveTable, code: u128) -> String {
    if table.name == TRIT_TABLE && code == 0b10 {
        return "?red?invalid -2".to_string();
    }
    format!("?red?invalid {code}")
}

/// Trit çeviri tablosu: `01` +1, `00` 0, `11` -1 (ADR-0062 kodlaması).
pub fn trit_table() -> WaveTable {
    WaveTable {
        name: TRIT_TABLE.to_string(),
        width: 2,
        entries: vec![
            (0b00, "0".to_string()),
            (0b01, "+1".to_string()),
            (0b11, "-1".to_string()),
        ],
    }
}

/// Oturum dosyası metni. `filters` her tablonun dosya yolu (tablo adı
/// sırasıyla `session.tables`'a karşılık gelir). Yollar olduğu gibi
/// yazılır; GTKWave göreli yolu `.gtkw`'nin dizinine göre değil çalışma
/// dizinine göre açar (ölçüm ADR-0092).
pub fn gtkw_text(session: &WaveSession<'_>, dumpfile: &str, filters: &[String]) -> String {
    let mut out = format!("{GTKW_MARKER}\n[dumpfile] \"{dumpfile}\"\n");
    for trace in &session.traces {
        let Some(idx) = session.tables.iter().position(|t| t.name == trace.table) else {
            continue;
        };
        let Some(path) = filters.get(idx) else {
            continue;
        };
        // ^N: N numaralı çeviri dosyası; @2028 = ikili + sağa yaslı +
        // dosya çevirisi (TR_BIN | TR_RJUSTIFY | TR_FTRANSLATED).
        let _ = write!(out, "^{} {path}\n@2028\n{}\n", idx + 1, trace.path);
    }
    out
}

impl<'a> Emitter<'a> {
    /// Üretilen modülün enum/Trit sinyallerini kaydeder. `body` modülün
    /// SV metnidir: yalnız orada bildirilen adlar kaydedilir (görüntüleyici
    /// olmayan sinyali aramasın).
    pub(crate) fn record_waves(&mut self, module: &'a ModuleDecl, body: &str) {
        let declared = declared_names(body);
        let mut names: Vec<String> = module.ports.iter().map(|p| p.name.text.clone()).collect();
        let mut instances = Vec::new();
        for &stmt_idx in &module.body {
            match &self.ast.stmts[stmt_idx].kind {
                StmtKind::Reg(r) => names.push(r.name.text.clone()),
                StmtKind::Wire(w) => names.push(w.name.text.clone()),
                StmtKind::Let(l) => names.push(l.name.text.clone()),
                StmtKind::Instance(inst) => {
                    let Some(info) = self.user_insts.get(&inst.name.text) else {
                        continue;
                    };
                    instances.push((inst.name.text.clone(), info.module.clone()));
                }
                // Dalga formuna ad eklemeyen deyimler.
                StmtKind::On(_)
                | StmtKind::Comb(_)
                | StmtKind::Assign(_)
                | StmtKind::For(_)
                | StmtKind::Expr(_)
                | StmtKind::Error => {}
            }
        }
        let mut signals = Vec::new();
        for name in names {
            if !declared.contains(&name) {
                continue;
            }
            if let Some(sig) = self.wave_signal_of(&name) {
                signals.push(sig);
            }
        }
        for (inst, target) in &instances {
            signals.extend(self.instance_wave_outputs(inst, target, &declared));
        }
        let entry = WaveModule {
            name: module.name.text.clone(),
            signals,
            instances,
        };
        self.wave_modules.push(entry);
    }

    /// Modül sinyali enum ya da tekil Trit ise kaydı (diziler paketlenmiş
    /// vektördür, eleman başına ad verilemez).
    fn wave_signal_of(&self, name: &str) -> Option<WaveSignal> {
        if let Some(enum_name) = self.enum_sigs.get(name) {
            let width = self.enum_width(enum_name)?;
            return Some(WaveSignal {
                name: name.to_string(),
                width,
                table: enum_name.clone(),
            });
        }
        let is_array = self.array_dims.contains_key(name) || self.packed_arrays.contains_key(name);
        if self.trits.contains(name) && !is_array {
            return Some(WaveSignal {
                name: name.to_string(),
                width: 2,
                table: TRIT_TABLE.to_string(),
            });
        }
        None
    }

    /// Örnek çıkış telleri (`<örnek>_<port>`): tip hedef modülün portundan.
    fn instance_wave_outputs(
        &self,
        inst: &str,
        target: &str,
        declared: &HashSet<String>,
    ) -> Vec<WaveSignal> {
        let Some(target) = self.inst_target_named(target) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for p in target
            .ports()
            .iter()
            .filter(|p| p.direction == PortDir::Out)
        {
            let wire = format!("{inst}_{}", p.name.text);
            if !declared.contains(&wire) {
                continue;
            }
            let resolved = crate::alias::resolve(self.ast, p.ty);
            if let Some(decl) = self.enum_of_type(p.ty) {
                if let Some(width) = self.enum_width(&decl.name.text) {
                    out.push(WaveSignal {
                        name: wire,
                        width,
                        table: decl.name.text.clone(),
                    });
                }
            } else if matches!(self.ast.types[resolved].kind, TypeRefKind::Trit) {
                out.push(WaveSignal {
                    name: wire,
                    width: 2,
                    table: TRIT_TABLE.to_string(),
                });
            }
        }
        out
    }

    /// Adı verilen enum'un geçerli kodlamasının genişliği.
    fn enum_width(&self, enum_name: &str) -> Option<u32> {
        self.enum_table(enum_name).map(|t| t.width)
    }

    /// Adı verilen enum'un çeviri tablosu (geçersiz kodlamada `None`).
    fn enum_table(&self, enum_name: &str) -> Option<WaveTable> {
        let decl = self.ast.items.iter().find_map(|&i| {
            let ItemKind::Enum(e) = &self.ast.items_arena[i].kind else {
                return None;
            };
            (e.name.text == enum_name).then_some(e)
        })?;
        let layout = self.enum_layout(decl)?;
        let mut entries: Vec<(u128, String)> = layout
            .values
            .iter()
            .zip(&decl.variants)
            .map(|(&code, v)| (code, v.name.text.clone()))
            .collect();
        entries.sort_by_key(|(c, _)| *c);
        Some(WaveTable {
            name: enum_name.to_string(),
            width: layout.width,
            entries,
        })
    }

    /// Kaydedilen modüllerin kullandığı tablolar (ilk kullanım sırası).
    pub(crate) fn wave_info(&self) -> WaveInfo {
        let mut tables: Vec<WaveTable> = Vec::new();
        for s in self.wave_modules.iter().flat_map(|m| &m.signals) {
            if tables.iter().any(|t| t.name == s.table) {
                continue;
            }
            let table = if s.table == TRIT_TABLE {
                Some(trit_table())
            } else {
                self.enum_table(&s.table)
            };
            tables.extend(table);
        }
        WaveInfo {
            modules: self.wave_modules.clone(),
            tables,
        }
    }
}

/// SV gövdesinde bildirilen adlar: `input`/`output`/`inout`/`logic`/
/// `wire`/`tri1` ile başlayan satırın `;`, `,` ya da `=` öncesindeki son
/// tanımlayıcısı.
fn declared_names(body: &str) -> HashSet<String> {
    const KEYWORDS: [&str; 6] = ["input", "output", "inout", "logic", "wire", "tri1"];
    let mut out = HashSet::new();
    for line in body.lines() {
        let code = line.split("//").next().unwrap_or("").trim();
        let Some(first) = code.split_whitespace().next() else {
            continue;
        };
        if !KEYWORDS.contains(&first) {
            continue;
        }
        let head = code.split([';', ',', '=']).next().unwrap_or("");
        let last = head
            .split(|c: char| c.is_whitespace() || c == '[' || c == ']')
            .rfind(|t| {
                t.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                    && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            });
        if let Some(name) = last {
            if !KEYWORDS.contains(&name) && name != "signed" {
                out.insert(name.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phase() -> WaveTable {
        WaveTable {
            name: "Phase".into(),
            width: 2,
            entries: vec![(0, "Idle".into()), (1, "Run".into()), (2, "Done".into())],
        }
    }

    fn info() -> WaveInfo {
        WaveInfo {
            modules: vec![
                WaveModule {
                    name: "Top".into(),
                    signals: vec![WaveSignal {
                        name: "st".into(),
                        width: 2,
                        table: "Phase".into(),
                    }],
                    instances: vec![("u".into(), "Sub".into())],
                },
                WaveModule {
                    name: "Sub".into(),
                    signals: vec![
                        WaveSignal {
                            name: "t".into(),
                            width: 2,
                            table: TRIT_TABLE.into(),
                        },
                        WaveSignal {
                            name: "b".into(),
                            width: 1,
                            table: "Bit".into(),
                        },
                    ],
                    instances: vec![],
                },
            ],
            tables: vec![phase(), trit_table()],
        }
    }

    #[test]
    fn session_walks_instances_with_the_viewer_prefix() {
        let info = info();
        let s = info.session("Top", "TOP.Top").expect("iz var");
        let paths: Vec<&str> = s.traces.iter().map(|t| t.path.as_str()).collect();
        assert_eq!(
            paths,
            ["TOP.Top.st[1:0]", "TOP.Top.u.t[1:0]", "TOP.Top.u.b"]
        );
        let formal = info.session("Top", "Top").expect("iz var");
        assert_eq!(formal.traces[1].path, "Top.u.t[1:0]");
        // Yalnız izlerin kullandığı tablolar.
        let names: Vec<&str> = s.tables.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Phase", "Trit"]);
    }

    #[test]
    fn module_without_enum_signals_has_no_session() {
        let info = info();
        assert!(info.session("Missing", "TOP.Missing").is_none());
        let empty = WaveInfo {
            modules: vec![WaveModule {
                name: "Plain".into(),
                ..WaveModule::default()
            }],
            tables: vec![],
        };
        assert!(empty.session("Plain", "TOP.Plain").is_none());
    }

    #[test]
    fn filter_lists_every_code_with_invalid_ones_in_red() {
        assert_eq!(
            filter_text(&phase()),
            "# Volt: Phase (2 bit) - GTKWave translate filter (ADR-0092)\n\
             00 Idle\n01 Run\n10 Done\n11 ?red?invalid 3\n"
        );
        assert_eq!(
            filter_text(&trit_table()),
            "# Volt: Trit (2 bit) - GTKWave translate filter (ADR-0092)\n\
             00 0\n01 +1\n10 ?red?invalid -2\n11 -1\n"
        );
    }

    #[test]
    fn wide_tables_list_only_the_variants() {
        let wide = WaveTable {
            name: "Op".into(),
            width: 9,
            entries: vec![(3, "Load".into()), (300, "Alu".into())],
        };
        let text = filter_text(&wide);
        assert_eq!(text.lines().count(), 3);
        assert!(text.contains("000000011 Load\n100101100 Alu\n"), "{text}");
    }

    #[test]
    fn gtkw_binds_each_trace_to_its_filter() {
        let info = info();
        let s = info.session("Top", "Top").expect("iz var");
        let text = gtkw_text(&s, "/w/c.vcd", &["/w/P.txt".into(), "/w/T.txt".into()]);
        assert_eq!(
            text,
            "[*] Volt waveform session (ADR-0092)\n[dumpfile] \"/w/c.vcd\"\n\
             ^1 /w/P.txt\n@2028\nTop.st[1:0]\n\
             ^2 /w/T.txt\n@2028\nTop.u.t[1:0]\n"
        );
    }

    #[test]
    fn declared_names_reads_ports_and_signals() {
        let body =
            "module M (\n    input  logic       clk,\n    output logic [2:0] state,  // S\n);\n\
                    logic signed [1:0] u_tv;\n    logic [7:0] mem [0:3];\n    logic x = 1'b0;\n\
                    assign y = x;\n";
        let names = declared_names(body);
        for n in ["clk", "state", "u_tv", "x"] {
            assert!(names.contains(n), "{n}: {names:?}");
        }
        assert!(!names.contains("y") && !names.contains("signed") && !names.contains("logic"));
    }
}
