//! `volt test` — test gruplarını Verilator ile derler, koşturur ve
//! cargo biçimli raporu basar.

use std::path::Path;
use std::process::ExitCode;

use volt_diagnostics::lstr;
use volt_sv_emit::{sim as tbgen, uses_sim_contracts, SimPort, SvaMode};

use super::contracts::{covers_in, merge_covers, ContractIndex, CoverCount};
use super::report::{print_summary, print_test_lines};
use super::tb_output::{parse_tb_output, TestOutcome};
use super::test_build::{collect_groups, compile_unit, TestGroup, TestUnit};
use super::test_files::resolve_files;
use super::verilator::{require_verilator, run_simulation, verilate, VerilateJob};
use super::{create_sim_dir, modules_of, write_file};
use crate::Compiled;

/// `volt test` seçenekleri.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TestOptions<'a> {
    pub nocapture: bool,
    /// Kontrat izleyicileri (ADR-0064); `--no-contracts` kapatır.
    pub contracts: bool,
    pub target_dir: &'a Path,
}

pub(crate) fn test(filter: Option<&str>, opts: TestOptions<'_>) -> ExitCode {
    match test_inner(filter, opts) {
        Ok(code) | Err(code) => code,
    }
}

fn test_inner(filter: Option<&str>, opts: TestOptions<'_>) -> Result<ExitCode, ExitCode> {
    let (files, name_filter) = resolve_files(filter)?;

    // ── Derle (test dosyası + kardeşi) ve testleri doğrula ──
    let sva_mode = if opts.contracts {
        SvaMode::Simulation
    } else {
        SvaMode::None
    };
    let mut units = Vec::new();
    for file in &files {
        units.push(compile_unit(file, sva_mode)?);
    }

    // ── Testleri topla, süz, modüle göre grupla ──
    let (groups, total) = collect_groups(&units, name_filter)?;
    println!("running {total} tests");
    if total == 0 {
        println!("\ntest result: ok. 0 passed; 0 failed");
        return Ok(ExitCode::SUCCESS);
    }

    let verilator = require_verilator("volt test")?;

    // ── Grup başına verilate + koştur ──
    let mut outcomes: Vec<TestOutcome> = Vec::new();
    let mut covers: Vec<CoverCount> = Vec::new();
    for group in &groups {
        let unit = &units[group.unit];
        let (group_outcomes, group_covers) = run_group(&verilator, unit, group, opts)?;
        outcomes.extend(group_outcomes);
        merge_covers(&mut covers, group_covers);
    }
    Ok(print_summary(&outcomes, &covers))
}

/// Modülün portları, SV'si ve onu üreten derleme (kontrat kimlikleri
/// için) test dosyasından ya da kardeşten gelir.
fn dut_of<'u>(
    unit: &'u TestUnit,
    module: &str,
) -> Option<(Vec<SimPort>, Option<String>, &'u Compiled)> {
    [Some(&unit.compiled), unit.sibling.as_ref()]
        .into_iter()
        .flatten()
        .find_map(|c| {
            modules_of(&c.ast)
                .into_iter()
                .find(|m| m.name.text == module)
                .map(|m| (tbgen::collect_sim_ports(&c.ast, m), c.sv.clone(), c))
        })
}

/// Grubun SV, testbench ve (varsa) `.vlt` dosyalarını yazar; Verilator
/// girdilerini sırasıyla döndürür.
fn write_group_files(
    sim_dir: &Path,
    group: &TestGroup,
    ports: &[SimPort],
    (sv, externs): (&str, &[volt_hir::ExternSourceFile]),
    tb_name: &str,
) -> Result<Vec<String>, ExitCode> {
    let module = &group.module;
    create_sim_dir(sim_dir)?;
    let sv_name = format!("{module}.sv");
    write_file(&sim_dir.join(&sv_name), sv)?;
    // İzleyici yalnız SV'de DPI çağrısı varsa (kontratsız tasarımda
    // testbench ADR-0033/0058 çıktısıyla bayt bayt aynı kalır).
    let contracts = uses_sim_contracts(sv);
    let tb = tbgen::test_testbench_cpp_with(module, ports, &group.tests, contracts);
    write_file(&sim_dir.join(tb_name), &tb)?;
    // `load` hedefleri yalnız adlarıyla açılır (ADR-0058): .vlt
    // dosyası SV'den ÖNCE verilir.
    let mut inputs = Vec::new();
    if !group.load_targets.is_empty() {
        let vlt_name = format!("load_{module}.vlt");
        let vlt = tbgen::load_config_vlt(&group.load_targets);
        write_file(&sim_dir.join(&vlt_name), &vlt)?;
        inputs.push(vlt_name);
    }
    // Extern gövdeleri (ADR-0076) üretilen SV'den önce.
    inputs.extend(crate::extern_stage::stage_extern_sources(
        externs,
        sim_dir,
        &[sv_name.as_str(), tb_name],
    )?);
    inputs.push(sv_name);
    Ok(inputs)
}

