//! Stability Guarantees Module
//!
//! Provides stability levels and API guarantees for the Nyx core.
//! This module defines the stability contract for public APIs and
//! helps maintain backward compatibility across versions.

use std::fmt;

use crate::core::diagnostics::{codes, ErrorCategory, NyxError};
use crate::core::version::{CompatibilityPolicy, Version};

/// Stability level for API elements
///
/// APIs progress through these stability levels over time:
/// - Experimental → Beta → Stable → Deprecated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StabilityLevel {
    /// Stable API - safe to use, guaranteed backward compatibility
    #[default]
    Stable,
    /// Beta API - mostly stable, may have minor changes
    Beta,
    /// Experimental API - subject to change, use with caution
    Experimental,
    /// Deprecated API - use is discouraged, may be removed in future
    Deprecated,
}

impl StabilityLevel {
    /// Check if this stability level represents a stable API
    pub fn is_stable(&self) -> bool {
        matches!(self, StabilityLevel::Stable)
    }

    /// Check if this stability level is still supported
    pub fn is_supported(&self) -> bool {
        !matches!(self, StabilityLevel::Deprecated)
    }

    /// Get a human-readable description of this stability level
    pub fn description(&self) -> &'static str {
        match self {
            StabilityLevel::Stable => "This API is stable and guaranteed to be supported.",
            StabilityLevel::Beta => {
                "This API is in beta and may have minor changes before stable release."
            }
            StabilityLevel::Experimental => {
                "This API is experimental and subject to breaking changes."
            }
            StabilityLevel::Deprecated => {
                "This API is deprecated and will be removed in a future version."
            }
        }
    }

    /// Get the display name for this stability level
    pub fn display_name(&self) -> &'static str {
        match self {
            StabilityLevel::Stable => "stable",
            StabilityLevel::Beta => "beta",
            StabilityLevel::Experimental => "experimental",
            StabilityLevel::Deprecated => "deprecated",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "stable" => Some(StabilityLevel::Stable),
            "beta" => Some(StabilityLevel::Beta),
            "experimental" => Some(StabilityLevel::Experimental),
            "deprecated" => Some(StabilityLevel::Deprecated),
            _ => None,
        }
    }
}

impl fmt::Display for StabilityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Trait for types that provide stability guarantees
///
/// Implement this trait on public API types to declare their
/// stability level and version information.
pub trait StableAPI {
    /// Returns the stability level of this API
    fn stability_level(&self) -> StabilityLevel;

    /// Returns the version this API was introduced
    fn api_version(&self) -> Version;

    /// Returns the version this API was deprecated (if applicable)
    fn deprecated_since(&self) -> Option<Version> {
        None
    }

    /// Returns the version this API will be removed (if deprecated)
    fn removed_in(&self) -> Option<Version> {
        None
    }

    /// Check if this API is deprecated
    fn is_deprecated(&self) -> bool {
        matches!(self.stability_level(), StabilityLevel::Deprecated)
    }

    /// Check if this API is stable
    fn is_stable(&self) -> bool {
        self.stability_level().is_stable()
    }

    /// Get a stability report for this API
    fn stability_report(&self) -> StabilityReport {
        StabilityReport {
            level: self.stability_level(),
            introduced_in: self.api_version(),
            deprecated_in: self.deprecated_since(),
            removed_in: self.removed_in(),
        }
    }
}

/// Stability report for an API element
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StabilityReport {
    /// Current stability level
    pub level: StabilityLevel,
    /// Version this API was introduced
    pub introduced_in: Version,
    /// Version this API was deprecated (if applicable)
    pub deprecated_in: Option<Version>,
    /// Version this API will be removed (if deprecated)
    pub removed_in: Option<Version>,
}

impl StabilityReport {
    /// Create a new stability report
    pub fn new(level: StabilityLevel, introduced_in: Version) -> Self {
        Self {
            level,
            introduced_in,
            deprecated_in: None,
            removed_in: None,
        }
    }

    /// Check if using this API will trigger a deprecation warning
    pub fn has_deprecation_warning(&self, current_version: &Version) -> bool {
        if let Some(deprecated_in) = &self.deprecated_in {
            current_version >= deprecated_in
        } else {
            false
        }
    }

