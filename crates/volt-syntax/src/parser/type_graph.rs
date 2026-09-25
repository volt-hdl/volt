//! Tip bildirimi çizgesi denetimleri (ADR-0069).
//!
//! Birimdeki her adlandırılmış tip — `struct`, `struct port`, `enum`
//! (tuple/struct varyant payload'ları ve temel tipi) ve `type` takma adı —
//! bir düğümdür; bir üyenin tipinde geçen her bildirilmiş ad bir kenardır.
//! Diziler (`[S; 4]`), demetler ve generic argümanlar da izlenir; generic
//! bir tipin argümanı ancak o parametre tipin üyelerinde geçiyorsa kenar
//! olur (`W<P>` → `P`, yalnız `W<T>` T'yi gerçekten taşıyorsa).
//!
//! Döngü = sonsuz genişlikli tip: donanımda anlamı yoktur ve bundle /
//! Handshake açılımını sonsuz yapar. Döngüdeki HER tip E4009 alır
//! (birincil etiket tip adında, ikincil etiket döngüyü kapatan üyede,
//! not olarak döngü yolu). Bu denetim tektir: bundle (ADR-0039) ve
//! Handshake (ADR-0050) açılımı buradaki `recursive_types` kümesini
//! kullanır, kendi döngü aramaları yoktur (ADR-0067'nin iki ayrı araması
//! bu modülde birleşti).
//!
//! Aynı geçit generic `struct port` bildirimini E0003 ile reddeder: bundle
//! düzleştirmesi tip parametresi ikame etmez (ADR-0041 yalnız modüllerde
//! const generic örnekler) ve eskiden port hatasız, açılmadan geçiyordu.

use std::collections::{HashMap, HashSet, VecDeque};

use volt_ast::{
    Arena, GenericArg, GenericParamKind, Idx, ItemKind, Name, TypeRef, TypeRefKind, VariantData,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::depth::{err_type_too_deep, MAX_DEPTH};
use super::Parser;

/// Tip bildiriminin türü (tanı metni için).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeclKind {
    Struct,
    StructPort,
    Enum,
    Alias,
}

/// Döngüyü taşıyabilen üye türü.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemberKind {
    Field,
    Variant,
    Target,
    Base,
}

/// Bir tip bildiriminin tip taşıyan üyesi.
struct Member {
    kind: MemberKind,
    /// Alan / varyant adı (takma ad hedefi ve temel tip için boş).
    name: String,
    span: Span,
    types: Vec<Idx<TypeRef>>,
}

/// Çizge düğümü: bir tip bildirimi.
struct Node {
    name: Name,
    kind: DeclKind,
    params: Vec<String>,
    members: Vec<Member>,
    /// Üye başına hedef düğüm indeksleri (bildirim sırasında).
    targets: Vec<Vec<usize>>,
}

/// Tip çizgesi: düğümler bildirim sırasında; aynı ad iki kez bildirilmişse
/// sonuncusu ilkinin yerini alır (E1xxx yinelenen ad tanısı çözümleyicide).
struct Graph {
    nodes: Vec<Node>,
}

impl Graph {
    fn build(
        types: &Arena<TypeRef>,
        items: impl Iterator<Item = (Name, DeclKind, Vec<String>, Vec<Member>)>,
    ) -> Self {
        let mut nodes: Vec<Node> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        for (name, kind, params, members) in items {
            let node = Node {
                name: name.clone(),
                kind,
                params,
                members,
                targets: Vec::new(),
            };
            match index.get(&name.text) {
                Some(&i) => nodes[i] = node,
                None => {
                    index.insert(name.text.clone(), nodes.len());
                    nodes.push(node);
                }
            }
        }
        // Generic tiplerin hangi parametreleri üyelerde geçiyor.
        let flows: HashMap<String, Vec<bool>> = nodes
            .iter()
            .filter(|n| !n.params.is_empty())
            .map(|n| {
                let used = n
                    .params
                    .iter()
                    .map(|p| {
                        n.members
                            .iter()
                            .flat_map(|m| &m.types)
                            .any(|&t| mentions(types, t, p))
                    })
                    .collect();
                (n.name.text.clone(), used)
            })
            .collect();
        for node in &mut nodes {
            node.targets = node
                .members
                .iter()
                .map(|m| {
                    let mut out = Vec::new();
                    for &t in &m.types {
                        member_targets(types, t, &index, &flows, &node.params, &mut out);
                    }
                    out
                })
                .collect();
        }
        Graph { nodes }
    }

