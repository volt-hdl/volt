//! Yerleşik stdlib primitiflerinin SV üretimi (ADR-0027, ADR-0029).
//!
//! `try_emit_sync_bridge` deseninin genellemesi: örnekleme deyimi
//! satır içi SV'ye açılır (ayrı modül örneklenmez), tüm üretilen
//! isimler `<örnek>_` önekini taşır. Doğrulama ön geçişte yapılır
//! (`collect_builtin_insts`) ki `f.rd_data` alan erişimleri deyim
//! sırasından bağımsız çevrilebilsin. Immediate SVA modunda her
//! primitif kendi formal kontratlarını da üretir (gözlemci register'lar
//! YALNIZ bu modda eklenir — normal build lint-temiz kalır).
//!
//! Tek saatli primitiflerde (ADR-0029) `clk` portu hem kaynak hem
//! hedef saattir: `dst_clock == src_clock` ve tüm portlar Src rolündedir.

use std::collections::HashMap;

use volt_ast::builtin::{BuiltinPrim, ConstRule, DomainRole, PortKind};
use volt_ast::{Expr, GenericArg, Idx, InstanceDecl, ModuleDecl, PortDir, StmtKind};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use crate::expr::Sig;
use crate::sva::{SvaMode, SvaProp};
use crate::{zero_of, ClockPort, Emitter};

/// Doğrulanmış yerleşik primitif örneği.
#[derive(Debug, Clone)]
pub(crate) struct BuiltinInst {
    pub(crate) prim: BuiltinPrim,
    /// `T`'nin genişlik/işaret bilgisi (veri portu olmayanlarda 1 bit).
    pub(crate) data: Sig,
    /// Sabit generic argüman: DEPTH/WIDTH/LEN/N (yoksa 0).
    pub(crate) dim: u64,
    /// Yazma/kaynak alanının saat portu.
    src_clock: ClockPort,
    /// Okuma/hedef alanının saat portu (tek saatlilerde src ile aynı).
    dst_clock: ClockPort,
    /// Giriş portu adı → bağlama ifadesi (None: `ad` kısayolu —
    /// port adıyla aynı isimli yerel sinyal).
    inputs: HashMap<&'static str, Option<Idx<Expr>>>,
}

impl BuiltinInst {
    /// Adres genişliği: clog2(DEPTH). Yalnız DEPTH'li primitiflerde
    /// anlamlı (DEPTH >= 2 iki kuvveti — doğrulama garanti eder).
    fn addr_width(&self) -> u32 {
        self.dim.trailing_zeros()
    }
}

impl<'a> Emitter<'a> {
    /// Modüldeki tüm yerleşik primitif örneklerini doğrulayıp kaydeder
    /// (sembol ön geçişinin eşleniği). Geçersiz örnekler tanı üretir ve
    /// tabloya girmez; emisyon aşaması onlar için boş chunk döndürür.
    pub(crate) fn collect_builtin_insts(&mut self, module: &'a ModuleDecl, clocks: &[ClockPort]) {
        self.builtin_insts.clear();
        let ast = self.ast;
        for &stmt_idx in &module.body {
            let StmtKind::Instance(inst) = &ast.stmts[stmt_idx].kind else {
                continue;
            };
            if inst.module_path.segments.len() != 1 {
                continue;
            }
            let Some(prim) = BuiltinPrim::from_name(&inst.module_path.segments[0].text) else {
                continue;
            };
            let span = ast.stmts[stmt_idx].span;
            if let Some(info) = self.builtin_inst_info(inst, prim, clocks, span) {
                self.builtin_insts.insert(inst.name.text.clone(), info);
            }
        }
    }

    /// Tek örneğin doğrulaması: generic argümanlar (T genişliği, sabit
    /// argüman kuralı — E2025), saat bağlamaları (modülün saat portları)
    /// ve giriş bağlamalarının varlığı. HIR ile örtüşen denetimler
    /// (bilinmeyen port, tip uyumu) burada YİNELENMEZ; sv-emit'in
    /// kendi başına koştuğu testler için asgari koruma tutulur.
    fn builtin_inst_info(
        &mut self,
        inst: &InstanceDecl,
        prim: BuiltinPrim,
        clocks: &[ClockPort],
        span: Span,
    ) -> Option<BuiltinInst> {
        let ast = self.ast;

        // T — ilk generic argüman (veri portlu primitifler).
        let data = if prim.type_arg_count() == 1 {
            match inst.generic_args.first() {
                Some(GenericArg::Type(t)) => {
                    let t = *t;
                    self.sig_of_typeref(t, span)?
                }
                _ => {
                    self.future(
                        span,
                        &lstr!(
                            en: "'{}' without a <T> type argument", prim.name();
                            tr: "<T> tip argümanı olmayan '{}'", prim.name()
                        ),
                    );
                    return None;
                }
            }
        } else {
            Sig {
                width: 1,
                signed: false,
            }
        };

        // Sabit argüman (T'den sonra): primitifin kuralına uymalı.
        // HIR'sız koşularda da kural korunur (E2025).
        let dim = if prim.const_arg_count() == 1 {
            let rule = prim.const_rule().expect("const_arg_count == 1");
            match inst.generic_args.get(prim.type_arg_count()) {
                Some(GenericArg::Const(e)) => match self.eval_const(*e) {
                    Some(n) if rule.allows(n) => n as u64,
                    Some(n) => {
                        let (name, param) = (prim.name(), prim.const_param_name());
                        let msg = match rule {
                            ConstRule::PowerOfTwo { .. } => lstr!(
                                en: "{name} {param} must be a power of two, got {n}";
                                tr: "{name} {param} iki kuvveti olmalı, {n} verildi"
                            ),
                            ConstRule::Range { .. } => lstr!(
                                en: "{name} {param} is out of range, got {n}";
                                tr: "{name} {param} aralık dışı, {n} verildi"
                            ),
                        };
                        self.error(
                            ErrorCode::E2025,
                            msg,
                            ast.exprs[*e].span,
                            &lstr!(en: "{}", prim.const_rule_hint_en();
                                   tr: "{}", prim.const_rule_hint_tr()),
                        );
                        return None;
                    }
                    None => {
                        self.future(
                            span,
                            &lstr!(
                                en: "'{}' with a non-literal {}", prim.name(), prim.const_param_name();
                                tr: "literal olmayan {}'li '{}'", prim.const_param_name(), prim.name()
                            ),
                        );
                        return None;
                    }
                },
                _ => {
                    self.future(
                        span,
                        &lstr!(
                            en: "'{}' without its generic arguments ({})",
                                prim.name(), prim.generic_shape();
                            tr: "generic argümanları olmayan '{}' ({})",
                                prim.name(), prim.generic_shape()
                        ),
                    );
                    return None;
                }
            }
        } else {
            0
        };

        // Bağlamalar: saatler modülün saat portu OLMALI (sync köprüsü
        // ile aynı doğrulama tarzı); girişler ifade olarak saklanır.
        let mut src_clock = None;
        let mut dst_clock = None;
        let mut inputs: HashMap<&'static str, Option<Idx<Expr>>> = HashMap::new();
        for b in &inst.bindings {
            let Some(port) = prim.port(&b.port_name.text) else {
                continue; // bilinmeyen port E1009'u HIR'da aldı
            };
            if port.dir == PortDir::Out {
                continue; // çıkışa bağlama E1009'u HIR'da aldı
            }
            if port.kind == PortKind::Clock {
                let name = match b.value {
                    Some(e) => crate::path_single(ast, e).map(str::to_owned),
                    None => Some(b.port_name.text.clone()),
                };
                let found = name
                    .as_deref()
                    .and_then(|n| clocks.iter().find(|c| c.name == n).cloned());
                let Some(cp) = found else {
                    self.future(
                        b.span,
                        &lstr!(
                            en: "'{}' whose '{}' binding is not a simple clock port of this module",
                                prim.name(), port.name;
                            tr: "'{}' bağlaması bu modülün basit bir saat portu olmayan '{}'",
                                port.name, prim.name()
                        ),
                    );
                    return None;
                };
                match port.role {
                    DomainRole::Src => src_clock = Some(cp),
                    DomainRole::Dst => dst_clock = Some(cp),
                }
            } else {
                inputs.insert(port.name, b.value);
            }
        }

        // Zorunlu girişler eksiksiz mi?
        for port in prim.ports() {
            if port.dir != PortDir::In {
                continue;
            }
            let missing = match port.kind {
                PortKind::Clock => match port.role {
                    DomainRole::Src => src_clock.is_none(),
                    DomainRole::Dst => dst_clock.is_none(),
                },
                _ => !inputs.contains_key(port.name),
            };
            if missing {
                self.future(
                    span,
                    &lstr!(
                        en: "'{}' instance without a '{}' binding", prim.name(), port.name;
                        tr: "'{}' bağlaması olmayan '{}' örneği", port.name, prim.name()
                    ),
                );
                return None;
            }
        }

        let src_clock = src_clock.expect("yukarıda denetlendi");
        // Tek saatli primitifte hedef saat = kaynak saat (ADR-0029).
        let dst_clock = if prim.has_dst_clock() {
            dst_clock.expect("yukarıda denetlendi")
        } else {
            src_clock.clone()
        };

        Some(BuiltinInst {
            prim,
            data,
            dim,
            src_clock,
            dst_clock,
            inputs,
        })
    }

