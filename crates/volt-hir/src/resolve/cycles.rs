//! E1006 — modül örnekleme döngüsü. Başlangıçlar ve döngü metni DefId
//! sırasına bağlıdır (determinizm): aynı döngü hep aynı tanıyı üretir.

use std::collections::{BTreeMap, HashSet};

use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};

use super::def::DefId;
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn check_instance_cycles(&mut self) {
        // BTreeMap: DFS başlangıçları DefId sırasıyla gezilir (determinizm).
        let mut edges: BTreeMap<DefId, Vec<DefId>> = BTreeMap::new();
        for &(from, to) in &self.instance_edges {
            edges.entry(from).or_default().push(to);
        }
        let starts: Vec<DefId> = edges.keys().copied().collect();
        let mut visited: HashSet<DefId> = HashSet::new();
        let mut reported: HashSet<DefId> = HashSet::new();
        for start in starts {
            let mut stack = vec![start];
            let mut path_set = HashSet::new();
            self.dfs_cycle(
                start,
                &edges,
                &mut visited,
                &mut path_set,
                &mut stack,
                &mut reported,
            );
        }
    }

    fn dfs_cycle(
        &mut self,
        node: DefId,
        edges: &BTreeMap<DefId, Vec<DefId>>,
        visited: &mut HashSet<DefId>,
        path_set: &mut HashSet<DefId>,
        stack: &mut Vec<DefId>,
        reported: &mut HashSet<DefId>,
    ) {
        if path_set.contains(&node) {
            // Yığının sonu `node`'un ikinci görünüşü; döngü ilkinden başlar.
            let first = stack.iter().position(|d| *d == node).unwrap_or(0);
            self.report_cycle(&stack[first..stack.len() - 1], reported);
            return;
        }
        if !visited.insert(node) {
            return;
        }
        path_set.insert(node);
        if let Some(next) = edges.get(&node) {
            for &n in next.clone().iter() {
                stack.push(n);
                self.dfs_cycle(n, edges, visited, path_set, stack, reported);
                stack.pop();
            }
        }
        path_set.remove(&node);
    }

    /// Döngü, girildiği yerden bağımsız olarak en küçük DefId'li modülden
    /// başlatılır: aynı döngü hep aynı metni üretir ve bir kez raporlanır.
    fn report_cycle(&mut self, members: &[DefId], reported: &mut HashSet<DefId>) {
        let pivot = (0..members.len()).min_by_key(|&i| members[i]).unwrap_or(0);
        let head = members[pivot];
        if !reported.insert(head) {
            return;
        }
        let cycle: Vec<String> = members[pivot..]
            .iter()
            .chain(&members[..pivot])
            .map(|&d| self.def(d).name.clone())
            .collect();
        let span = self.def(head).span;
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E1006,
                lstr!(en: "cyclic module dependency"; tr: "döngüsel modül bağımlılığı"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "the cycle starts at this module";
                          tr: "döngü bu modülden başlıyor"),
                ),
                lstr!(en: "remove one of the dependencies in the instantiation chain";
                      tr: "örnekleme zincirindeki bağımlılıklardan birini kaldırın"),
            )
            .with_note(
                NoteKind::Note,
                lstr!(en: "cycle: {} → {}", cycle.join(" → "), cycle[0];
                      tr: "döngü: {} → {}", cycle.join(" → "), cycle[0]),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::resolved;

    const RING: &str = "module A { in x : u8 out y : u8 let b = B { p: x } y = b.q }
         module B { in p : u8 out q : u8 let a = A { x: p } q = a.y }";

    #[test]
    fn a_cycle_is_reported_once_from_its_smallest_def_id() {
        let r = resolved(RING);
        let cycles: Vec<_> = r
            .diagnostics
            .iter()
            .filter(|d| d.code.as_str() == "E1006")
            .collect();
        assert_eq!(cycles.len(), 1);
        let (a, _) = r.def_by_name("A").expect("A");
        let head = cycles[0].primary_span().expect("birincil").span;
        assert_eq!(head, r.defs[a.0 as usize].span);
    }

    #[test]
    fn acyclic_instantiation_is_clean() {
        let r = resolved(
            "module Alt { in a : u8 out s : u8 s = a }
             module Top { in x : u8 out y : u8 let u = Alt { a: x } let v = Alt { a: x } y = u.s + v.s }",
        );
        assert!(!r.error_codes().contains(&"E1006"));
    }
}
