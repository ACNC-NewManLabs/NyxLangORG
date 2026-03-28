//! Plugin Loader Module
//!
//! Provides dynamic plugin loading and management for the Nyx compiler.
//! This module enables runtime extension of compiler functionality through
//! loadable plugins using libloading for dynamic symbol resolution.

use std::collections::HashMap;
use std::path::Path;

#[cfg(feature = "dynamic_loading")]
use libloading::{Library, Symbol};

use crate::core::diagnostics::{codes, ErrorCategory, NyxError};

/// Plugin API version - increment when breaking changes are made
pub const PLUGIN_API_VERSION: u32 = 1;

/// Global registry for statically compiled plugins
static STATIC_PLUGINS: std::sync::OnceLock<std::sync::Mutex<HashMap<String, fn() -> Box<dyn Plugin + Send + Sync>>>> = 
    std::sync::OnceLock::new();

/// Get the static plugins registry
fn get_static_plugins() -> &'static std::sync::Mutex<HashMap<String, fn() -> Box<dyn Plugin + Send + Sync>>> {
    STATIC_PLUGINS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Configuration for a plugin
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Unique identifier for the plugin
    pub name: String,
    /// Plugin version string
    pub version: String,
    /// Path to the plugin binary or source
    pub path: String,
    /// Optional configuration parameters
    pub parameters: HashMap<String, String>,
    /// Whether the plugin is enabled
    pub enabled: bool,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: String::from("0.1.0"),
            path: String::new(),
            parameters: HashMap::new(),
            enabled: true,
        }
    }
}

impl PluginConfig {
    /// Create a new plugin configuration
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            version: String::from("0.1.0"),
            parameters: HashMap::new(),
            enabled: true,
        }
    }

    /// Set a configuration parameter
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.parameters.insert(key.into(), value.into());
        self
    }
}

/// Plugin trait that all plugins must implement
///
/// Implement this trait to create a loadable plugin for the Nyx compiler.
/// Plugins can extend compiler passes, runtime backends, target backends,
/// and other extensible components.
pub trait Plugin: Send + Sync {
    /// Returns the unique name of the plugin
    fn name(&self) -> &str;

    /// Returns the version string of the plugin
    fn version(&self) -> &str;

    /// Initialize the plugin with the given configuration
    ///
    /// This method is called when the plugin is first loaded.
    /// Use it to set up any required resources or state.
    fn initialize(&mut self, config: PluginConfig) -> Result<(), NyxError>;

    /// Shutdown the plugin gracefully
    ///
    /// This method is called when the plugin is unloaded or the
    /// compiler is shutting down. Use it to clean up resources.
    fn shutdown(&mut self) -> Result<(), NyxError>;

    /// Get the plugin configuration
    fn get_config(&self) -> &PluginConfig;

    /// Check if the plugin is initialized
    fn is_initialized(&self) -> bool;
}

/// Plugin metadata for discovery
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Plugin author
    pub author: String,
    /// Plugin dependencies
    pub dependencies: Vec<String>,
    /// Compatible Nyx compiler version range
    pub compatible_version: VersionRange,
}

/// Version range for compatibility checking
#[derive(Debug, Clone)]
pub struct VersionRange {
    /// Minimum compatible version
    pub min: Version,
    /// Maximum compatible version (exclusive)
    pub max: Version,
}

impl VersionRange {
    /// Check if a version is within the compatible range
    pub fn contains(&self, version: &Version) -> bool {
        version >= &self.min && version < &self.max
    }
}

/// Version information
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Major version number
    pub major: u16,
    /// Minor version number
    pub minor: u16,
    /// Patch version number
    pub patch: u16,
}

