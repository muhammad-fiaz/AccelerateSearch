//! Evaluates a parsed [`Filter`] against a document (a `serde_json::Value`
//! representing a flat JSON object).

use serde_json::Value;

use errors::{AppError, AppResult};

use crate::ast::Filter;

/// Evaluator that runs a filter against a JSON document.
pub struct FilterEvaluator;

impl FilterEvaluator {
    /// Returns true if `doc` matches `filter`. The document must be a
    /// JSON object.
    ///
    /// # Errors
    /// Returns an error if the document is not an object or the filter
    /// references a value with an unsupported type.
    pub fn matches(filter: &Filter, doc: &Value) -> AppResult<bool> {
        let obj = doc.as_object().ok_or_else(|| {
            AppError::bad_request("filter can only be evaluated against an object")
        })?;
        Self::eval(filter, obj)
    }

    fn eval(filter: &Filter, obj: &serde_json::Map<String, Value>) -> AppResult<bool> {
        Ok(match filter {
            Filter::True => true,
            Filter::False => false,
            Filter::And(a, b) => Self::eval(a, obj)? && Self::eval(b, obj)?,
            Filter::Or(a, b) => Self::eval(a, obj)? || Self::eval(b, obj)?,
            Filter::Not(f) => !Self::eval(f, obj)?,
            Filter::Eq(f, v) => obj.get(f).map(|x| json_eq(x, v)).unwrap_or(false),
            Filter::NotEq(f, v) => !obj.get(f).map(|x| json_eq(x, v)).unwrap_or(false),
            Filter::Gt(f, v) => obj
                .get(f)
                .map(|x| json_cmp(x, v) == Some(std::cmp::Ordering::Greater))
                .unwrap_or(false),
            Filter::Gte(f, v) => matches!(
                json_cmp(obj.get(f).unwrap_or(&Value::Null), v),
                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            ),
            Filter::Lt(f, v) => obj
                .get(f)
                .map(|x| json_cmp(x, v) == Some(std::cmp::Ordering::Less))
                .unwrap_or(false),
            Filter::Lte(f, v) => matches!(
                json_cmp(obj.get(f).unwrap_or(&Value::Null), v),
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            ),
            Filter::Between(f, lo, hi) => {
                let v = obj.get(f).unwrap_or(&Value::Null);
                matches!(
                    json_cmp(v, lo),
                    Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                ) && matches!(
                    json_cmp(v, hi),
                    Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                )
            }
            Filter::In(f, list) => obj
                .get(f)
                .map(|x| list.iter().any(|v| json_eq(x, v)))
                .unwrap_or(false),
            Filter::NotIn(f, list) => !obj
                .get(f)
                .map(|x| list.iter().any(|v| json_eq(x, v)))
                .unwrap_or(false),
            Filter::Exists(f) => obj.contains_key(f),
            Filter::IsNull(f) => obj.get(f).map(|v| v.is_null()).unwrap_or(true),
            Filter::IsNotNull(f) => obj.get(f).map(|v| !v.is_null()).unwrap_or(false),
        })
    }
}

fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x
            .as_f64()
            .zip(y.as_f64())
            .map(|(a, b)| (a - b).abs() < f64::EPSILON)
            .unwrap_or(false),
        _ => a == b,
    }
}

fn json_cmp(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let x = x.as_f64()?;
            let y = y.as_f64()?;
            x.partial_cmp(&y)
        }
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn doc() -> Value {
        serde_json::json!({
            "name": "alice",
            "age": 30,
            "active": true,
            "tags": ["admin", "user"]
        })
    }

    #[test]
    fn equality_matches() {
        let f = Parser::parse(r#"name = "alice""#).unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn range_matches() {
        let f = Parser::parse("age 20 TO 40").unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn and_or_not() {
        let f = Parser::parse(r#"name = "alice" AND age > 18"#).unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
        let f = Parser::parse(r#"name = "alice" AND NOT active = false"#).unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn exists_works() {
        let f = Parser::parse("EXISTS name").unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn double_equals_works() {
        let f = Parser::parse(r#"name == "alice""#).unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn not_in_works() {
        let f = Parser::parse(r#"name NOT IN ["bob", "charlie"]"#).unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
        let f = Parser::parse(r#"name NOT IN ["alice", "bob"]"#).unwrap();
        assert!(!FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn complex_nested_filter() {
        let f = Parser::parse(r#"name = "alice" AND (age > 18 OR active = true)"#).unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn is_null_on_missing_field() {
        let f = Parser::parse("missing IS NULL").unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn is_not_null_on_existing_field() {
        let f = Parser::parse("name IS NOT NULL").unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn between_works() {
        let f = Parser::parse("age 20 TO 40").unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
        let f = Parser::parse("age 40 TO 50").unwrap();
        assert!(!FilterEvaluator::matches(&f, &doc()).unwrap());
    }
}
