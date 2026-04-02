//! Deprecation System Module
//!
//! Provides tracking and warnings for deprecated APIs in the Nyx core.
//! This module enables systematic tracking of API deprecations with
//! automatic warning generation and migration suggestions.

use std::collections::HashMap;
use std::fmt;

use crate::core::version::Version;

/// A deprecation notice for an API
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeprecationNotice {
    /// Version this API was deprecated in
    pub deprecated_in: Version,
    /// Version this API will be removed in (if known)
    pub removed_in: Option<Version>,
    /// Human-readable message about the deprecation
    pub message: String,
    /// Suggested alternative API (if any)
    pub suggestion: Option<String>,
}

impl DeprecationNotice {
    /// Create a new deprecation notice
    pub fn new(deprecated_in: Version, message: impl Into<String>) -> Self {
        Self {
            deprecated_in,
            removed_in: None,
            message: message.into(),
            suggestion: None,
        }
    }

    /// Create with a removal version
    pub fn with_removal(mut self, version: Version) -> Self {
        self.removed_in = Some(version);
        self
    }

    /// Create with a suggestion
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Check if this API should be removed in the given version
    pub fn should_remove(&self, current_version: &Version) -> bool {
        if let Some(removed_in) = &self.removed_in {
            current_version >= removed_in
        } else {
            false
        }
    }

    /// Get the warning level for this deprecation
    pub fn warning_level(&self) -> DeprecationWarningLevel {
        if self.removed_in.is_some() {
            DeprecationWarningLevel::Critical
        } else {
            DeprecationWarningLevel::Normal
        }
    }
}

impl fmt::Display for DeprecationNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Deprecated in v{}: {}", self.deprecated_in, self.message)?;
        
        if let Some(removed_in) = &self.removed_in {
            write!(f, " (will be removed in v{})", removed_in)?;
        }
        
        if let Some(suggestion) = &self.suggestion {
            write!(f, ". Use {} instead", suggestion)?;
        }
        
        Ok(())
    }
}

/// Warning level for deprecations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeprecationWarningLevel {
    /// Normal deprecation warning
    Normal,
    /// Critical deprecation - API will be removed soon
    Critical,
}

impl DeprecationWarningLevel {
    /// Get the prefix for this warning level
    pub fn prefix(&self) -> &'static str {
        match self {
            DeprecationWarningLevel::Normal => "WARNING",
            DeprecationWarningLevel::Critical => "CRITICAL",
        }
    }
}

/// Tracker for API deprecations
///
/// Maintains a registry of deprecated APIs and provides warnings
/// when deprecated APIs are used.
#[derive(Debug, Default)]
pub struct DeprecationTracker {
    /// Map of API names to their deprecation notices
    notices: HashMap<String, DeprecationNotice>,
    /// History of all deprecation warnings issued
    warning_history: Vec<DeprecationWarningRecord>,
    /// Whether to track warning history
    track_history: bool,
}

impl DeprecationTracker {
    /// Create a new deprecation tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new tracker with history tracking enabled
    pub fn with_history() -> Self {
        Self {
            notices: HashMap::new(),
            warning_history: Vec::new(),
            track_history: true,
        }
    }

    /// Register a new deprecation notice for an API
    ///
    /// # Example
    /// ```
    /// use nyx::core::deprecation::{DeprecationTracker, DeprecationNotice};
    /// use nyx::core::version::Version;
    ///
    /// let mut tracker = DeprecationTracker::new();
    /// tracker.register(
    ///     "old_function".to_string(),
    ///     DeprecationNotice::new(Version::new(1, 5, 0), "Use new_function instead")
    /// );
    /// ```
    pub fn register(&mut self, api_name: String, notice: DeprecationNotice) {
        self.notices.insert(api_name, notice);
    }

    /// Register a deprecation with a removal version and suggestion
    pub fn register_full(
        &mut self,
        api_name: String,
        deprecated_in: Version,
        removed_in: Version,
        message: impl Into<String>,
        suggestion: Option<String>,
    ) {
        let notice = DeprecationNotice::new(deprecated_in, message)
            .with_removal(removed_in);
        
        let notice = if let Some(suggestion) = suggestion {
            notice.with_suggestion(suggestion)
        } else {
            notice
        };
        
        self.notices.insert(api_name, notice);
    }

    /// Check if an API has been deprecated
    pub fn is_deprecated(&self, api_name: &str) -> bool {
        self.notices.contains_key(api_name)
    }

    /// Get the deprecation notice for an API
    pub fn get_notice(&self, api_name: &str) -> Option<&DeprecationNotice> {
        self.notices.get(api_name)
    }

