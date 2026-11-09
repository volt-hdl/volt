//! A light, line-based reading of Volt source: enough to find mutation
//! sites (modules, ports, `on` blocks, instances, calls) without the
//! compiler. The seeded-bug test must not ask the compiler where a
//! clock crossing is: it reads the text and the expected code comes from
//! the specification (ADR-0113).

/// The code part of a line: everything before a `//` that is not inside
/// a string literal.
pub fn code_part(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_str = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_str = !in_str,
            b'/' if !in_str && i + 1 < bytes.len() && bytes[i + 1] == b'/' => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

pub fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

pub fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(is_ident_char)
        && !s.chars().next().is_some_and(|c| c.is_ascii_digit())
}

fn brace_delta(code: &str) -> i32 {
    let mut d = 0;
    let mut in_str = false;
    for c in code.chars() {
        match c {
            '"' => in_str = !in_str,
            '{' if !in_str => d += 1,
            '}' if !in_str => d -= 1,
            _ => {}
        }
    }
    d
}

#[derive(Debug, Clone)]
pub struct Port {
    pub line: usize,
    pub dir: String,
    pub name: String,
    pub ty: String,
    pub annot: Option<String>,
}

impl Port {
    pub fn is_clock(&self) -> bool {
        self.ty == "clock"
    }
    pub fn is_reset(&self) -> bool {
        self.ty == "reset" || self.ty.starts_with("reset(") || self.ty.starts_with("reset ")
    }
}

#[derive(Debug, Clone)]
pub struct OnBlock {
    /// Line of `on <clock> {`.
    pub line: usize,
    /// Line of the closing brace (the same line for a one-line block).
    pub end: usize,
    pub clock: String,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub is_extern: bool,
    /// Line of the `module` keyword.
    pub start: usize,
    /// Line of the closing brace.
    pub end: usize,
    pub ports: Vec<Port>,
    pub on_blocks: Vec<OnBlock>,
}

impl Module {
    pub fn clocks(&self) -> Vec<&Port> {
        self.ports.iter().filter(|p| p.is_clock()).collect()
    }
    /// The first clock port of the module other than `clock`.
    pub fn other_clock(&self, clock: &str) -> Option<String> {
        self.clocks()
            .into_iter()
            .find(|p| p.name != clock)
            .map(|p| p.name.clone())
    }
    pub fn port(&self, name: &str) -> Option<&Port> {
        self.ports.iter().find(|p| p.name == name)
    }
    pub fn contains(&self, line: usize) -> bool {
        line > self.start && line < self.end
    }
}

pub struct Source {
    pub lines: Vec<String>,
    pub trailing_newline: bool,
}

impl Source {
    pub fn new(text: &str) -> Self {
        let text = text.replace("\r\n", "\n");
        let trailing_newline = text.ends_with('\n');
        let lines = text.lines().map(str::to_string).collect();
        Source {
            lines,
            trailing_newline,
        }
    }

    pub fn join(lines: &[String], trailing_newline: bool) -> String {
        let mut s = lines.join("\n");
        if trailing_newline {
            s.push('\n');
        }
        s
    }

    pub fn code(&self, line: usize) -> &str {
        code_part(&self.lines[line])
    }

    /// Replaces line `line` and returns the whole new text.
    pub fn with_line(&self, line: usize, new: &str) -> String {
        let mut lines = self.lines.clone();
        lines[line] = new.to_string();
        Source::join(&lines, self.trailing_newline)
    }

    /// Removes line `line`.
    pub fn without_line(&self, line: usize) -> String {
        let mut lines = self.lines.clone();
        lines.remove(line);
        Source::join(&lines, self.trailing_newline)
    }

    /// Inserts `new` after line `line`.
    pub fn with_inserted(&self, line: usize, new: &str) -> String {
        let mut lines = self.lines.clone();
        lines.insert(line + 1, new.to_string());
        Source::join(&lines, self.trailing_newline)
    }