impl Default for Version {
    fn default() -> Self {
        Self {
            major: 0,
            minor: 1,
            patch: 0,
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl std::str::FromStr for Version {
    type Err = NyxError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(NyxError::new(
                codes::INTERNAL_UNKNOWN_ERROR,
                "Invalid version format: expected major.minor.patch",
                ErrorCategory::Internal,
            ));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| NyxError::new(codes::INTERNAL_UNKNOWN_ERROR, "Invalid major version", ErrorCategory::Internal))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| NyxError::new(codes::INTERNAL_UNKNOWN_ERROR, "Invalid minor version", ErrorCategory::Internal))?;
        let patch = parts[2]
            .parse()
            .map_err(|_| NyxError::new(codes::INTERNAL_UNKNOWN_ERROR, "Invalid patch version", ErrorCategory::Internal))?;

        Ok(Self { major, minor, patch })
    }
}

/// Plugin manager for loading and managing plugins
#[allow(dead_code)]
pub struct PluginManager {
    /// Currently loaded plugins indexed by name
    loaded_plugins: HashMap<String, Box<dyn Plugin>>,
    /// Global plugin configuration
    config: PluginConfig,
    /// Plugin search paths
    search_paths: Vec<String>,
    /// Whether the manager is initialized
    initialized: bool,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new() -> Self {
        Self {
            loaded_plugins: HashMap::new(),
            config: PluginConfig::default(),
            search_paths: Vec::new(),
            initialized: false,
        }
    }

    /// Create a plugin manager with the given configuration
    pub fn with_config(config: PluginConfig) -> Self {
        Self {
            loaded_plugins: HashMap::new(),
            config,
            search_paths: Vec::new(),
            initialized: false,
        }
    }

    /// Add a search path for plugins
    pub fn add_search_path(&mut self, path: impl Into<String>) {
        self.search_paths.push(path.into());
    }

    /// Initialize the plugin manager
    pub fn initialize(&mut self) -> Result<(), NyxError> {
        if self.initialized {
            return Err(NyxError::new(
                codes::INTERNAL_UNKNOWN_ERROR,
                "Plugin manager already initialized",
                ErrorCategory::Internal,
            ));
        }

        // Set up default search paths
        if self.search_paths.is_empty() {
            // Add default plugin directories
            if let Ok(current_dir) = std::env::current_dir() {
                self.search_paths.push(current_dir.join("plugins").to_string_lossy().to_string());
            }
            self.search_paths.push("/usr/local/lib/nyx/plugins".to_string());
            self.search_paths.push(format!("{}/.nyx/plugins", std::env::var("HOME").unwrap_or_default()));
        }

        self.initialized = true;
        Ok(())
    }

