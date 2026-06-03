//! Hit highlighting and field cropping.

use std::collections::BTreeMap;

use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

use errors::AppResult;

use indexing::analyzer;

/// Highlighter configuration.
#[derive(Debug, Clone)]
pub struct HighlighterConfig {
    /// Pre-tag (default `<em>`).
    pub pre_tag: String,
    /// Post-tag (default `</em>`).
    pub post_tag: String,
    /// Maximum length (in characters) of a cropped snippet. `None` disables
    /// cropping.
    pub crop_length: Option<usize>,
    /// Marker inserted where a snippet has been truncated.
    pub crop_marker: String,
}

impl Default for HighlighterConfig {
    fn default() -> Self {
        Self {
            pre_tag: "<em>".into(),
            post_tag: "</em>".into(),
            crop_length: None,
            crop_marker: "…".into(),
        }
    }
}

/// Highlighted field result, suitable for returning in the `_formatted`
/// block of a search hit.
pub type HighlightedFields = BTreeMap<String, String>;

/// Highlighter engine.
pub struct Highlighter {
    cfg: HighlighterConfig,
}

impl Highlighter {
    /// Creates a new highlighter with the given configuration.
    #[must_use]
    pub fn new(cfg: HighlighterConfig) -> Self {
        Self { cfg }
    }

    /// Highlights a list of `terms` inside the value of `field` from the
    /// given document.
    pub fn highlight(
        &self,
        doc: &Value,
        field: &str,
        terms: &[String],
    ) -> AppResult<Option<String>> {
        let v = match doc.get(field) {
            Some(v) => v,
            None => return Ok(None),
        };
        let text = match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        Ok(Some(self.highlight_text(&text, terms)))
    }

    /// Highlights the matching terms inside `text`.
    #[must_use]
    pub fn highlight_text(&self, text: &str, terms: &[String]) -> String {
        if terms.is_empty() {
            return text.to_string();
        }
        let lowered: Vec<String> = terms.iter().map(|t| t.to_lowercase()).collect();
        let mut out = String::with_capacity(text.len());
        let mut buf = String::new();
        for word in text.unicode_words() {
            let lower = word.to_lowercase();
            if lowered.iter().any(|t| t == &lower) {
                out.push_str(&self.cfg.pre_tag);
                out.push_str(&buf);
                buf.clear();
                out.push_str(word);
                out.push_str(&self.cfg.post_tag);
            } else {
                out.push_str(&buf);
                buf.clear();
                out.push_str(word);
            }
            buf.clear();
        }
        if let Some(max) = self.cfg.crop_length {
            self.crop(&out, max)
        } else {
            out
        }
    }

    /// Crops the text to at most `max_chars` characters, inserting the
    /// crop marker at the truncation point.
    #[must_use]
    pub fn crop(&self, text: &str, max_chars: usize) -> String {
        let count = text.chars().count();
        if count <= max_chars {
            return text.to_string();
        }
        let mut out: String = text.chars().take(max_chars).collect();
        out.push_str(&self.cfg.crop_marker);
        out
    }
}

/// Computes the query terms used for highlighting by tokenising the query
/// with a default [`analyzer::Analyzer`].
#[must_use]
pub fn terms_from_query(query: &str) -> Vec<String> {
    let a = analyzer::Analyzer::new(analyzer::AnalyzerConfig::default());
    a.analyze(query).into_iter().map(|t| t.term).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_wraps_matches() {
        let h = Highlighter::new(HighlighterConfig::default());
        let text = h.highlight_text("the quick brown fox", &["quick".into()]);
        assert!(text.contains("<em>quick</em>"));
    }

    #[test]
    fn highlight_is_case_insensitive() {
        let h = Highlighter::new(HighlighterConfig::default());
        let text = h.highlight_text("Hello World", &["hello".into()]);
        assert!(text.contains("<em>Hello</em>"));
    }

    #[test]
    fn crop_shortens_text() {
        let h = Highlighter::new(HighlighterConfig {
            crop_length: Some(5),
            ..Default::default()
        });
        let out = h.crop("abcdefghij", 5);
        assert!(out.starts_with("abcde"));
        assert!(out.ends_with('…'));
    }

    #[test]
    fn highlight_field_picks_string_value() {
        let h = Highlighter::new(HighlighterConfig::default());
        let v = serde_json::json!({"title": "hello world"});
        let s = h.highlight(&v, "title", &["hello".into()]).unwrap();
        assert!(s.unwrap().contains("<em>hello</em>"));
    }

    #[test]
    fn terms_from_query_extracts() {
        let t = terms_from_query("hello world");
        assert!(t.contains(&"hello".to_string()));
        assert!(t.contains(&"world".to_string()));
    }
}