/// Tek grubu derler ve koşturur; test satırlarını basar. Kontrat
/// ihlalleri ve cover sayıları kaynak konumuna eşlenmiş döner.
fn run_group(
    verilator: &Path,
    unit: &TestUnit,
    group: &TestGroup,
    opts: TestOptions<'_>,
) -> Result<(Vec<TestOutcome>, Vec<CoverCount>), ExitCode> {
    let module = &group.module;
    let Some((ports, Some(sv), compiled)) = dut_of(unit, module) else {
        eprintln!(
            "{}",
            lstr!(
                en: "error: internal: module '{module}' passed checks but has no generated SV";
                tr: "hata: içsel: '{module}' modülü denetimden geçti ama üretilmiş SV'si yok"
            )
        );
        return Err(ExitCode::from(3));
    };

    let stem = unit.file_label.trim_end_matches(".volt").to_string();
    let sim_dir = opts.target_dir.join("sim").join(&stem);
    let tb_name = format!("tb_{module}.cpp");
    let inputs = write_group_files(
        &sim_dir,
        group,
        &ports,
        (&sv, &compiled.extern_sources),
        &tb_name,
    )?;

    let job = VerilateJob {
        sim_dir: &sim_dir,
        inputs: &inputs,
        tb_file: &tb_name,
        module,
        trace: false,
        mdir: &format!("obj_{}", module.to_lowercase()),
    };
    let exe = verilate(verilator, &job)?;
    let output = run_simulation(&exe, &sim_dir)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if opts.nocapture {
        print!("{stdout}");
    }
    let index = ContractIndex::new(&compiled.sva_props, &compiled.map, module);
    let parsed: Vec<TestOutcome> = parse_tb_output(&stdout)
        .into_iter()
        .map(|mut o| {
            o.contract = o.contract.map(|v| index.resolve(v));
            if let Some(f) = o.failure.as_mut() {
                f.labels = group
                    .value_asserts
                    .iter()
                    .find(|(loc, _)| *loc == f.loc)
                    .map(|(_, l)| l.clone());
            }
            o
        })
        .collect();
    print_test_lines(&parsed);
    Ok((parsed, index.covers(&covers_in(&stdout))))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "\
module Echo {
    in  clk : clock
    in  d   : u4
    out q   : u4

    reg hold : u4 = 0

    on clk {
        hold <= d
    }

    q = hold
}

test \"echo\" {
    let dut = Echo { };
    dut.d = 5;
    step(1);
    assert_eq(dut.q, 5);
}
";

    #[test]
    fn dut_of_finds_ports_and_sv_of_the_tested_module() {
        // Arrange
        let dir = std::env::temp_dir().join(format!("volt-sim-cmd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dizini");
        let file = dir.join("echo_test.volt");
        std::fs::write(&file, SOURCE).expect("yaz");

        // Act
        let unit = compile_unit(&file, SvaMode::None).unwrap_or_else(|_| panic!("derlenmeli"));
        let (groups, total) =
            collect_groups(std::slice::from_ref(&unit), None).unwrap_or_else(|_| panic!("grup"));

        // Assert
        assert_eq!(total, 1);
        assert_eq!(groups[0].module, "Echo");
        let (ports, sv, _) = dut_of(&unit, "Echo").expect("DUT bulunmalı");
        let names: Vec<&str> = ports.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["clk", "d", "q"]);
        assert!(sv.is_some_and(|text| text.contains("module Echo")));
        assert!(dut_of(&unit, "Missing").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
