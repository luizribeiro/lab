use std::hash::{Hash, Hasher};

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum VarValueError {
    #[error("NaN is not a valid VarValue::Float")]
    NaN,
    #[error("infinity is not a valid VarValue::Float")]
    Infinite,
    #[error("unsupported toml value type: {0}")]
    UnsupportedToml(&'static str),
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum VarValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

impl VarValue {
    pub fn float(f: f64) -> Result<Self, VarValueError> {
        if f.is_nan() {
            return Err(VarValueError::NaN);
        }
        if f.is_infinite() {
            return Err(VarValueError::Infinite);
        }
        // to_bits() distinguishes -0.0 from 0.0, but IEEE says they're equal;
        // collapse to +0.0 so PartialEq/Hash agree with float arithmetic.
        let canonical = if f == 0.0 { 0.0 } else { f };
        Ok(Self::Float(canonical))
    }
}

impl TryFrom<f64> for VarValue {
    type Error = VarValueError;
    fn try_from(f: f64) -> Result<Self, Self::Error> {
        Self::float(f)
    }
}

impl From<i64> for VarValue {
    fn from(i: i64) -> Self {
        Self::Integer(i)
    }
}

impl From<bool> for VarValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<String> for VarValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for VarValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_owned())
    }
}

impl PartialEq for VarValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a.to_bits() == b.to_bits(),
            (Self::Bool(a), Self::Bool(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for VarValue {}

impl Hash for VarValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::String(s) => s.hash(state),
            Self::Integer(i) => i.hash(state),
            Self::Float(f) => f.to_bits().hash(state),
            Self::Bool(b) => b.hash(state),
        }
    }
}

impl<'de> Deserialize<'de> for VarValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Bool(bool),
            Integer(i64),
            Float(f64),
            String(String),
        }
        match Raw::deserialize(d)? {
            Raw::Bool(b) => Ok(Self::Bool(b)),
            Raw::Integer(i) => Ok(Self::Integer(i)),
            Raw::Float(f) => Self::float(f).map_err(serde::de::Error::custom),
            Raw::String(s) => Ok(Self::String(s)),
        }
    }
}

impl TryFrom<toml::Value> for VarValue {
    type Error = VarValueError;
    fn try_from(v: toml::Value) -> Result<Self, Self::Error> {
        match v {
            toml::Value::String(s) => Ok(Self::String(s)),
            toml::Value::Integer(i) => Ok(Self::Integer(i)),
            toml::Value::Float(f) => Self::float(f),
            toml::Value::Boolean(b) => Ok(Self::Bool(b)),
            toml::Value::Datetime(_) => Err(VarValueError::UnsupportedToml("datetime")),
            toml::Value::Array(_) => Err(VarValueError::UnsupportedToml("array")),
            toml::Value::Table(_) => Err(VarValueError::UnsupportedToml("table")),
        }
    }
}

impl From<VarValue> for toml::Value {
    fn from(v: VarValue) -> Self {
        match v {
            VarValue::String(s) => toml::Value::String(s),
            VarValue::Integer(i) => toml::Value::Integer(i),
            VarValue::Float(f) => toml::Value::Float(f),
            VarValue::Bool(b) => toml::Value::Boolean(b),
        }
    }
}