    /// Örnekleme deyiminin SV chunk'ı. Ön geçiş örneği reddettiyse
    /// tanılar üretildi — boş chunk döner (sync köprüsü kalıbı).
    pub(crate) fn emit_builtin_instance(
        &mut self,
        module_name: &str,
        name: &str,
        span: Span,
    ) -> Option<String> {
        let Some(info) = self.builtin_insts.get(name).cloned() else {
            return Some(String::new());
        };
        let mut out = match info.prim {
            BuiltinPrim::AsyncFifo => self.emit_async_fifo(name, &info),
            BuiltinPrim::HandshakeSync => self.emit_handshake_sync(name, &info),
            BuiltinPrim::PulseSync => self.emit_pulse_sync(name, &info),
            BuiltinPrim::SyncFifo => self.emit_sync_fifo(name, &info),
            BuiltinPrim::Ram => self.emit_ram(name, &info),
            BuiltinPrim::DualPortRam => self.emit_dual_port_ram(name, &info),
            BuiltinPrim::Counter => self.emit_counter(name, &info),
            BuiltinPrim::ShiftRegister => self.emit_shift_register(name, &info),
            BuiltinPrim::RoundRobinArbiter => self.emit_round_robin_arbiter(name, &info),
            BuiltinPrim::PriorityArbiter => self.emit_priority_arbiter(name, &info),
            BuiltinPrim::EdgeDetect => self.emit_edge_detect(name, &info),
        };
        if self.sva_mode == SvaMode::Immediate {
            out.push_str("\n\n");
            out.push_str(&self.builtin_contracts(module_name, name, &info, span));
        }
        Some(out)
    }

    /// Giriş bağlaması ifadesi; kısayolda port adı yerel sinyaldir.
    /// Bileşik ifadeler her zaman parantezlenir.
    fn builtin_input(&mut self, info: &BuiltinInst, port: &'static str, ctx: Sig) -> String {
        match info.inputs.get(port) {
            Some(Some(e)) => format!("({})", self.emit_expr(*e, Some(ctx))),
            Some(None) => format!("({port})"),
            None => "(1'b0)".to_string(), // erişilmez: ön geçiş eksikliği reddetti
        }
    }

    // ═══ AsyncFifo — gray kod pointer'lı çift saatli FIFO ═══════════

    fn emit_async_fifo(&mut self, i: &str, info: &BuiltinInst) -> String {
        let one_bit = Sig {
            width: 1,
            signed: false,
        };
        let w = info.data.decl_type();
        let aw = info.dim.trailing_zeros(); // adres genişliği
        let pw = aw + 1; // pointer genişliği (sarma biti dahil)
        let depth = info.dim;
        let wr_en = self.builtin_input(info, "wr_en", one_bit);
        let rd_en = self.builtin_input(info, "rd_en", one_bit);
        let wr_data = self.builtin_input(info, "wr_data", info.data);
        let src = info.src_clock.clone();
        let dst = info.dst_clock.clone();
        let zero = zero_of(info.data);

        let mut out = format!(
            "    // AsyncFifo '{i}': {} -> {}, depth {depth} (gray-code pointers)\n",
            src.name, dst.name
        );
        out.push_str(&format!("    localparam int {i}_DEPTH = {depth};\n"));
        out.push_str(&format!("    {w} {i}_mem [{i}_DEPTH];\n"));
        for reg in [
            "wbin",
            "wgray",
            "rbin",
            "rgray",
            "rgray_s0",
            "rgray_s1",
            "wgray_s0",
            "wgray_s1",
            "wbin_next",
            "rbin_next",
        ] {
            out.push_str(&format!("    logic [{aw}:0] {i}_{reg};\n"));
        }
        out.push_str(&format!("    logic {i}_wr_full;\n"));
        out.push_str(&format!("    logic {i}_rd_empty;\n"));
        out.push_str(&format!("    {w} {i}_rd_data;\n\n"));

        // Pointer artışları — tek bir always_comb, boyutlu literallerle.
        out.push_str("    always_comb begin\n");
        out.push_str(&format!(
            "        {i}_wbin_next = {i}_wbin + (({wr_en} && !{i}_wr_full) ? {pw}'d1 : {pw}'d0);\n"
        ));
        out.push_str(&format!(
            "        {i}_rbin_next = {i}_rbin + (({rd_en} && !{i}_rd_empty) ? {pw}'d1 : {pw}'d0);\n"
        ));
        out.push_str("    end\n\n");

        // Yazma alanı: bellek yazımı + pointer/gray + okuma pointer'ının
        // iki-flop senkronizasyonu. Bellek resetlenmez (bilinçli).
        let idx_hi = aw.saturating_sub(1);
        let wr_body = vec![
            format!("if ({wr_en} && !{i}_wr_full) begin"),
            format!("    {i}_mem[{i}_wbin[{idx_hi}:0]] <= {wr_data};"),
            "end".to_string(),
            format!("{i}_wbin <= {i}_wbin_next;"),
            format!("{i}_wgray <= ({i}_wbin_next >> 1) ^ {i}_wbin_next;"),
            format!("{i}_rgray_s0 <= {i}_rgray;"),
            format!("{i}_rgray_s1 <= {i}_rgray_s0;"),
        ];
        let wr_reset = ["wbin", "wgray", "rgray_s0", "rgray_s1"]
            .iter()
            .map(|r| format!("{i}_{r} <= {pw}'d0;"))
            .collect::<Vec<_>>();
        out.push_str(&builtin_always_ff(&src, &wr_reset, &wr_body));
        out.push_str("\n\n");

        // Okuma alanı: senkron okuma register'ı + pointer/gray + yazma
        // pointer'ının iki-flop senkronizasyonu.
        let rd_body = vec![
            format!("if ({rd_en} && !{i}_rd_empty) begin"),
            format!("    {i}_rd_data <= {i}_mem[{i}_rbin[{idx_hi}:0]];"),
            "end".to_string(),
            format!("{i}_rbin <= {i}_rbin_next;"),
            format!("{i}_rgray <= ({i}_rbin_next >> 1) ^ {i}_rbin_next;"),
            format!("{i}_wgray_s0 <= {i}_wgray;"),
            format!("{i}_wgray_s1 <= {i}_wgray_s0;"),
        ];
        let mut rd_reset = ["rbin", "rgray", "wgray_s0", "wgray_s1"]
            .iter()
            .map(|r| format!("{i}_{r} <= {pw}'d0;"))
            .collect::<Vec<_>>();
        rd_reset.push(format!("{i}_rd_data <= {zero};"));
        out.push_str(&builtin_always_ff(&dst, &rd_reset, &rd_body));
        out.push_str("\n\n");

        // Dolu/boş bayrakları: gray karşılaştırması. AW==1'de üst iki
        // bitin tersi pointer'ın tamamıdır (sıfır genişlikli dilim yok).
        if aw == 1 {
            out.push_str(&format!(
                "    assign {i}_wr_full = ({i}_wgray == ~{i}_rgray_s1);\n"
            ));
        } else {
            out.push_str(&format!(
                "    assign {i}_wr_full = ({i}_wgray == {{~{i}_rgray_s1[{aw}:{}], \
                 {i}_rgray_s1[{}:0]}});\n",
                aw - 1,
                aw - 2
            ));
        }
        out.push_str(&format!(
            "    assign {i}_rd_empty = ({i}_rgray == {i}_wgray_s1);"
        ));
        out
    }

