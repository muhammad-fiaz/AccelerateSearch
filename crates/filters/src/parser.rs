//! Recursive-descent parser for filter expressions.
//!
//! Grammar (informal):
//!
//! ```text
//! expr      := or
//! or        := and ( "OR" and )*
//! and       := not ( "AND" not )*
//! not       := "NOT" not | atom
//! atom      := "(" expr ")" | comparison
//! comparison := field op value
//! op        := "=" | "!=" | ">" | ">=" | "<" | "<=" | "TO" | "IN" | "NOT" "IN" | "EXISTS" | "IS" "NULL" | "IS" "NOT" "NULL"
//! value     := number | string | bool | null | array
//! ```

use serde_json::Value;

use errors::{AppError, AppResult};

use crate::ast::Filter;

/// Token kinds emitted by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// Identifier (field name or keyword).
    Ident(String),
    /// Number literal.
    Number(f64),
    /// String literal (without quotes in the resulting value).
    String(String),
    /// Boolean literal.
    Bool(bool),
    /// Null literal.
    Null,
    /// `(`.
    LParen,
    /// `)`.
    RParen,
    /// `[`.
    LBracket,
    /// `]`.
    RBracket,
    /// `,`.
    Comma,
    /// End of input.
    Eof,
}

/// A simple lexer over the filter source string.
pub struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a new lexer.
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    fn skip_whitespace(&mut self) {
        while let Some(b) = self.peek() {
            if (b as char).is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Lexes the entire input into a vector of tokens (terminated by `Eof`).
    pub fn tokenize(mut self) -> AppResult<Vec<Token>> {
        let mut out = Vec::new();
        loop {
            self.skip_whitespace();
            let Some(b) = self.peek() else {
                out.push(Token::Eof);
                return Ok(out);
            };
            match b {
                b'(' => {
                    self.pos += 1;
                    out.push(Token::LParen);
                }
                b')' => {
                    self.pos += 1;
                    out.push(Token::RParen);
                }
                b'[' => {
                    self.pos += 1;
                    out.push(Token::LBracket);
                }
                b']' => {
                    self.pos += 1;
                    out.push(Token::RBracket);
                }
                b',' => {
                    self.pos += 1;
                    out.push(Token::Comma);
                }
                b'"' | b'\'' => {
                    out.push(self.read_string(b)?);
                }
                b'=' | b'!' | b'>' | b'<' => {
                    out.push(self.read_operator()?);
                }
                b if b.is_ascii_digit() || b == b'-' => {
                    out.push(self.read_number()?);
                }
                b if b.is_ascii_alphabetic() || b == b'_' => {
                    out.push(self.read_ident());
                }
                _ => {
                    return Err(AppError::bad_request(format!(
                        "unexpected character '{}' at position {}",
                        b as char, self.pos
                    )));
                }
            }
        }
    }

    fn read_string(&mut self, quote: u8) -> AppResult<Token> {
        self.pos += 1; // consume opening quote
        let mut s = String::new();
        while let Some(b) = self.advance() {
            if b == quote {
                return Ok(Token::String(s));
            }
            if b == b'\\' {
                if let Some(escaped) = self.advance() {
                    match escaped {
                        b'n' => s.push('\n'),
                        b't' => s.push('\t'),
                        b'r' => s.push('\r'),
                        b'\\' => s.push('\\'),
                        b'"' => s.push('"'),
                        b'\'' => s.push('\''),
                        other => {
                            return Err(AppError::bad_request(format!(
                                "invalid escape '\\{}' in string",
                                other as char
                            )));
                        }
                    }
                } else {
                    return Err(AppError::bad_request("unterminated string"));
                }
            } else {
                s.push(b as char);
            }
        }
        Err(AppError::bad_request("unterminated string"))
    }

    fn read_operator(&mut self) -> AppResult<Token> {
        let _start = self.pos;
        let first = self.advance().unwrap();
        let second = self.peek();
        let op: &[u8] = match (first, second) {
            (b'=', Some(b'=')) => {
                self.pos += 1;
                b"=="
            }
            (b'!', Some(b'=')) => {
                self.pos += 1;
                b"!="
            }
            (b'>', Some(b'=')) => {
                self.pos += 1;
                b">="
            }
            (b'<', Some(b'=')) => {
                self.pos += 1;
                b"<="
            }
            (b'=', _) => b"=",
            (b'>', _) => b">",
            (b'<', _) => b"<",
            (b'!', _) => b"!",
            _ => unreachable!(),
        };
        let s = std::str::from_utf8(op).unwrap_or("");
        Ok(Token::Ident(s.to_string()))
    }

    fn read_number(&mut self) -> AppResult<Token> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() || b == b'.' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|e| AppError::bad_request(format!("invalid number: {e}")))?;
        let n: f64 = s
            .parse()
            .map_err(|e| AppError::bad_request(format!("invalid number '{s}': {e}")))?;
        Ok(Token::Number(n))
    }

    fn read_ident(&mut self) -> Token {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.input[start..self.pos])
            .unwrap_or("")
            .to_string();
        match s.as_str() {
            "true" => Token::Bool(true),
            "false" => Token::Bool(false),
            "null" => Token::Null,
            other => Token::Ident(other.to_string()),
        }
    }
}

