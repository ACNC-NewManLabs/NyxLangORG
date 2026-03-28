//! NYX Format Layer

pub mod format {
    pub fn format(args: std::fmt::Arguments) -> String {
        args.to_string()
    }
    pub fn print(args: std::fmt::Arguments) {
        print!("{}", args);
    }
    pub fn println(args: std::fmt::Arguments) {
        println!("{}", args);
    }
    pub fn eprint(args: std::fmt::Arguments) {
        eprint!("{}", args);
    }
    pub fn eprintln(args: std::fmt::Arguments) {
        eprintln!("{}", args);
    }
}

pub mod debug {
    pub fn debug_fmt<T: std::fmt::Debug>(value: &T) -> String {
        format!("{:?}", value)
    }
}

pub mod log {
    pub fn log(level: &str, msg: &str) {
        println!("[{}] {}", level, msg);
    }
    pub fn info(msg: &str) { log("INFO", msg); }
    pub fn warn(msg: &str) { log("WARN", msg); }
    pub fn error(msg: &str) { log("ERROR", msg); }
    pub fn debug(msg: &str) { log("DEBUG", msg); }
}

pub use format::*;

