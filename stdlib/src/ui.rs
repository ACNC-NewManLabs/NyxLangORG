//! NYX UI Layer [Layer 19]
//! Reactive UI primitives and state management.

pub mod widgets {
    use crate::collections::string::String as NyxString;

    pub struct Window {
        pub title: NyxString,
    }

    pub struct Button {
        pub label: NyxString,
    }

    pub struct Label {
        pub text: NyxString,
    }
}

pub mod reactive {
    use crate::collections::vec::Vec as NyxVec;

    pub struct State<T> {
        _val: T,
        _listeners: NyxVec<fn(&T)>,
    }
}