/// Recursive-descent parser. Holds a token stream and a position.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    /// Creates a parser from a token vector.
    #[must_use]
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Parses a filter expression from the source string.
    pub fn parse(source: &str) -> AppResult<Filter> {
        let tokens = Lexer::new(source).tokenize()?;
        let mut parser = Self::new(tokens);
        let filter = parser.parse_or()?;
        parser.expect_eof()?;
        Ok(filter)
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn expect_eof(&self) -> AppResult<()> {
        if matches!(self.peek(), Token::Eof) {
            Ok(())
        } else {
            Err(AppError::bad_request(format!(
                "unexpected trailing token: {:?}",
                self.peek()
            )))
        }
    }

    fn parse_or(&mut self) -> AppResult<Filter> {
        let mut left = self.parse_and()?;
        while let Token::Ident(name) = self.peek() {
            if name == "OR" {
                self.advance();
                let right = self.parse_and()?;
                left = Filter::Or(Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> AppResult<Filter> {
        let mut left = self.parse_not()?;
        loop {
            match self.peek() {
                Token::Ident(name) if name == "AND" => {
                    self.advance();
                    let right = self.parse_not()?;
                    left = Filter::And(Box::new(left), Box::new(right));
                }
                Token::Ident(name) if name == "OR" => break,
                Token::Eof | Token::RParen => break,
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> AppResult<Filter> {
        if let Token::Ident(name) = self.peek()
            && name == "NOT"
        {
            self.advance();
            let inner = self.parse_not()?;
            return Ok(Filter::Not(Box::new(inner)));
        }
        self.parse_atom()
    }

    fn parse_atom(&mut self) -> AppResult<Filter> {
        if matches!(self.peek(), Token::LParen) {
            self.advance();
            let inner = self.parse_or()?;
            self.expect(Token::RParen)?;
            return Ok(inner);
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> AppResult<Filter> {
        // Special prefix forms: `EXISTS field` or `field IS [NOT] NULL`.
        if let Token::Ident(name) = self.peek()
            && name == "EXISTS"
        {
            self.advance();
            let field = self.expect_ident()?;
            return Ok(Filter::Exists(field));
        }
        let field = self.expect_ident()?;
        // Implicit range syntax: `field 10 TO 100`.
        if matches!(self.peek(), Token::Number(_)) {
            let lo = self.parse_value()?;
            self.expect_ident_must("TO")?;
            let hi = self.parse_value()?;
            return Ok(Filter::Between(field, lo, hi));
        }
        let op = self.advance();
        match op {
            Token::Ident(s) => match s.as_str() {
                "=" => {
                    let v = self.parse_value()?;
                    Ok(Filter::Eq(field, v))
                }
                "!=" => {
                    let v = self.parse_value()?;
                    Ok(Filter::NotEq(field, v))
                }
                ">" => {
                    let v = self.parse_value()?;
                    Ok(Filter::Gt(field, v))
                }
                ">=" => {
                    let v = self.parse_value()?;
                    Ok(Filter::Gte(field, v))
                }
                "<" => {
                    let v = self.parse_value()?;
                    Ok(Filter::Lt(field, v))
                }
                "<=" => {
                    let v = self.parse_value()?;
                    Ok(Filter::Lte(field, v))
                }
                "==" => {
                    let v = self.parse_value()?;
                    Ok(Filter::Eq(field, v))
                }
                "TO" => {
                    let lo = self.parse_value()?;
                    self.expect_ident_must("TO")?;
                    let hi = self.parse_value()?;
                    Ok(Filter::Between(field, lo, hi))
                }
                "IN" => {
                    let list = self.parse_array()?;
                    Ok(Filter::In(field, list))
                }
                "NOT" => {
                    self.expect_ident_must("IN")?;
                    let list = self.parse_array()?;
                    Ok(Filter::NotIn(field, list))
                }
                "IS" => {
                    if let Token::Ident(s) = self.peek() {
                        if s == "NOT" {
                            self.advance();
                            self.expect_ident_must("NULL")?;
                            return Ok(Filter::IsNotNull(field));
                        }
                        if s == "NULL" {
                            self.advance();
                            return Ok(Filter::IsNull(field));
                        }
                    }
                    Err(AppError::bad_request("expected NULL or NOT NULL after IS"))
                }
                other => Err(AppError::bad_request(format!(
                    "unknown operator '{other}' in filter"
                ))),
            },
            _ => Err(AppError::bad_request(format!(
                "expected operator after field '{field}', got {op:?}"
            ))),
        }
    }

    fn parse_value(&mut self) -> AppResult<Value> {
        let t = self.advance();
        Ok(match t {
            Token::String(s) => Value::String(s),
            Token::Number(n) => serde_json::Number::from_f64(n)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            Token::Bool(b) => Value::Bool(b),
            Token::Null => Value::Null,
            Token::LBracket => {
                let mut arr = Vec::new();
                if !matches!(self.peek(), Token::RBracket) {
                    loop {
                        arr.push(self.parse_value()?);
                        match self.peek() {
                            Token::Comma => {
                                self.advance();
                            }
                            Token::RBracket => break,
                            _ => return Err(AppError::bad_request("expected ',' or ']'")),
                        }
                    }
                }
                self.expect(Token::RBracket)?;
                Value::Array(arr)
            }
            other => {
                return Err(AppError::bad_request(format!(
                    "expected value, got {other:?}"
                )));
            }
        })
    }

    fn parse_array(&mut self) -> AppResult<Vec<Value>> {
        self.expect(Token::LBracket)?;
        let mut out = Vec::new();
        if !matches!(self.peek(), Token::RBracket) {
            loop {
                out.push(self.parse_value()?);
                match self.peek() {
                    Token::Comma => {
                        self.advance();
                    }
                    Token::RBracket => break,
                    _ => return Err(AppError::bad_request("expected ',' or ']'")),
                }
            }
        }
        self.expect(Token::RBracket)?;
        Ok(out)
    }

    fn expect(&mut self, expected: Token) -> AppResult<()> {
        let got = self.advance();
        if got == expected {
            Ok(())
        } else {
            Err(AppError::bad_request(format!(
                "expected {expected:?}, got {got:?}"
            )))
        }
    }

    fn expect_ident(&mut self) -> AppResult<String> {
        let t = self.advance();
        match t {
            Token::Ident(s) => Ok(s),
            other => Err(AppError::bad_request(format!(
                "expected identifier, got {other:?}"
            ))),
        }
    }

    fn expect_ident_must(&mut self, want: &str) -> AppResult<String> {
        let s = self.expect_ident()?;
        if s == want {
            Ok(s)
        } else {
            Err(AppError::bad_request(format!(
                "expected '{want}', got '{s}'"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Filter {
        Parser::parse(s).expect("parse")
    }

    #[test]
    fn parse_equality() {
        let f = parse(r#"name = "alice""#);
        assert!(matches!(f, Filter::Eq(_, Value::String(_))));
    }

    #[test]
    fn parse_numeric_inequality() {
        let f = parse("age >= 18");
        assert!(matches!(f, Filter::Gte(_, _)));
    }

    #[test]
    fn parse_range() {
        let f = parse("price 10 TO 100");
        assert!(matches!(f, Filter::Between(_, _, _)));
    }

    #[test]
    fn parse_in() {
        let f = parse(r#"category IN ["a", "b"]"#);
        assert!(matches!(f, Filter::In(_, _)));
    }

    #[test]
    fn parse_exists() {
        let f = parse("EXISTS title");
        assert!(matches!(f, Filter::Exists(_)));
    }

    #[test]
    fn parse_is_null() {
        let f = parse("title IS NULL");
        assert!(matches!(f, Filter::IsNull(_)));
        let f = parse("title IS NOT NULL");
        assert!(matches!(f, Filter::IsNotNull(_)));
    }

    #[test]
    fn parse_boolean() {
        let f = parse(r#"a = 1 AND (b = 2 OR c = 3)"#);
        assert!(matches!(f, Filter::And(_, _)));
        let f = parse("NOT x = 1");
        assert!(matches!(f, Filter::Not(_)));
    }

    #[test]
    fn parse_nested_groups() {
        let f = parse(r#"(a = 1 AND b = 2) OR c = 3"#);
        assert!(matches!(f, Filter::Or(_, _)));
    }

    #[test]
    fn parse_invalid_string() {
        assert!(Parser::parse(r#"a = "unterminated"#).is_err());
    }

    #[test]
    fn parse_rejects_trailing_garbage() {
        assert!(Parser::parse("a = 1 garbage").is_err());
    }

    #[test]
    fn referenced_fields_collect() {
        let f = parse("a = 1 AND (b = 2 OR c IN [3, 4])");
        let fields = f.referenced_fields();
        assert!(fields.contains(&"a".to_string()));
        assert!(fields.contains(&"b".to_string()));
        assert!(fields.contains(&"c".to_string()));
    }

    #[test]
    fn parse_double_equals() {
        let f = parse(r#"name == "alice""#);
        assert!(matches!(f, Filter::Eq(_, Value::String(_))));
    }

    #[test]
    fn parse_not_in() {
        let f = parse(r#"category NOT IN ["a", "b"]"#);
        assert!(matches!(f, Filter::NotIn(_, _)));
    }
}
