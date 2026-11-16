//! The known-failures list (#254, ADR-0117 §4): every failure of the
//! real-bug test or of the metamorphic test that an open issue reproduces,
//! with that issue's number. A failure that is not in the list fails the
//! run; an entry that no failure matches fails it too (remove the entry).
//! `scripts/ci/known_failures.py` fails when an entry's issue is closed,
//! or when the pull request closes it while the entry still stands (the
//! mechanism of the mutation exclusions, #283).

use std::collections::BTreeSet;
use std::path::Path;

pub const FILE: &str = "crates/volt-driver/tests/metamorphic/known_failures.json";

/// What failed: the check (`real-bug` or a rewrite id), the design (its
/// corpus id) and the relation (`expect`, `test`, `codes`, `safety`,
/// `location`, `hardware`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub check: String,
    pub design: String,
    pub relation: String,
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} [{}]", self.check, self.design, self.relation)
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub key: Key,
    pub issue: u64,
    pub reason: String,
}

pub const RELATIONS: &[&str] = &["expect", "test", "codes", "safety", "location", "hardware"];

/// Reads and validates the list: every entry has a known relation, an
/// issue number and a reason, and no key repeats.
pub fn load(repo: &Path) -> Vec<Entry> {
    let text = std::fs::read_to_string(repo.join(FILE)).unwrap_or_else(|e| panic!("{FILE}: {e}"));
    parse(&text).unwrap_or_else(|e| panic!("{FILE}: {e}"))
}

pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let json: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let arr = json["known_failures"]
        .as_array()
        .ok_or("no `known_failures` array")?;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (i, e) in arr.iter().enumerate() {
        let s = |k: &str| {
            e[k].as_str()
                .filter(|v| !v.trim().is_empty())
                .map(str::to_string)
                .ok_or(format!("entry {i}: `{k}` is missing or empty"))
        };
        let key = Key {
            check: s("check")?,
            design: s("design")?,
            relation: s("relation")?,
        };
        if !RELATIONS.contains(&key.relation.as_str()) {
            return Err(format!("entry {i}: unknown relation `{}`", key.relation));
        }
        let issue = e["issue"]
            .as_u64()
            .ok_or(format!("entry {i}: `issue` must be a number"))?;
        let reason = s("reason")?;
        if !seen.insert(key.clone()) {
            return Err(format!("entry {i}: `{key}` is listed twice"));
        }
        out.push(Entry { key, issue, reason });
    }
    Ok(out)
}

/// The verdict of a run against the list: failures that are not listed,
/// and entries whose check ran on their design and relation without
/// failing (stale). `evaluated` holds every key that was checked.
pub fn compare(
    entries: &[Entry],
    failures: &[(Key, String)],
    evaluated: &BTreeSet<Key>,
    checks: &[&str],
) -> (Vec<String>, Vec<String>) {
    let failed: BTreeSet<&Key> = failures.iter().map(|(k, _)| k).collect();
    let listed: BTreeSet<&Key> = entries.iter().map(|e| &e.key).collect();
    let unlisted = failures
        .iter()
        .filter(|(k, _)| !listed.contains(k))
        .map(|(k, why)| format!("{k}: {why}"))
        .collect();
    let stale = entries
        .iter()
        .filter(|e| checks.contains(&e.key.check.as_str()))
        .filter(|e| evaluated.contains(&e.key) && !failed.contains(&e.key))
        .map(|e| {
            format!(
                "{} (#{}) no longer fails: remove the entry from {FILE}",
                e.key, e.issue
            )
        })
        .collect();
    (unlisted, stale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: &str, d: &str, r: &str) -> Key {
        Key {
            check: c.into(),
            design: d.into(),
            relation: r.into(),
        }
    }

    #[test]
    fn parse_rejects_malformed_entries() {
        let ok = r#"{"known_failures": [{"check": "real-bug", "design": "a.volt", "relation": "test", "issue": 216, "reason": "r"}]}"#;
        assert_eq!(parse(ok).unwrap()[0].issue, 216);
        assert!(parse(&ok.replace("216", "\"#216\"")).is_err());
        assert!(parse(&ok.replace("\"test\"", "\"other\"")).is_err());
        assert!(parse(&ok.replace("\"r\"", "\"\"")).is_err());
        let twice = r#"{"known_failures": [
            {"check": "c", "design": "d", "relation": "codes", "issue": 1, "reason": "r"},
            {"check": "c", "design": "d", "relation": "codes", "issue": 2, "reason": "r"}]}"#;
        assert!(parse(twice).unwrap_err().contains("twice"));
    }

    #[test]
    fn unlisted_and_stale() {
        let entries = parse(
            r#"{"known_failures": [
            {"check": "c", "design": "d1", "relation": "hardware", "issue": 1, "reason": "r"},
            {"check": "c", "design": "d2", "relation": "hardware", "issue": 2, "reason": "r"},
            {"check": "c", "design": "d3", "relation": "hardware", "issue": 3, "reason": "r"}]}"#,
        )
        .unwrap();
        let failures = vec![
            (key("c", "d1", "hardware"), "x".to_string()),
            (key("c", "d4", "codes"), "y".to_string()),
        ];
        // d2 was checked and passed: stale. d3 was not checked (no
        // Verilator, or another tier): not stale.
        let evaluated: BTreeSet<Key> =
            [key("c", "d1", "hardware"), key("c", "d2", "hardware")].into();
        let (unlisted, stale) = compare(&entries, &failures, &evaluated, &["c"]);
        assert_eq!(unlisted, ["c d4 [codes]: y"]);
        assert_eq!(stale.len(), 1);
        assert!(stale[0].starts_with("c d2 [hardware] (#2)"), "{stale:?}");
        // Another test's checks are not judged here.
        let (_, stale) = compare(&entries, &failures, &evaluated, &["real-bug"]);
        assert!(stale.is_empty());
    }
}
