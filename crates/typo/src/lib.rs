//! Typo tolerance and fuzzy matching for AccelerateSearch.
//!
//! Implements:
//!
//! * Damerau-Levenshtein distance.
//! * Typo tolerance levels (`disabled`, `min`, `default`).
//! * Configurable minimum word size thresholds.
//! * Typo penalty scoring for ranking.
//! * Prefix matching for the last query token.

use std::cmp::min;

use serde::{Deserialize, Serialize};

use models::TypoToleranceSettings;

/// Typo tolerance level. The `Disabled` level rejects any typo; `Min` allows
/// at most 1 typo and only on words above a length threshold; `Default`
/// allows 1 or 2 typos with appropriate thresholds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TypoLevel {
    /// Typo tolerance is disabled.
    Disabled,
    /// Minimum tolerance (1 typo allowed on long words).
    Min,
    /// Default tolerance (1-2 typos depending on word length).
    Default,
}

/// Returns the Damerau-Levenshtein distance between `a` and `b`.
///
/// This implementation includes transpositions in addition to insertions,
/// deletions, and substitutions.
#[must_use]
pub fn damerau_levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    // 2D matrix stored as a flat vector for performance.
    let mut d = vec![0usize; (m + 1) * (n + 1)];
    let idx = |i: usize, j: usize| i * (n + 1) + j;
    for i in 0..=m {
        d[idx(i, 0)] = i;
    }
    for j in 0..=n {
        d[idx(0, j)] = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            d[idx(i, j)] = min(
                min(
                    d[idx(i - 1, j)] + 1, // deletion
                    d[idx(i, j - 1)] + 1, // insertion
                ),
                d[idx(i - 1, j - 1)] + cost, // substitution
            );
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[idx(i, j)] = min(d[idx(i, j)], d[idx(i - 2, j - 2)] + cost); // transposition
            }
        }
    }
    d[idx(m, n)]
}

/// Returns the maximum number of typos permitted for a word of `len` characters
/// at the given tolerance level.
#[must_use]
pub fn max_typos_for(len: usize, level: TypoLevel, settings: &TypoToleranceSettings) -> usize {
    if !settings.enabled {
        return 0;
    }
    match level {
        TypoLevel::Disabled => 0,
        TypoLevel::Min => {
            if len >= settings.min_word_size_for_one_typo {
                1
            } else {
                0
            }
        }
        TypoLevel::Default => {
            if len >= settings.min_word_size_for_two_typos {
                2
            } else if len >= settings.min_word_size_for_one_typo {
                1
            } else {
                0
            }
        }
    }
}

/// Returns the typo penalty (0..=1) for `typos` occurrences at distance `d`.
#[must_use]
pub fn typo_penalty(d: usize) -> f64 {
    if d == 0 { 0.0 } else { 0.5 / d as f64 }
}

/// True if `candidate` is a prefix of `query` (case-insensitive).
#[must_use]
pub fn is_prefix(query: &str, candidate: &str) -> bool {
    if candidate.len() > query.len() {
        return false;
    }
    query
        .chars()
        .zip(candidate.chars())
        .all(|(a, b)| a.eq_ignore_ascii_case(&b))
}

/// Iterator over candidate strings within `max_distance` Damerau-Levenshtein
/// edits of `term`, restricted to prefix-style candidates of the same length.
/// Used by [`crate::apply_typo`] (in the search crate) to inject typo
/// corrections for the last query token.
pub fn iter_prefix_candidates<'a>(
    term: &'a str,
    max_distance: usize,
) -> Box<dyn Iterator<Item = String> + 'a> {
    if max_distance == 0 {
        return Box::new(std::iter::empty());
    }
    let mut out: Vec<String> = Vec::new();
    let chars: Vec<char> = term.chars().collect();
    let len = chars.len();
    if len == 0 {
        return Box::new(out.into_iter());
    }
    for i in 0..len {
        for c in 'a'..='z' {
            if c == chars[i] {
                continue;
            }
            let mut alt = chars.clone();
            alt[i] = c;
            let candidate: String = alt.iter().collect();
            if damerau_levenshtein(term, &candidate) <= max_distance {
                out.push(candidate);
            }
        }
    }
    Box::new(out.into_iter())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damerau_levenshtein_handles_basic_cases() {
        assert_eq!(damerau_levenshtein("", ""), 0);
        assert_eq!(damerau_levenshtein("abc", "abc"), 0);
        assert_eq!(damerau_levenshtein("abc", "abd"), 1);
        assert_eq!(damerau_levenshtein("kitten", "sitting"), 3);
        assert_eq!(damerau_levenshtein("abcd", "acbd"), 1); // transposition
    }

    #[test]
    fn max_typos_for_respects_levels() {
        let s = TypoToleranceSettings::default();
        assert_eq!(max_typos_for(3, TypoLevel::Min, &s), 0);
        assert_eq!(max_typos_for(5, TypoLevel::Min, &s), 1);
        assert_eq!(max_typos_for(8, TypoLevel::Default, &s), 1);
        assert_eq!(max_typos_for(10, TypoLevel::Default, &s), 2);
        assert_eq!(max_typos_for(5, TypoLevel::Disabled, &s), 0);
    }

    #[test]
    fn typo_penalty_decreases_with_distance() {
        assert!(typo_penalty(0) < typo_penalty(1));
        assert!(typo_penalty(2) < typo_penalty(1));
    }

    #[test]
    fn is_prefix_works() {
        assert!(is_prefix("helloworld", "hello"));
        assert!(is_prefix("Hello", "hello"));
        assert!(!is_prefix("helloworld", "world"));
    }

    #[test]
    fn iter_prefix_candidates_produces_close_matches() {
        let cands: Vec<String> = iter_prefix_candidates("rust", 1).collect();
        assert!(!cands.is_empty(), "should produce at least one candidate");
        assert!(cands.iter().all(|c| c.len() == 4 && c != "rust"));
        // The candidates should differ from `rust` by exactly one character.
        assert!(cands.iter().all(|c| c != "rust" && c.chars().count() == 4));
    }
}