    /// Warn about using a deprecated API
    ///
    /// Returns the deprecation notice if the API is deprecated,
    /// or None if the API is not in the registry.
    ///
    /// # Example
    /// ```
    /// use nyx::core::deprecation::DeprecationTracker;
    /// use nyx::core::version::Version;
    ///
    /// let tracker = DeprecationTracker::new();
    /// if let Some(notice) = tracker.warn("old_api") {
    ///     eprintln!("{}", notice);
    /// }
    /// ```
    pub fn warn(&self, api_name: &str) -> Option<DeprecationNotice> {
        self.notices.get(api_name).cloned()
    }

    /// Warn about using a deprecated API and record in history
    pub fn warn_with_history(&mut self, api_name: &str, current_version: Version) -> Option<DeprecationNotice> {
        let notice = self.warn(api_name)?;
        
        if self.track_history {
            self.warning_history.push(DeprecationWarningRecord {
                api_name: api_name.to_string(),
                version: current_version,
                notice: notice.clone(),
            });
        }
        
        Some(notice)
    }

    /// Get all registered deprecation notices
    pub fn all_notices(&self) -> &HashMap<String, DeprecationNotice> {
        &self.notices
    }

    /// Get the count of registered deprecations
    pub fn deprecation_count(&self) -> usize {
        self.notices.len()
    }

    /// Get warning history
    pub fn warning_history(&self) -> &[DeprecationWarningRecord] {
        &self.warning_history
    }

    /// Clear all warning history
    pub fn clear_history(&mut self) {
        self.warning_history.clear();
    }

    /// Check if any deprecations have critical warnings (will be removed soon)
    pub fn has_critical_warnings(&self, current_version: &Version) -> bool {
        self.notices.values().any(|n| {
            n.warning_level() == DeprecationWarningLevel::Critical && 
            n.should_remove(current_version)
        })
    }

    /// Get all APIs deprecated in a specific version
    pub fn deprecated_in(&self, version: &Version) -> Vec<&String> {
        self.notices
            .iter()
            .filter(|(_, notice)| &notice.deprecated_in == version)
            .map(|(name, _)| name)
            .collect()
    }

    /// Get all APIs to be removed in or before a specific version
    pub fn removed_in_or_before(&self, version: &Version) -> Vec<&String> {
        self.notices
            .iter()
            .filter(|(_, notice)| {
                if let Some(removed_in) = &notice.removed_in {
                    removed_in <= version
                } else {
                    false
                }
            })
            .map(|(name, _)| name)
            .collect()
    }

    /// Remove a deprecation notice (e.g., after API is removed)
    pub fn unregister(&mut self, api_name: &str) -> Option<DeprecationNotice> {
        self.notices.remove(api_name)
    }
}

/// Record of a deprecation warning being issued
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeprecationWarningRecord {
    /// Name of the API that triggered the warning
    pub api_name: String,
    /// Current version when warning was issued
    pub version: Version,
    /// The deprecation notice
    pub notice: DeprecationNotice,
}

/// Global deprecation tracker instance
///
/// Use this for tracking deprecations across the entire codebase.
/// Note: This is a placeholder - in production, you'd use a lazy_static or similar
pub static GLOBAL_DEPRECATION_TRACKER: std::sync::OnceLock<DeprecationTracker> = std::sync::OnceLock::new();

/// Initialize the global deprecation tracker with known deprecations
pub fn init_global_tracker() -> &'static DeprecationTracker {
    GLOBAL_DEPRECATION_TRACKER.get_or_init(|| {
        
        
        // Register known deprecations here
        // Example:
        // tracker.register_full(
        //     "legacy_parser".to_string(),
        //     Version::new(0, 5, 0),
        //     Version::new(1, 0, 0),
        //     "Use the new neuro_parser instead",
        //     Some("nyx::core::parser::neuro_parser".to_string())
        // );
        
        DeprecationTracker::new()
    })
}

/// Trait for types that can track their own deprecation
pub trait Deprecatable {
    /// Get the deprecation notice for this type
    fn deprecation_notice(&self) -> Option<DeprecationNotice>;
    
    /// Check if this type is deprecated
    fn is_deprecated(&self) -> bool {
        self.deprecation_notice().is_some()
    }
}

/// Helper to emit a deprecation warning at compile time
///
/// This can be used to create compile-time deprecation warnings
/// when deprecated items are used.
#[macro_export]
macro_rules! deprecated {
    ($msg:expr) => {
        #[deprecated(since = "0.1.0", note = $msg)]
    };
    
    ($msg:expr, $removed:expr) => {
        #[deprecated(since = $removed, note = $msg)]
    };
}

/// Helper to emit a deprecation warning with a suggestion
#[macro_export]
macro_rules! deprecated_with_suggestion {
    ($msg:expr, $suggestion:expr) => {
        #[deprecated(since = "0.1.0", note = concat!($msg, ". Use ", $suggestion, " instead"))]
    };
}