    /// Check if this API should not be used (removed)
    pub fn is_removed(&self, current_version: &Version) -> bool {
        if let Some(removed_in) = &self.removed_in {
            current_version >= removed_in
        } else {
            false
        }
    }
}

impl fmt::Display for StabilityReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (since v{})", self.level, self.introduced_in)?;

        if let Some(deprecated_in) = &self.deprecated_in {
            write!(f, ", deprecated in v{}", deprecated_in)?;
        }

        if let Some(removed_in) = &self.removed_in {
            write!(f, ", removed in v{}", removed_in)?;
        }

        Ok(())
    }
}

/// API deprecation warning
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeprecationWarning {
    /// Name of the deprecated API
    pub api_name: String,
    /// Version it was deprecated in
    pub deprecated_in: Version,
    /// Suggested alternative (if any)
    pub suggestion: Option<String>,
    /// Version it will be removed in
    pub removed_in: Option<Version>,
}

impl DeprecationWarning {
    /// Create a new deprecation warning
    pub fn new(api_name: String, deprecated_in: Version) -> Self {
        Self {
            api_name,
            deprecated_in,
            suggestion: None,
            removed_in: None,
        }
    }

    /// Create with a suggestion
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Create with removal version
    pub fn with_removal_version(mut self, version: Version) -> Self {
        self.removed_in = Some(version);
        self
    }

    /// Get the warning message
    pub fn message(&self) -> String {
        let mut msg = format!(
            "API '{}' is deprecated since version {}",
            self.api_name, self.deprecated_in
        );

        if let Some(removed_in) = &self.removed_in {
            msg.push_str(&format!(" and will be removed in version {}", removed_in));
        }

        if let Some(suggestion) = &self.suggestion {
            msg.push_str(&format!(". Use {} instead", suggestion));
        }

        msg
    }
}

/// Stability policy for the entire crate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StabilityPolicy {
    /// Minimum stability level allowed for public APIs
    pub minimum_stability: StabilityLevel,
    /// Whether to allow experimental features
    pub allow_experimental: bool,
    /// Default compatibility policy for version checks
    pub compatibility_policy: CompatibilityPolicy,
}

impl StabilityPolicy {
    /// Create a new stability policy
    pub fn new(minimum_stability: StabilityLevel) -> Self {
        Self {
            minimum_stability,
            allow_experimental: false,
            compatibility_policy: CompatibilityPolicy::default(),
        }
    }

    /// Create a permissive policy allowing experimental features
    pub fn permissive() -> Self {
        Self {
            minimum_stability: StabilityLevel::Experimental,
            allow_experimental: true,
            compatibility_policy: CompatibilityPolicy::Lenient,
        }
    }

    /// Create a strict policy for production use
    pub fn strict() -> Self {
        Self {
            minimum_stability: StabilityLevel::Stable,
            allow_experimental: false,
            compatibility_policy: CompatibilityPolicy::Strict,
        }
    }

    /// Check if an API meets the stability requirements
    pub fn meets_requirements(&self, level: StabilityLevel) -> bool {
        match self.minimum_stability {
            StabilityLevel::Stable => level == StabilityLevel::Stable,
            StabilityLevel::Beta => {
                matches!(level, StabilityLevel::Stable | StabilityLevel::Beta)
            }
            StabilityLevel::Experimental => true,
            StabilityLevel::Deprecated => true, // Deprecated still works but warns
        }
    }

    /// Check if experimental features are allowed
    pub fn allows_experimental(&self) -> bool {
        self.allow_experimental
    }
}

impl fmt::Display for StabilityPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "minimum: {}, experimental: {}, policy: {}",
            self.minimum_stability, self.allow_experimental, self.compatibility_policy
        )
    }
}

/// Helper macro to implement StableAPI with const version
#[macro_export]
macro_rules! impl_stable_api {
    ($type:ty, $level:expr, $version:expr) => {
        impl $crate::core::stability::StableAPI for $type {
            fn stability_level(&self) -> $crate::core::stability::StabilityLevel {
                $level
            }

            fn api_version(&self) -> $crate::core::version::Version {
                $version
            }
        }
    };
}

