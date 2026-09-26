//! Yönlü çizge döngü algoritmaları — tip çizgesi (ADR-0069, volt-syntax)
//! ve fonksiyon çağrı çizgesi (ADR-0081, volt-hir) ortak kullanır.
//!
//! Düğümler `0..n` indeksleridir; her düğümün ardılları **üye**lere
//! gruplanmıştır (tip çizgesinde alan/varyant, çağrı çizgesinde çağrı
//! yeri): `targets(v)[m]` = v'nin m. üyesinden çıkan kenarların hedefleri.
//! Tanılar üyeyi ("döngüyü kapatan alan/çağrı") gösterdiği için grup
//! korunur. İki algoritma da yinelemelidir; derin zincirde yığın taşmaz.

use std::collections::{HashMap, VecDeque};

/// Kuvvetli bağlı bileşenler (yinelemeli Tarjan). Dönüş: düğüm →
/// bileşen kimliği.
pub fn components<'a>(n: usize, targets: impl Fn(usize) -> &'a [Vec<usize>]) -> Vec<usize> {
    let succ: Vec<Vec<usize>> = (0..n)
        .map(|v| targets(v).iter().flatten().copied().collect())
        .collect();
    let mut idx = vec![usize::MAX; n];
    let mut low = vec![0; n];
    let mut on_stack = vec![false; n];
    let mut comp = vec![usize::MAX; n];
    let mut stack = Vec::new();
    let mut next = 0;
    let mut ncomp = 0;
    for root in 0..n {
        if idx[root] != usize::MAX {
            continue;
        }
        // (düğüm, sıradaki ardıl konumu)
        let mut work = vec![(root, 0usize)];
        idx[root] = next;
        low[root] = next;
        next += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&(v, k)) = work.last() {
            if let Some(&w) = succ[v].get(k) {
                if let Some(top) = work.last_mut() {
                    top.1 += 1;
                }
                if idx[w] == usize::MAX {
                    idx[w] = next;
                    low[w] = next;
                    next += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    work.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(idx[w]);
                }
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[v]);
            }
            if low[v] == idx[v] {
                while let Some(w) = stack.pop() {
                    on_stack[w] = false;
                    comp[w] = ncomp;
                    if w == v {
                        break;
                    }
                }
                ncomp += 1;
            }
        }
    }
    comp
}

/// `from`'dan `to`'ya, `comp` bileşeni içinde en kısa yol (düğüm ve üye
/// sırasıyla BFS): (düğüm, üye) adımları. `from == to` ise boş.
pub fn path_within<'a>(
    targets: impl Fn(usize) -> &'a [Vec<usize>],
    comp: &[usize],
    from: usize,
    to: usize,
) -> Vec<(usize, usize)> {
    if from == to {
        return Vec::new();
    }
    let mut prev: HashMap<usize, (usize, usize)> = HashMap::new();
    let mut queue = VecDeque::from([from]);
    while let Some(v) = queue.pop_front() {
        for (m, ts) in targets(v).iter().enumerate() {
            for &w in ts {
                if comp[w] != comp[from] || w == from || prev.contains_key(&w) {
                    continue;
                }
                prev.insert(w, (v, m));
                if w == to {
                    let mut path = Vec::new();
                    let mut cur = to;
                    while let Some(&(p, pm)) = prev.get(&cur) {
                        path.push((p, pm));
                        cur = p;
                    }
                    path.reverse();
                    return path;
                }
                queue.push_back(w);
            }
        }
    }
    Vec::new()
}

/// Bir döngüdeki düğümün, döngüye giren ilk kenarı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleEdge {
    /// Döngüdeki düğüm.
    pub node: usize,
    /// Döngüyü kapatan üye (alan / çağrı yeri).
    pub member: usize,
    /// O üyenin döngüdeki ilk hedefi.
    pub target: usize,
    /// Düğümün bileşenindeki düğüm sayısı (1: öz-döngü).
    pub size: usize,
}

/// Döngüdeki HER düğüm için (düğüm sırasıyla) döngüye giren ilk kenar.
/// Tek düğümlü bileşen yalnız öz-döngüsü varsa döngüdür.
pub fn cycle_edges<'a>(
    n: usize,
    targets: impl Fn(usize) -> &'a [Vec<usize>] + Copy,
    comp: &[usize],
) -> Vec<CycleEdge> {
    let mut comp_size: HashMap<usize, usize> = HashMap::new();
    for &c in comp {
        *comp_size.entry(c).or_default() += 1;
    }
    let mut out = Vec::new();
    for i in 0..n {
        let size = comp_size[&comp[i]];
        let in_cycle = |w: usize| comp[w] == comp[i] && (size > 1 || w == i);
        if let Some((member, target)) = targets(i)
            .iter()
            .enumerate()
            .find_map(|(m, ts)| ts.iter().copied().find(|&w| in_cycle(w)).map(|w| (m, w)))
        {
            out.push(CycleEdge {
                node: i,
                member,
                target,
                size,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(t: &[Vec<Vec<usize>>]) -> Vec<CycleEdge> {
        let comp = components(t.len(), |v| &t[v]);
        cycle_edges(t.len(), |v| &t[v], &comp)
    }

    #[test]
    fn acyclic_graph_has_no_cycle_edges() {
        let t = vec![vec![vec![1]], vec![vec![2]], vec![]];
        assert!(run(&t).is_empty());
    }

    #[test]
    fn self_loop_and_mutual_cycle_report_every_member() {
        // 0 → 0; 1 → 2 → 1; 3 → 1 (döngüye yalnız ulaşır)
        let t = vec![
            vec![vec![0]],
            vec![vec![], vec![2]],
            vec![vec![1]],
            vec![vec![1]],
        ];
        let nodes: Vec<(usize, usize)> = run(&t).iter().map(|e| (e.node, e.member)).collect();
        assert_eq!(nodes, vec![(0, 0), (1, 1), (2, 0)]);
    }

    #[test]
    fn path_within_returns_the_shortest_member_path() {
        // 0 → 1 → 2 → 0 ve kısayol 0 → 2
        let t = [vec![vec![1], vec![2]], vec![vec![2]], vec![vec![0]]];
        let comp = components(3, |v| &t[v]);
        assert_eq!(path_within(|v| &t[v], &comp, 0, 2), vec![(0, 1)]);
        assert_eq!(path_within(|v| &t[v], &comp, 1, 0), vec![(1, 0), (2, 0)]);
    }

    #[test]
    fn deep_chain_does_not_overflow_the_stack() {
        let n = 200_000;
        let t: Vec<Vec<Vec<usize>>> = (0..n)
            .map(|v| {
                if v + 1 < n {
                    vec![vec![v + 1]]
                } else {
                    vec![vec![0]]
                }
            })
            .collect();
        let comp = components(n, |v| &t[v]);
        assert!(comp.iter().all(|&c| c == comp[0]));
    }
}
