//! Bare-Metal Runtime Module

#[macro_export]
macro_rules! no_std {
    () => {
        #![no_std]
    };
}

pub trait KernelInterface {
    fn init(&mut self);
    fn panic(&self, message: &str) -> !;
    fn debug_write(&self, msg: &str);
}
