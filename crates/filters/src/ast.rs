//! AST for filter expressions.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Filter {
    /// Logical AND of two sub-filters.
    And(Box<Filter>, Box<Filter>),
    /// Logical OR of two sub-filters.
    Or(Box<Filter>, Box<Filter>),
    /// Logical NOT of a sub-filter.
    Not(Box<Filter>),
    /// `field = value`
    Eq(String, Value),
    /// `field != value`
    NotEq(String, Value),
    /// `field > value`
    Gt(String, Value),
    /// `field >= value`
    Gte(String, Value),
    /// `field < value`
    Lt(String, Value),
    /// `field <= value`
    Lte(String, Value),
    /// `field a TO b` (inclusive range)
    Between(String, Value, Value),
    /// `field IN [...]`
    In(String, Vec<Value>),
    /// `field NOT IN [...]`
    NotIn(String, Vec<Value>),
    /// `field EXISTS`
    Exists(String),
    /// `field IS NULL`
    IsNull(String),
    /// `field IS NOT NULL`
    IsNotNull(String),
    /// `field CONTAINS value` — case-sensitive substring match.
    Contains(String, String),
    /// `field STARTS_WITH value` — case-sensitive prefix match.
    StartsWith(String, String),
    /// `field ENDS_WITH value` — case-sensitive suffix match.
    EndsWith(String, String),
    /// `field LIKE pattern` — SQL-style pattern (`%` = any, `_` = single char).
    Like(String, String),
    /// `field GEO_BBOX {lat, lng, ...}` — bounding box.
    GeoBoundingBox {
        /// Field containing the geo coordinate object `{lat, lng}`.
        field: String,
        /// Top-right corner.
        top_right: GeoPoint,
        /// Bottom-left corner.
        bottom_left: GeoPoint,
    },
    /// `field GEO_RADIUS {lat, lng} distance_in_meters` — radius search.
    GeoRadius {
        /// Field containing the geo coordinate object `{lat, lng}`.
        field: String,
        /// Center point.
        center: GeoPoint,
        /// Radius in meters.
        distance_meters: f64,
    },
    /// `true` — always matches.
    True,
    /// `false` — never matches.
    False,
}

/// A geographic coordinate.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct GeoPoint {
    /// Latitude in degrees, -90.0..=90.0.
    pub lat: f64,
    /// Longitude in degrees, -180.0..=180.0.
    pub lng: f64,
}

impl Filter {
    /// Returns true if this filter trivially matches everything.
    #[must_use]
    pub fn is_true(&self) -> bool {
        matches!(self, Self::True)
    }

    /// Returns the set of field names referenced by this filter.
    #[must_use]
    pub fn referenced_fields(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_fields(&mut out);
        out.sort();
        out.dedup();
        out
    }

    fn collect_fields(&self, out: &mut Vec<String>) {
        match self {
            Self::And(a, b) | Self::Or(a, b) => {
                a.collect_fields(out);
                b.collect_fields(out);
            }
            Self::Not(f) => f.collect_fields(out),
            Self::Eq(f, _)
            | Self::NotEq(f, _)
            | Self::Gt(f, _)
            | Self::Gte(f, _)
            | Self::Lt(f, _)
            | Self::Lte(f, _)
            | Self::Between(f, _, _)
            | Self::In(f, _)
            | Self::NotIn(f, _)
            | Self::Contains(f, _)
            | Self::StartsWith(f, _)
            | Self::EndsWith(f, _)
            | Self::Like(f, _)
            | Self::Exists(f)
            | Self::IsNull(f)
            | Self::IsNotNull(f) => out.push(f.clone()),
            Self::GeoBoundingBox { field, .. } | Self::GeoRadius { field, .. } => {
                out.push(field.clone());
            }
            Self::True | Self::False => {}
        }
    }
}
