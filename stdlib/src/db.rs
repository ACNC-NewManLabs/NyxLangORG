//! NYX Database Layer [Layer 17]
//! Unified interface for SQL and NoSQL databases.

pub mod database {
    use crate::error::NyxError;
    use crate::collections::vec::Vec as NyxVec;
    use crate::collections::string::String as NyxString;

    pub trait Connection {
        fn execute(&mut self, query: &str) -> Result<ResultSet, NyxError>;
        fn query(&mut self, query: &str) -> Result<ResultSet, NyxError>;
    }

    pub struct ResultSet {
        pub columns: NyxVec<NyxString>,
        pub rows: NyxVec<NyxVec<Value>>,
    }

    pub enum Value {
        Null,
        Int(i64),
        Float(f64),
        Text(NyxString),
        Binary(NyxVec<u8>),
    }

    pub mod sqlite {
        use super::*;
        pub struct SqliteConnection;
        impl Connection for SqliteConnection {
            fn execute(&mut self, _query: &str) -> Result<ResultSet, NyxError> {
                Ok(ResultSet { columns: NyxVec::new(), rows: NyxVec::new() })
            }
            fn query(&mut self, _query: &str) -> Result<ResultSet, NyxError> {
                Ok(ResultSet { columns: NyxVec::new(), rows: NyxVec::new() })
            }
        }
    }
}

pub use database::*;
