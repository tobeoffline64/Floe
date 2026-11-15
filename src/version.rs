//! Comparing release tags and app versions ("v1.3.0", "1.3.0", "release-2.0-beta1", ...).

use std::cmp::Ordering;

/// Strip a leading "v"/"V" and anything before the first digit ("release-1.2" -> "1.2").
pub fn normalize(v: &str) -> &str {
    let v = v.trim();
    match v.find(|c: char| c.is_ascii_digit()) {
        Some(i) => &v[i..],
        None => v,
    }
}

/// Numeric parts plus an optional pre-release suffix: "1.3.0-beta.2" -> ([1,3,0], Some("beta.2")).
fn parse(v: &str) -> Option<(Vec<u64>, Option<String>)> {
    let v = normalize(v);
    // Build metadata ("+abc") never affects ordering.
    let v = v.split('+').next().unwrap_or(v);
    let end = v.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(v.len());
    let (nums, rest) = v.split_at(end);
    let parts: Vec<u64> = nums.split('.').filter(|p| !p.is_empty()).map(|p| p.parse().ok()).collect::<Option<_>>()?;
    if parts.is_empty() {
        return None;
    }
    let pre = rest.trim_start_matches(['-', '_', '~', '.']);
    Some((parts, (!pre.is_empty()).then(|| pre.to_ascii_lowercase())))
}

/// Compare two versions. `None` when either doesn't look like a version number.
/// Missing parts count as 0 ("1.2" == "1.2.0"); a pre-release sorts before its release.
pub fn compare(a: &str, b: &str) -> Option<Ordering> {
    let (an, ap) = parse(a)?;
    let (bn, bp) = parse(b)?;
    let len = an.len().max(bn.len());
    for i in 0..len {
        let (x, y) = (an.get(i).copied().unwrap_or(0), bn.get(i).copied().unwrap_or(0));
        match x.cmp(&y) {
            Ordering::Equal => {}
            o => return Some(o),
        }
    }
    Some(match (ap, bp) {
        (None, None) => Ordering::Equal,
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(x), Some(y)) => compare_pre(&x, &y),
    })
}

/// "beta.2" vs "beta.10": compare dot/dash separated pieces, numerically where both are numbers.
fn compare_pre(a: &str, b: &str) -> Ordering {
    let split = |s: &str| s.split(['.', '-']).map(str::to_string).collect::<Vec<_>>();
    let (xa, xb) = (split(a), split(b));
    for (p, q) in xa.iter().zip(xb.iter()) {
        let o = match (p.parse::<u64>(), q.parse::<u64>()) {
            (Ok(m), Ok(n)) => m.cmp(&n),
            _ => p.cmp(q),
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    xa.len().cmp(&xb.len())
}

/// Same version, ignoring a "v" prefix and trailing ".0"s.
pub fn same(a: &str, b: &str) -> bool {
    match compare(a, b) {
        Some(o) => o == Ordering::Equal,
        None => normalize(a).eq_ignore_ascii_case(normalize(b)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering::*;

    #[test]
    fn compares_versions() {
        assert_eq!(compare("v1.3.0", "1.3.0"), Some(Equal));
        assert_eq!(compare("1.2", "1.2.0"), Some(Equal));
        assert_eq!(compare("v1.10.0", "v1.9.9"), Some(Greater));
        assert_eq!(compare("1.3.0-beta", "1.3.0"), Some(Less));
        assert_eq!(compare("1.3.0-beta.2", "1.3.0-beta.10"), Some(Less));
        assert_eq!(compare("release-2.0", "v1.9"), Some(Greater));
        assert_eq!(compare("1.0+build5", "1.0"), Some(Equal));
        assert_eq!(compare("nightly", "1.0"), None);
        assert!(same("V2.0", "2.0.0"));
        assert!(same("nightly", "nightly"));
        assert!(!same("1.0", "1.1"));
    }
}
