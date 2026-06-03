//! Evaluates a parsed [`Filter`] against a document (a `serde_json::Value`
//! representing a flat JSON object).

use serde_json::Value;

use errors::{AppError, AppResult};

use crate::ast::{Filter, GeoPoint};

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
            Filter::Contains(f, needle) => obj
                .get(f)
                .and_then(Value::as_str)
                .map(|s| s.contains(needle.as_str()))
                .unwrap_or(false),
            Filter::StartsWith(f, prefix) => obj
                .get(f)
                .and_then(Value::as_str)
                .map(|s| s.starts_with(prefix.as_str()))
                .unwrap_or(false),
            Filter::EndsWith(f, suffix) => obj
                .get(f)
                .and_then(Value::as_str)
                .map(|s| s.ends_with(suffix.as_str()))
                .unwrap_or(false),
            Filter::Like(f, pattern) => obj
                .get(f)
                .and_then(Value::as_str)
                .map(|s| like_match(s, pattern))
                .unwrap_or(false),
            Filter::GeoBoundingBox {
                field,
                top_right,
                bottom_left,
            } => obj
                .get(field)
                .and_then(geo_value)
                .map(|p| geo_in_bbox(p, *top_right, *bottom_left))
                .unwrap_or(false),
            Filter::GeoRadius {
                field,
                center,
                distance_meters,
            } => obj
                .get(field)
                .and_then(geo_value)
                .map(|p| haversine_meters(p, *center) <= *distance_meters)
                .unwrap_or(false),
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

fn geo_value(v: &Value) -> Option<GeoPoint> {
    let obj = v.as_object()?;
    let lat = obj.get("lat").and_then(Value::as_f64)?;
    let lng = obj.get("lng").and_then(Value::as_f64)?;
    Some(GeoPoint { lat, lng })
}

fn geo_in_bbox(p: GeoPoint, tr: GeoPoint, bl: GeoPoint) -> bool {
    p.lat <= tr.lat && p.lat >= bl.lat && p.lng <= tr.lng && p.lng >= bl.lng
}

const EARTH_RADIUS_M: f64 = 6_371_000.0;

fn haversine_meters(a: GeoPoint, b: GeoPoint) -> f64 {
    let lat1 = a.lat.to_radians();
    let lat2 = b.lat.to_radians();
    let dlat = (b.lat - a.lat).to_radians();
    let dlng = (b.lng - a.lng).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlng / 2.0).sin().powi(2);
    let c = 2.0 * h.sqrt().atan2((1.0 - h).sqrt());
    EARTH_RADIUS_M * c
}

/// SQL-style LIKE pattern match.
///
/// * `%` matches zero or more characters.
/// * `_` matches exactly one character.
/// * All other characters match literally.
#[must_use]
pub fn like_match(input: &str, pattern: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = input.chars().collect();
    like_match_recursive(&s, 0, &p, 0)
}

fn like_match_recursive(s: &[char], si: usize, p: &[char], pi: usize) -> bool {
    let mut sp = si;
    let mut pp = pi;
    while pp < p.len() {
        match p[pp] {
            '%' => {
                // Try every possible suffix position.
                for k in sp..=s.len() {
                    if like_match_recursive(s, k, p, pp + 1) {
                        return true;
                    }
                }
                return false;
            }
            '_' => {
                if sp >= s.len() {
                    return false;
                }
                sp += 1;
                pp += 1;
            }
            c => {
                if sp >= s.len() || s[sp] != c {
                    return false;
                }
                sp += 1;
                pp += 1;
            }
        }
    }
    sp == s.len()
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

    #[test]
    fn contains_works() {
        let f = Parser::parse(r#"name CONTAINS "ali""#).unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
        let f = Parser::parse(r#"name CONTAINS "zzz""#).unwrap();
        assert!(!FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn starts_with_works() {
        let f = Parser::parse(r#"name STARTS_WITH "al""#).unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn ends_with_works() {
        let f = Parser::parse(r#"name ENDS_WITH "ce""#).unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn like_works() {
        let f = Parser::parse(r#"name LIKE "a%""#).unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
        let f = Parser::parse(r#"name LIKE "%ce""#).unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
        let f = Parser::parse(r#"name LIKE "a_ice""#).unwrap();
        assert!(FilterEvaluator::matches(&f, &doc()).unwrap());
    }

    #[test]
    fn like_helper_basic() {
        assert!(like_match("hello", "h%o"));
        assert!(like_match("hello", "%"));
        assert!(like_match("hello", "_____"));
        assert!(!like_match("hello", "____"));
        assert!(!like_match("hello", "world"));
    }

    #[test]
    fn geo_bbox_works() {
        let d = serde_json::json!({
            "loc": {"lat": 10.0, "lng": 20.0}
        });
        let f = Parser::parse("loc GEO_BBOX 11.0 21.0 9.0 19.0").unwrap();
        assert!(FilterEvaluator::matches(&f, &d).unwrap());
        // A point at (10, 20) is inside a box from (9, 19.5) to (11, 21).
        let f = Parser::parse("loc GEO_BBOX 11.0 21.0 9.0 19.5").unwrap();
        assert!(FilterEvaluator::matches(&f, &d).unwrap());
        // The point (10, 20) is OUTSIDE a box from (9, 21) to (11, 22) because 20 < 21.
        let f = Parser::parse("loc GEO_BBOX 11.0 22.0 9.0 21.0").unwrap();
        assert!(!FilterEvaluator::matches(&f, &d).unwrap());
    }

    #[test]
    fn geo_radius_works() {
        let d = serde_json::json!({
            "loc": {"lat": 0.0, "lng": 0.0}
        });
        let f = Parser::parse("loc GEO_RADIUS 0.0 0.0 100").unwrap();
        assert!(FilterEvaluator::matches(&f, &d).unwrap());
        // New York is ~5,500 km from (0,0); reject with a 1km radius.
        let f = Parser::parse("loc GEO_RADIUS 40.7128 -74.0060 1000").unwrap();
        assert!(!FilterEvaluator::matches(&f, &d).unwrap());
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]

        /// `field = "value"` round-trips through the parser and only
        /// matches docs whose `field` equals `value`.
        #[test]
        fn equality_round_trips(
            field in "[a-z]{1,8}",
            value in "[a-zA-Z0-9]{0,8}",
        ) {
            let f = Parser::parse(&format!(r#"{field} = "{value}""#)).unwrap();
            let mut d = serde_json::Map::new();
            d.insert(field.clone(), serde_json::Value::String(value.clone()));
            let obj = serde_json::Value::Object(d);
            assert!(FilterEvaluator::matches(&f, &obj).unwrap());
        }

        /// A parser that doesn't panic on arbitrary input.
        #[test]
        fn parser_never_panics(s in "[A-Za-z0-9 _=\"\\[\\],.<>!\\(\\)\\-]{0,80}") {
            let _ = Parser::parse(&s);
        }
    }
}