    pub fn modules(&self) -> Vec<Module> {
        let mut out = Vec::new();
        let mut depth = 0i32;
        // The module being read and whether its body brace was seen.
        let mut current: Option<(Module, bool)> = None;
        for (i, raw) in self.lines.iter().enumerate() {
            let code = code_part(raw);
            let t = code.trim();
            if current.is_none() && depth == 0 {
                let words: Vec<&str> = t.split_whitespace().collect();
                if let Some(at) = words.iter().position(|w| *w == "module") {
                    let prefix_ok = words[..at].iter().all(|w| *w == "pub" || *w == "extern");
                    if prefix_ok && at + 1 < words.len() {
                        let name: String = words[at + 1]
                            .chars()
                            .take_while(|c| is_ident_char(*c))
                            .collect();
                        current = Some((
                            Module {
                                name,
                                is_extern: words[..at].contains(&"extern"),
                                start: i,
                                end: i,
                                ports: Vec::new(),
                                on_blocks: Vec::new(),
                            },
                            false,
                        ));
                    }
                }
            }
            let before = depth;
            depth += brace_delta(code);
            if let Some((m, opened)) = current.as_mut() {
                if before == 1 && i != m.start {
                    if let Some(p) = parse_port(i, t) {
                        m.ports.push(p);
                    }
                }
                if i != m.start {
                    if let Some(clock) = on_header(t) {
                        m.on_blocks.push(OnBlock {
                            line: i,
                            end: i,
                            clock,
                        });
                    }
                }
                if depth > 0 {
                    *opened = true;
                }
                if *opened && depth == 0 {
                    m.end = i;
                    out.push(current.take().unwrap().0);
                }
            }
        }
        // Close the `on` blocks: the end is where the depth returns.
        for m in &mut out {
            for b in &mut m.on_blocks {
                let mut d = 0i32;
                for (j, line) in self.lines.iter().enumerate().take(m.end + 1).skip(b.line) {
                    d += brace_delta(code_part(line));
                    if d == 0 {
                        b.end = j;
                        break;
                    }
                }
            }
        }
        out
    }

    pub fn module_at(&self, line: usize) -> Option<Module> {
        self.modules().into_iter().find(|m| m.contains(line))
    }

    /// Calls `name(` in code parts, with the byte offset of the name and
    /// the text of the argument list (without the outer parentheses).
    pub fn calls(&self, name: &str) -> Vec<Call> {
        let mut out = Vec::new();
        for (i, raw) in self.lines.iter().enumerate() {
            let code = code_part(raw);
            let mut from = 0;
            while let Some(pos) = code[from..].find(name) {
                let at = from + pos;
                from = at + name.len();
                let prev_ok = code[..at]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !is_ident_char(c));
                if !prev_ok || !code[at + name.len()..].starts_with('(') {
                    continue;
                }
                let open = at + name.len();
                if let Some(close) = matching_paren(code, open) {
                    out.push(Call {
                        line: i,
                        start: at,
                        end: close + 1,
                        args: split_top_level(&code[open + 1..close]),
                    });
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct Call {
    pub line: usize,
    /// Byte offset of the call name in the line.
    pub start: usize,
    /// Byte offset just after the closing parenthesis.
    pub end: usize,
    pub args: Vec<String>,
}

pub fn matching_paren(code: &str, open: usize) -> Option<usize> {
    let mut d = 0i32;
    let mut in_str = false;
    for (j, c) in code[open..].char_indices() {
        match c {
            '"' => in_str = !in_str,
            '(' if !in_str => d += 1,
            ')' if !in_str => {
                d -= 1;
                if d == 0 {
                    return Some(open + j);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn split_top_level(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut d = 0i32;
    let mut in_str = false;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '"' => in_str = !in_str,
            '(' | '[' | '{' | '<' if !in_str => d += 1,
            ')' | ']' | '}' | '>' if !in_str => d -= 1,
            ',' if !in_str && d == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn parse_port(line: usize, t: &str) -> Option<Port> {
    let (dir, rest) = t.split_once(char::is_whitespace)?;
    if !matches!(dir, "in" | "out" | "inout" | "opendrain") {
        return None;
    }
    let (name, ty) = rest.split_once(':')?;
    let name = name.trim().to_string();
    if !is_ident(&name) {
        return None;
    }
    let ty = ty.trim();
    let (ty, annot) = match ty.rfind('@') {
        Some(at) if is_ident(ty[at + 1..].trim()) => (
            ty[..at].trim().to_string(),
            Some(ty[at + 1..].trim().to_string()),
        ),
        _ => (ty.to_string(), None),
    };
    Some(Port {
        line,
        dir: dir.to_string(),
        name,
        ty,
        annot,
    })
}

fn on_header(t: &str) -> Option<String> {
    let rest = t.strip_prefix("on ")?;
    let name: String = rest
        .trim_start()
        .chars()
        .take_while(|c| is_ident_char(*c))
        .collect();
    let after = rest.trim_start()[name.len()..].trim_start();
    (is_ident(&name) && after.starts_with('{')).then_some(name)
}

/// Replaces `old` with `new` in `line` only where `old` is a whole
/// identifier; returns `None` when there is no such occurrence.
pub fn replace_ident(line: &str, old: &str, new: &str) -> Option<String> {
    let mut out = String::new();
    let mut from = 0;
    let mut hit = false;
    while let Some(pos) = line[from..].find(old) {
        let at = from + pos;
        let before_ok = line[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !is_ident_char(c));
        let after_ok = line[at + old.len()..]
            .chars()
            .next()
            .is_none_or(|c| !is_ident_char(c));
        out.push_str(&line[from..at]);
        if before_ok && after_ok {
            out.push_str(new);
            hit = true;
        } else {
            out.push_str(old);
        }
        from = at + old.len();
    }
    out.push_str(&line[from..]);
    hit.then_some(out)
}
