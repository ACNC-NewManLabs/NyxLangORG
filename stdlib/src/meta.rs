//! NYX Meta Layer [Layer 30]
//! Reflection and Metaprogramming.

pub mod reflection {
    pub struct TypeInfo;
    pub fn typeof_t<T>() -> TypeInfo {
        TypeInfo
    }
}
