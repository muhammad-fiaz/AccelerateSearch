//! Filter expression parsing and evaluation.

pub mod ast;
pub mod evaluator;
pub mod parser;

pub use ast::Filter;
pub use evaluator::FilterEvaluator;
pub use parser::{Lexer, Parser, Token};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_to_end_parse_and_evaluate() {
        let f = Parser::parse(r#"name = "alice" AND age >= 18"#).unwrap();
        let doc = serde_json::json!({"name": "alice", "age": 30});
        assert!(FilterEvaluator::matches(&f, &doc).unwrap());
    }
}
