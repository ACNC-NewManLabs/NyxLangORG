use std::path::Path;

use crate::devtools::protocol::{DevtoolsPayload, DevtoolsStream};
use crate::runtime::execution::ui_runtime::{AstRuntimeSession, NativeRuntime};
use crate::runtime::execution::{RuntimeError, RuntimeValue};

use super::bootstrap::RuntimeBootstrap;
use super::frame_clock::FrameClock;

pub struct AppLauncher {
    pub bootstrap: RuntimeBootstrap,
    pub clock: FrameClock,
}

impl AppLauncher {
    pub fn new(entry: &Path, runtime: NativeRuntime) -> Result<Self, RuntimeError> {
        Ok(Self {
            bootstrap: RuntimeBootstrap::new(entry, runtime)?,
            clock: FrameClock::new(60),
        })
    }

    pub fn session_mut(&mut self) -> &mut AstRuntimeSession {
        &mut self.bootstrap.session
    }

    pub fn run_main(&mut self) -> Result<RuntimeValue, RuntimeError> {
        let frame_id = self.clock.begin_frame();
        self.bootstrap.trace.emit(
            DevtoolsStream::Timeline,
            DevtoolsPayload::FrameStarted { frame_id },
        );
        let value = self
            .bootstrap
            .session
            .vm_mut()
            .execute_main()
            .map_err(RuntimeError::from)?;
        self.bootstrap.trace.emit(
            DevtoolsStream::Timeline,
            DevtoolsPayload::FrameEnded {
                frame_id,
                total_micros: self.clock.elapsed_micros(),
            },
        );
        Ok(value)
    }
}
