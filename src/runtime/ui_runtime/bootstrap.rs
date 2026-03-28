use std::path::Path;

use crate::devtools::server::DevtoolsServer;
use crate::runtime::execution::ui_runtime::{build_session, AstRuntimeSession, NativeRuntime};
use crate::runtime::execution::RuntimeError;

use super::tracing::RuntimeTrace;

pub struct RuntimeBootstrap {
    pub session: AstRuntimeSession,
    pub trace: RuntimeTrace,
}

impl RuntimeBootstrap {
    pub fn new(entry: &Path, runtime: NativeRuntime) -> Result<Self, RuntimeError> {
        let session = build_session(entry, runtime)?;
        let trace = RuntimeTrace::new(DevtoolsServer::default());
        Ok(Self { session, trace })
    }
}
