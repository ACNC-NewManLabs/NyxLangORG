//! NYX Hardware Layer [Layer 26]
//! HAL and Driver Primitives.

pub mod hal {
    pub struct Bus;
    pub struct Port;
}

pub mod drivers {
    pub mod keyboard {
        pub fn read_key() -> char {
            ' '
        }
    }
}
