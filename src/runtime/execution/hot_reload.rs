use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::runtime::execution::reload::{ModulePatch, ReloadSnapshot};
use crate::runtime::execution::nyx_vm::NyxVM;
use crate::devtools::protocol::{DevtoolsPayload, DevtoolsStream};

pub struct HotReloadManager {
    vm: NyxVM,
    reload_boundaries: HashMap<String, ReloadBoundary>,
    state_snapshots: HashMap<String, StateSnapshot>,
    patch_history: Vec<PatchApplication>,
    devtools_sender: Option<tokio::sync::mpsc::UnboundedSender<DevtoolsPayload>>,
}

#[derive(Debug, Clone)]
pub struct ReloadBoundary {
    pub boundary_id: String,
    pub module_ids: Vec<String>,
    pub state_preservation: StatePreservation,
    pub reload_strategy: ReloadStrategy,
}

#[derive(Debug, Clone)]
pub struct StatePreservation {
    pub preserve_widget_state: bool,
    pub preserve_element_state: bool,
    pub preserve_render_object_state: bool,
    pub custom_preservers: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ReloadStrategy {
    FullRestart,
    SubtreeRemount,
    StatePreservingPatch,
    IncrementalPatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub boundary_id: String,
    pub timestamp: std::time::SystemTime,
    pub widget_states: HashMap<String, WidgetState>,
    pub element_states: HashMap<String, ElementState>,
    pub global_state: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetState {
    pub widget_id: String,
    pub widget_type: String,
    pub key: String,
    pub state_data: serde_json::Value,
    pub children_snapshot: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementState {
    pub element_id: String,
    pub widget_type: String,
    pub lifecycle_state: String,
    pub dirty_flags: u32,
    pub state_slots: HashMap<String, serde_json::Value>,
    pub inherited_scope: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct PatchApplication {
    pub timestamp: std::time::SystemTime,
    pub patches: Vec<ModulePatch>,
    pub strategy_used: ReloadStrategy,
    pub success: bool,
    pub error_message: Option<String>,
    pub state_preserved: bool,
}

impl HotReloadManager {
    pub fn new(vm: NyxVM) -> Self {
        Self {
            vm,
            reload_boundaries: HashMap::new(),
            state_snapshots: HashMap::new(),
            patch_history: Vec::new(),
            devtools_sender: None,
        }
    }

    pub fn set_devtools_sender(&mut self, sender: tokio::sync::mpsc::UnboundedSender<DevtoolsPayload>) {
        self.devtools_sender = Some(sender);
    }

    pub fn add_reload_boundary(&mut self, boundary: ReloadBoundary) {
        self.reload_boundaries.insert(boundary.boundary_id.clone(), boundary);
    }

    pub fn apply_patches(&mut self, patches: Vec<ModulePatch>) -> Result<ReloadResult, HotReloadError> {
        let start_time = std::time::SystemTime::now();
        
        // Determine affected boundaries
        let affected_boundaries = self.find_affected_boundaries(&patches);
        
        // Choose reload strategy for each boundary
        let mut strategies = Vec::new();
        for boundary_id in affected_boundaries {
            let boundary = self.reload_boundaries.get(&boundary_id)
                .ok_or(HotReloadError::BoundaryNotFound(boundary_id))?;
            
            let strategy = self.choose_reload_strategy(boundary, &patches)?;
            strategies.push((boundary_id, strategy));
        }

        // Apply patches with state preservation
        let mut successful_patches = Vec::new();
        let mut state_preserved = true;
        
        for (boundary_id, strategy) in strategies {
            match self.apply_boundary_patches(boundary_id, &patches, strategy) {
                Ok(result) => {
                    if result.state_preserved {
                        // Update state snapshot
                        self.update_state_snapshot(boundary_id, &result.new_state)?;
                    } else {
                        state_preserved = false;
                    }
                    successful_patches.extend(result.applied_patches);
                }
                Err(e) => {
                    // Rollback if possible
                    self.rollback_patches(&successful_patches)?;
                    return Err(e);
                }
            }
        }

        // Record patch application
        let patch_application = PatchApplication {
            timestamp: start_time,
            patches: patches.clone(),
            strategy_used: ReloadStrategy::StatePreservingPatch, // Simplified
            success: true,
            error_message: None,
            state_preserved,
        };
        
        self.patch_history.push(patch_application);

        // Send devtools event
        if let Some(sender) = &self.devtools_sender {
            let payload = DevtoolsPayload::HotReloadPatched {
                modules: patches.iter().map(|p| p.module_id.clone()).collect(),
            };
            let _ = sender.send(DevtoolsPayload::HotReloadPatched {
                modules: patches.iter().map(|p| p.module_id.clone()).collect(),
            });
        }

        Ok(ReloadResult {
            applied_patches: successful_patches,
            state_preserved,
            reload_time: start_time.elapsed().unwrap_or_default(),
        })
    }

    fn find_affected_boundaries(&self, patches: &[ModulePatch]) -> Vec<String> {
        let mut affected = Vec::new();
        
        for boundary in self.reload_boundaries.values() {
            for patch in patches {
                if boundary.module_ids.contains(&patch.module_id) {
                    affected.push(boundary.boundary_id.clone());
                    break;
                }
            }
        }
        
        affected
    }

    fn choose_reload_strategy(&self, boundary: &ReloadBoundary, patches: &[ModulePatch]) -> Result<ReloadStrategy, HotReloadError> {
        // Check if patches are ABI-compatible
        let abi_compatible = self.check_abi_compatibility(patches)?;
        
        // Check if state can be preserved
        let state_preservable = self.check_state_preservation(boundary, patches)?;
        
        if !abi_compatible {
            return Ok(ReloadStrategy::FullRestart);
        }
        
        if !state_preservable {
            return Ok(ReloadStrategy::SubtreeRemount);
        }
        
        Ok(ReloadStrategy::StatePreservingPatch)
    }

    fn check_abi_compatibility(&self, patches: &[ModulePatch]) -> Result<bool, HotReloadError> {
        // Check if patches change public signatures
        // For now, assume compatible
        Ok(true)
    }

    fn check_state_preservation(&self, boundary: &ReloadBoundary, patches: &[ModulePatch]) -> Result<bool, HotReloadError> {
        // Check if state structure is compatible
        // For now, assume preservable if boundary allows it
        Ok(boundary.state_preservation.preserve_widget_state)
    }

    fn apply_boundary_patches(&mut self, boundary_id: &str, patches: &[ModulePatch], strategy: ReloadStrategy) -> Result<BoundaryReloadResult, HotReloadError> {
        match strategy {
            ReloadStrategy::FullRestart => {
                // Full restart - no state preservation
                self.full_restart(boundary_id, patches)
            }
            ReloadStrategy::SubtreeRemount => {
                // Remount subtree - partial state preservation
                self.subtree_remount(boundary_id, patches)
            }
            ReloadStrategy::StatePreservingPatch => {
                // Incremental patch with state preservation
                self.state_preserving_patch(boundary_id, patches)
            }
            ReloadStrategy::IncrementalPatch => {
                // Simple incremental patch
                self.incremental_patch(boundary_id, patches)
            }
        }
    }

    fn state_preserving_patch(&mut self, boundary_id: &str, patches: &[ModulePatch]) -> Result<BoundaryReloadResult, HotReloadError> {
        // 1. Take state snapshot before patching
        let old_state = self.capture_state_snapshot(boundary_id)?;
        
        // 2. Apply patches to VM
        for patch in patches {
            self.vm.apply_module_patch(patch)?;
        }
        
        // 3. Restore state with compatibility checking
        let new_state = self.restore_state_with_compatibility(boundary_id, &old_state)?;
        
        Ok(BoundaryReloadResult {
            boundary_id: boundary_id.to_string(),
            applied_patches: patches.to_vec(),
            state_preserved: true,
            new_state,
        })
    }

    fn subtree_remount(&mut self, boundary_id: &str, patches: &[ModulePatch]) -> Result<BoundaryReloadResult, HotReloadError> {
        // Apply patches and remount the subtree
        for patch in patches {
            self.vm.apply_module_patch(patch)?;
        }
        
        // Trigger subtree remount in the UI engine
        self.trigger_subtree_remount(boundary_id)?;
        
        Ok(BoundaryReloadResult {
            boundary_id: boundary_id.to_string(),
            applied_patches: patches.to_vec(),
            state_preserved: false,
            new_state: StateSnapshot::default(),
        })
    }

    fn full_restart(&mut self, boundary_id: &str, patches: &[ModulePatch]) -> Result<BoundaryReloadResult, HotReloadError> {
        // Full application restart
        // This would require coordination with the runtime host
        Err(HotReloadError::FullRestartRequired)
    }

    fn incremental_patch(&mut self, boundary_id: &str, patches: &[ModulePatch]) -> Result<BoundaryReloadResult, HotReloadResult> {
        // Simple patch without state preservation
        for patch in patches {
            self.vm.apply_module_patch(patch)?;
        }
        
        Ok(BoundaryReloadResult {
            boundary_id: boundary_id.to_string(),
            applied_patches: patches.to_vec(),
            state_preserved: false,
            new_state: StateSnapshot::default(),
        })
    }

    fn capture_state_snapshot(&self, boundary_id: &str) -> Result<StateSnapshot, HotReloadError> {
        // Capture current state from the UI engine
        // This would integrate with the engine's state inspection
        Ok(StateSnapshot {
            boundary_id: boundary_id.to_string(),
            timestamp: std::time::SystemTime::now(),
            widget_states: HashMap::new(),
            element_states: HashMap::new(),
            global_state: HashMap::new(),
        })
    }

    fn restore_state_with_compatibility(&mut self, boundary_id: &str, old_state: &StateSnapshot) -> Result<StateSnapshot, HotReloadError> {
        // Restore state with compatibility checking
        // This would integrate with the engine's state restoration
        Ok(StateSnapshot {
            boundary_id: boundary_id.to_string(),
            timestamp: std::time::SystemTime::now(),
            widget_states: old_state.widget_states.clone(),
            element_states: old_state.element_states.clone(),
            global_state: old_state.global_state.clone(),
        })
    }

    fn trigger_subtree_remount(&mut self, boundary_id: &str) -> Result<(), HotReloadError> {
        // Trigger subtree remount in the UI engine
        // This would send a signal to the engine to remount the boundary
        Ok(())
    }

    fn rollback_patches(&mut self, applied_patches: &[ModulePatch]) -> Result<(), HotReloadError> {
        // Rollback applied patches if possible
        // This would require keeping track of previous module versions
        Ok(())
    }

    fn update_state_snapshot(&mut self, boundary_id: &str, new_state: &StateSnapshot) -> Result<(), HotReloadError> {
        self.state_snapshots.insert(boundary_id.to_string(), new_state.clone());
        Ok(())
    }
}

#[derive(Debug)]
pub struct ReloadResult {
    pub applied_patches: Vec<ModulePatch>,
    pub state_preserved: bool,
    pub reload_time: std::time::Duration,
}

#[derive(Debug)]
pub struct BoundaryReloadResult {
    pub boundary_id: String,
    pub applied_patches: Vec<ModulePatch>,
    pub state_preserved: bool,
    pub new_state: StateSnapshot,
}

#[derive(Debug, thiserror::Error)]
pub enum HotReloadError {
    #[error("Boundary not found: {0}")]
    BoundaryNotFound(String),
    
    #[error("ABI incompatible - full restart required")]
    AbiIncompatible,
    
    #[error("State preservation failed: {0}")]
    StatePreservationFailed(String),
    
    #[error("Full restart required")]
    FullRestartRequired,
    
    #[error("VM error: {0}")]
    VmError(#[from] crate::runtime::execution::RuntimeError),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

impl Default for StateSnapshot {
    fn default() -> Self {
        Self {
            boundary_id: String::new(),
            timestamp: std::time::SystemTime::now(),
            widget_states: HashMap::new(),
            element_states: HashMap::new(),
            global_state: HashMap::new(),
        }
    }
}