/// Helper macro to implement StableAPI with deprecation info
#[macro_export]
macro_rules! impl_stable_api_deprecated {
    ($type:ty, $level:expr, $version:expr, $deprecated:expr, $removed:expr) => {
        impl $crate::core::stability::StableAPI for $type {
            fn stability_level(&self) -> $crate::core::stability::StabilityLevel {
                $level
            }

            fn api_version(&self) -> $crate::core::version::Version {
                $version
            }

            fn deprecated_since(&self) -> Option<$crate::core::version::Version> {
                Some($deprecated)
            }

            fn removed_in(&self) -> Option<$crate::core::version::Version> {
                Some($removed)
            }
        }
    };
}

/// Extension trait for Result types to add stability checking
pub trait ResultStabilityExt<T, E> {
    /// Map error to stability-related error
    fn map_stability_error(self, api_name: &str, version: Version) -> Result<T, NyxError>;
}

impl<T, E> ResultStabilityExt<T, E> for Result<T, E> {
    fn map_stability_error(self, api_name: &str, version: Version) -> Result<T, NyxError> {
        self.map_err(|_| {
            NyxError::new(
                codes::API_STABILITY_ERROR,
                format!("API '{}' is not available in version {}", api_name, version),
                ErrorCategory::Internal,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stability_level_display() {
        assert_eq!(StabilityLevel::Stable.display_name(), "stable");
        assert_eq!(StabilityLevel::Beta.display_name(), "beta");
        assert_eq!(StabilityLevel::Experimental.display_name(), "experimental");
        assert_eq!(StabilityLevel::Deprecated.display_name(), "deprecated");
    }

    #[test]
    fn test_stability_level_parse() {
        assert_eq!(
            StabilityLevel::from_str("stable"),
            Some(StabilityLevel::Stable)
        );
        assert_eq!(StabilityLevel::from_str("beta"), Some(StabilityLevel::Beta));
        assert_eq!(
            StabilityLevel::from_str("experimental"),
            Some(StabilityLevel::Experimental)
        );
        assert_eq!(
            StabilityLevel::from_str("deprecated"),
            Some(StabilityLevel::Deprecated)
        );
        assert_eq!(StabilityLevel::from_str("unknown"), None);
    }

    #[test]
    fn test_stability_level_checks() {
        assert!(StabilityLevel::Stable.is_stable());
        assert!(!StabilityLevel::Beta.is_stable());
        assert!(!StabilityLevel::Experimental.is_stable());

        assert!(StabilityLevel::Stable.is_supported());
        assert!(StabilityLevel::Beta.is_supported());
        assert!(StabilityLevel::Experimental.is_supported());
        assert!(!StabilityLevel::Deprecated.is_supported());
    }

    #[test]
    fn test_stability_report() {
        let report = StabilityReport::new(StabilityLevel::Beta, Version::new(1, 2, 0));

        assert_eq!(report.level, StabilityLevel::Beta);
        assert_eq!(report.introduced_in, Version::new(1, 2, 0));
        assert!(report.deprecated_in.is_none());
    }

    #[test]
    fn test_stability_policy() {
        let strict = StabilityPolicy::strict();
        assert!(strict.meets_requirements(StabilityLevel::Stable));
        assert!(!strict.meets_requirements(StabilityLevel::Beta));
        assert!(!strict.meets_requirements(StabilityLevel::Experimental));

        let permissive = StabilityPolicy::permissive();
        assert!(permissive.meets_requirements(StabilityLevel::Stable));
        assert!(permissive.meets_requirements(StabilityLevel::Beta));
        assert!(permissive.meets_requirements(StabilityLevel::Experimental));
    }

    #[test]
    fn test_deprecation_warning() {
        let warning = DeprecationWarning::new("old_api".to_string(), Version::new(1, 5, 0))
            .with_suggestion("new_api")
            .with_removal_version(Version::new(2, 0, 0));

        assert_eq!(warning.api_name, "old_api");
        assert!(warning.suggestion.is_some());
        assert!(warning.removed_in.is_some());

        let msg = warning.message();
        assert!(msg.contains("deprecated"));
        assert!(msg.contains("old_api"));
        assert!(msg.contains("new_api"));
    }
}
