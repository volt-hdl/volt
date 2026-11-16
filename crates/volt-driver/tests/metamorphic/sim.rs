//! Equivalent hardware by simulation with random stimulus (#254, ADR-0117
//! §3): the generated SV of a design and of its rewrite run in Verilator
//! with the same input sequence; the outputs printed after every input
//! change and every clock edge must be equal. No SV text is compared.
//!
//! The testbench instantiates every root module of the generated SV (one
//! that no other generated module instantiates), so a rewrite inside a
//! child is seen through its parent. Each step sets the data inputs,
//! prints the outputs, raises a random subset of the clocks, prints again
//! and lowers the clocks. Reset inputs change seldom, so that a design
//! spends long stretches both in and out of reset whatever its polarity.

use std::path::{Path, PathBuf};
use std::process::Command;

pub const STEPS: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Port {
    pub input: bool,
    pub width: usize,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvModule {
    pub name: String,
    pub ports: Vec<Port>,
    body: String,
}

/// Modules of generated SV: name, ports and body. `Err` names a port form
/// the testbench does not drive (inout, unpacked array, interface).
pub fn modules(sv: &str) -> Result<Vec<SvModule>, String> {
    let mut out = Vec::new();
    let mut rest = sv;
    while let Some(at) = find_module(rest) {
        let after = &rest[at + "module ".len()..];
        let name: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let header_end = after.find(");").ok_or("module header without `);`")?;
        let header = &after[..header_end];
        let end = after.find("endmodule").ok_or("module without endmodule")?;
        let mut ports = Vec::new();
        for line in header.lines() {
            let l = line
                .split("//")
                .next()
                .unwrap()
                .trim()
                .trim_end_matches(',');
            let Some((dir, decl)) = l.split_once(char::is_whitespace) else {
                continue;
            };
            let input = match dir {
                "input" => true,
                "output" => false,
                "inout" => return Err(format!("{name}: inout port `{l}`")),
                _ => continue,
            };
            let decl = decl.trim();
            let decl = decl
                .strip_prefix("logic")
                .or_else(|| decl.strip_prefix("wire"))
                .or_else(|| decl.strip_prefix("reg"))
                .unwrap_or(decl)
                .trim();
            let decl = decl.strip_prefix("signed").unwrap_or(decl).trim();
            let (width, pname) = if let Some(r) = decl.strip_prefix('[') {
                let (range, n) = r.split_once(']').ok_or(format!("{name}: `{l}`"))?;
                let (hi, lo) = range.split_once(':').ok_or(format!("{name}: `{l}`"))?;
                let hi: usize = hi.trim().parse().map_err(|_| format!("{name}: `{l}`"))?;
                let lo: usize = lo.trim().parse().map_err(|_| format!("{name}: `{l}`"))?;
                (hi.abs_diff(lo) + 1, n.trim())
            } else {
                (1, decl)
            };
            if !pname.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || pname.is_empty() {
                return Err(format!("{name}: port form `{l}`"));
            }
            ports.push(Port {
                input,
                width,
                name: pname.to_string(),
            });
        }
        out.push(SvModule {
            name,
            ports,
            body: after[header_end..end].to_string(),
        });
        rest = &after[end..];
    }
    Ok(out)
}

fn find_module(s: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = s[from..].find("module ") {
        let at = from + i;
        let line_start = at == 0 || s.as_bytes()[at - 1] == b'\n';
        if line_start {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

/// Modules that no other module of the set instantiates.
pub fn roots(mods: &[SvModule]) -> Vec<&SvModule> {
    mods.iter()
        .filter(|m| {
            !mods.iter().any(|o| {
                o.name != m.name
                    && o.body.lines().any(|l| {
                        let t = l.trim_start();
                        t.strip_prefix(m.name.as_str())
                            .is_some_and(|r| r.starts_with(' ') || r.starts_with('#'))
                    })
            })
        })
        .collect()
}

/// The names of clock and reset ports declared in the Volt source:
/// `in <name> : clock` (or a type alias of `clock`), `in <name> : reset...`.
pub fn clock_and_reset_names(volt: &str) -> (Vec<String>, Vec<String>) {
    let mut clock_types = vec!["clock".to_string()];
    for l in volt.lines() {
        let t = l.split("//").next().unwrap().trim();
        if let Some(r) = t.strip_prefix("type ") {
            if let Some((n, ty)) = r.split_once('=') {
                if ty.trim().trim_end_matches(';') == "clock" {
                    clock_types.push(n.trim().to_string());
                }
            }
        }
    }
    let (mut clocks, mut resets) = (Vec::new(), Vec::new());
    for l in volt.lines() {
        let t = l.split("//").next().unwrap().trim();
        let Some(r) = t.strip_prefix("in ") else {
            continue;
        };
        let Some((n, ty)) = r.split_once(':') else {
            continue;
        };
        let ty = ty.split('@').next().unwrap().trim();
        let n = n.trim().to_string();
        if clock_types.iter().any(|c| c == ty) {
            clocks.push(n);
        } else if ty.starts_with("reset") {
            resets.push(n);
        }
    }
    (clocks, resets)
}

/// xorshift64*: the same sequence on every platform.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: &str) -> Rng {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in seed.bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3);
        }
        Rng(h | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    /// `width` random bits as a bit vector (LSB first); a quarter of the
    /// values are 0, all ones or 1.
    fn bits(&mut self, width: usize) -> Vec<bool> {
        let pick = self.next() % 8;
        (0..width)
            .map(|i| match pick {
                0 => false,
                1 => true,
                2 => i == 0,
                _ => (self.next() >> 7) & 1 == 1,
            })
            .collect()
    }
}

/// What the testbench drives: one entry per root input, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Drive {
    Clock,
    Reset,
    Data,
}

pub struct Bench {
    pub tb: String,
    pub stim: String,
}

/// The testbench and its stimulus for `roots`.
pub fn bench(roots: &[&SvModule], clocks: &[String], resets: &[String], seed: &str) -> Bench {
    let mut decl = String::new();
    let mut inst = String::new();
    let mut fields: Vec<(String, usize, Drive)> = Vec::new(); // (signal, width, drive)
    let mut outs: Vec<String> = Vec::new();
    for (ri, m) in roots.iter().enumerate() {
        let mut conns = Vec::new();
        for p in &m.ports {
            let sig = format!("r{ri}__{}", p.name);
            let range = if p.width > 1 {
                format!("[{}:0] ", p.width - 1)
            } else {
                String::new()
            };
            decl.push_str(&format!("    logic {range}{sig};\n"));
            conns.push(format!(".{}({sig})", p.name));
            if p.input {
                let drive = if clocks.contains(&p.name) && p.width == 1 {
                    Drive::Clock
                } else if p.width == 1 && (resets.contains(&p.name) || p.name.starts_with("rst")) {
                    Drive::Reset
                } else {
                    Drive::Data
                };
                fields.push((sig, p.width, drive));
            } else {
                outs.push(sig);
            }
        }
        inst.push_str(&format!("    {} r{ri} ({});\n", m.name, conns.join(", ")));
    }
    // Stimulus word per step: data and reset inputs, then one enable bit
    // per clock.
    let data: Vec<&(String, usize, Drive)> =
        fields.iter().filter(|f| f.2 != Drive::Clock).collect();
    let clk: Vec<&(String, usize, Drive)> = fields.iter().filter(|f| f.2 == Drive::Clock).collect();
    let total = data.iter().map(|f| f.1).sum::<usize>() + clk.len();
    let total = total.max(1);
    let mut rng = Rng::new(seed);
    let mut reset_state: Vec<bool> = vec![true; data.len()];
    let mut stim = String::new();
    for _ in 0..STEPS {
        // MSB first: the order of the concatenation below.
        let mut bits: Vec<bool> = Vec::with_capacity(total);
        for (k, f) in data.iter().enumerate() {
            if f.2 == Drive::Reset {
                if rng.next().is_multiple_of(12) {
                    reset_state[k] = !reset_state[k];
                }
                bits.push(reset_state[k]);
            } else {
                let mut v = rng.bits(f.1);
                v.reverse();
                bits.extend(v);
            }
        }
        for _ in &clk {
            bits.push(!rng.next().is_multiple_of(3));
        }
        while bits.len() < total {
            bits.insert(0, false);
        }
        let pad = (4 - total % 4) % 4;
        let mut padded = vec![false; pad];
        padded.extend(bits);
        let hex: String = padded
            .chunks(4)
            .map(|c| {
                let v = c.iter().fold(0u8, |a, &b| (a << 1) | u8::from(b));
                char::from_digit(u32::from(v), 16).unwrap()
            })
            .collect();
        stim.push_str(&hex);
        stim.push('\n');
    }
    let mut word: Vec<String> = data.iter().map(|f| f.0.clone()).collect();
    if !clk.is_empty() {
        word.push("ce".into());
    }
    if word.is_empty() {
        word.push("unused".into());
    }
    let show = if outs.is_empty() {
        String::new()
    } else {
        format!(
            ", {}",
            outs.iter()
                .map(|o| o.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let fmt: String = outs.iter().map(|_| " %h").collect();
    let ce_bits = clk.len().max(1);
    let clk_up: String = clk
        .iter()
        .enumerate()
        .map(|(k, f)| format!("            {} = ce[{}];\n", f.0, clk.len() - 1 - k))
        .collect();
    let clk_down: String = clk
        .iter()
        .map(|f| format!("            {} = 1'b0;\n", f.0))
        .collect();
    let tb = format!(
        "module tb;\n{decl}    logic [{}:0] ce;\n    logic unused;\n    logic [{}:0] vec [0:{}];\n    integer i;\n{inst}    initial begin\n{clk_down0}        $readmemh(\"stim.hex\", vec);\n        for (i = 0; i < {STEPS}; i = i + 1) begin\n            {{{}}} = vec[i];\n            #1;\n            $display(\"A %0d{fmt}\", i{show});\n{clk_up}            #1;\n            $display(\"B %0d{fmt}\", i{show});\n{clk_down}            #1;\n        end\n        $finish;\n    end\nendmodule\n",
        ce_bits - 1,
        total - 1,
        STEPS - 1,
        word.join(", "),
        clk_down0 = clk_down.replace("            ", "        "),
    );
    Bench { tb, stim }
}

/// Builds and runs the testbench in `dir`; the printed output lines.
/// Verilator gets the flags `volt test` passes it (`-fno-dfg`, #148).
pub fn run(
    verilator: &Path,
    dir: &Path,
    bench: &Bench,
    svs: &[PathBuf],
) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).expect("sim dir");
    std::fs::write(dir.join("tb.sv"), &bench.tb).expect("tb.sv");
    std::fs::write(dir.join("stim.hex"), &bench.stim).expect("stim.hex");
    let out = Command::new(verilator)
        .current_dir(dir)
        .args([
            "--binary",
            "--timing",
            "-Wno-fatal",
            "-Wno-lint",
            "-Wno-style",
            "--top-module",
            "tb",
            "-Mdir",
            "obj",
            "-o",
            "vtb",
            "-j",
            "2",
            "-CFLAGS",
            "-O0",
        ])
        .args(volt_tools::VERILATOR_BUILD_FLAGS)
        .arg("tb.sv")
        .args(svs)
        .output()
        .map_err(|e| format!("verilator: {e}"))?;
    if !out.status.success() {
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        // `%Error-CODE: /tmp/.../x.sv:3:5: message`: the code and the
        // message, without the temporary path.
        let errs: Vec<String> = text
            .lines()
            .filter(|l| l.starts_with("%Error"))
            .take(3)
            .map(|l| {
                l.split(": ")
                    .filter(|part| !part.contains('/') && !part.contains('\\'))
                    .collect::<Vec<_>>()
                    .join(": ")
            })
            .collect();
        return Err(format!("verilator failed: {}", errs.join(" | ")));
    }
    let run = Command::new(dir.join("obj").join("vtb"))
        .current_dir(dir)
        .output()
        .map_err(|e| format!("vtb: {e}"))?;
    let stdout = String::from_utf8_lossy(&run.stdout);
    let lines: Vec<String> = stdout
        .lines()
        .filter(|l| l.starts_with("A ") || l.starts_with("B "))
        .map(str::to_string)
        .collect();
    if !run.status.success() || lines.len() != 2 * STEPS {
        return Err(format!(
            "simulation ended early (exit {:?}, {} of {} lines): {}",
            run.status.code(),
            lines.len(),
            2 * STEPS,
            String::from_utf8_lossy(&run.stderr)
                .lines()
                .take(3)
                .collect::<Vec<_>>()
                .join(" | ")
        ));
    }
    Ok(lines)
}

/// The output names of the testbench, for a readable difference.
pub fn output_names(roots: &[&SvModule]) -> Vec<String> {
    roots
        .iter()
        .flat_map(|m| {
            m.ports
                .iter()
                .filter(|p| !p.input)
                .map(move |p| format!("{}.{}", m.name, p.name))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SV: &str = "module Child (\n    input  logic        clk,\n    input  logic [7:0]  d,\n    output logic [7:0]  q\n);\nendmodule\nmodule M (\n    input  logic        clk,\n    input  logic signed [15:0] a,\n    // struct S o : d[8:1] x[0]\n    output logic [7:0]  o_d\n);\n    Child c (\n        .clk(clk)\n    );\nendmodule\n";

    #[test]
    fn modules_ports_and_roots() {
        let mods = modules(SV).unwrap();
        assert_eq!(mods.len(), 2);
        assert_eq!(
            mods[1].ports,
            [
                Port {
                    input: true,
                    width: 1,
                    name: "clk".into()
                },
                Port {
                    input: true,
                    width: 16,
                    name: "a".into()
                },
                Port {
                    input: false,
                    width: 8,
                    name: "o_d".into()
                },
            ]
        );
        let r: Vec<&str> = roots(&mods).iter().map(|m| m.name.as_str()).collect();
        assert_eq!(r, ["M"]);
        assert!(modules("module X (\n    inout logic a\n);\nendmodule\n").is_err());
        assert!(modules("module X (\n    input logic [7:0] a [0:3]\n);\nendmodule\n").is_err());
    }

    #[test]
    fn clocks_resets_and_aliases() {
        let volt = "type Clk = clock\nmodule M {\n    in  clk : Clk\n    in  c2 : clock @D\n    in  rst_n : reset(async, active_low)\n    in  d : u8\n}\n";
        let (c, r) = clock_and_reset_names(volt);
        assert_eq!(c, ["clk", "c2"]);
        assert_eq!(r, ["rst_n"]);
    }

    #[test]
    fn stimulus_is_deterministic_and_sized() {
        let mods = modules(SV).unwrap();
        let roots = roots(&mods);
        let a = bench(&roots, &["clk".into()], &[], "seed");
        let b = bench(&roots, &["clk".into()], &[], "seed");
        assert_eq!(a.stim, b.stim);
        let lines: Vec<&str> = a.stim.lines().collect();
        assert_eq!(lines.len(), STEPS);
        // 16 data bits + 1 clock enable = 17 bits = 5 hex digits.
        assert!(lines.iter().all(|l| l.len() == 5), "{lines:?}");
        assert!(a.tb.contains("{r0__a, ce} = vec[i];"), "{}", a.tb);
        assert!(a.tb.contains("r0__clk = ce[0];"), "{}", a.tb);
        assert_ne!(bench(&roots, &["clk".into()], &[], "other").stim, a.stim);
    }
}
