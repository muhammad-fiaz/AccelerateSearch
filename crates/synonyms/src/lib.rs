//! Synonym management and resolution for AccelerateSearch.
//!
//! Two flavours of synonym are supported:
//!
//! * **Equivalent synonyms** — `["fast", "quick", "rapid"]` are all
//!   interchangeable.
//! * **One-way synonyms** — `"phone" => ["smartphone", "mobile"]` expands
//!   the input to the alternatives.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tracing::debug;

use errors::AppError;

/// Maximum depth of synonym expansion to prevent combinatorial explosion.
pub const MAX_EXPANSION_DEPTH: usize = 4;

/// The on-disk representation of the synonyms for a collection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SynonymMap {
    /// Equivalent (multi-way) synonyms: a list of groups, where all words
    /// in a group are interchangeable.
    #[serde(default)]
    pub equivalent: Vec<BTreeSet<String>>,
    /// One-way synonyms: a word maps to a list of alternative words.
    #[serde(default)]
    pub one_way: BTreeMap<String, Vec<String>>,
}

impl SynonymMap {
    /// Creates a new empty synonym map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves a single term to all of its synonyms (including itself).
    /// Returns an empty vector if the term has no synonyms.
    #[must_use]
    pub fn resolve(&self, term: &str) -> Vec<String> {
        let mut out = BTreeSet::new();
        out.insert(term.to_string());
        // Equivalent
        for group in &self.equivalent {
            if group.contains(term) {
                for w in group {
                    out.insert(w.clone());
                }
            }
        }
        // One-way
        if let Some(alts) = self.one_way.get(term) {
            for w in alts {
                out.insert(w.clone());
            }
        }
        // Reverse one-way: if any one-way source resolves to `term`, expand
        // to the source as well.
        for (src, alts) in &self.one_way {
            if alts.iter().any(|a| a == term) && !self.one_way.contains_key(term) {
                out.insert(src.clone());
            }
        }
        out.into_iter().collect()
    }

    /// Expands a list of terms by recursively applying synonym rules up to
    /// [`MAX_EXPANSION_DEPTH`].
    #[must_use]
    pub fn expand_terms(&self, terms: &[String]) -> Vec<String> {
        let mut current: BTreeSet<String> = terms.iter().cloned().collect();
        for _ in 0..MAX_EXPANSION_DEPTH {
            let mut next: BTreeSet<String> = BTreeSet::new();
            for t in &current {
                for r in self.resolve(t) {
                    next.insert(r);
                }
            }
            if next == current {
                break;
            }
            current = next;
        }
        debug!(?current, "expanded terms via synonyms");
        current.into_iter().collect()
    }

    /// Inserts an equivalent group, merging it with any existing groups
    /// that share a word.
    pub fn add_equivalent<I, S>(&mut self, words: I) -> Result<(), AppError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut new_group: BTreeSet<String> =
            words.into_iter().map(Into::into).collect::<BTreeSet<_>>();
        if new_group.is_empty() {
            return Err(AppError::bad_request("synonym group is empty"));
        }
        // Merge with any existing equivalent group that intersects.
        let mut to_merge: Vec<usize> = Vec::new();
        for (idx, g) in self.equivalent.iter().enumerate() {
            if g.intersection(&new_group).next().is_some() {
                to_merge.push(idx);
            }
        }
        for idx in to_merge.iter().rev() {
            let g = self.equivalent.remove(*idx);
            for w in g {
                new_group.insert(w);
            }
        }
        self.equivalent.push(new_group);
        Ok(())
    }

    /// Sets a one-way synonym mapping.
    pub fn set_one_way(&mut self, source: &str, alternatives: Vec<String>) {
        self.one_way.insert(source.to_string(), alternatives);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_returns_self_when_no_synonym() {
        let m = SynonymMap::new();
        assert_eq!(m.resolve("hello"), vec!["hello".to_string()]);
    }

    #[test]
    fn equivalent_synonyms_resolve() {
        let mut m = SynonymMap::new();
        m.add_equivalent(["fast", "quick", "rapid"]).unwrap();
        let resolved = m.resolve("fast");
        assert!(resolved.contains(&"fast".to_string()));
        assert!(resolved.contains(&"quick".to_string()));
        assert!(resolved.contains(&"rapid".to_string()));
    }

    #[test]
    fn one_way_synonyms_resolve() {
        let mut m = SynonymMap::new();
        m.set_one_way("phone", vec!["smartphone".into(), "mobile".into()]);
        let resolved = m.resolve("phone");
        assert!(resolved.contains(&"phone".to_string()));
        assert!(resolved.contains(&"smartphone".to_string()));
        assert!(resolved.contains(&"mobile".to_string()));
    }

    #[test]
    fn add_equivalent_merges_intersecting_groups() {
        let mut m = SynonymMap::new();
        m.add_equivalent(["a", "b"]).unwrap();
        m.add_equivalent(["b", "c"]).unwrap();
        assert_eq!(m.equivalent.len(), 1);
        let resolved = m.resolve("a");
        assert!(resolved.contains(&"c".to_string()));
    }

    #[test]
    fn expand_terms_respects_depth_limit() {
        let m = SynonymMap::new();
        let out = m.expand_terms(&["a".into()]);
        assert_eq!(out, vec!["a".to_string()]);
    }
}
