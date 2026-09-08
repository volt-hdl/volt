//! Yerleşik CDC primitiflerinin SV üretimi (ADR-0027).
//!
//! `try_emit_sync_bridge` deseninin genellemesi: örnekleme deyimi
//! satır içi SV'ye açılır (ayrı modül örneklenmez), tüm üretilen
//! isimler `<örnek>_` önekini taşır. Doğrulama ön geçişte yapılır
//! (`collect_builtin_insts`) ki `f.rd_data` alan erişimleri deyim
//! sırasından bağımsız çevrilebilsin. Immediate SVA modunda her
//! primitif kendi formal kontratlarını da üretir (gözlemci register'lar
//! YALNIZ bu modda eklenir — normal build lint-temiz kalır).

use std::collections::HashMap;

use volt_ast::builtin::{BuiltinPrim, DomainRole, PortKind};
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
    /// `T`'nin genişlik/işaret bilgisi (PulseSync: 1 bit).
    pub(crate) data: Sig,
    /// AsyncFifo DEPTH (diğer primitiflerde kullanılmaz).
    depth: u64,
    /// Yazma/kaynak alanının saat portu.
    src_clock: ClockPort,
    /// Okuma/hedef alanının saat portu.
    dst_clock: ClockPort,
    /// Giriş portu adı → bağlama ifadesi (None: `ad` kısayolu —
    /// port adıyla aynı isimli yerel sinyal).
    inputs: HashMap<&'static str, Option<Idx<Expr>>>,
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

    /// Tek örneğin doğrulaması: generic argümanlar (T genişliği, DEPTH
    /// iki kuvveti — E2025), saat bağlamaları (modülün saat portları)
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

        // T — ilk generic argüman (AsyncFifo/HandshakeSync).
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

        // DEPTH — ikinci generic argüman (yalnız AsyncFifo). HIR'sız
        // koşularda da iki kuvveti kısıtı korunur (E2025).
        let depth = if prim.const_arg_count() == 1 {
            match inst.generic_args.get(1) {
                Some(GenericArg::Const(e)) => match self.eval_const(*e) {
                    Some(n) if (2..=65_536).contains(&n) && (n & (n - 1)) == 0 => n as u64,
                    Some(n) => {
                        self.error(
                            ErrorCode::E2025,
                            lstr!(
                                en: "AsyncFifo DEPTH must be a power of two, got {n}";
                                tr: "AsyncFifo DEPTH iki kuvveti olmalı, {n} verildi"
                            ),
                            ast.exprs[*e].span,
                            &lstr!(
                                en: "use 2, 4, 8, 16, ... for gray-code pointers to work";
                                tr: "gray kod pointer'larının çalışması için 2, 4, 8, 16, ... kullanın"
                            ),
                        );
                        return None;
                    }
                    None => {
                        self.future(
                            span,
                            &lstr!(
                                en: "AsyncFifo with a non-literal DEPTH";
                                tr: "literal olmayan DEPTH'li AsyncFifo"
                            ),
                        );
                        return None;
                    }
                },
                _ => {
                    self.future(
                        span,
                        &lstr!(
                            en: "AsyncFifo without <T, DEPTH> generic arguments";
                            tr: "<T, DEPTH> generic argümanları olmayan AsyncFifo"
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

        Some(BuiltinInst {
            prim,
            data,
            depth,
            src_clock: src_clock.expect("yukarıda denetlendi"),
            dst_clock: dst_clock.expect("yukarıda denetlendi"),
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
        let aw = info.depth.trailing_zeros(); // adres genişliği
        let pw = aw + 1; // pointer genişliği (sarma biti dahil)
        let depth = info.depth;
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

    // ═══ Formal kontratlar (yalnız SvaMode::Immediate) ══════════════

    /// ADR-0027 kontratları. Gözlemci (gölge) register'lar yalnız bu
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
        let mut assumed: Vec<&'static str> = Vec::new();
        let mut edge_assumed: Vec<String> = Vec::new();
        for clock in [&info.src_clock, &info.dst_clock] {
            if !clock.info.reset.is_none() {
                let cond = clock.info.reset.condition();
                if !assumed.contains(&cond) {
                    assumed.push(cond);
                    out.push_str(&format!("    initial assume ({cond});\n"));
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
                let pw = info.depth.trailing_zeros() + 1;
                let depth = info.depth;
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
            let pw = info.depth.trailing_zeros() + 1;
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
