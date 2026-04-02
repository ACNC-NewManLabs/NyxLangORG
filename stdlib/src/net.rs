//! NYX Networking Layer [Layer 16]
//! TCP, UDP, and QUIC protocols.

pub mod tcp {
    use crate::error::{ErrorCategory, NyxError};
    use std::net::{TcpListener as StdListener, ToSocketAddrs};

    pub struct TcpListener {
        inner: StdListener,
    }

    impl TcpListener {
        pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<Self, NyxError> {
            let listener = StdListener::bind(addr).map_err(|e| {
                NyxError::new(
                    "NET001",
                    format!("TCP bind failure: {}", e),
                    ErrorCategory::Io,
                )
                .with_suggestion(
                    "Check if the port is already in use or if you have necessary permissions.",
                )
            })?;
            Ok(Self { inner: listener })
        }

        pub fn set_nonblocking(&self, nonblocking: bool) -> Result<(), NyxError> {
            self.inner.set_nonblocking(nonblocking).map_err(|e| {
                NyxError::new(
                    "NET002",
                    format!("TCP config failure: {}", e),
                    ErrorCategory::Io,
                )
            })
        }
    }
}
