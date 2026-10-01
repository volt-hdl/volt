//! `volt verify` görev planı (ADR-0097): hangi modül bir sby görevi olur,
//! görevde hangi özellik DENETLENİR, hangisi yalnız VARSAYILIR.
//!
//! Modül kendi görevinde doğrulanırken kendi `requires`/`assume`'u
//! varsayımdır, `invariant`/`ensures`/`assert`/`cover`'ı denetlenir. Modül
//! bir üst modülün örneğiyken `requires`/`assume`'u onu süren üst modülün
//! YÜKÜMLÜLÜĞÜDÜR: üst görevde `assert` olur (`VOLT_SUB_<modül>` makrosu,
//! `sva.rs`). Bu yüzden görev kümesi kendi kontratı olan modüllere ek
//! olarak, altında yükümlülüklü bir örnek bulunan her modülü içerir —
//! kendi kontratı olmayan üst modül de çocuğunun ön koşulunu kanıtlamak
//! zorundadır.
//!
//! Sayım dürüsttür: bir görevin "N properties" değeri yalnız orada
//! denetlenenlerdir. Hiçbir şey denetlemeyen modül görev olmaz; hiçbir
//! görev yoksa koşu başarı değildir (E5006).

use volt_sv_emit::SvaProp;

/// Tek sby görevinin planı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskPlan {
    /// Görevin tepe modülü (`prep -top`).
    pub(crate) module: String,
    /// Bu görevde denetlenen özelliklerin `sva_props` indisleri: önce
    /// modülün kendi iddiaları, sonra altındaki örneklerin yükümlülükleri
    /// (kaynak sırasında).
    pub(crate) checked: Vec<usize>,
    /// Modülün bu görevde varsayılan kendi `requires`/`assume`'ları.
    pub(crate) assumed: Vec<usize>,
    /// Görev-koşullu `read -define` makroları (yükümlülüklü alt modüller).
    pub(crate) sub_defines: Vec<String>,
    /// Görevde elaborasyona giren modüller: tepe + altındaki örnekler.
    /// Karşı örnek yalnız bunların kontratlarına eşlenir.
    pub(crate) scope: Vec<String>,
}

/// `requires`/`assume` (ADR-0097 yükümlülüğü) mü? Primitif kontratları
/// yükümlülük taşımaz (yalnız iddia ve cover üretirler).
pub(crate) fn is_obligation(prop: &SvaProp) -> bool {
    prop.primitive.is_none() && matches!(prop.keyword, "requires" | "assume")
}

/// Görev planı. `modules` SV modül adları kaynak sırasında, `children`
/// doğrudan örnek hedefleri (`volt_sv_emit::instance_children`).
pub(crate) fn plan_tasks(
    modules: &[String],
    children: &[(String, Vec<String>)],
    props: &[SvaProp],
) -> Vec<TaskPlan> {
    let mut plans = Vec::new();
    for module in modules {
        let below = volt_sv_emit::instance_subtree(module, children);
        let own = |i: &usize| props[*i].module_name == *module;
        let mut checked: Vec<usize> = (0..props.len())
            .filter(|i| own(i) && !is_obligation(&props[*i]))
            .collect();
        let assumed: Vec<usize> = (0..props.len())
            .filter(|i| own(i) && is_obligation(&props[*i]))
            .collect();
        checked.extend(
            (0..props.len())
                .filter(|i| is_obligation(&props[*i]) && below.contains(&props[*i].module_name)),
        );
        if checked.is_empty() {
            continue;
        }
        let mut sub_defines: Vec<String> = Vec::new();
        for m in modules.iter().filter(|m| below.contains(m)) {
            let has = props
                .iter()
                .any(|p| p.module_name == *m && is_obligation(p));
            if has {
                sub_defines.push(volt_sv_emit::sub_instance_macro(m));
            }
        }
        let mut scope = vec![module.clone()];
        scope.extend(below);
        plans.push(TaskPlan {
            module: module.clone(),
            checked,
            assumed,
            sub_defines,
            scope,
        });
    }
    plans
}

/// Hiçbir görevde denetlenmeyen yükümlülükler: birimin tepe modüllerinin
/// `requires`/`assume`'u — ortam hakkında varsayım (raporlanır, sayılmaz).
pub(crate) fn environment_assumptions(plans: &[TaskPlan], props: &[SvaProp]) -> Vec<usize> {
    (0..props.len())
        .filter(|i| is_obligation(&props[*i]))
        .filter(|i| !plans.iter().any(|p| p.checked.contains(i)))
        .collect()
}