    // ═══ HandshakeSync — 4-fazlı req/ack, veri kaynakta stabil ══════

    fn emit_handshake_sync(&mut self, i: &str, info: &BuiltinInst) -> String {
        let one_bit = Sig {
            width: 1,
            signed: false,
        };
        let w = info.data.decl_type();
        let send = self.builtin_input(info, "send", one_bit);
        let data_in = self.builtin_input(info, "data_in", info.data);
        let src = info.src_clock.clone();
        let dst = info.dst_clock.clone();
        let zero = zero_of(info.data);

        let mut out = format!(
            "    // HandshakeSync '{i}': {} -> {} (4-phase req/ack, data held in source domain)\n",
            src.name, dst.name
        );
        for reg in [
            "req", "ack", "req_s0", "req_s1", "ack_s0", "ack_s1", "ack_d",
        ] {
            out.push_str(&format!("    logic {i}_{reg};\n"));
        }
        out.push_str(&format!("    {w} {i}_data_q;\n"));
        out.push_str(&format!("    {w} {i}_data_out;\n"));
        out.push_str(&format!("    logic {i}_busy;\n"));
        out.push_str(&format!("    logic {i}_valid;\n\n"));

        // Kaynak alan: ack'in iki-flop senkronizasyonu + req üretimi.
        // Veri register'ı req yükselirken yakalanır, ack gelene dek
        // DEĞİŞMEZ (hedef alandaki okuma bu yüzden güvenlidir).
        let src_body = vec![
            format!("{i}_ack_s0 <= {i}_ack;"),
            format!("{i}_ack_s1 <= {i}_ack_s0;"),
            format!("if ({send} && !{i}_req && !{i}_ack_s1) begin"),
            format!("    {i}_data_q <= {data_in};"),
            format!("    {i}_req <= 1'b1;"),
            format!("end else if ({i}_req && {i}_ack_s1) begin"),
            format!("    {i}_req <= 1'b0;"),
            "end".to_string(),
        ];
        let mut src_reset = ["req", "ack_s0", "ack_s1"]
            .iter()
            .map(|r| format!("{i}_{r} <= 1'b0;"))
            .collect::<Vec<_>>();
        src_reset.push(format!("{i}_data_q <= {zero};"));
        out.push_str(&builtin_always_ff(&src, &src_reset, &src_body));
        out.push_str("\n\n");

        // Hedef alan: req'in iki-flop senkronizasyonu + veri yakalama
        // + ack üretimi. ack'in yükselişi valid strobe'unu tanımlar.
        let dst_body = vec![
            format!("{i}_req_s0 <= {i}_req;"),
            format!("{i}_req_s1 <= {i}_req_s0;"),
            format!("{i}_ack_d <= {i}_ack;"),
            format!("if ({i}_req_s1 && !{i}_ack) begin"),
            format!("    {i}_data_out <= {i}_data_q;"),
            format!("    {i}_ack <= 1'b1;"),
            format!("end else if (!{i}_req_s1 && {i}_ack) begin"),
            format!("    {i}_ack <= 1'b0;"),
            "end".to_string(),
        ];
        let mut dst_reset = ["ack", "req_s0", "req_s1", "ack_d"]
            .iter()
            .map(|r| format!("{i}_{r} <= 1'b0;"))
            .collect::<Vec<_>>();
        dst_reset.push(format!("{i}_data_out <= {zero};"));
        out.push_str(&builtin_always_ff(&dst, &dst_reset, &dst_body));
        out.push_str("\n\n");

        out.push_str(&format!("    assign {i}_busy = {i}_req || {i}_ack_s1;\n"));
        out.push_str(&format!("    assign {i}_valid = {i}_ack && !{i}_ack_d;"));
        out
    }

    // ═══ PulseSync — toggle + kenar sezimi ══════════════════════════

    fn emit_pulse_sync(&mut self, i: &str, info: &BuiltinInst) -> String {
        let one_bit = Sig {
            width: 1,
            signed: false,
        };
        let pulse_in = self.builtin_input(info, "pulse_in", one_bit);
        let src = info.src_clock.clone();
        let dst = info.dst_clock.clone();

        let mut out = format!(
            "    // PulseSync '{i}': {} -> {} (toggle + edge detect)\n",
            src.name, dst.name
        );
        for reg in ["toggle", "sync0", "sync1", "sync2"] {
            out.push_str(&format!("    logic {i}_{reg};\n"));
        }
        out.push_str(&format!("    logic {i}_pulse_out;\n\n"));

        let src_body = vec![
            format!("if ({pulse_in}) begin"),
            format!("    {i}_toggle <= ~{i}_toggle;"),
            "end".to_string(),
        ];
        let src_reset = vec![format!("{i}_toggle <= 1'b0;")];
        out.push_str(&builtin_always_ff(&src, &src_reset, &src_body));
        out.push_str("\n\n");

        let dst_body = vec![
            format!("{i}_sync0 <= {i}_toggle;"),
            format!("{i}_sync1 <= {i}_sync0;"),
            format!("{i}_sync2 <= {i}_sync1;"),
        ];
        let dst_reset = ["sync0", "sync1", "sync2"]
            .iter()
            .map(|r| format!("{i}_{r} <= 1'b0;"))
            .collect::<Vec<_>>();
        out.push_str(&builtin_always_ff(&dst, &dst_reset, &dst_body));
        out.push_str("\n\n");

        out.push_str(&format!(
            "    assign {i}_pulse_out = {i}_sync1 ^ {i}_sync2;"
        ));
        out
    }