impl From<VarValue> for serde_json::Value {
    fn from(v: VarValue) -> Self {
        match v {
            VarValue::String(s) => serde_json::Value::String(s),
            VarValue::Integer(i) => serde_json::Value::Number(i.into()),
            VarValue::Float(f) => serde_json::Value::Number(
                serde_json::Number::from_f64(f)
                    .expect("VarValue::Float invariant: finite, non-NaN"),
            ),
            VarValue::Bool(b) => serde_json::Value::Bool(b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::collections::HashSet;

    fn h(v: &VarValue) -> u64 {
        let mut s = DefaultHasher::new();
        v.hash(&mut s);
        s.finish()
    }

    #[test]
    fn equality_within_variants() {
        assert_eq!(VarValue::from("a"), VarValue::from("a"));
        assert_ne!(VarValue::from("a"), VarValue::from("b"));
        assert_eq!(VarValue::from(1i64), VarValue::from(1i64));
        assert_eq!(VarValue::from(true), VarValue::from(true));
        assert_ne!(VarValue::from(true), VarValue::from(false));
        assert_eq!(VarValue::float(1.5).unwrap(), VarValue::float(1.5).unwrap());
    }

    #[test]
    fn integer_and_float_are_distinct() {
        let i = VarValue::from(1i64);
        let f = VarValue::float(1.0).unwrap();
        assert_ne!(i, f);
        assert_ne!(h(&i), h(&f));
    }

    #[test]
    fn bool_and_integer_are_distinct() {
        assert_ne!(VarValue::from(true), VarValue::from(1i64));
        assert_ne!(VarValue::from(false), VarValue::from(0i64));
    }

    #[test]
    fn nan_rejected_at_construction() {
        assert!(matches!(VarValue::float(f64::NAN), Err(VarValueError::NaN)));
        assert!(matches!(
            VarValue::try_from(f64::NAN),
            Err(VarValueError::NaN)
        ));
    }

    #[test]
    fn negative_zero_canonicalizes() {
        let pos = VarValue::float(0.0).unwrap();
        let neg = VarValue::float(-0.0).unwrap();
        assert_eq!(pos, neg);
        assert_eq!(h(&pos), h(&neg));

        let mut set = HashSet::new();
        set.insert(pos);
        assert!(set.contains(&VarValue::float(-0.0).unwrap()));
    }

    #[test]
    fn hash_set_treats_equal_values_as_one() {
        let mut set = HashSet::new();
        set.insert(VarValue::from("k"));
        set.insert(VarValue::from("k"));
        set.insert(VarValue::from(1i64));
        set.insert(VarValue::from(1i64));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn round_trip_toml_value() {
        let cases = [
            toml::Value::String("hello".into()),
            toml::Value::Integer(42),
            toml::Value::Float(3.5),
            toml::Value::Boolean(true),
        ];
        for original in cases {
            let v = VarValue::try_from(original.clone()).unwrap();
            let back: toml::Value = v.into();
            assert_eq!(original, back);
        }
    }

    #[test]
    fn toml_nan_rejected() {
        let nan = toml::Value::Float(f64::NAN);
        assert!(matches!(VarValue::try_from(nan), Err(VarValueError::NaN)));
    }

    #[test]
    fn toml_array_and_table_rejected() {
        assert!(matches!(
            VarValue::try_from(toml::Value::Array(vec![])),
            Err(VarValueError::UnsupportedToml("array"))
        ));
        assert!(matches!(
            VarValue::try_from(toml::Value::Table(toml::map::Map::new())),
            Err(VarValueError::UnsupportedToml("table"))
        ));
        let dt: toml::value::Datetime = "1979-05-27T07:32:00Z".parse().unwrap();
        assert!(matches!(
            VarValue::try_from(toml::Value::Datetime(dt)),
            Err(VarValueError::UnsupportedToml("datetime"))
        ));
    }

    #[test]
    fn infinity_rejected_at_construction() {
        assert!(matches!(
            VarValue::float(f64::INFINITY),
            Err(VarValueError::Infinite)
        ));
        assert!(matches!(
            VarValue::float(f64::NEG_INFINITY),
            Err(VarValueError::Infinite)
        ));
    }

    #[test]
    fn round_trip_serde_json_value() {
        let cases = [
            VarValue::from("hello"),
            VarValue::from(42i64),
            VarValue::float(3.5).unwrap(),
            VarValue::from(true),
        ];
        for v in cases {
            let json: serde_json::Value = v.clone().into();
            let parsed: VarValue = serde_json::from_value(json).unwrap();
            assert_eq!(v, parsed);
        }
    }

    #[test]
    fn serde_json_deserialize_integer_vs_float() {
        let i: VarValue = serde_json::from_str("1").unwrap();
        let f: VarValue = serde_json::from_str("1.0").unwrap();
        assert!(matches!(i, VarValue::Integer(1)));
        assert!(matches!(f, VarValue::Float(_)));
        assert_ne!(i, f);
    }

    #[test]
    fn toml_deserialize_via_var_value() {
        #[derive(Deserialize)]
        struct W {
            v: VarValue,
        }
        let w: W = toml::from_str("v = 7").unwrap();
        assert_eq!(w.v, VarValue::from(7i64));
        let w: W = toml::from_str("v = 7.5").unwrap();
        assert_eq!(w.v, VarValue::float(7.5).unwrap());
        let w: W = toml::from_str(r#"v = "hi""#).unwrap();
        assert_eq!(w.v, VarValue::from("hi"));
        let w: W = toml::from_str("v = true").unwrap();
        assert_eq!(w.v, VarValue::from(true));
    }
}