    /// Kuvvetli bağlı bileşenler (yinelemeli Tarjan; derin zincirde yığın
    /// taşmaz). Dönüş: düğüm → bileşen kimliği.
    fn components(&self) -> Vec<usize> {
        let n = self.nodes.len();
        let succ: Vec<Vec<usize>> = self
            .nodes
            .iter()
            .map(|nd| nd.targets.iter().flatten().copied().collect())
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

    /// `from`'dan `to`'ya, `comp` bileşeni içinde en kısa yol (bildirim
    /// sırasıyla BFS): (düğüm, üye) adımları.
    fn path_within(&self, comp: &[usize], from: usize, to: usize) -> Vec<(usize, usize)> {
        if from == to {
            return Vec::new();
        }
        let mut prev: HashMap<usize, (usize, usize)> = HashMap::new();
        let mut queue = VecDeque::from([from]);
        while let Some(v) = queue.pop_front() {
            for (m, ts) in self.nodes[v].targets.iter().enumerate() {
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
}

/// `ty` içinde `name` adında tek segmentli bir yol geçiyor mu?
fn mentions(types: &Arena<TypeRef>, ty: Idx<TypeRef>, name: &str) -> bool {
    let mut stack = vec![ty];
    while let Some(t) = stack.pop() {
        match &types[t].kind {
            TypeRefKind::Array { elem, .. } => stack.push(*elem),
            TypeRefKind::Tuple(items) => stack.extend(items.iter().copied()),
            TypeRefKind::Path { path, args } => {
                if path.segments.len() == 1 && path.segments[0].text == name {
                    return true;
                }
                stack.extend(args.iter().filter_map(|a| match a {
                    GenericArg::Type(t) => Some(*t),
                    GenericArg::Const(_) => None,
                }));
            }
            _ => {}
        }
    }
    false
}

/// Bir üye tipinin ulaştığı bildirilmiş tipler (sırayla). Kendi generic
/// parametreleri kenar değildir; generic bir tipin argümanı yalnız o
/// parametre tipin üyelerinde geçiyorsa izlenir; bilinmeyen / yerleşik
/// tiplerin (`Handshake<P>`) argümanları her zaman izlenir.
fn member_targets(
    types: &Arena<TypeRef>,
    ty: Idx<TypeRef>,
    index: &HashMap<String, usize>,
    flows: &HashMap<String, Vec<bool>>,
    own_params: &[String],
    out: &mut Vec<usize>,
) {
    let mut stack = vec![ty];
    while let Some(t) = stack.pop() {
        match &types[t].kind {
            TypeRefKind::Array { elem, .. } => stack.push(*elem),
            TypeRefKind::Tuple(items) => stack.extend(items.iter().rev().copied()),
            TypeRefKind::Path { path, args } => {
                let single = (path.segments.len() == 1).then(|| path.segments[0].text.as_str());
                if single.is_some_and(|n| own_params.iter().any(|p| p == n)) {
                    continue;
                }
                if let Some(&i) = single.and_then(|n| index.get(n)) {
                    out.push(i);
                }
                let flow = single.and_then(|n| flows.get(n));
                for (k, a) in args.iter().enumerate().rev() {
                    if let GenericArg::Type(at) = a {
                        if flow.is_none_or(|f| f.get(k).copied().unwrap_or(false)) {
                            stack.push(*at);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Bildirimin tip taşıyan üyeleri.
fn struct_members(fields: &[volt_ast::StructField]) -> Vec<Member> {
    fields
        .iter()
        .map(|f| Member {
            kind: MemberKind::Field,
            name: f.name.text.clone(),
            span: f.span,
            types: vec![f.ty],
        })
        .collect()
}

fn generic_names(generics: &[volt_ast::GenericParam]) -> Vec<String> {
    generics
        .iter()
        .map(|g| match &g.kind {
            GenericParamKind::Type { name, .. } | GenericParamKind::Const { name, .. } => {
                name.text.clone()
            }
        })
        .collect()
}

/// Bir öğe tip bildirimi ise çizge girdisi: (ad, tür, generic adları, üyeler).
type Decl = (Name, DeclKind, Vec<String>, Vec<Member>);

fn decl_of(types: &Arena<TypeRef>, kind: &ItemKind) -> Option<Decl> {
    match kind {
        ItemKind::Struct(s) => {
            let k = if s.is_port {
                DeclKind::StructPort
            } else {
                DeclKind::Struct
            };
            Some((
                s.name.clone(),
                k,
                generic_names(&s.generics),
                struct_members(&s.fields),
            ))
        }
        ItemKind::Enum(e) => {
            let base = e.repr.map(|r| Member {
                kind: MemberKind::Base,
                name: String::new(),
                span: types[r].span,
                types: vec![r],
            });
            let variants = e.variants.iter().filter_map(|v| {
                let tys = match &v.data {
                    VariantData::Unit => return None,
                    VariantData::Tuple(ts) => ts.clone(),
                    VariantData::Struct(fs) => fs.iter().map(|f| f.ty).collect(),
                };
                Some(Member {
                    kind: MemberKind::Variant,
                    name: v.name.text.clone(),
                    span: v.span,
                    types: tys,
                })
            });
            let members = base.into_iter().chain(variants).collect();
            Some((
                e.name.clone(),
                DeclKind::Enum,
                generic_names(&e.generics),
                members,
            ))
        }
        ItemKind::TypeAlias(a) => {
            let target = Member {
                kind: MemberKind::Target,
                name: String::new(),
                span: types[a.target].span,
                types: vec![a.target],
            };
            Some((
                a.name.clone(),
                DeclKind::Alias,
                generic_names(&a.generics),
                vec![target],
            ))
        }
        _ => None,
    }
}

/// Döngü yolu notu bu boyuta kadar olan bileşenlerde hesaplanır (bileşen
/// başına BFS); daha büyük döngüde yalnız boyut yazılır — maliyet
/// O(düğüm × sınır), dev bir karşılıklı döngü karesel değildir.
const MAX_PATH_COMPONENT: usize = 64;

/// Döngü üzerindeki düğümler için E4009 tanıları (bildirim sırasında) ve
/// o düğümlerin listesi.
fn cycle_diagnostics(graph: &Graph) -> (Vec<Diagnostic>, Vec<usize>) {
    let comp = graph.components();
    let mut comp_size: HashMap<usize, usize> = HashMap::new();
    for &c in &comp {
        *comp_size.entry(c).or_default() += 1;
    }
    let mut diags = Vec::new();
    let mut cyclic = Vec::new();
    for (i, node) in graph.nodes.iter().enumerate() {
        let size = comp_size[&comp[i]];
        let in_cycle = |w: usize| comp[w] == comp[i] && (size > 1 || w == i);
        let Some((m, first)) = node
            .targets
            .iter()
            .enumerate()
            .find_map(|(m, ts)| ts.iter().copied().find(|&w| in_cycle(w)).map(|w| (m, w)))
        else {
            continue;
        };
        cyclic.push(i);
        let path = if size <= MAX_PATH_COMPONENT {
            let mut steps = vec![(i, m)];
            steps.extend(graph.path_within(&comp, first, i));
            cycle_path(graph, &steps)
        } else {
            lstr!(en: "{size} types"; tr: "{size} tip")
        };
        diags.push(err_recursive_type(graph, i, m, &path));
    }
    (diags, cyclic)
}

/// Açılmış tip derinliği (ADR-0080): bir tip, takma ad / alan / varyant
/// zinciri boyunca açıldığında [`MAX_DEPTH`]'ten derin olamaz — tip
/// denetimi, genişlik hesabı ve SV üretimi tipleri özyinelemeyle açar;
/// parser'ın ağaç sınırı tek `TypeRef`'i sınırlar, bildirimden bildirime
/// uzanan zinciri sınırlamaz. Döngüdeki kenarlar E4009 aldı, sayılmaz.
/// Tanı zincirin sınırı İLK aştığı bildirimde (tek tanı / zincir).
fn depth_diagnostics(types: &Arena<TypeRef>, graph: &Graph) -> Vec<Diagnostic> {
    let comp = graph.components();
    // Tarjan bileşenleri ters topolojik sırada numaralar: hedefler önce.
    let mut order: Vec<usize> = (0..graph.nodes.len()).collect();
    order.sort_by_key(|&i| comp[i]);
    let mut depth = vec![0u32; graph.nodes.len()];
    let mut diags = Vec::new();
    for i in order {
        let node = &graph.nodes[i];
        let mut d = 1u32;
        let mut inherited = false;
        for (m, ts) in node.members.iter().zip(&node.targets) {
            let own = m
                .types
                .iter()
                .map(|&t| type_height(types, t))
                .max()
                .unwrap_or(0);
            let below = ts
                .iter()
                .filter(|&&w| comp[w] != comp[i])
                .map(|&w| depth[w])
                .max()
                .unwrap_or(0);
            inherited |= below > MAX_DEPTH;
            d = d.max(own.saturating_add(below).saturating_add(1));
        }
        depth[i] = d;
        if d > MAX_DEPTH && !inherited {
            diags.push(err_type_too_deep(&node.name));
        }
    }
    diags
}

/// Tek `TypeRef` ağacının yüksekliği (yinelemeli).
fn type_height(types: &Arena<TypeRef>, ty: Idx<TypeRef>) -> u32 {
    let mut max = 0;
    let mut stack = vec![(ty, 1u32)];
    while let Some((t, h)) = stack.pop() {
        max = max.max(h);
        match &types[t].kind {
            TypeRefKind::Array { elem, .. } => stack.push((*elem, h + 1)),
            TypeRefKind::Tuple(items) => stack.extend(items.iter().map(|&i| (i, h + 1))),
            TypeRefKind::Path { args, .. } => stack.extend(args.iter().filter_map(|a| match a {
                GenericArg::Type(t) => Some((*t, h + 1)),
                GenericArg::Const(_) => None,
            })),
            _ => {}
        }
    }
    max
}

/// `cyclic` düğümlerine ulaşan her düğümün adı (ters kenarlarda BFS).
fn reaching_names(graph: &Graph, cyclic: Vec<usize>) -> HashSet<String> {
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); graph.nodes.len()];
    for (v, node) in graph.nodes.iter().enumerate() {
        for &w in node.targets.iter().flatten() {
            preds[w].push(v);
        }
    }
    let mut reach: HashSet<usize> = cyclic.iter().copied().collect();
    let mut queue: VecDeque<usize> = cyclic.into_iter().collect();
    while let Some(w) = queue.pop_front() {
        for &v in &preds[w] {
            if reach.insert(v) {
                queue.push_back(v);
            }
        }
    }
    reach
        .into_iter()
        .map(|i| graph.nodes[i].name.text.clone())
        .collect()
}

impl Parser<'_> {
    /// Birim sonu tip çizgesi denetimi (ADR-0069): özyineli tipler E4009,
    /// generic `struct port` E0003. Döngüye ULAŞAN tüm tip adları
    /// `recursive_types`'a yazılır — bundle ve Handshake açılımı onları
    /// açmaz (sonlu açılım, kaskad yok).
    pub(super) fn check_type_graph(&mut self) {
        let types = &self.ast.types;
        let decls = self
            .ast
            .items
            .iter()
            .filter_map(|&i| decl_of(types, &self.ast.items_arena[i].kind));
        let graph = Graph::build(types, decls);
        let (mut diags, cyclic) = cycle_diagnostics(&graph);
        // Parser bu dosyada ağacı zaten kestiyse (derin tek tip de burada
        // yeniden sayılırdı) ikinci E0018 yok: dosya başına bir (ADR-0080).
        if !self.depth_reported {
            diags.extend(depth_diagnostics(types, &graph));
        }
        self.recursive_types = reaching_names(&graph, cyclic);
        for node in &graph.nodes {
            if node.kind == DeclKind::StructPort && !node.params.is_empty() {
                diags.push(err_generic_struct_port(&node.name));
            }
        }
        self.diagnostics.extend(diags);
    }
}

/// Üyenin tanı metnindeki adı: (İngilizce, Türkçe yalın, Türkçe belirtme).
fn member_words(m: &Member) -> (String, String, String) {
    let n = &m.name;
    match m.kind {
        MemberKind::Field => (
            format!("field '{n}'"),
            format!("'{n}' alanı"),
            format!("'{n}' alanını"),
        ),
        MemberKind::Variant => (
            format!("variant '{n}'"),
            format!("'{n}' varyantı"),
            format!("'{n}' varyantını"),
        ),
        MemberKind::Target => (
            "the target type".into(),
            "hedef tip".into(),
            "hedef tipi".into(),
        ),
        MemberKind::Base => (
            "the base type".into(),
            "temel tip".into(),
            "temel tipi".into(),
        ),
    }
}

/// Döngü yolu: `A.b → B.a → A` (takma ad hedefi ve temel tip için ad yalın).
fn cycle_path(graph: &Graph, steps: &[(usize, usize)]) -> String {
    let mut s = String::new();
    for &(v, m) in steps {
        let node = &graph.nodes[v];
        let member = &node.members[m];
        s.push_str(&node.name.text);
        if !member.name.is_empty() {
            s.push('.');
            s.push_str(&member.name);
        }
        s.push_str(" → ");
    }
    if let Some(&(v, _)) = steps.first() {
        s.push_str(&graph.nodes[v].name.text);
    }
    s
}

/// E4009 — özyineli tip (ADR-0067 struct port, ADR-0069 tüm tipler).
fn err_recursive_type(graph: &Graph, i: usize, m: usize, path: &str) -> Diagnostic {
    let node = &graph.nodes[i];
    let member = &node.members[m];
    let n = node.name.text.as_str();
    let (kind_en, kind_tr) = match node.kind {
        DeclKind::Struct => ("struct", "struct'ı"),
        DeclKind::StructPort => ("struct port", "struct port'u"),
        DeclKind::Enum => ("enum", "enum'u"),
        DeclKind::Alias => ("type alias", "tip takma adı"),
    };
    let (m_en, m_tr, m_tr_acc) = member_words(member);
    let (what_en, what_tr) = match member.kind {
        MemberKind::Field => ("field", "alan"),
        MemberKind::Variant => ("variant", "varyant"),
        MemberKind::Target | MemberKind::Base => ("type", "tip"),
    };
    Diagnostic::error(
        ErrorCode::E4009,
        lstr!(
            en: "{kind_en} '{n}' contains itself ({m_en} leads back to '{n}')";
            tr: "'{n}' {kind_tr} kendini içeriyor ({m_tr} '{n}' tipine geri dönüyor)"
        ),
        LabeledSpan::primary(
            node.name.span,
            lstr!(en: "recursive type"; tr: "özyineli tip"),
        ),
        lstr!(
            en: "break the cycle: change {m_en} so that it does not lead back to '{n}'";
            tr: "döngüyü kırın: {m_tr_acc} '{n}' tipine geri dönmeyecek biçimde değiştirin"
        ),
    )
    .with_secondary(
        member.span,
        lstr!(en: "this {what_en} closes the cycle"; tr: "döngüyü bu {what_tr} kapatıyor"),
    )
    .with_note(
        NoteKind::Note,
        lstr!(en: "cycle: {path}"; tr: "döngü: {path}"),
    )
    .with_note(NoteKind::Note, recursive_note())
}

/// E4009 notu: donanım tipinin genişliği sonludur (ADR-0067, ADR-0069).
fn recursive_note() -> String {
    lstr!(
        en: "a hardware type has a fixed, finite bit width and a port group is flattened field by field at compile time, so no type can contain itself (ADR-0067, ADR-0069)";
        tr: "donanım tipinin bit genişliği sabit ve sonludur, port grubu derleme zamanında alan alan açılır; hiçbir tip kendini içeremez (ADR-0067, ADR-0069)"
    )
}

/// E0003 — generic `struct port` henüz desteklenmiyor (ADR-0069).
fn err_generic_struct_port(name: &Name) -> Diagnostic {
    let n = name.text.as_str();
    Diagnostic::error(
        ErrorCode::E0003,
        lstr!(
            en: "generic struct ports are not supported yet ('{n}')";
            tr: "generic struct port henüz desteklenmiyor ('{n}')"
        ),
        LabeledSpan::primary(
            name.span,
            lstr!(en: "declared with generic parameters"; tr: "generic parametreyle bildirilmiş"),
        ),
        lstr!(
            en: "declare a separate, non-generic struct port for each concrete type";
            tr: "her somut tip için ayrı, generic olmayan bir struct port bildirin"
        ),
    )
    .with_note(
        NoteKind::Note,
        lstr!(
            en: "a port group is flattened to plain ports at parse time and generic parameters are not substituted there; only modules take (const) generic arguments (ADR-0041, ADR-0069)";
            tr: "port grubu ayrıştırma sonunda düz portlara açılır ve orada generic parametre ikame edilmez; (const) generic argüman yalnız modüllerde var (ADR-0041, ADR-0069)"
        ),
    )
}