    // ═══ SyncFifo — tek saatli, sayaç tabanlı FIFO (ADR-0029) ═══════

    fn emit_sync_fifo(&mut self, i: &str, info: &BuiltinInst) -> String {
        let one_bit = Sig {
            width: 1,
            signed: false,
        };
        let w = info.data.decl_type();
        let aw = info.addr_width();
        let pw = aw + 1; // doluluk sayacı genişliği (DEPTH dahil)
        let depth = info.dim;
        let wr_en = self.builtin_input(info, "wr_en", one_bit);
        let rd_en = self.builtin_input(info, "rd_en", one_bit);
        let wr_data = self.builtin_input(info, "wr_data", info.data);
        let clk = info.src_clock.clone();
        let zero = zero_of(info.data);
        let idx_hi = aw - 1;

        let mut out = format!(
            "    // SyncFifo '{i}': single-clock FIFO on {}, depth {depth}\n",
            clk.name
        );
        out.push_str(&format!("    localparam int {i}_DEPTH = {depth};\n"));
        out.push_str(&format!("    {w} {i}_mem [{i}_DEPTH];\n"));
        out.push_str(&format!("    logic [{idx_hi}:0] {i}_wptr;\n"));
        out.push_str(&format!("    logic [{idx_hi}:0] {i}_rptr;\n"));
        out.push_str(&format!("    logic [{aw}:0] {i}_count;\n"));
        out.push_str(&format!("    logic {i}_full;\n"));
        out.push_str(&format!("    logic {i}_empty;\n"));
        out.push_str(&format!("    logic {i}_do_wr;\n"));
        out.push_str(&format!("    logic {i}_do_rd;\n"));
        out.push_str(&format!("    {w} {i}_rd_data;\n\n"));

        out.push_str(&format!("    assign {i}_do_wr = {wr_en} && !{i}_full;\n"));
        out.push_str(&format!(
            "    assign {i}_do_rd = {rd_en} && !{i}_empty;\n\n"
        ));

        // Tek alan: bellek + pointer'lar + doluluk sayacı. count tek
        // gerçek kaynak; full/empty ondan türetilir. Bellek resetlenmez.
        let body = vec![
            format!("if ({i}_do_wr) begin"),
            format!("    {i}_mem[{i}_wptr] <= {wr_data};"),
            format!("    {i}_wptr <= {i}_wptr + {aw}'d1;"),
            "end".to_string(),
            format!("if ({i}_do_rd) begin"),
            format!("    {i}_rd_data <= {i}_mem[{i}_rptr];"),
            format!("    {i}_rptr <= {i}_rptr + {aw}'d1;"),
            "end".to_string(),
            format!(
                "{i}_count <= {i}_count + ({i}_do_wr ? {pw}'d1 : {pw}'d0) - \
                 ({i}_do_rd ? {pw}'d1 : {pw}'d0);"
            ),
        ];
        let mut reset = vec![
            format!("{i}_wptr <= {aw}'d0;"),
            format!("{i}_rptr <= {aw}'d0;"),
            format!("{i}_count <= {pw}'d0;"),
        ];
        reset.push(format!("{i}_rd_data <= {zero};"));
        out.push_str(&builtin_always_ff(&clk, &reset, &body));
        out.push_str("\n\n");

        out.push_str(&format!(
            "    assign {i}_full = ({i}_count == {pw}'d{depth});\n"
        ));
        out.push_str(&format!("    assign {i}_empty = ({i}_count == {pw}'d0);"));
        out
    }

    // ═══ Ram — tek portlu senkron RAM (ADR-0029) ════════════════════

    fn emit_ram(&mut self, i: &str, info: &BuiltinInst) -> String {
        let one_bit = Sig {
            width: 1,
            signed: false,
        };
        let w = info.data.decl_type();
        let aw = info.addr_width();
        let depth = info.dim;
        let addr_sig = Sig {
            width: aw,
            signed: false,
        };
        let addr = self.builtin_input(info, "addr", addr_sig);
        let wr_en = self.builtin_input(info, "wr_en", one_bit);
        let wr_data = self.builtin_input(info, "wr_data", info.data);
        let clk = info.src_clock.clone();
        let zero = zero_of(info.data);

        let mut out = format!(
            "    // Ram '{i}': single-port synchronous RAM on {}, depth {depth} (read-first)\n",
            clk.name
        );
        out.push_str(&format!("    localparam int {i}_DEPTH = {depth};\n"));
        out.push_str(&format!("    {w} {i}_mem [{i}_DEPTH];\n"));
        out.push_str(&format!("    {w} {i}_rd_data;\n\n"));

        // Okuma-önce (read-first): aynı çevrimde yazılan adres okunursa
        // ESKİ değer döner. Bellek resetlenmez (bilinçli).
        let body = vec![
            format!("if ({wr_en}) begin"),
            format!("    {i}_mem[{addr}] <= {wr_data};"),
            "end".to_string(),
            format!("{i}_rd_data <= {i}_mem[{addr}];"),
        ];
        let reset = vec![format!("{i}_rd_data <= {zero};")];
        out.push_str(&builtin_always_ff(&clk, &reset, &body));
        out
    }

    // ═══ DualPortRam — aynı saatte iki bağımsız port (ADR-0029) ═════