/// Attribute macro for stable APIs
///
/// Use this to mark APIs with their stability level and version.
#[macro_export]
macro_rules! stability {
    (stable, $version:expr) => {
        #[stable(since = $version)]
    };
    
    (beta, $version:expr) => {
        #[unstable(feature = "beta_api", issue = "none")]
    };
    
    (experimental, $version:expr) => {
        #[unstable(feature = "experimental_api", issue = "none")]
    };
}

/// Function to check deprecation and emit warning
///
/// # Example
/// ```
/// use nyx::core::deprecation::check_deprecated;
/// use nyx::core::version::Version;
///
/// check_deprecated("my_api", &Version::new(1, 5, 0));
/// ```
pub fn check_deprecated(api_name: &str, current_version: &Version) -> Option<DeprecationNotice> {
    let tracker = init_global_tracker();
    tracker.warn(api_name)
        .filter(|notice| current_version >= &notice.deprecated_in)
}

/// Function to register a deprecation globally
pub fn register_deprecation(api_name: String, notice: DeprecationNotice) {
    // Note: Cannot mutate lazy_static directly, need to use a different pattern
    // This function is provided for API consistency
    let _ = (api_name, notice);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deprecation_notice_creation() {
        let notice = DeprecationNotice::new(
            Version::new(1, 5, 0),
            "This API is deprecated"
        );
        
        assert_eq!(notice.deprecated_in, Version::new(1, 5, 0));
        assert!(notice.removed_in.is_none());
        assert!(notice.suggestion.is_none());
    }

    #[test]
    fn test_deprecation_notice_with_options() {
        let notice = DeprecationNotice::new(Version::new(1, 5, 0), "Old API")
            .with_removal(Version::new(2, 0, 0))
            .with_suggestion("new_api");
        
        assert_eq!(notice.removed_in, Some(Version::new(2, 0, 0)));
        assert_eq!(notice.suggestion, Some("new_api".to_string()));
        assert_eq!(notice.warning_level(), DeprecationWarningLevel::Critical);
    }

    #[test]
    fn test_deprecation_tracker_register() {
        let mut tracker = DeprecationTracker::new();
        
        tracker.register(
            "old_api".to_string(),
            DeprecationNotice::new(Version::new(1, 0, 0), "Use new_api")
        );
        
        assert!(tracker.is_deprecated("old_api"));
        assert!(!tracker.is_deprecated("new_api"));
    }

    #[test]
    fn test_deprecation_tracker_warn() {
        let mut tracker = DeprecationTracker::new();
        
        tracker.register(
            "old_api".to_string(),
            DeprecationNotice::new(Version::new(1, 0, 0), "Use new_api")
        );
        
        let notice = tracker.warn("old_api");
        assert!(notice.is_some());
        
        let notice = tracker.warn("new_api");
        assert!(notice.is_none());
    }

    #[test]
    fn test_deprecation_tracker_warning_history() {
        let mut tracker = DeprecationTracker::with_history();
        
        tracker.register(
            "old_api".to_string(),
            DeprecationNotice::new(Version::new(1, 0, 0), "Use new_api")
        );
        
        tracker.warn_with_history("old_api", Version::new(1, 5, 0));
        
        assert_eq!(tracker.warning_history().len(), 1);
        
        tracker.warn_with_history("old_api", Version::new(1, 6, 0));
        
        assert_eq!(tracker.warning_history().len(), 2);
    }

    #[test]
    fn test_deprecation_notice_display() {
        let notice = DeprecationNotice::new(Version::new(1, 5, 0), "Old API")
            .with_removal(Version::new(2, 0, 0))
            .with_suggestion("new_api");
        
        let display = notice.to_string();
        assert!(display.contains("Deprecated"));
        assert!(display.contains("1.5.0"));
        assert!(display.contains("2.0.0"));
        assert!(display.contains("new_api"));
    }

    #[test]
    fn test_should_remove() {
        let notice = DeprecationNotice::new(Version::new(1, 0, 0), "Old API")
            .with_removal(Version::new(2, 0, 0));
        
        assert!(!notice.should_remove(&Version::new(1, 5, 0)));
        assert!(!notice.should_remove(&Version::new(1, 9, 0)));
        assert!(notice.should_remove(&Version::new(2, 0, 0)));
        assert!(notice.should_remove(&Version::new(2, 5, 0)));
    }

    #[test]
    fn test_deprecated_in_query() {
        let mut tracker = DeprecationTracker::new();
        
        tracker.register(
            "api1".to_string(),
            DeprecationNotice::new(Version::new(1, 0, 0), "API 1")
        );
        
        tracker.register(
            "api2".to_string(),
            DeprecationNotice::new(Version::new(1, 0, 0), "API 2")
        );
        
        tracker.register(
            "api3".to_string(),
            DeprecationNotice::new(Version::new(2, 0, 0), "API 3")
        );
        
        let deprecated_in_v1 = tracker.deprecated_in(&Version::new(1, 0, 0));
        assert_eq!(deprecated_in_v1.len(), 2);
    }
}