    /// Load a plugin from the given path
    ///
    /// The path can be:
    /// - A path to a dynamic library (.so, .dll, .dylib)
    /// - A path to a plugin source directory
    /// - A plugin name (will be searched in search paths)
    pub fn load_plugin(&mut self, path: &str) -> Result<(), NyxError> {
        if !self.initialized {
            return Err(NyxError::new(
                codes::INTERNAL_UNKNOWN_ERROR,
                "Plugin manager not initialized",
                ErrorCategory::Internal,
            ));
        }

        // Validate path
        let plugin_path = Path::new(path);
        if !plugin_path.exists() {
            return Err(NyxError::new(
                codes::IO_FILE_NOT_FOUND,
                format!("Plugin not found: {}", path),
                ErrorCategory::Io,
            ));
        }

        let plugin_name = plugin_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        // Check if already loaded
        if self.loaded_plugins.contains_key(plugin_name) {
            return Err(NyxError::new(
                codes::INTERNAL_UNKNOWN_ERROR,
                format!("Plugin already loaded: {}", plugin_name),
                ErrorCategory::Internal,
            ));
        }

        let config = PluginConfig::new(plugin_name, path);
        
        // Attempt to load the dynamic library
        #[cfg(feature = "dynamic_loading")]
        let mut dynamic_load_error: Option<NyxError> = None;
        #[cfg(not(feature = "dynamic_loading"))]
        let dynamic_load_error: Option<NyxError> = None;
        #[cfg(feature = "dynamic_loading")]
        {
            match Library::new(path) {
                Ok(library) => {
                    // Try to load the plugin_create symbol
                    unsafe {
                        let create_symbol: Result<Symbol<extern "C" fn(PluginConfig) -> Box<dyn Plugin>>, _> = 
                            library.get(b"plugin_create");
                        
                        if let Ok(create_fn) = create_symbol {
                            let plugin = create_fn(config);
                            let name = plugin.name().to_string();
                            self.loaded_plugins.insert(name, plugin);
                            log::info!("Loaded plugin: {} from {}", plugin_name, path);
                            return Ok(());
                        }
                        
                        // Try alternative symbol name
                        let create_symbol2: Result<Symbol<extern "C" fn() -> Box<dyn Plugin + Send + Sync>>, _> = 
                            library.get(b"nyx_plugin_create");
                        
                        if let Ok(create_fn) = create_symbol2 {
                            let mut plugin = create_fn();
                            plugin.initialize(config.clone())?;
                            let name = plugin.name().to_string();
                            self.loaded_plugins.insert(name, plugin);
                            log::info!("Loaded plugin: {} from {}", plugin_name, path);
                            return Ok(());
                        }
                    }
                    
                    dynamic_load_error = Some(NyxError::new(
                        codes::PLUGIN_NOT_FOUND,
                        format!(
                            "Plugin '{}' was found at {} but did not export 'plugin_create' or 'nyx_plugin_create'.",
                            plugin_name, path
                        ),
                        ErrorCategory::Extension,
                    ));
                }
                Err(e) => {
                    log::warn!("Failed to load library {}: {}. Using static plugin registration.", path, e);
                }
            }
        }

        // Try to find and load from registered static plugins
        // This allows plugins to be compiled-in instead of dynamically loaded
        if let Some(plugin_creator) = self.find_static_plugin(plugin_name) {
            let mut plugin = (plugin_creator)();
            plugin.initialize(config)?;
            let name = plugin.name().to_string();
            self.loaded_plugins.insert(name, plugin);
            log::info!("Loaded static plugin: {}", plugin_name);
            return Ok(());
        }

        if let Some(err) = dynamic_load_error {
            return Err(err);
        }

        // No plugin found - return error with helpful message
        Err(NyxError::new(
            codes::PLUGIN_NOT_FOUND,
            format!("Plugin '{}' could not be loaded from {}. Ensure the plugin is compiled as a shared library with 'plugin_create' or 'nyx_plugin_create' export.", plugin_name, path),
            ErrorCategory::Extension,
        ))
    }

    /// Find a statically registered plugin by name
    fn find_static_plugin(&self, name: &str) -> Option<fn() -> Box<dyn Plugin + Send + Sync>> {
        // Check the static plugin registry
        get_static_plugins().lock().ok()?.get(name).copied()
    }

    /// Register a static plugin (for compiled-in plugins)
    pub fn register_static_plugin(&mut self, name: &str, creator: fn() -> Box<dyn Plugin + Send + Sync>) -> Result<(), NyxError> {
        if let Ok(mut plugins) = get_static_plugins().lock() {
            plugins.insert(name.to_string(), creator);
        }
        log::info!("Registered static plugin: {}", name);
        Ok(())
    }

    /// Get plugin API version
    pub fn get_api_version(&self) -> u32 {
        PLUGIN_API_VERSION
    }

    /// Load a plugin with the given configuration
    pub fn load_plugin_with_config(&mut self, config: PluginConfig) -> Result<(), NyxError> {
        self.load_plugin(&config.path)
    }

    /// Unload a plugin by name
    pub fn unload_plugin(&mut self, name: &str) -> Result<(), NyxError> {
        if !self.initialized {
            return Err(NyxError::new(
                codes::INTERNAL_UNKNOWN_ERROR,
                "Plugin manager not initialized",
                ErrorCategory::Internal,
            ));
        }

        // Find and remove the plugin
        let mut plugin = self.loaded_plugins.remove(name)
            .ok_or_else(|| NyxError::new(
                codes::INTERNAL_UNKNOWN_ERROR,
                format!("Plugin not found: {}", name),
                ErrorCategory::Internal,
            ))?;

        // Call shutdown to clean up
        plugin.shutdown()?;

        log::debug!("Unloaded plugin: {}", name);
        Ok(())
    }