    fn emit_dual_port_ram(&mut self, i: &str, info: &BuiltinInst) -> String {
        let one_bit = Sig {
            width: 1,
            signed: false,
        };
        let w = info.data.decl_type();
        let aw = info.addr_width();
        let depth = info.dim;
        let addr_sig = Sig {
            width: aw,
            signed: false,
        };
        let a_addr = self.builtin_input(info, "a_addr", addr_sig);
        let a_wr_en = self.builtin_input(info, "a_wr_en", one_bit);
        let a_wr_data = self.builtin_input(info, "a_wr_data", info.data);
        let b_addr = self.builtin_input(info, "b_addr", addr_sig);
        let b_wr_en = self.builtin_input(info, "b_wr_en", one_bit);
        let b_wr_data = self.builtin_input(info, "b_wr_data", info.data);
        let clk = info.src_clock.clone();
        let zero = zero_of(info.data);

        let mut out = format!(
            "    // DualPortRam '{i}': two independent ports on {}, depth {depth} \
             (same-address write-write: port B wins, W3006)\n",
            clk.name
        );
        out.push_str(&format!("    localparam int {i}_DEPTH = {depth};\n"));
        out.push_str(&format!("    {w} {i}_mem [{i}_DEPTH];\n"));
        out.push_str(&format!("    {w} {i}_a_rd_data;\n"));
        out.push_str(&format!("    {w} {i}_b_rd_data;\n\n"));

        // Tek always_ff (Verilator MULTIDRIVEN'dan kaçınmak için): A
        // portu önce, B portu sonra yazar — aynı adreste B kazanır.
        // Her iki okuma da okuma-önce (eski değer). Bellek resetlenmez.
        let body = vec![
            format!("if ({a_wr_en}) begin"),
            format!("    {i}_mem[{a_addr}] <= {a_wr_data};"),
            "end".to_string(),
            format!("{i}_a_rd_data <= {i}_mem[{a_addr}];"),
            format!("if ({b_wr_en}) begin"),
            format!("    {i}_mem[{b_addr}] <= {b_wr_data};"),
            "end".to_string(),
            format!("{i}_b_rd_data <= {i}_mem[{b_addr}];"),
        ];
        let reset = vec![
            format!("{i}_a_rd_data <= {zero};"),
            format!("{i}_b_rd_data <= {zero};"),
        ];
        out.push_str(&builtin_always_ff(&clk, &reset, &body));
        out
    }

    // ═══ Counter — enable/clear'lı sarmalı sayaç (ADR-0029) ═════════

    fn emit_counter(&mut self, i: &str, info: &BuiltinInst) -> String {
        let one_bit = Sig {
            width: 1,
            signed: false,
        };
        let width = info.dim;
        let idx_hi = width - 1;
        let enable = self.builtin_input(info, "enable", one_bit);
        let clear = self.builtin_input(info, "clear", one_bit);
        let clk = info.src_clock.clone();
        let ones = if width == 1 {
            "1'b1".to_string()
        } else {
            format!("{{{width}{{1'b1}}}}")
        };

        let mut out = format!(
            "    // Counter '{i}': {width}-bit wrap-around counter on {} (clear wins over enable)\n",
            clk.name
        );
        out.push_str(&format!("    logic [{idx_hi}:0] {i}_count;\n"));
        out.push_str(&format!("    logic {i}_overflow;\n\n"));

        // overflow tek çevrimlik darbedir: sayaç tüm birlerden sarar.
        let body = vec![
            format!("{i}_overflow <= 1'b0;"),
            format!("if ({clear}) begin"),
            format!("    {i}_count <= {width}'d0;"),
            format!("end else if ({enable}) begin"),
            format!("    {i}_count <= {i}_count + {width}'d1;"),
            format!("    if ({i}_count == {ones}) begin"),
            format!("        {i}_overflow <= 1'b1;"),
            "    end".to_string(),
            "end".to_string(),
        ];
        let reset = vec![
            format!("{i}_count <= {width}'d0;"),
            format!("{i}_overflow <= 1'b0;"),
        ];
        out.push_str(&builtin_always_ff(&clk, &reset, &body));
        out
    }

    // ═══ ShiftRegister — seri-paralel dönüşüm (ADR-0029) ════════════

    fn emit_shift_register(&mut self, i: &str, info: &BuiltinInst) -> String {
        let one_bit = Sig {
            width: 1,
            signed: false,
        };
        let len = info.dim;
        let w = info.data.width as u64;
        let tw = len * w; // taps genişliği
        let data_in = self.builtin_input(info, "data_in", info.data);
        let shift_en = self.builtin_input(info, "shift_en", one_bit);
        let clk = info.src_clock.clone();
        let dt = info.data.decl_type();

        let mut out = format!(
            "    // ShiftRegister '{i}': {len} stages of {w} bit(s) on {} \
             (serial in, parallel taps)\n",
            clk.name
        );
        out.push_str(&format!("    logic [{}:0] {i}_shift;\n", tw - 1));
        out.push_str(&format!("    logic [{}:0] {i}_taps;\n", tw - 1));
        out.push_str(&format!("    {dt} {i}_data_out;\n\n"));

        // Yeni öğe alttan girer; en eski öğe üst dilimde (data_out).
        // LEN >= 2 doğrulama garantisi: üst dilim hiç boş kalmaz.
        let body = vec![
            format!("if ({shift_en}) begin"),
            format!(
                "    {i}_shift <= {{{i}_shift[{}:0], {data_in}}};",
                tw - w - 1
            ),
            "end".to_string(),
        ];
        let reset = vec![format!("{i}_shift <= {tw}'d0;")];
        out.push_str(&builtin_always_ff(&clk, &reset, &body));
        out.push_str("\n\n");

        out.push_str(&format!("    assign {i}_taps = {i}_shift;\n"));
        out.push_str(&format!(
            "    assign {i}_data_out = {i}_shift[{}:{}];",
            tw - 1,
            tw - w
        ));
        out
    }

    // ═══ RoundRobinArbiter — dönen öncelik (ADR-0029) ═══════════════

    fn emit_round_robin_arbiter(&mut self, i: &str, info: &BuiltinInst) -> String {
        let n = info.dim;
        let idx_hi = n - 1;
        let req_sig = Sig {
            width: n as u32,
            signed: false,
        };
        let req = self.builtin_input(info, "req", req_sig);
        let clk = info.src_clock.clone();

        let mut out = format!(
            "    // RoundRobinArbiter '{i}': {n} requesters on {} \
             (rotating priority, lowest index first)\n",
            clk.name
        );
        for regn in ["req_v", "mask", "masked", "grant"] {
            out.push_str(&format!("    logic [{idx_hi}:0] {i}_{regn};\n"));
        }
        out.push('\n');
        out.push_str(&format!("    assign {i}_req_v = {req};\n"));
        out.push_str(&format!("    assign {i}_masked = {i}_req_v & {i}_mask;\n"));
        // En düşük set bit izolasyonu: x & (~x + 1). Maskeli istek varsa
        // önce o küme (pointer üstü), yoksa maskesiz küme kazanır.
        out.push_str(&format!(
            "    assign {i}_grant = ({i}_masked != {n}'d0)\n        \
             ? ({i}_masked & (~{i}_masked + {n}'d1))\n        \
             : ({i}_req_v & (~{i}_req_v + {n}'d1));\n\n"
        ));

        // Pointer maskesi: grant edilen bitin ÜSTÜ öncelikli kalır;
        // en üst bit grant edilince maske sıfırlanır (sarma).
        let body = vec![
            format!("if ({i}_grant != {n}'d0) begin"),
            format!("    {i}_mask <= ~(({i}_grant << 1) - {n}'d1);"),
            "end".to_string(),
        ];
        let reset = vec![format!("{i}_mask <= {n}'d0;")];
        out.push_str(&builtin_always_ff(&clk, &reset, &body));
        out
    }

    // ═══ PriorityArbiter — sabit öncelik, req[0] en yüksek ══════════

