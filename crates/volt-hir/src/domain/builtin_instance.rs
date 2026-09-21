//! Yerleşik CDC primitifleri (ADR-0027; K8'in yerleşik eşleniği) ve
//! kullanım kısıtı hatırlatmaları: PulseSync W3005, DualPortRam
//! (ADR-0029) ve AsyncDualPortRam (ADR-0049) W3006.

use std::collections::HashMap;

use volt_ast::InstanceDecl;
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::Span;

use super::{DomainId, Inferencer};
use crate::builtin::{BuiltinPrim, DomainRole, PortKind};
use crate::resolve::DefId;

impl Inferencer<'_> {
    /// Yerleşik CDC primitifi (ADR-0027, K8'in yerleşik eşleniği):
    /// yazma/kaynak domain'i `wr_clk`/`src_clk` bağlamasından,
    /// okuma/hedef domain'i `rd_clk`/`dst_clk` bağlamasından gelir.
    /// Port→domain haritası doldurulur ki `f.rd_data` gibi alan
    /// okumaları hedef alanda görünsün ve mevcut CDC denetimleri
    /// (K5-K7) doğal olarak çalışsın. Kaynak taraf girişleri kaynak
    /// alanda olmalı (check_compat). PulseSync her örneklemede W3005
    /// kullanım kısıtını hatırlatır.
    pub(super) fn check_builtin_instance(
        &mut self,
        inst: &InstanceDecl,
        inst_def: DefId,
        prim: BuiltinPrim,
    ) {
        // 1. Saat bağlamalarından src/dst domain'leri.
        let (src_dom, dst_dom) = self.builtin_clock_domains(inst, prim);

        // 2. Port→domain haritası (alan okumaları için).
        let mut port_domains: HashMap<String, DomainId> = HashMap::new();
        for port in prim.ports() {
            let dom = match port.role {
                DomainRole::Src => src_dom,
                DomainRole::Dst => dst_dom,
            };
            port_domains.insert(port.name.to_string(), dom);
        }

        // 3. Saat dışı giriş bağlamaları kendi tarafının alanında olmalı.
        for b in &inst.bindings {
            let Some(port) = prim.port(&b.port_name.text) else {
                self.propagate_binding(b);
                continue;
            };
            if port.kind == PortKind::Clock {
                continue;
            }
            let expected = port_domains
                .get(port.name)
                .copied()
                .unwrap_or(DomainId::Error);
            self.check_binding(expected, b);
        }

        self.instance_ports.insert(inst_def, port_domains);

        // 4. Kullanım kısıtı hatırlatmaları (W3005, W3006).
        self.warn_builtin_usage(inst, prim);
    }

    /// Yazma/kaynak ve okuma/hedef saat bağlamalarının alanları.
    fn builtin_clock_domains(
        &mut self,
        inst: &InstanceDecl,
        prim: BuiltinPrim,
    ) -> (DomainId, DomainId) {
        let mut src_dom = DomainId::Error;
        let mut dst_dom = DomainId::Error;
        for b in &inst.bindings {
            let Some(port) = prim.port(&b.port_name.text) else {
                continue;
            };
            if port.kind == PortKind::Clock {
                let dom = self.binding_domain(b);
                match port.role {
                    DomainRole::Src => src_dom = dom,
                    DomainRole::Dst => dst_dom = dom,
                }
            }
        }
        (src_dom, dst_dom)
    }

    /// Statik bilinemeyen kullanım kısıtları her örneklemede hatırlatılır.
    fn warn_builtin_usage(&mut self, inst: &InstanceDecl, prim: BuiltinPrim) {
        let span = inst.name.span;
        let diag = match prim {
            BuiltinPrim::DualPortRam => dual_port_ram_reminder(span),
            BuiltinPrim::AsyncDualPortRam => async_dual_port_ram_reminder(span),
            BuiltinPrim::PulseSync => pulse_sync_reminder(span),
            _ => return,
        };
        self.diagnostics.push(diag);
    }
}

/// DualPortRam kullanım kısıtı (ADR-0029 W3006): iki port aynı adrese
/// aynı çevrimde yazarsa B portu kazanır; adres çakışması statik olarak
/// bilinemediğinden her örneklemede hatırlatılır (W3005 kalıbı).
fn dual_port_ram_reminder(span: Span) -> Diagnostic {
    Diagnostic::warning(
        ErrorCode::W3006,
        lstr!(en: "DualPortRam write-write collisions resolve in favor of port B";
              tr: "DualPortRam yazma-yazma çakışmalarında B portu kazanır"),
        LabeledSpan::primary(
            span,
            lstr!(en: "simultaneous writes to the same address are not detected";
                  tr: "aynı adrese eş zamanlı yazma algılanmaz"),
        ),
        lstr!(en: "ensure the two ports never write the same address in the same cycle, \
                   or arbitrate writes before the RAM";
              tr: "iki portun aynı çevrimde aynı adrese yazmadığından emin olun ya da \
                   yazmaları RAM'den önce arbitre edin"),
    )
}

