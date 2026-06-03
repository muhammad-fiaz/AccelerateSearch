//! Text analysis: tokenisation, normalisation, stop-word removal, stemming.

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

/// A token produced by the analyser.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    /// The lower-cased, stemmed form of the token.
    pub term: String,
    /// The original token text (pre-stemming).
    pub original: String,
    /// The start byte offset in the source field (if available).
    pub start: usize,
    /// The end byte offset in the source field (if available).
    pub end: usize,
}

/// Configuration for the [`Analyzer`].
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    /// Whether to lower-case all tokens.
    pub lowercase: bool,
    /// Whether to apply Unicode NFC normalisation.
    pub normalize: bool,
    /// Whether to remove stop words.
    pub remove_stop_words: bool,
    /// Optional stemming language (`"en"`, `"fr"`, ...). `None` disables
    /// stemming.
    pub stem_lang: Option<String>,
    /// Optional list of stop words to use (overrides the per-collection
    /// stop-words list when provided).
    pub stop_words: Option<StopWords>,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            lowercase: true,
            normalize: true,
            remove_stop_words: true,
            stem_lang: Some("en".into()),
            stop_words: None,
        }
    }
}

/// A reusable text analyser.
pub struct Analyzer {
    cfg: AnalyzerConfig,
    stemmer: Option<rust_stemmers::Stemmer>,
}

impl Analyzer {
    /// Creates a new analyser.
    #[must_use]
    pub fn new(cfg: AnalyzerConfig) -> Self {
        let stemmer = cfg
            .stem_lang
            .as_deref()
            .and_then(parse_algorithm)
            .map(rust_stemmers::Stemmer::create);
        Self { cfg, stemmer }
    }

    /// Returns true if the given token is a stop word.
    #[must_use]
    pub fn is_stop_word(&self, token: &str) -> bool {
        if !self.cfg.remove_stop_words {
            return false;
        }
        match &self.cfg.stop_words {
            Some(sw) => sw.contains(token),
            None => false,
        }
    }

    /// Tokenises and analyses the input string.
    #[must_use]
    pub fn analyze(&self, input: &str) -> Vec<Token> {
        let mut out = Vec::new();
        for (idx, word) in input.unicode_words().enumerate() {
            let start = find_word_offset(input, word, idx);
            let end = start + word.len();
            let normalized = self.normalize(word);
            if normalized.is_empty() {
                continue;
            }
            if self.is_stop_word(&normalized) {
                continue;
            }
            let stemmed = self
                .stemmer
                .as_ref()
                .map(|s| s.stem(&normalized).to_string())
                .unwrap_or_else(|| normalized.clone());
            out.push(Token {
                term: stemmed,
                original: normalized,
                start,
                end,
            });
        }
        out
    }

    /// Tokenises only (no stemming, no stop word removal) and returns raw
    /// tokens.
    #[must_use]
    pub fn tokenize(&self, input: &str) -> Vec<Token> {
        input
            .unicode_words()
            .map(|w| Token {
                term: self.normalize(w),
                original: self.normalize(w),
                start: 0,
                end: w.len(),
            })
            .collect()
    }

    fn normalize(&self, input: &str) -> String {
        let mut s: String = if self.cfg.normalize {
            input.nfc().collect()
        } else {
            input.to_string()
        };
        if self.cfg.lowercase {
            s = s.to_lowercase();
        }
        s
    }
}

fn find_word_offset(input: &str, word: &str, occurrence: usize) -> usize {
    let mut seen = 0;
    let mut start = 0;
    for w in input.unicode_words() {
        if w == word {
            if seen == occurrence {
                return start;
            }
            seen += 1;
        }
        start += w.len();
        // Skip non-word characters between words. Use unicode segmenter.
        if let Some(rest) = input.get(start..) {
            for c in rest.chars() {
                if c.is_alphanumeric() {
                    break;
                }
                start += c.len_utf8();
            }
        }
    }
    0
}

/// Maps a language name to a [`rust_stemmers::Algorithm`].
fn parse_algorithm(lang: &str) -> Option<rust_stemmers::Algorithm> {
    use rust_stemmers::Algorithm;
    Some(match lang.to_ascii_lowercase().as_str() {
        "arabic" | "ar" => Algorithm::Arabic,
        "danish" | "da" => Algorithm::Danish,
        "dutch" | "nl" => Algorithm::Dutch,
        "english" | "en" => Algorithm::English,
        "finnish" | "fi" => Algorithm::Finnish,
        "french" | "fr" => Algorithm::French,
        "german" | "de" => Algorithm::German,
        "greek" | "el" => Algorithm::Greek,
        "hungarian" | "hu" => Algorithm::Hungarian,
        "italian" | "it" => Algorithm::Italian,
        "norwegian" | "no" => Algorithm::Norwegian,
        "portuguese" | "pt" => Algorithm::Portuguese,
        "romanian" | "ro" => Algorithm::Romanian,
        "russian" | "ru" => Algorithm::Russian,
        "spanish" | "es" => Algorithm::Spanish,
        "swedish" | "sv" => Algorithm::Swedish,
        "tamil" | "ta" => Algorithm::Tamil,
        "turkish" | "tr" => Algorithm::Turkish,
        _ => return None,
    })
}

use std::collections::HashSet;

/// A pre-computed set of stop words with O(1) lookup via `HashSet`.
#[derive(Debug, Clone, Default)]
pub struct StopWords {
    words: HashSet<String>,
}

impl StopWords {
    /// Creates a new stop-words set from an iterator.
    #[must_use]
    pub fn new<I, S>(words: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            words: words.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns true if `token` is in the stop-words set.
    #[must_use]
    pub fn contains(&self, token: &str) -> bool {
        self.words.contains(token)
    }

    /// Returns the number of stop words.
    #[must_use]
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// Returns true if the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_splits_on_whitespace() {
        let a = Analyzer::new(AnalyzerConfig {
            stem_lang: None,
            ..Default::default()
        });
        let toks = a.analyze("Hello world!");
        assert_eq!(toks.len(), 2);
        assert_eq!(toks[0].original, "hello");
        assert_eq!(toks[1].original, "world");
    }

    #[test]
    fn analyzer_lowercases_and_stems() {
        let a = Analyzer::new(AnalyzerConfig::default());
        let toks = a.analyze("Running quickly");
        assert_eq!(toks[0].term, "run");
        // Stemmer output for "quickly" varies across algorithm versions.
        assert!(toks[1].term.starts_with("quick"));
    }

    #[test]
    fn analyzer_respects_stop_words() {
        let sw = StopWords::new(["the", "a", "an"]);
        let a = Analyzer::new(AnalyzerConfig {
            stop_words: Some(sw),
            ..Default::default()
        });
        let toks = a.analyze("the quick brown fox");
        assert_eq!(toks.len(), 3);
        assert_eq!(toks[0].term, "quick");
    }

    #[test]
    fn unicode_normalisation() {
        let a = Analyzer::new(AnalyzerConfig {
            stem_lang: None,
            ..Default::default()
        });
        let toks = a.analyze("café");
        assert_eq!(toks[0].original, "café");
    }

    #[test]
    fn stop_words_contains_and_len() {
        let sw = StopWords::new(["a", "b"]);
        assert!(sw.contains("a"));
        assert!(!sw.contains("c"));
        assert_eq!(sw.len(), 2);
        assert!(!sw.is_empty());
    }
}