    fn emit_priority_arbiter(&mut self, i: &str, info: &BuiltinInst) -> String {
        let n = info.dim;
        let idx_hi = n - 1;
        let req_sig = Sig {
            width: n as u32,
            signed: false,
        };
        let req = self.builtin_input(info, "req", req_sig);

        let mut out = format!(
            "    // PriorityArbiter '{i}': {n} requesters, fixed priority (req[0] highest)\n"
        );
        out.push_str(&format!("    logic [{idx_hi}:0] {i}_req_v;\n"));
        out.push_str(&format!("    logic [{idx_hi}:0] {i}_grant;\n\n"));
        out.push_str(&format!("    assign {i}_req_v = {req};\n"));
        out.push_str(&format!(
            "    assign {i}_grant = {i}_req_v & (~{i}_req_v + {n}'d1);"
        ));
        out
    }

    // ═══ EdgeDetect — tek saatli kenar algılama (ADR-0029) ══════════

    fn emit_edge_detect(&mut self, i: &str, info: &BuiltinInst) -> String {
        let one_bit = Sig {
            width: 1,
            signed: false,
        };
        let signal = self.builtin_input(info, "signal", one_bit);
        let clk = info.src_clock.clone();

        let mut out = format!(
            "    // EdgeDetect '{i}': single-domain edge detector on {} \
             (NOT a synchronizer -- use PulseSync across domains)\n",
            clk.name
        );
        for regn in ["prev", "rising", "falling", "both"] {
            out.push_str(&format!("    logic {i}_{regn};\n"));
        }
        out.push('\n');

        let body = vec![format!("{i}_prev <= {signal};")];
        let reset = vec![format!("{i}_prev <= 1'b0;")];
        out.push_str(&builtin_always_ff(&clk, &reset, &body));
        out.push_str("\n\n");

        out.push_str(&format!("    assign {i}_rising = {signal} & ~{i}_prev;\n"));
        out.push_str(&format!("    assign {i}_falling = ~{signal} & {i}_prev;\n"));
        out.push_str(&format!("    assign {i}_both = {signal} ^ {i}_prev;"));
        out
    }

    // ═══ Formal kontratlar (yalnız SvaMode::Immediate) ══════════════

