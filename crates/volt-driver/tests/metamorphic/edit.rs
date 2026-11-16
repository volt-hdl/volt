//! Text edits of a rewrite and the map from the rewritten text back to the
//! original (#254, ADR-0117): "for errors, the same primary location up to
//! the rewrite".

/// One replacement: original bytes `start..end` become `text`. An anchor
/// says that bytes `new.0..new.1` of `text` are a copy of original bytes
/// `orig.0..orig.1` (a pattern, a value, a name the rewrite moved).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub anchors: Vec<Anchor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    pub new: (usize, usize),
    pub orig: (usize, usize),
}

/// Builds the replacement text of an edit from pieces: `lit` adds text of
/// its own, `copy` adds original text and anchors it.
pub struct Builder<'a> {
    src: &'a str,
    pub text: String,
    pub anchors: Vec<Anchor>,
}

impl<'a> Builder<'a> {
    pub fn new(src: &'a str) -> Self {
        Builder {
            src,
            text: String::new(),
            anchors: Vec::new(),
        }
    }
    pub fn lit(&mut self, s: &str) -> &mut Self {
        self.text.push_str(s);
        self
    }
    pub fn copy(&mut self, from: usize, to: usize) -> &mut Self {
        let at = self.text.len();
        self.text.push_str(&self.src[from..to]);
        self.anchors.push(Anchor {
            new: (at, self.text.len()),
            orig: (from, to),
        });
        self
    }
    pub fn finish(self, start: usize, end: usize) -> Edit {
        Edit {
            start,
            end,
            text: self.text,
            anchors: self.anchors,
        }
    }
}

/// The rewritten text and its map back to the original.
pub struct Rewritten {
    pub text: String,
    /// `(edit, start of its text in the rewritten source)`, in order.
    placed: Vec<(Edit, usize)>,
}

/// Applies non-overlapping edits. Overlapping edits are a bug of the
/// rewrite: panic.
pub fn apply(src: &str, mut edits: Vec<Edit>) -> Rewritten {
    edits.sort_by_key(|e| e.start);
    let mut out = String::with_capacity(src.len() + 64);
    let mut placed = Vec::with_capacity(edits.len());
    let mut at = 0;
    for e in edits {
        assert!(e.start >= at, "overlapping edits at byte {}", e.start);
        out.push_str(&src[at..e.start]);
        let new_start = out.len();
        out.push_str(&e.text);
        at = e.end;
        placed.push((e, new_start));
    }
    out.push_str(&src[at..]);
    Rewritten { text: out, placed }
}

impl Rewritten {
    /// The original byte range a byte of the rewritten text comes from:
    /// a single byte outside the edits and inside an anchor, the whole
    /// replaced range inside an edit's own text.
    pub fn to_orig(&self, pos: usize) -> (usize, usize) {
        let mut shift: isize = 0;
        for (e, new_start) in &self.placed {
            let new_end = new_start + e.text.len();
            if pos < *new_start {
                break;
            }
            if pos < new_end || (pos == new_end && e.text.is_empty()) {
                let off = pos - new_start;
                for a in &e.anchors {
                    if off >= a.new.0 && off < a.new.1 {
                        let p = a.orig.0 + (off - a.new.0);
                        return (p, p);
                    }
                }
                return (e.start, e.end);
            }
            shift += e.text.len() as isize - (e.end - e.start) as isize;
        }
        let p = (pos as isize - shift) as usize;
        (p, p)
    }

    pub fn edits(&self) -> usize {
        self.placed.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_and_map_back() {
        let src = "o = S { a, b: 1 }\nq = a\n";
        // `a` (byte 8) becomes `a: a`, both copies anchored to it.
        let mut b = Builder::new(src);
        b.copy(8, 9).lit(": ").copy(8, 9);
        let r = apply(src, vec![b.finish(8, 9)]);
        assert_eq!(r.text, "o = S { a: a, b: 1 }\nq = a\n");
        assert_eq!(r.to_orig(8), (8, 8)); // the name
        assert_eq!(r.to_orig(11), (8, 8)); // the value
        assert_eq!(r.to_orig(9), (8, 9)); // ": " is the edit's own text
        assert_eq!(r.to_orig(0), (0, 0));
        // After the edit, positions shift back by the 3 added bytes.
        let q = r.text.find("q =").unwrap();
        assert_eq!(
            r.to_orig(q),
            (src.find("q =").unwrap(), src.find("q =").unwrap())
        );
    }

    #[test]
    #[should_panic(expected = "overlapping")]
    fn overlapping_edits_panic() {
        let e = |s, t| Edit {
            start: s,
            end: t,
            text: String::new(),
            anchors: vec![],
        };
        apply("abcdef", vec![e(0, 3), e(2, 4)]);
    }
}
