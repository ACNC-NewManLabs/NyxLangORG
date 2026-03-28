use super::runtime_host::{HostError, SemanticsDelta};

#[derive(Debug, Clone, Default)]
pub struct AccessibilityHost {
    pub last_changed_nodes: usize,
}

impl AccessibilityHost {
    pub fn publish(&mut self, delta: SemanticsDelta) -> Result<(), HostError> {
        self.last_changed_nodes = delta.updates.len();
        Ok(())
    }
}