    /// ADR-0027/0029 kontratları. Gözlemci (gölge) register'lar yalnız bu
    /// modda üretilir; `volt build` çıktısı onları hiç görmez. Reset'li
    /// alanlarda sva_immediate ile aynı BMC init varsayımı eklenir.
    fn builtin_contracts(
        &mut self,
        module_name: &str,
        i: &str,
        info: &BuiltinInst,
        span: Span,
    ) -> String {
        let mut out = format!("    // formal contracts: '{i}' ({})\n", info.prim.name());

        // BMC başlangıcı: ilk döngüde reset varsayılır (sva_immediate
        // gerekçesi). Aynı koşul iki alanda da olsa yinelenmesi zararsız.
        //
        // Ortam varsayımı (E5001'in kökü): `multiclock on` altında reset
        // serbest bir girdidir ve tek alanın kenarında bir çevrim yüksek
        // kalabilir — KISMİ reset (ör. rbin sıfırlanır, wbin kalır) tüm
        // pointer kontratlarını geçersiz kılar. Gerçek donanımda reset iki
        // alan da görene dek tutulur; formal ortamda bunu "iz başından
        // sonra örneklenen hiçbir kenarda reset yok" varsayımıyla kuruyoruz.
        // Sync reset yalnız örneklenen kenarda etkili olduğundan bu kenar
        // bazlı assume yeterlidir ve `initial assume` ile ÇELİŞMEZ (o,
        // yalnız 0. zaman adımını bağlar).
        //
        // Kenar bazlı assume YALNIZ çift saatli (clk2fflogic) primitifler
        // içindir: tek saatli modelde her BMC adımı bir kenar örneklemesi
        // olduğundan aynı satır 0. adımda `initial assume` ile çelişir
        // (PREUNSAT) — ve tek alanda "kısmi reset" zaten imkânsızdır.
        let mut assumed: Vec<&'static str> = Vec::new();
        let mut edge_assumed: Vec<String> = Vec::new();
        for clock in [&info.src_clock, &info.dst_clock] {
            if !clock.info.reset.is_none() {
                let cond = clock.info.reset.condition();
                if !assumed.contains(&cond) {
                    assumed.push(cond);
                    out.push_str(&format!("    initial assume ({cond});\n"));
                }
                if !info.prim.has_dst_clock() {
                    continue;
                }
                let edge = edge_of(clock);
                let line = format!(
                    "    always @({edge} {}) assume (!({cond})); // formal: no mid-trace reset\n",
                    clock.name
                );
                if !edge_assumed.contains(&line) {
                    out.push_str(&line);
                    edge_assumed.push(line);
                }
            }
        }

        let push_prop = |emitter: &mut Emitter<'a>, name: String, keyword: &'static str| {
            emitter.sva_props.push(SvaProp {
                module_name: module_name.to_string(),
                name,
                keyword,
                span,
            });
        };

        match info.prim {
            BuiltinPrim::AsyncFifo => {
                let pw = info.dim.trailing_zeros() + 1;
                let depth = info.dim;
                // Doluluk hiç DEPTH'i aşmaz: ikili pointer farkı (mod
                // 2^pw) gerçek doluluk sayısıdır. `!(full && empty)`
                // bilinçli olarak ZAYIFLATILDI (ADR-0027): iki-flop
                // gecikmesi bayrakları geçici olarak örtüştürebilir.
                out.push_str(&contract_line(
                    &info.src_clock,
                    "assert",
                    &format!("({i}_wbin - {i}_rbin) <= {pw}'d{depth}"),
                    &format!("{i}_inv_0"),
                ));
                push_prop(self, format!("{i}_inv_0"), "invariant");
                out.push_str(&contract_line(
                    &info.src_clock,
                    "cover",
                    &format!("{i}_wr_full"),
                    &format!("{i}_cov_0"),
                ));
                push_prop(self, format!("{i}_cov_0"), "cover");
                out.push_str(&contract_line(
                    &info.dst_clock,
                    "cover",
                    &format!("{i}_rd_empty"),
                    &format!("{i}_cov_1"),
                ));
                push_prop(self, format!("{i}_cov_1"), "cover");
            }
            BuiltinPrim::HandshakeSync => {
                // Gözlemciler: 1 çevrim gecikmeli req ve data_reg kopyası.
                let w = info.data.decl_type();
                let src_edge = edge_of(&info.src_clock);
                out.push_str(&format!("    logic {i}_req_d;\n"));
                out.push_str(&format!("    {w} {i}_data_prev;\n"));
                out.push_str(&format!(
                    "    always @({src_edge} {}) begin\n        {i}_req_d <= {i}_req;\n        \
                     {i}_data_prev <= {i}_data_q;\n    end\n",
                    info.src_clock.name
                ));
                // req yüksek kaldığı sürece veri stabil.
                out.push_str(&contract_line(
                    &info.src_clock,
                    "assert",
                    &format!("!({i}_req && {i}_req_d) || ({i}_data_q == {i}_data_prev)"),
                    &format!("{i}_inv_0"),
                ));
                push_prop(self, format!("{i}_inv_0"), "invariant");
                out.push_str(&contract_line(
                    &info.dst_clock,
                    "cover",
                    &format!("{i}_valid"),
                    &format!("{i}_cov_0"),
                ));
                push_prop(self, format!("{i}_cov_0"), "cover");
            }
            BuiltinPrim::PulseSync => {
                // ZAYIFLATMA (ADR-0027): "pulse_out iki ardışık dst çevrimi
                // yüksek kalamaz" kontratı serbest saat oranları altında
                // KANITLANAMAZ — kaynak darbeleri dst periyodundan sık
                // gelirse ihlal gerçektir (tam W3005'in uyardığı durum) ve
                // hiçbir kaynak-alan aralık varsayımı dst saatinin keyfî
                // yavaşlığını dışlayamaz; dst-alan varsayımı ise özelliğin
                // kendisini varsaymak olurdu (döngüsel). Kontrat kanıtlanabilir
                // toggle bütünlüğüne indirgendi: toggle yalnız pulse_in'e
                // yanıt olarak değişir. Gözlemciler: 1 çevrim gecikmeli
                // toggle ve pulse_in (kaynak alan).
                let one_bit = Sig {
                    width: 1,
                    signed: false,
                };
                let pulse_in = self.builtin_input(info, "pulse_in", one_bit);
                let src_edge = edge_of(&info.src_clock);
                out.push_str(&format!("    logic {i}_toggle_d;\n"));
                out.push_str(&format!("    logic {i}_pin_d;\n"));
                out.push_str(&format!(
                    "    always @({src_edge} {}) begin\n        {i}_toggle_d <= {i}_toggle;\n        \
                     {i}_pin_d <= {pulse_in};\n    end\n",
                    info.src_clock.name
                ));
                out.push_str(&contract_line(
                    &info.src_clock,
                    "assert",
                    &format!("({i}_toggle == {i}_toggle_d) || {i}_pin_d"),
                    &format!("{i}_inv_0"),
                ));
                push_prop(self, format!("{i}_inv_0"), "invariant");
                out.push_str(&contract_line(
                    &info.dst_clock,
                    "cover",
                    &format!("{i}_pulse_out"),
                    &format!("{i}_cov_0"),
                ));
                push_prop(self, format!("{i}_cov_0"), "cover");
            }
            BuiltinPrim::SyncFifo => {
                let pw = info.addr_width() + 1;
                let depth = info.dim;
                // Tek saatte iki-flop gecikmesi yoktur: hem doluluk sınırı
                // hem bayrak ayrıklığı KANITLANABİLİR (AsyncFifo'daki
                // zayıflatmanın tersine — ADR-0029).
                out.push_str(&contract_line(
                    &info.src_clock,
                    "assert",
                    &format!("{i}_count <= {pw}'d{depth}"),
                    &format!("{i}_inv_0"),
                ));
                push_prop(self, format!("{i}_inv_0"), "invariant");
                out.push_str(&contract_line(
                    &info.src_clock,
                    "assert",
                    &format!("!({i}_full && {i}_empty)"),
                    &format!("{i}_inv_1"),
                ));
                push_prop(self, format!("{i}_inv_1"), "invariant");
                out.push_str(&contract_line(
                    &info.src_clock,
                    "cover",
                    &format!("{i}_full"),
                    &format!("{i}_cov_0"),
                ));
                push_prop(self, format!("{i}_cov_0"), "cover");
                out.push_str(&contract_line(
                    &info.src_clock,
                    "cover",
                    &format!("{i}_empty"),
                    &format!("{i}_cov_1"),
                ));
                push_prop(self, format!("{i}_cov_1"), "cover");
            }
            BuiltinPrim::Ram => {
                let aw = info.addr_width();
                let pw = aw + 1;
                let depth = info.dim;
                let addr_sig = Sig {
                    width: aw,
                    signed: false,
                };
                let addr = self.builtin_input(info, "addr", addr_sig);
                // DEPTH iki kuvveti olduğundan aw-bit adres tanım gereği
                // aralıktadır; kontrat bu yapısal garantiyi belgeler.
                out.push_str(&contract_line(
                    &info.src_clock,
                    "assert",
                    &format!("{addr} < {pw}'d{depth}"),
                    &format!("{i}_inv_0"),
                ));
                push_prop(self, format!("{i}_inv_0"), "invariant");
            }
            BuiltinPrim::DualPortRam => {
                let aw = info.addr_width();
                let pw = aw + 1;
                let depth = info.dim;
                let addr_sig = Sig {
                    width: aw,
                    signed: false,
                };
                let a_addr = self.builtin_input(info, "a_addr", addr_sig);
                let b_addr = self.builtin_input(info, "b_addr", addr_sig);
                out.push_str(&contract_line(
                    &info.src_clock,
                    "assert",
                    &format!("{a_addr} < {pw}'d{depth}"),
                    &format!("{i}_inv_0"),
                ));
                push_prop(self, format!("{i}_inv_0"), "invariant");
                out.push_str(&contract_line(
                    &info.src_clock,
                    "assert",
                    &format!("{b_addr} < {pw}'d{depth}"),
                    &format!("{i}_inv_1"),
                ));
                push_prop(self, format!("{i}_inv_1"), "invariant");
            }
            BuiltinPrim::Counter => {
                let width = info.dim;
                let pw = width + 1;
                let bound = 1u128 << width;
                out.push_str(&contract_line(
                    &info.src_clock,
                    "assert",
                    &format!("{i}_count < {pw}'d{bound}"),
                    &format!("{i}_inv_0"),
                ));
                push_prop(self, format!("{i}_inv_0"), "invariant");
                out.push_str(&contract_line(
                    &info.src_clock,
                    "cover",
                    &format!("{i}_overflow"),
                    &format!("{i}_cov_0"),
                ));
                push_prop(self, format!("{i}_cov_0"), "cover");
            }
            BuiltinPrim::ShiftRegister => {
                let one_bit = Sig {
                    width: 1,
                    signed: false,
                };
                let shift_en = self.builtin_input(info, "shift_en", one_bit);
                out.push_str(&contract_line(
                    &info.src_clock,
                    "cover",
                    &shift_en,
                    &format!("{i}_cov_0"),
                ));
                push_prop(self, format!("{i}_cov_0"), "cover");
            }
            BuiltinPrim::RoundRobinArbiter | BuiltinPrim::PriorityArbiter => {
                let n = info.dim;
                // popcount(grant) <= 1: en düşük set bit izolasyonu tanım
                // gereği bir-sıcak-ya-da-sıfırdır; x & (x-1) == 0 biçimi
                // $countones gerektirmez (her aracın desteklediği saf mantık).
                out.push_str(&contract_line(
                    &info.src_clock,
                    "assert",
                    &format!("({i}_grant & ({i}_grant - {n}'d1)) == {n}'d0"),
                    &format!("{i}_inv_0"),
                ));
                push_prop(self, format!("{i}_inv_0"), "invariant");
                out.push_str(&contract_line(
                    &info.src_clock,
                    "assert",
                    &format!("({i}_grant & {i}_req_v) == {i}_grant"),
                    &format!("{i}_inv_1"),
                ));
                push_prop(self, format!("{i}_inv_1"), "invariant");
                // Her istekçi grant alabilir (sınırlı erişilebilirlik):
                // istekçi başına bir cover.
                for k in 0..n {
                    out.push_str(&contract_line(
                        &info.src_clock,
                        "cover",
                        &format!("{i}_grant[{k}]"),
                        &format!("{i}_cov_{k}"),
                    ));
                    push_prop(self, format!("{i}_cov_{k}"), "cover");
                }
            }
            BuiltinPrim::EdgeDetect => {
                // both, tanım gereği rising|falling'dir; kontrat üretilen
                // mantığın tutarlılığını belgeler.
                out.push_str(&contract_line(
                    &info.src_clock,
                    "assert",
                    &format!("{i}_both == ({i}_rising | {i}_falling)"),
                    &format!("{i}_inv_0"),
                ));
                push_prop(self, format!("{i}_inv_0"), "invariant");
                out.push_str(&contract_line(
                    &info.src_clock,
                    "cover",
                    &format!("{i}_rising"),
                    &format!("{i}_cov_0"),
                ));
                push_prop(self, format!("{i}_cov_0"), "cover");
                out.push_str(&contract_line(
                    &info.src_clock,
                    "cover",
                    &format!("{i}_falling"),
                    &format!("{i}_cov_1"),
                ));
                push_prop(self, format!("{i}_cov_1"), "cover");
            }
        }

        // Formal başlangıç durumu: `multiclock on` + clk2fflogic altında
        // `initial assume (rst)` reset'i yalnız 0. zaman adımında sabitler;
        // o aralıkta örneklenmiş kenar görmeyen saat alanı HİÇ sıfırlanmaz
        // ve pointer/senkron register'ları keyfî değerle başlar (sahte
        // karşı örnek). Tüm iç durum register'ları reset değerlerinden
        // başlatılır — Yosys `read -formal` bunu init değerine çevirir.
        // Gözlemciler dahildir; blok bildirimlerden SONRA üretilir.
        out.push_str("    // formal init: start BMC from the reset state\n");
        out.push_str("    initial begin\n");
        for line in builtin_init_lines(i, info) {
            out.push_str(&format!("        {line}\n"));
        }
        out.push_str("    end\n");
        // Son satır sonu kaldırılır (chunk birleştirme boş satır ekler).
        while out.ends_with('\n') {
            out.pop();
        }
        out
    }
}