/// Üretilen SV'de 1-tabanlı satırın ait olduğu modül: yukarı doğru ilk
/// `module <Ad>` başlığı (birleşik SV her modülü sütun 0'da açar).
pub(crate) fn owner_module_at(sv: &str, line_1based: usize) -> Option<String> {
    let lines: Vec<&str> = sv.lines().collect();
    if lines.is_empty() {
        return None;
    }
    let idx = line_1based.saturating_sub(1).min(lines.len() - 1);
    lines[..=idx].iter().rev().find_map(|l| {
        let rest = l.strip_prefix("module ")?;
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        (!name.is_empty()).then_some(name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prop(module: &str, name: &str, keyword: &'static str) -> SvaProp {
        SvaProp {
            module_name: module.to_string(),
            name: name.to_string(),
            keyword,
            span: volt_span::Span::new(volt_span::FileId(0), 0, 0),
            primitive: None,
            auto: None,
        }
    }

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn kids(v: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
        v.iter().map(|(m, c)| (m.to_string(), names(c))).collect()
    }

    #[test]
    fn parent_without_contracts_gets_a_task_for_its_childs_requires() {
        let props = [prop("Child", "req_0", "requires")];
        let plans = plan_tasks(
            &names(&["Child", "Parent"]),
            &kids(&[("Child", &[]), ("Parent", &["Child"])]),
            &props,
        );
        // Child yalnız varsayım taşır: denetleyecek bir şeyi yok, görev değil.
        assert_eq!(plans.len(), 1, "{plans:?}");
        assert_eq!(plans[0].module, "Parent");
        assert_eq!(plans[0].checked, [0]);
        assert!(plans[0].assumed.is_empty());
        assert_eq!(plans[0].sub_defines, ["VOLT_SUB_Child"]);
    }

    #[test]
    fn own_requires_are_assumed_and_own_assertions_checked() {
        let props = [
            prop("Child", "req_0", "requires"),
            prop("Child", "ens_0", "ensures"),
            prop("Child", "asm_0", "assume"),
        ];
        let plans = plan_tasks(&names(&["Child"]), &kids(&[("Child", &[])]), &props);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].checked, [1]);
        assert_eq!(plans[0].assumed, [0, 2]);
        assert!(plans[0].sub_defines.is_empty());
        assert_eq!(environment_assumptions(&plans, &props), [0, 2]);
    }

    #[test]
    fn three_levels_check_every_obligation_below_the_task_top() {
        let props = [
            prop("Leaf", "req_0", "requires"),
            prop("Mid", "req_0", "requires"),
            prop("Top", "inv_0", "invariant"),
        ];
        let plans = plan_tasks(
            &names(&["Leaf", "Mid", "Top"]),
            &kids(&[("Leaf", &[]), ("Mid", &["Leaf"]), ("Top", &["Mid"])]),
            &props,
        );
        let mods: Vec<&str> = plans.iter().map(|p| p.module.as_str()).collect();
        assert_eq!(mods, ["Mid", "Top"]);
        assert_eq!(plans[0].checked, [0]);
        assert_eq!(plans[0].assumed, [1]);
        assert_eq!(plans[0].sub_defines, ["VOLT_SUB_Leaf"]);
        assert_eq!(plans[1].checked, [2, 0, 1]);
        assert_eq!(plans[1].sub_defines, ["VOLT_SUB_Leaf", "VOLT_SUB_Mid"]);
        let mut scope = plans[1].scope.clone();
        scope.sort();
        assert_eq!(scope, ["Leaf", "Mid", "Top"]);
        assert!(environment_assumptions(&plans, &props).is_empty());
    }

    #[test]
    fn primitive_contracts_are_never_obligations() {
        let mut p = prop("Top", "f_inv_0", "assume");
        p.primitive = Some("AsyncFifo");
        assert!(!is_obligation(&p));
    }

    #[test]
    fn nothing_to_check_means_no_task() {
        let props = [prop("Top", "req_0", "requires")];
        let plans = plan_tasks(&names(&["Top"]), &kids(&[("Top", &[])]), &props);
        assert!(plans.is_empty());
        assert_eq!(environment_assumptions(&plans, &props), [0]);
    }

    #[test]
    fn owner_module_is_the_nearest_module_header_above_the_line() {
        let sv =
            "// gen\nmodule Child (\n    input clk\n);\n    assert (x); // volt:req_0\nendmodule\n\
                  \nmodule Parent (\n);\n    assert (y); // volt:inv_0\nendmodule\n";
        assert_eq!(owner_module_at(sv, 5).as_deref(), Some("Child"));
        assert_eq!(owner_module_at(sv, 10).as_deref(), Some("Parent"));
        assert_eq!(owner_module_at(sv, 1), None);
    }
}
