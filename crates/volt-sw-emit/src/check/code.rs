//! Yorumsuz kod karşılaştırması — görünümün yakalayamadığı el düzenlemesi.
//!
//! Görünüm sabitleri ve erişimci adlarını görür; bir getter gövdesindeki
//! kaydırma sayısını değiştirmek görünümü değiştirmez. Dosya ŞU ANKİ Volt
//! sürümünce üretilmişse, aynı haritadan yeniden üretilen metinle
//! belirteç belirteç karşılaştırılır: yorumlar ve boşluk atılır, yani
//! yorum/biçim farkı yine ayrışma değildir. JSON'da değerler (doc,
//! generator, source, regmap_hash hariç) karşılaştırılır.

use serde_json::Value;

use super::{Drift, DriftKind, Format};

/// İlk farkı `Code` kalemi olarak döndürür; fark yoksa `None`.
pub fn compare(format: Format, module: &str, file: &str, generated: &str) -> Option<Drift> {
    let (file_at, gen_at) = match format {
        Format::Json => json_difference(file, generated)?,
        Format::C => token_difference(file, generated, false)?,
        Format::Rust => token_difference(file, generated, true)?,
    };
    Some(Drift {
        subject: module.to_string(),
        kind: DriftKind::Code,
        file: Some(file_at),
        rtl: Some(gen_at),
    })
}

/// `//` ve `/* */` yorumlarını boşlukla değiştirir (satır sonları
/// korunur, satır numaraları kaymaz); `"..."` içi yorum sayılmaz.
/// `nested`: Rust blok yorumları iç içe geçer (`/* a /* b */ c */`), C'ninkiler
/// geçmez.
pub fn strip_comments(text: &str, nested: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                out.push(c);
                while let Some(s) = chars.next() {
                    out.push(s);
                    if s == '\\' {
                        if let Some(esc) = chars.next() {
                            out.push(esc);
                        }
                    } else if s == '"' || s == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => {
                for s in chars.by_ref() {
                    if s == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut depth = 1;
                let mut prev = ' ';
                for s in chars.by_ref() {
                    if s == '\n' {
                        out.push('\n');
                    }
                    if prev == '*' && s == '/' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                        prev = ' ';
                        continue;
                    }
                    if nested && prev == '/' && s == '*' {
                        depth += 1;
                        prev = ' ';
                        continue;
                    }
                    prev = s;
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    out
}

/// (1 tabanlı satır, belirteç) listesi: tanımlayıcı/sayı koşuları, dize
/// literal'leri ve tek noktalama karakterleri.
fn tokens(code: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut line = 1;
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            line += 1;
        } else if c.is_whitespace() {
        } else if c.is_ascii_alphanumeric() || c == '_' {
            let mut tok = c.to_string();
            while let Some(&n) = chars.peek() {
                if !(n.is_ascii_alphanumeric() || n == '_') {
                    break;
                }
                tok.push(n);
                chars.next();
            }
            out.push((line, tok));
        } else if c == '"' {
            let mut tok = c.to_string();
            for s in chars.by_ref() {
                tok.push(s);
                if s == '"' {
                    break;
                }
            }
            out.push((line, tok));
        } else {
            out.push((line, c.to_string()));
        }
    }
    out
}

fn token_difference(file: &str, generated: &str, nested: bool) -> Option<(String, String)> {
    let (fc, gc) = (
        strip_comments(file, nested),
        strip_comments(generated, nested),
    );
    let (ft, gt) = (tokens(&fc), tokens(&gc));
    let at = ft
        .iter()
        .zip(&gt)
        .position(|(a, b)| a.1 != b.1)
        .or_else(|| (ft.len() != gt.len()).then(|| ft.len().min(gt.len())))?;
    let show = |code: &str, toks: &[(usize, String)]| match toks.get(at) {
        Some((line, _)) => (
            *line,
            code.lines()
                .nth(line - 1)
                .unwrap_or_default()
                .trim()
                .to_string(),
        ),
        None => (code.lines().count(), "<end of file>".to_string()),
    };
    let (fl, ftext) = show(&fc, &ft);
    let (_, gtext) = show(&gc, &gt);
    Some((format!("line {fl}: `{ftext}`"), format!("`{gtext}`")))
}

/// Karşılaştırmadan çıkan anahtarlar: yorum ve kaynak kimliği.
const JSON_IGNORED: &[&str] = &["doc", "generator", "source", "regmap_hash"];

fn json_difference(file: &str, generated: &str) -> Option<(String, String)> {
    let a: Value = serde_json::from_str(file).ok()?;
    let b: Value = serde_json::from_str(generated).ok()?;
    let (path, x, y) = first_json_diff("$", &a, &b)?;
    Some((format!("{path} = {x}"), y))
}

fn first_json_diff(path: &str, a: &Value, b: &Value) -> Option<(String, String, String)> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys = x.keys().chain(y.keys().filter(|k| !x.contains_key(*k)));
            for k in keys {
                if JSON_IGNORED.contains(&k.as_str()) {
                    continue;
                }
                let p = format!("{path}.{k}");
                match (x.get(k), y.get(k)) {
                    (Some(va), Some(vb)) => {
                        if let Some(d) = first_json_diff(&p, va, vb) {
                            return Some(d);
                        }
                    }
                    (va, vb) => return Some((p, show(va), show(vb))),
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            for i in 0..x.len().max(y.len()) {
                let p = format!("{path}[{i}]");
                match (x.get(i), y.get(i)) {
                    (Some(va), Some(vb)) => {
                        if let Some(d) = first_json_diff(&p, va, vb) {
                            return Some(d);
                        }
                    }
                    (va, vb) => return Some((p, show(va), show(vb))),
                }
            }
            None
        }
        _ if a == b => None,
        _ => Some((path.to_string(), a.to_string(), b.to_string())),
    }
}

fn show(v: Option<&Value>) -> String {
    v.map_or_else(|| "<absent>".to_string(), Value::to_string)
}