/// SvaMode::Immediate formal init atamaları: primitifin tüm iç durum
/// register'ları (bellek dizisi hariç — kontratlar ona bakmaz) ve
/// yalnız-formal gözlemci register'ları reset değerlerine çekilir.
fn builtin_init_lines(i: &str, info: &BuiltinInst) -> Vec<String> {
    let zero = zero_of(info.data);
    match info.prim {
        BuiltinPrim::AsyncFifo => {
            let pw = info.dim.trailing_zeros() + 1;
            let mut lines: Vec<String> = [
                "wbin", "wgray", "rbin", "rgray", "rgray_s0", "rgray_s1", "wgray_s0", "wgray_s1",
            ]
            .iter()
            .map(|r| format!("{i}_{r} = {pw}'d0;"))
            .collect();
            lines.push(format!("{i}_rd_data = {zero};"));
            lines
        }
        BuiltinPrim::HandshakeSync => {
            let mut lines: Vec<String> = [
                "req", "ack", "req_s0", "req_s1", "ack_s0", "ack_s1", "ack_d", "req_d",
            ]
            .iter()
            .map(|r| format!("{i}_{r} = 1'b0;"))
            .collect();
            lines.push(format!("{i}_data_q = {zero};"));
            lines.push(format!("{i}_data_out = {zero};"));
            lines.push(format!("{i}_data_prev = {zero};"));
            lines
        }
        BuiltinPrim::PulseSync => ["toggle", "sync0", "sync1", "sync2", "toggle_d", "pin_d"]
            .iter()
            .map(|r| format!("{i}_{r} = 1'b0;"))
            .collect(),
        BuiltinPrim::SyncFifo => {
            let aw = info.addr_width();
            let pw = aw + 1;
            vec![
                format!("{i}_wptr = {aw}'d0;"),
                format!("{i}_rptr = {aw}'d0;"),
                format!("{i}_count = {pw}'d0;"),
                format!("{i}_rd_data = {zero};"),
            ]
        }
        BuiltinPrim::Ram => vec![format!("{i}_rd_data = {zero};")],
        BuiltinPrim::DualPortRam => vec![
            format!("{i}_a_rd_data = {zero};"),
            format!("{i}_b_rd_data = {zero};"),
        ],
        BuiltinPrim::Counter => {
            let width = info.dim;
            vec![
                format!("{i}_count = {width}'d0;"),
                format!("{i}_overflow = 1'b0;"),
            ]
        }
        BuiltinPrim::ShiftRegister => {
            let tw = info.dim * info.data.width as u64;
            vec![format!("{i}_shift = {tw}'d0;")]
        }
        BuiltinPrim::RoundRobinArbiter => {
            let n = info.dim;
            vec![format!("{i}_mask = {n}'d0;")]
        }
        // Durumsuz: PriorityArbiter'ın register'ı yok.
        BuiltinPrim::PriorityArbiter => Vec::new(),
        BuiltinPrim::EdgeDetect => vec![format!("{i}_prev = 1'b0;")],
    }
}

/// `sva_immediate` kalıbında tek kontrat satırı: saat kenarında,
/// reset guard'ıyla, `// volt:<ad>` işaretli immediate assertion.
fn contract_line(clock: &ClockPort, verb: &str, expr: &str, name: &str) -> String {
    let edge = edge_of(clock);
    let stmt = if clock.info.reset.is_none() {
        format!("{verb} ({expr}); // volt:{name}")
    } else {
        format!(
            "if (!({})) {verb} ({expr}); // volt:{name}",
            clock.info.reset.condition()
        )
    };
    format!("    always @({edge} {})\n        {stmt}\n", clock.name)
}

fn edge_of(clock: &ClockPort) -> &'static str {
    match clock.info.edge {
        volt_ast::ClockEdge::Negedge => "negedge",
        _ => "posedge",
    }
}

/// Reset varyantlı always_ff (sync_always_ff'in çok satırlı gövdeye
/// izin veren eşleniği). `body` satırları 0 girintiyle gelir; taban
/// girinti burada eklenir.
fn builtin_always_ff(clock: &ClockPort, reset_lines: &[String], body: &[String]) -> String {
    let edge = edge_of(clock);
    let clk = &clock.name;
    let cfg = clock.info.reset;
    let mut out = String::new();
    if cfg.is_none() {
        out.push_str(&format!("    always_ff @({edge} {clk}) begin\n"));
        for line in body {
            out.push_str(&format!("        {line}\n"));
        }
        out.push_str("    end");
    } else {
        out.push_str(&format!(
            "    always_ff @({edge} {clk}{}) begin\n",
            cfg.async_sensitivity()
        ));
        out.push_str(&format!("        if ({}) begin\n", cfg.condition()));
        for line in reset_lines {
            out.push_str(&format!("            {line}\n"));
        }
        out.push_str("        end else begin\n");
        for line in body {
            out.push_str(&format!("            {line}\n"));
        }
        out.push_str("        end\n    end");
    }
    out
}
