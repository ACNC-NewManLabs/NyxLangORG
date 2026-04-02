use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessibilityRole {
    // Generic roles
    None,
    Generic,

    // Interactive elements
    Button,
    Link,
    Checkbox,
    RadioButton,
    Switch,
    Slider,
    ProgressIndicator,
    ScrollBar,

    // Text elements
    Text,
    Heading,
    Label,
    Caption,

    // Container elements
    Group,
    List,
    ListItem,
    Table,
    Row,
    Cell,
    Grid,
    GridCell,

    // Navigation
    Tab,
    TabList,
    TabPanel,
    Menu,
    MenuItem,
    MenuBar,

    // Media
    Image,
    Audio,
    Video,

    // Form elements
    TextField,
    TextArea,
    ComboBox,
    SearchBox,

    // Structural
    Application,
    Document,
    Dialog,
    Alert,
    Tooltip,

    // Custom
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum AccessibilityState {
    // Basic states
    Disabled,
    Hidden,
    ReadOnly,
    Required,

    // Selection states
    Selected,
    Checked,
    Pressed,

    // Focus states
    Focused,
    Focusable,

    // Value states
    Invalid,
    Expanded,
    Collapsed,

    // Interactive states
    Busy,
    Live,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityAttributes {
    pub role: AccessibilityRole,
    pub label: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub current_value: Option<f64>,
    pub step: Option<f64>,
    pub placeholder: Option<String>,
    pub states: HashMap<AccessibilityState, bool>,
    pub properties: HashMap<String, String>,
}

impl Default for AccessibilityAttributes {
    fn default() -> Self {
        Self {
            role: AccessibilityRole::Generic,
            label: None,
            description: None,
            value: None,
            min_value: None,
            max_value: None,
            current_value: None,
            step: None,
            placeholder: None,
            states: HashMap::new(),
            properties: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticsNode {
    pub id: String,
    pub role: AccessibilityRole,
    pub label: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub bounds: AccessibilityBounds,
    pub children: Vec<String>,
    pub parent: Option<String>,
    pub states: HashMap<AccessibilityState, bool>,
    pub actions: Vec<AccessibilityAction>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AccessibilityAction {
    // Basic actions
    Tap,
    Focus,
    Select,
    Deselect,

    // Value actions
    Increment,
    Dismiss,

    // Navigation actions
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,

    // Text actions
    Copy,
    Cut,
    Paste,
    InsertText,
    DeleteText,

    // Custom actions
    Custom(String),
}

pub struct SemanticsTree {
    nodes: HashMap<String, SemanticsNode>,
    root_id: String,
    focused_node: Option<String>,
    live_regions: Vec<String>,
}

impl SemanticsTree {
    pub fn new(root_id: String) -> Self {
        Self {
            nodes: HashMap::new(),
            root_id,
            focused_node: None,
            live_regions: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: SemanticsNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn update_node(
        &mut self,
        node_id: &str,
        updates: SemanticsUpdate,
    ) -> Result<(), AccessibilityError> {
        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or(AccessibilityError::NodeNotFound(node_id.to_string()))?;

        if let Some(role) = updates.role {
            node.role = role;
        }
        if let Some(label) = updates.label {
            node.label = label;
        }
        if let Some(description) = updates.description {
            node.description = description;
        }
        if let Some(value) = updates.value {
            node.value = value;
        }
        if let Some(bounds) = updates.bounds {
            node.bounds = bounds;
        }

        for (state, enabled) in updates.states {
            node.states.insert(state, enabled);
        }

        for (key, value) in updates.properties {
            node.properties.insert(key, value);
        }

        Ok(())
    }

    pub fn remove_node(&mut self, node_id: &str) -> Result<SemanticsNode, AccessibilityError> {
        // Remove from parent's children
        let parent_id = self.nodes.get(node_id).and_then(|n| n.parent.clone());
        if let Some(p_id) = parent_id {
            if let Some(parent) = self.nodes.get_mut(&p_id) {
                parent.children.retain(|id| id != node_id);
            }
        }

        // Remove node and all descendants
        let node = self
            .nodes
            .remove(node_id)
            .ok_or(AccessibilityError::NodeNotFound(node_id.to_string()))?;

        // Remove descendants
        for child_id in &node.children {
            self.remove_node_recursive(child_id);
        }

        Ok(node)
    }

    fn remove_node_recursive(&mut self, node_id: &str) {
        if let Some(node) = self.nodes.remove(node_id) {
            for child_id in &node.children {
                self.remove_node_recursive(child_id);
            }
        }
    }

    pub fn get_node(&self, node_id: &str) -> Option<&SemanticsNode> {
        self.nodes.get(node_id)
    }

    pub fn get_root(&self) -> Option<&SemanticsNode> {
        self.nodes.get(&self.root_id)
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<String> {
        // Find the deepest node at the given coordinates
        let mut candidates = Vec::new();

        for (id, node) in &self.nodes {
            if self.point_in_bounds(x, y, &node.bounds) {
                candidates.push((id.clone(), node));
            }
        }

        // Sort by depth (deepest first)
        candidates.sort_by(|a, b| self.get_node_depth(&a.0).cmp(&self.get_node_depth(&b.0)));

        candidates.first().map(|(id, _)| id.clone())
    }

    fn point_in_bounds(&self, x: f32, y: f32, bounds: &AccessibilityBounds) -> bool {
        x >= bounds.x
            && x <= bounds.x + bounds.width
            && y >= bounds.y
            && y <= bounds.y + bounds.height
    }

    fn get_node_depth(&self, node_id: &str) -> usize {
        let mut depth = 0;
        let mut current_id = node_id;

        while let Some(node) = self.nodes.get(current_id) {
            if let Some(parent_id) = &node.parent {
                current_id = parent_id;
                depth += 1;
            } else {
                break;
            }
        }

        depth
    }

    pub fn set_focus(&mut self, node_id: Option<&str>) -> Result<(), AccessibilityError> {
        if let Some(id) = self.focused_node.as_ref() {
            if let Some(node) = self.nodes.get_mut(id) {
                node.states.insert(AccessibilityState::Focused, false);
            }
        }

        if let Some(new_id) = node_id {
            let node = self
                .nodes
                .get_mut(new_id)
                .ok_or(AccessibilityError::NodeNotFound(new_id.to_string()))?;

            node.states.insert(AccessibilityState::Focused, true);
            self.focused_node = Some(new_id.to_string());
        } else {
            self.focused_node = None;
        }

        Ok(())
    }

    pub fn get_focused_node(&self) -> Option<&SemanticsNode> {
        self.focused_node.as_ref().and_then(|id| self.nodes.get(id))
    }

    pub fn navigate(&self, from_id: &str, direction: NavigationDirection) -> Option<String> {
        let _from_node = self.nodes.get(from_id)?;

        match direction {
            NavigationDirection::Next => self.find_next_sibling(from_id),
            NavigationDirection::Previous => self.find_previous_sibling(from_id),
            NavigationDirection::Up => self.find_parent(from_id),
            NavigationDirection::Down => self.find_first_child(from_id),
            NavigationDirection::First => self.find_first_focusable(),
            NavigationDirection::Last => self.find_last_focusable(),
        }
    }

    fn find_next_sibling(&self, node_id: &str) -> Option<String> {
        let node = self.nodes.get(node_id)?;
        if let Some(parent_id) = &node.parent {
            let parent = self.nodes.get(parent_id)?;
            let index = parent.children.iter().position(|id| id == node_id)?;

            // Find next focusable sibling
            for sibling_id in parent.children.iter().skip(index + 1) {
                if let Some(sibling) = self.nodes.get(sibling_id) {
                    if self.is_focusable(sibling) {
                        return Some(sibling_id.clone());
                    }
                }
            }
        }
        None
    }

    fn find_previous_sibling(&self, node_id: &str) -> Option<String> {
        let node = self.nodes.get(node_id)?;
        if let Some(parent_id) = &node.parent {
            let parent = self.nodes.get(parent_id)?;
            let index = parent.children.iter().position(|id| id == node_id)?;

            // Find previous focusable sibling (in reverse)
            for sibling_id in parent.children.iter().take(index).rev() {
                if let Some(sibling) = self.nodes.get(sibling_id) {
                    if self.is_focusable(sibling) {
                        return Some(sibling_id.clone());
                    }
                }
            }
        }
        None
    }

    fn find_parent(&self, node_id: &str) -> Option<String> {
        self.nodes.get(node_id)?.parent.clone()
    }

    fn find_first_child(&self, node_id: &str) -> Option<String> {
        let node = self.nodes.get(node_id)?;
        for child_id in &node.children {
            if let Some(child) = self.nodes.get(child_id) {
                if self.is_focusable(child) {
                    return Some(child_id.clone());
                }
            }
        }
        None
    }

    fn find_first_focusable(&self) -> Option<String> {
        // Depth-first search for first focusable node
        for (id, node) in &self.nodes {
            if self.is_focusable(node) {
                return Some(id.clone());
            }
        }
        None
    }

    fn find_last_focusable(&self) -> Option<String> {
        // Reverse depth-first search for last focusable node
        let mut ids: Vec<_> = self.nodes.keys().collect();
        ids.reverse();

        for id in ids {
            if let Some(node) = self.nodes.get(id) {
                if self.is_focusable(node) {
                    return Some(id.clone());
                }
            }
        }
        None
    }

    fn is_focusable(&self, node: &SemanticsNode) -> bool {
        // Check if node is focusable based on role and states
        if node
            .states
            .get(&AccessibilityState::Disabled)
            .copied()
            .unwrap_or(false)
        {
            return false;
        }

        if node
            .states
            .get(&AccessibilityState::Hidden)
            .copied()
            .unwrap_or(false)
        {
            return false;
        }

        match node.role {
            AccessibilityRole::Button
            | AccessibilityRole::Link
            | AccessibilityRole::TextField
            | AccessibilityRole::TextArea
            | AccessibilityRole::ComboBox
            | AccessibilityRole::Checkbox
            | AccessibilityRole::RadioButton
            | AccessibilityRole::Switch
            | AccessibilityRole::Slider => true,

            AccessibilityRole::Generic => {
                // Generic elements are focusable if explicitly marked
                node.states
                    .get(&AccessibilityState::Focusable)
                    .copied()
                    .unwrap_or(false)
            }

            _ => false,
        }
    }

    pub fn add_live_region(&mut self, node_id: &str) {
        if !self.live_regions.contains(&node_id.to_string()) {
            self.live_regions.push(node_id.to_string());
        }
    }

    pub fn remove_live_region(&mut self, node_id: &str) {
        self.live_regions.retain(|id| id != node_id);
    }

    pub fn get_live_regions(&self) -> Vec<&SemanticsNode> {
        self.live_regions
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .collect()
    }

    pub fn perform_action(
        &mut self,
        node_id: &str,
        action: AccessibilityAction,
    ) -> Result<(), AccessibilityError> {
        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or(AccessibilityError::NodeNotFound(node_id.to_string()))?;

        // Check if action is supported
        if !node.actions.contains(&action) {
            return Err(AccessibilityError::ActionNotSupported(action));
        }

        // Perform action (this would be handled by the UI engine)
        match action {
            AccessibilityAction::Tap => {
                // Trigger tap event
                node.states.insert(AccessibilityState::Pressed, true);
                // Reset pressed state after a short delay
                node.states.insert(AccessibilityState::Pressed, false);
            }
            AccessibilityAction::Focus => {
                self.set_focus(Some(node_id))?;
            }
            AccessibilityAction::Select => {
                node.states.insert(AccessibilityState::Selected, true);
            }
            AccessibilityAction::Deselect => {
                node.states.insert(AccessibilityState::Selected, false);
            }
            AccessibilityAction::Increment => {
                if let Some(current) = node.properties.get("value") {
                    if let Ok(mut value) = current.parse::<f64>() {
                        if let Some(step) = node.properties.get("step") {
                            if let Ok(step_val) = step.parse::<f64>() {
                                value += step_val;
                                node.properties
                                    .insert("value".to_string(), value.to_string());
                            }
                        }
                    }
                }
            }
            AccessibilityAction::Dismiss => {
                node.states.insert(AccessibilityState::Expanded, false);
            }
            _ => {
                return Err(AccessibilityError::ActionNotImplemented(action));
            }
        }

        Ok(())
    }

    pub fn generate_tree_snapshot(&self) -> SemanticsTreeSnapshot {
        SemanticsTreeSnapshot {
            nodes: self.nodes.clone(),
            root_id: self.root_id.clone(),
            focused_node: self.focused_node.clone(),
            live_regions: self.live_regions.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticsUpdate {
    pub role: Option<AccessibilityRole>,
    pub label: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub value: Option<Option<String>>,
    pub bounds: Option<AccessibilityBounds>,
    pub states: HashMap<AccessibilityState, bool>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticsTreeSnapshot {
    pub nodes: HashMap<String, SemanticsNode>,
    pub root_id: String,
    pub focused_node: Option<String>,
    pub live_regions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationDirection {
    Next,
    Previous,
    Up,
    Down,
    First,
    Last,
}

#[derive(Debug, thiserror::Error)]
pub enum AccessibilityError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Action not supported: {0:?}")]
    ActionNotSupported(AccessibilityAction),

    #[error("Action not implemented: {0:?}")]
    ActionNotImplemented(AccessibilityAction),

    #[error("Invalid bounds")]
    InvalidBounds,

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

// Platform bridge interface
pub trait AccessibilityBridge {
    fn announce(&self, message: &str);
    fn set_focus(&self, node_id: &str);
    fn update_node(&self, node_id: &str, node: &SemanticsNode);
    fn remove_node(&self, node_id: &str);
}

// Web accessibility bridge
pub struct WebAccessibilityBridge {
    // Implementation for web platform using ARIA
}

impl AccessibilityBridge for WebAccessibilityBridge {
    fn announce(&self, _message: &str) {
        // Use aria-live regions for screen readers
        // println!("Announce: {}", message);
    }

    fn set_focus(&self, _node_id: &str) {
        // Set focus on DOM element
        // println!("Set focus: {}", node_id);
    }

    fn update_node(&self, _node_id: &str, _node: &SemanticsNode) {
        // Update DOM element attributes
        // println!("Update node: {}", node_id);
    }

    fn remove_node(&self, _node_id: &str) {
        // Remove DOM element
        // println!("Remove node: {}", node_id);
    }
}

// Desktop accessibility bridge (AT-SPI for Linux, NSAccessibility for macOS, UI Automation for Windows)
#[derive(Debug, Clone)]
pub struct DesktopAccessibilityBridge {
    // Implementation for desktop platforms
}

impl AccessibilityBridge for DesktopAccessibilityBridge {
    fn announce(&self, message: &str) {
        // Use platform-specific accessibility APIs
        println!("Announce: {}", message);
    }

    fn set_focus(&self, node_id: &str) {
        // Use platform-specific focus APIs
        println!("Set focus: {}", node_id);
    }

    fn update_node(&self, node_id: &str, _node: &SemanticsNode) {
        // Update platform accessibility tree
        println!("Update node: {}", node_id);
    }

    fn remove_node(&self, node_id: &str) {
        // Remove from platform accessibility tree
        println!("Remove node: {}", node_id);
    }
}
