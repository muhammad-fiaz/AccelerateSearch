//! Query language: parsing and AST for the search query string.

use serde::{Deserialize, Serialize};

/// A parsed search query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Query {
    /// `q` — match the literal query string.
    Simple(String),
    /// `q1 AND q2` — both must match.
    And(Box<Query>, Box<Query>),
    /// `q1 OR q2` — either must match.
    Or(Box<Query>, Box<Query>),
    /// `NOT q` — exclude documents matching `q`.
    Not(Box<Query>),
    /// Empty query — match nothing / return everything depending on caller.
    Empty,
}

impl Query {
    /// Returns true if this query is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Returns the list of leaf terms used by the query (deduplicated).
    #[must_use]
    pub fn terms(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out.sort();
        out.dedup();
        out
    }

    fn collect(&self, out: &mut Vec<String>) {
        match self {
            Self::Simple(s) => out.push(s.clone()),
            Self::And(a, b) | Self::Or(a, b) => {
                a.collect(out);
                b.collect(out);
            }
            Self::Not(q) => q.collect(out),
            Self::Empty => {}
        }
    }
}

/// Tokenises and parses a search query string.
///
/// Supports quoted strings (`"exact phrase"`), `AND`, `OR`, `NOT`, and
/// parentheses. Implicit boolean is `AND`.
pub fn parse_query(input: &str) -> Query {
    let input = input.trim();
    if input.is_empty() {
        return Query::Empty;
    }
    let mut tokens = tokenize(input);
    if tokens.is_empty() {
        return Query::Empty;
    }
    tokens.reverse();
    let mut state = ParserState { tokens };
    let q = parse_or(&mut state);
    // Combine trailing terms (implicit AND).
    q
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Quoted(String),
    And,
    Or,
    Not,
    LParen,
    RParen,
}

struct ParserState {
    tokens: Vec<Tok>,
}

impl ParserState {
    fn pop(&mut self) -> Option<Tok> {
        self.tokens.pop()
    }
}

fn tokenize(input: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\n' => continue,
            '(' => out.push(Tok::LParen),
            ')' => out.push(Tok::RParen),
            '"' => {
                let mut s = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc == '"' {
                        chars.next();
                        break;
                    }
                    s.push(nc);
                    chars.next();
                }
                out.push(Tok::Quoted(s));
            }
            _ => {
                let mut s = String::new();
                s.push(c);
                while let Some(&nc) = chars.peek() {
                    if nc.is_whitespace() || nc == '(' || nc == ')' || nc == '"' {
                        break;
                    }
                    s.push(nc);
                    chars.next();
                }
                let upper = s.to_uppercase();
                match upper.as_str() {
                    "AND" => out.push(Tok::And),
                    "OR" => out.push(Tok::Or),
                    "NOT" => out.push(Tok::Not),
                    _ => out.push(Tok::Word(s.to_lowercase())),
                }
            }
        }
    }
    out
}

fn parse_or(s: &mut ParserState) -> Query {
    let mut left = parse_and(s);
    loop {
        match s.pop() {
            Some(Tok::Or) => {
                let right = parse_and(s);
                left = Query::Or(Box::new(left), Box::new(right));
            }
            Some(other) => {
                // Push back the consumed token by re-prepending.
                s.tokens.push(other);
                break;
            }
            None => break,
        }
    }
    left
}

fn parse_and(s: &mut ParserState) -> Query {
    let mut left = parse_not(s);
    while let Some(next) = s.pop() {
        match next {
            Tok::And => {
                let right = parse_not(s);
                left = Query::And(Box::new(left), Box::new(right));
            }
            Tok::Or | Tok::RParen => {
                s.tokens.push(next);
                break;
            }
            Tok::Word(w) | Tok::Quoted(w) => {
                // Implicit AND
                left = Query::And(Box::new(left), Box::new(Query::Simple(w)));
            }
            Tok::Not => {
                let inner = parse_not(s);
                left = Query::And(Box::new(left), Box::new(Query::Not(Box::new(inner))));
            }
            Tok::LParen => {
                let q = parse_or(s);
                let _ = s.pop(); // discard matching RParen if present
                left = Query::And(Box::new(left), Box::new(q));
            }
        }
    }
    left
}

fn parse_not(s: &mut ParserState) -> Query {
    match s.pop() {
        Some(Tok::Not) => {
            let inner = parse_not(s);
            Query::Not(Box::new(inner))
        }
        Some(Tok::LParen) => {
            let q = parse_or(s);
            let _ = s.pop(); // discard matching RParen if present
            q
        }
        Some(Tok::Word(w)) | Some(Tok::Quoted(w)) => Query::Simple(w),
        _ => Query::Empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_query() {
        let q = parse_query("hello world");
        assert!(q.terms().contains(&"hello".to_string()));
    }

    #[test]
    fn parse_quoted_phrase() {
        let q = parse_query(r#""hello world""#);
        assert!(q.terms().contains(&"hello world".to_string()));
    }

    #[test]
    fn parse_or_query() {
        let q = parse_query("a OR b");
        assert!(matches!(q, Query::Or(_, _)));
    }

    #[test]
    fn parse_and_query() {
        let q = parse_query("a AND b");
        assert!(matches!(q, Query::And(_, _)));
    }

    #[test]
    fn parse_not_query() {
        let q = parse_query("NOT a");
        assert!(matches!(q, Query::Not(_)));
    }

    #[test]
    fn parse_nested() {
        let q = parse_query("(a OR b) AND c");
        assert!(matches!(q, Query::And(_, _)));
    }

    #[test]
    fn parse_empty() {
        assert_eq!(parse_query(""), Query::Empty);
    }
}
