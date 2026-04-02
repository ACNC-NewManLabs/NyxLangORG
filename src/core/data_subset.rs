use serde_json::Value;
use std::collections::HashMap;

/// NyxData is the strict, deterministic, code-free data representation format
/// meant to replace JSON within the Nyx Ecosystem. It represents only primitives,
/// avoiding any executable logic components.
#[derive(Debug, Clone, PartialEq)]
pub enum NyxData {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<NyxData>),
    Map(HashMap<String, NyxData>),
}

impl NyxData {
    /// Zero-overhead parsing into the native NyxData AST structure.
    pub fn parse(input: &str) -> Result<Self, String> {
        let val: Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
        Ok(Self::from_serde_value(val))
    }

    /// Serializes NyxData into a highly-compatible primitive JSON string representation.
    pub fn to_string(&self) -> String {
        self.to_serde_value().to_string()
    }

    /// Serializes NyxData into formatted representation for human readability.
    pub fn to_pretty_string(&self) -> String {
        serde_json::to_string_pretty(&self.to_serde_value()).unwrap_or_default()
    }

    fn from_serde_value(val: Value) -> Self {
        match val {
            Value::Null => NyxData::Null,
            Value::Bool(b) => NyxData::Bool(b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    NyxData::Int(i)
                } else if let Some(f) = n.as_f64() {
                    NyxData::Float(f)
                } else {
                    NyxData::Float(0.0)
                }
            }
            Value::String(s) => NyxData::String(s),
            Value::Array(a) => NyxData::Array(a.into_iter().map(Self::from_serde_value).collect()),
            Value::Object(m) => {
                let mut map = HashMap::new();
                for (k, v) in m {
                    map.insert(k, Self::from_serde_value(v));
                }
                NyxData::Map(map)
            }
        }
    }

    fn to_serde_value(&self) -> Value {
        match self {
            NyxData::Null => Value::Null,
            NyxData::Bool(b) => Value::Bool(*b),
            NyxData::Int(i) => Value::Number(serde_json::Number::from(*i)),
            NyxData::Float(f) => {
                if let Some(n) = serde_json::Number::from_f64(*f) {
                    Value::Number(n)
                } else {
                    Value::Null
                }
            }
            NyxData::String(s) => Value::String(s.clone()),
            NyxData::Array(a) => Value::Array(a.iter().map(|item| item.to_serde_value()).collect()),
            NyxData::Map(m) => {
                let mut map = serde_json::Map::new();
                for (k, v) in m {
                    map.insert(k.clone(), v.to_serde_value());
                }
                Value::Object(map)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nyxdata_parse_primitives() {
        assert_eq!(NyxData::parse("null").unwrap(), NyxData::Null);
        assert_eq!(NyxData::parse("true").unwrap(), NyxData::Bool(true));
        assert_eq!(NyxData::parse("42").unwrap(), NyxData::Int(42));
        assert_eq!(NyxData::parse("3.14").unwrap(), NyxData::Float(3.14));
        assert_eq!(NyxData::parse("\"hello\"").unwrap(), NyxData::String("hello".to_string()));
    }

    #[test]
    fn test_nyxdata_arrays_and_maps() {
        let arr_str = "[1, 2, \"nyx\"]";
        let arr_data = NyxData::parse(arr_str).unwrap();
        assert_eq!(arr_data, NyxData::Array(vec![NyxData::Int(1), NyxData::Int(2), NyxData::String("nyx".to_string())]));

        let map_str = "{\"key\": 99}";
        let map_data = NyxData::parse(map_str).unwrap();
        let mut expected_map = HashMap::new();
        expected_map.insert("key".to_string(), NyxData::Int(99));
        assert_eq!(map_data, NyxData::Map(expected_map));
    }

    #[test]
    fn test_nyxdata_serialization() {
        let mut map = HashMap::new();
        map.insert("val".to_string(), NyxData::Int(10));
        let data = NyxData::Map(map);
        
        let s = data.to_string();
        assert_eq!(s, "{\"val\":10}");
    }
}
