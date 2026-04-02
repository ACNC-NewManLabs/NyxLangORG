//! NYX Serialization Layer [Layer 18]
//! JSON, Protobuf, and Bincode support.

use crate::collections::string::String as NyxString;
use crate::collections::vec::Vec as NyxVec;
use crate::error::{ErrorCategory, NyxError};
use serde::{Deserialize, Serialize};

pub trait Serializable: Serialize {
    fn to_json(&self) -> Result<NyxString, NyxError> {
        let s = serde_json::to_string(self).map_err(|e| {
            NyxError::new(
                "SER001",
                format!("JSON serialization failure: {}", e),
                ErrorCategory::Runtime,
            )
        })?;
        Ok(NyxString::from(&s))
    }

    fn to_binary(&self) -> Result<NyxVec<u8>, NyxError> {
        let b = bincode::serialize(self).map_err(|e| {
            NyxError::new(
                "SER002",
                format!("Binary serialization failure: {}", e),
                ErrorCategory::Runtime,
            )
        })?;
        let mut v = NyxVec::with_capacity(b.len());
        for b_val in b {
            v.push(b_val);
        }
        Ok(v)
    }
}

pub trait Deserializable<'a>: Deserialize<'a> {
    fn from_json(s: &'a str) -> Result<Self, NyxError> {
        serde_json::from_str(s).map_err(|e| {
            NyxError::new(
                "SER003",
                format!("JSON deserialization failure: {}", e),
                ErrorCategory::Runtime,
            )
        })
    }

    fn from_binary(data: &'a [u8]) -> Result<Self, NyxError> {
        bincode::deserialize(data).map_err(|e| {
            NyxError::new(
                "SER004",
                format!("Binary deserialization failure: {}", e),
                ErrorCategory::Runtime,
            )
        })
    }
}

// Blanket implementations
impl<T: Serialize> Serializable for T {}
impl<'a, T: Deserialize<'a>> Deserializable<'a> for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct MockData {
        id: u32,
        name: String,
    }

    #[test]
    fn test_serialization_json() {
        let data = MockData {
            id: 42,
            name: "Nyx".to_string(),
        };
        let json = data.to_json().expect("JSON serialization failed");
        assert!(json.as_str().contains("\"id\":42"));

        let decoded: MockData =
            MockData::from_json(json.as_str()).expect("JSON deserialization failed");
        assert_eq!(data, decoded);
    }

    #[test]
    fn test_serialization_binary() {
        let data = MockData {
            id: 101,
            name: "BinaryTest".to_string(),
        };
        let bin = data.to_binary().expect("Binary serialization failed");

        let decoded: MockData =
            MockData::from_binary(bin.as_slice()).expect("Binary deserialization failed");
        assert_eq!(data, decoded);
    }
}