    /// Get a reference to a loaded plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<&dyn Plugin> {
        self.loaded_plugins.get(name).map(|p| p.as_ref() as &dyn Plugin)
    }

    /// Get a mutable reference to a loaded plugin by name
    /// Get a mutable reference to a loaded plugin by name
    /// Note: This returns a reference with the same lifetime as self
    pub fn get_plugin_mut(&mut self, name: &str) -> Option<&mut dyn Plugin> {
        self.loaded_plugins.get_mut(name).map(|p| p.as_mut() as &mut dyn Plugin)
    }

    /// List all loaded plugin names
    pub fn list_plugins(&self) -> Vec<String> {
        self.loaded_plugins.keys().cloned().collect()
    }

    /// Get the number of loaded plugins
    pub fn plugin_count(&self) -> usize {
        self.loaded_plugins.len()
    }

    /// Check if a plugin is loaded
    pub fn is_plugin_loaded(&self, name: &str) -> bool {
        self.loaded_plugins.contains_key(name)
    }

    /// Get all loaded plugins
    pub fn get_all_plugins(&self) -> Vec<&dyn Plugin> {
        self.loaded_plugins.values().map(|p| p.as_ref() as &dyn Plugin).collect()
    }

    /// Reload a plugin (unload and load again)
    pub fn reload_plugin(&mut self, name: &str) -> Result<(), NyxError> {
        // Get the path from the existing plugin config
        let config = self.get_plugin(name)
            .ok_or_else(|| NyxError::new(
                codes::INTERNAL_UNKNOWN_ERROR,
                format!("Plugin not found: {}", name),
                ErrorCategory::Internal,
            ))?
            .get_config()
            .clone();

        // Unload the plugin
        self.unload_plugin(name)?;

        // Load again with original config
        self.load_plugin_with_config(config)
    }

    /// Shutdown all plugins and the manager
    pub fn shutdown(&mut self) -> Result<(), NyxError> {
        // Unload all plugins in reverse order
        let plugin_names: Vec<String> = self.loaded_plugins.keys().cloned().collect();
        for name in plugin_names {
            if let Err(e) = self.unload_plugin(&name) {
                log::warn!("Failed to unload plugin {}: {}", name, e);
            }
        }

        self.initialized = false;
        Ok(())
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        if self.initialized {
            let _ = self.shutdown();
        }
    }
}

/// Builder for creating plugin configurations
pub struct PluginConfigBuilder {
    config: PluginConfig,
}

impl PluginConfigBuilder {
    /// Create a new config builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            config: PluginConfig::new(name, ""),
        }
    }

    /// Set the plugin path
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.config.path = path.into();
        self
    }

    /// Set the plugin version
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.config.version = version.into();
        self
    }

    /// Add a parameter
    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.parameters.insert(key.into(), value.into());
        self
    }

    /// Set enabled state
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Build the configuration
    pub fn build(self) -> PluginConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        let v1 = Version { major: 1, minor: 0, patch: 0 };
        let v2 = Version { major: 1, minor: 0, patch: 1 };
        let v3 = Version { major: 2, minor: 0, patch: 0 };

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }

    #[test]
    fn test_version_parse() {
        let v: Version = "1.2.3".parse().unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_plugin_manager() {
        let mut manager = PluginManager::new();
        assert!(manager.initialize().is_ok());
        assert_eq!(manager.plugin_count(), 0);
        assert!(manager.list_plugins().is_empty());
    }

    #[test]
    fn test_plugin_config_builder() {
        let config = PluginConfigBuilder::new("test_plugin")
            .path("/path/to/plugin")
            .version("1.0.0")
            .param("option", "value")
            .enabled(true)
            .build();

        assert_eq!(config.name, "test_plugin");
        assert_eq!(config.path, "/path/to/plugin");
        assert_eq!(config.version, "1.0.0");
        assert_eq!(config.parameters.get("option"), Some(&"value".to_string()));
        assert!(config.enabled);
    }
}
