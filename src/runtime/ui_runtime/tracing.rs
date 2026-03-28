use crate::devtools::protocol::{DevtoolsEnvelope, DevtoolsPayload, DevtoolsStream};
use crate::devtools::server::DevtoolsServer;
use crate::devtools::timeline::Timeline;

#[derive(Debug, Clone)]
pub struct RuntimeTrace {
    timeline: Timeline,
    server: DevtoolsServer,
}

impl RuntimeTrace {
    pub fn new(server: DevtoolsServer) -> Self {
        Self {
            timeline: Timeline::new(),
            server,
        }
    }

    pub fn emit(&mut self, stream: DevtoolsStream, payload: DevtoolsPayload) -> DevtoolsEnvelope {
        let envelope = self.timeline.next(stream, payload);
        self.server.emit(envelope.clone());
        envelope
    }
}
