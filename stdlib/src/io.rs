pub mod io {
    pub use std::io::{Read, Write};
    use crate::error::{NyxError, ErrorCategory};
    use crate::collections::string::String as NyxString;

    /// Industrial Safe Reader wrapper
    pub struct Reader<R: Read> {
        inner: R,
    }

    impl<R: Read> Reader<R> {
        pub fn new(inner: R) -> Self { Self { inner } }
        
        pub fn read_to_string(&mut self) -> Result<NyxString, NyxError> {
            let mut s = String::new();
            self.inner.read_to_string(&mut s)
                .map_err(|e| NyxError::new("IO001", format!("Read failure: {}", e), ErrorCategory::Io)
                    .with_suggestion("Check if the stream is closed or if you have necessary permissions."))?;
            Ok(NyxString::from(&s))
        }

        pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), NyxError> {
            self.inner.read_exact(buf)
                .map_err(|e| NyxError::new("IO002", format!("Read exact failure: {}", e), ErrorCategory::Io))
        }
    }

    /// Industrial Safe Writer wrapper
    pub struct Writer<W: Write> {
        inner: W,
    }

    impl<W: Write> Writer<W> {
        pub fn new(inner: W) -> Self { Self { inner } }

        pub fn write_all(&mut self, buf: &[u8]) -> Result<(), NyxError> {
            self.inner.write_all(buf)
                .map_err(|e| NyxError::new("IO003", format!("Write failure: {}", e), ErrorCategory::Io)
                    .with_suggestion("Check disk space or stream connection."))
        }

        pub fn flush(&mut self) -> Result<(), NyxError> {
            self.inner.flush()
                .map_err(|e| NyxError::new("IO004", format!("Flush failure: {}", e), ErrorCategory::Io))
        }
    }

    /// Global print with audit hooks
    pub fn print(msg: &str) {
        use std::io::{Write, stdout};
        let _ = stdout().write_all(msg.as_bytes());
    }

    pub fn println(msg: &str) {
        print(msg);
        print("\n");
    }
}

pub use io::*;