/// AsyncDualPortRam kullanım kısıtı (ADR-0049, W3006 çift saatli biçim):
/// diğer saatten yazılmakta olan adresin okunması TANIMSIZ değer döndürür
/// (yazıcı + okuyucu çakışması; DualPortRam'ın "B kazanır" kuralı burada
/// yoktur). Adres çakışması statik bilinemediğinden her örneklemede
/// hatırlatılır.
fn async_dual_port_ram_reminder(span: Span) -> Diagnostic {
    Diagnostic::warning(
        ErrorCode::W3006,
        lstr!(en: "AsyncDualPortRam read of an address being written from the other \
                   clock is undefined";
              tr: "AsyncDualPortRam'da diğer saatten yazılmakta olan adresin okunması \
                   tanımsız"),
        LabeledSpan::primary(
            span,
            lstr!(en: "a read that overlaps a write to the same address from the other \
                       clock domain returns an undefined value";
                  tr: "diğer saat alanından aynı adrese yazmayla çakışan okuma \
                       tanımsız değer döndürür"),
        ),
        lstr!(en: "keep the reader off addresses that are being written (e.g. ping-pong \
                   regions or a handshake before reading), or tolerate one stale sample";
              tr: "okuyucuyu yazılmakta olan adreslerden uzak tutun (ör. ping-pong \
                   bölgeler ya da okumadan önce el sıkışma) ya da tek bayat örneğe \
                   tahammül edin"),
    )
}

/// PulseSync kullanım kısıtı (ADR-0027 W3005): toggle protokolü sık
/// darbeleri yutar; saat oranı statik olarak bilinemediğinden her
/// örneklemede hatırlatılır.
fn pulse_sync_reminder(span: Span) -> Diagnostic {
    Diagnostic::warning(
        ErrorCode::W3005,
        lstr!(en: "PulseSync requires spacing between source pulses";
              tr: "PulseSync kaynak darbeleri arasında aralık gerektirir"),
        LabeledSpan::primary(
            span,
            lstr!(en: "toggle protocol drops closely spaced pulses";
                  tr: "toggle protokolü sık darbeleri düşürür"),
        ),
        lstr!(en: "guarantee at least 3 destination clock cycles between consecutive \
                   pulses, or use HandshakeSync/AsyncFifo";
              tr: "ardışık darbeler arasında en az 3 hedef saat çevrimi bırakın ya da \
                   HandshakeSync/AsyncFifo kullanın"),
    )
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{inferred, two_clock};

    #[test]
    fn pulse_sync_ports_follow_their_side_and_remind_w3005() {
        let r = inferred(&two_clock(
            "    in p : bool @Fast\n    out q : bool @Slow\n    \
             let ps = PulseSync { src_clk: fast_clk, pulse_in: p, dst_clk: slow_clk }\n    q = ps.pulse_out",
        ));
        assert_eq!(r.codes(), ["W3005"]);
    }

    #[test]
    fn source_side_input_from_the_destination_domain_is_e3001() {
        let r = inferred(&two_clock(
            "    in p : bool @Slow\n    let ps = PulseSync { src_clk: fast_clk, pulse_in: p, dst_clk: slow_clk }",
        ));
        assert_eq!(r.codes(), ["E3001", "W3005"]);
    }

    #[test]
    fn destination_side_output_read_in_the_source_domain_is_e3001() {
        let r = inferred(&two_clock(
            "    in p : bool @Fast\n    out q : bool @Fast\n    \
             let ps = PulseSync { src_clk: fast_clk, pulse_in: p, dst_clk: slow_clk }\n    q = ps.pulse_out",
        ));
        assert_eq!(r.codes(), ["W3005", "E3001"]);
    }

    #[test]
    fn dual_port_ram_reminds_w3006() {
        let r = inferred(
            "module M {\n    in clk : clock\n    in aa : bits<6>\n    in ad : u8\n    in aw : bool\n    \
             in ba : bits<6>\n    in bd : u8\n    in bw : bool\n    out aq : u8\n\n    \
             let m = DualPortRam<u8, 64> { clk: clk, a_addr: aa, a_wr_data: ad, a_wr_en: aw, \
             b_addr: ba, b_wr_data: bd, b_wr_en: bw }\n\n    aq = m.a_rd_data\n}\n",
        );
        assert_eq!(r.codes(), ["W3006"]);
        assert!(r.dom.diagnostics[0].message.starts_with("DualPortRam"));
    }
}
