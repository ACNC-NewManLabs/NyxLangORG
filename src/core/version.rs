//! Version Module
//!
//! Provides comprehensive semantic versioning support for the Nyx core.
//! This module implements version parsing, comparison, and compatibility checking
//! following Semantic Versioning 2.0.0 specification (https://semver.org/).

use std::cmp::Ordering;
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

use crate::core::diagnostics::{codes, ErrorCategory, NyxError};

/// Semantic versioning implementation following SemVer 2.0.0
///
/// # Example
/// ```
/// use nyx::core::version::Version;
///
/// let version = Version::new(1, 2, 3);
/// assert_eq!(version.major, 1);
/// assert_eq!(version.minor, 2);
/// assert_eq!(version.patch, 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Version {
    /// Major version number - breaking changes
    pub major: u16,
    /// Minor version number - new features (backward compatible)
    pub minor: u16,
    /// Patch version number - bug fixes (backward compatible)
    pub patch: u16,
    /// Pre-release identifier (e.g., "alpha.1", "beta.2", "rc.1")
    pub pre: Option<String>,
    /// Build metadata (e.g., "202303151200", "build.123")
    pub build: Option<String>,
}

impl Version {
    /// Create a new version with major, minor, and patch
    ///
    /// # Example
    /// ```
    /// use nyx::core::version::Version;
    ///
    /// let v = Version::new(2, 0, 0);
    /// ```
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: None,
            build: None,
        }
    }

    /// Create a new version with pre-release identifier
    ///
    /// # Example
    /// ```
    /// use nyx::core::version::Version;
    ///
    /// let v = Version::new_prerelease(1, 0, 0, "alpha.1");
    /// ```
    pub fn new_prerelease(major: u16, minor: u16, patch: u16, pre: impl Into<String>) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: Some(pre.into()),
            build: None,
        }
    }

    /// Create a new version with build metadata
    ///
    /// # Example
    /// ```
    /// use nyx::core::version::Version;
    ///
    /// let v = Version::new_with_build(1, 0, 0, "build.123");
    /// ```
    pub fn new_with_build(major: u16, minor: u16, patch: u16, build: impl Into<String>) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: None,
            build: Some(build.into()),
        }
    }

    /// Create a version from major.minor.patch string
    ///
    /// # Errors
    /// Returns an error if the version string is invalid
    ///
    /// # Example
    /// ```
    /// use nyx::core::version::Version;
    ///
    /// let v = Version::parse("1.2.3").unwrap();
    /// assert_eq!(v.major, 1);
    /// ```
    pub fn parse(version: &str) -> Result<Self, NyxError> {
        Self::from_str(version)
    }

    /// Check if this version is a pre-release version
    pub fn is_prerelease(&self) -> bool {
        self.pre.is_some()
    }

    /// Check if this version has build metadata
    pub fn has_build_metadata(&self) -> bool {
        self.build.is_some()
    }

    /// Get the base version without pre-release or build metadata
    pub fn base(&self) -> Version {
        Version {
            major: self.major,
            minor: self.minor,
            patch: self.patch,
            pre: None,
            build: None,
        }
    }

    /// Increment the major version, resetting minor and patch to 0
    pub fn bump_major(&self) -> Self {
        Self {
            major: self.major + 1,
            minor: 0,
            patch: 0,
            pre: None,
            build: None,
        }
    }

    /// Increment the minor version, resetting patch to 0
    pub fn bump_minor(&self) -> Self {
        Self {
            major: self.major,
            minor: self.minor + 1,
            patch: 0,
            pre: None,
            build: None,
        }
    }

    /// Increment the patch version
    pub fn bump_patch(&self) -> Self {
        Self {
            major: self.major,
            minor: self.minor,
            patch: self.patch + 1,
            pre: None,
            build: None,
        }
    }

    /// Check version compatibility with another version using the given policy
    ///
    /// # Arguments
    /// * `other` - The version to check compatibility against
    /// * `policy` - The compatibility policy to use
    ///
    /// # Example
    /// ```
    /// use nyx::core::version::{Version, CompatibilityPolicy};
    ///
    /// let v1 = Version::new(1, 2, 0);
    /// let v2 = Version::new(1, 3, 0);
    /// assert!(v1.is_compatible(&v2, CompatibilityPolicy::Strict));
    /// ```
    pub fn is_compatible(&self, other: &Version, policy: CompatibilityPolicy) -> bool {
        match policy {
            CompatibilityPolicy::Strict => self.major == other.major,
            CompatibilityPolicy::Moderate => self.major == other.major && other.minor >= self.minor,
            CompatibilityPolicy::Lenient => {
                // Lenient: any version with same major is compatible,
                // or any higher major version
                self.major == other.major || other.major > self.major
            }
        }
    }

    /// Compare versions for ordering (ignores build metadata per SemVer spec)
    ///
    /// Pre-release versions have lower precedence than the normal version.
    /// Example: 1.0.0-alpha < 1.0.0
    fn cmp_pre(&self, other: &Version) -> Ordering {
        match (&self.pre, &other.pre) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(pre1), Some(pre2)) => pre1.cmp(pre2),
        }
    }
}

impl Default for Version {
    fn default() -> Self {
        Self {
            major: 0,
            minor: 1,
            patch: 0,
            pre: None,
            build: None,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;

        if let Some(ref pre) = self.pre {
            write!(f, "-{}", pre)?;
        }

        if let Some(ref build) = self.build {
            write!(f, "+{}", build)?;
        }

        Ok(())
    }
}

impl FromStr for Version {
    type Err = NyxError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        // Split on + first to separate version from build metadata
        let (version_part, build_part) = match s.find('+') {
            Some(idx) => (&s[..idx], Some(&s[idx + 1..])),
            None => (s, None),
        };

        // Split on - to separate version from pre-release
        let (base_part, pre_part) = match version_part.find('-') {
            Some(idx) => (&version_part[..idx], Some(&version_part[idx + 1..])),
            None => (version_part, None),
        };

        // Parse base version (major.minor.patch)
        let parts: Vec<&str> = base_part.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return Err(NyxError::new(
                codes::VERSION_INVALID_FORMAT,
                format!(
                    "Invalid version format: expected major.minor.patch, got '{}'",
                    base_part
                ),
                ErrorCategory::Internal,
            ));
        }

        let major = parts[0].parse().map_err(|_| {
            NyxError::new(
                codes::VERSION_INVALID_FORMAT,
                format!("Invalid major version number: '{}'", parts[0]),
                ErrorCategory::Internal,
            )
        })?;

        let minor = if parts.len() > 1 {
            parts[1].parse().map_err(|_| {
                NyxError::new(
                    codes::VERSION_INVALID_FORMAT,
                    format!("Invalid minor version number: '{}'", parts[1]),
                    ErrorCategory::Internal,
                )
            })?
        } else {
            0
        };

        let patch = if parts.len() > 2 {
            parts[2].parse().map_err(|_| {
                NyxError::new(
                    codes::VERSION_INVALID_FORMAT,
                    format!("Invalid patch version number: '{}'", parts[2]),
                    ErrorCategory::Internal,
                )
            })?
        } else {
            0
        };

        // Validate pre-release format (alphanumeric with dots, separated by -)
        let pre = pre_part
            .map(|p| {
                if !p
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '.' || c == '-')
                {
                    Err(NyxError::new(
                        codes::VERSION_INVALID_FORMAT,
                        format!("Invalid pre-release identifier: '{}'", p),
                        ErrorCategory::Internal,
                    ))
                } else {
                    Ok(p.to_string())
                }
            })
            .transpose()?;

        // Validate build metadata format
        let build = build_part
            .map(|b| {
                if !b
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '.' || c == '-')
                {
                    Err(NyxError::new(
                        codes::VERSION_INVALID_FORMAT,
                        format!("Invalid build metadata: '{}'", b),
                        ErrorCategory::Internal,
                    ))
                } else {
                    Ok(b.to_string())
                }
            })
            .transpose()?;

        Ok(Self {
            major,
            minor,
            patch,
            pre,
            build,
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Version) -> Ordering {
        // Compare major.minor.patch first
        let base_cmp =
            (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch));

        if base_cmp != Ordering::Equal {
            return base_cmp;
        }

        // Pre-release versions have lower precedence
        // Per SemVer: 1.0.0-alpha < 1.0.0
        self.cmp_pre(other)
    }
}

/// Compatibility policy for version checking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompatibilityPolicy {
    /// Strict: Major version must match exactly
    /// 1.x.x is NOT compatible with 2.x.x
    #[default]
    Strict,
    /// Moderate: Major must match, minor >= required
    /// 1.2.x is compatible with 1.1.x but not 1.0.x
    Moderate,
    /// Lenient: Any compatible version (same major or higher)
    /// 1.x.x is compatible with any 1.x.x, but 2.x.x is also acceptable
    Lenient,
}

impl fmt::Display for CompatibilityPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompatibilityPolicy::Strict => write!(f, "strict"),
            CompatibilityPolicy::Moderate => write!(f, "moderate"),
            CompatibilityPolicy::Lenient => write!(f, "lenient"),
        }
    }
}

/// Version range for compatibility checking
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRange {
    /// Minimum compatible version (inclusive)
    pub min: Version,
    /// Maximum compatible version (exclusive)
    pub max: Version,
}

impl VersionRange {
    /// Create a new version range
    pub fn new(min: Version, max: Version) -> Self {
        Self { min, max }
    }

    /// Check if a version is within the compatible range
    ///
    /// # Example
    /// ```
    /// use nyx::core::version::{Version, VersionRange};
    ///
    /// let range = VersionRange::new(
    ///     Version::new(1, 0, 0),
    ///     Version::new(2, 0, 0)
    /// );
    /// assert!(range.contains(&Version::new(1, 5, 0)));
    /// ```
    pub fn contains(&self, version: &Version) -> bool {
        version >= &self.min && version < &self.max
    }

    /// Check if this range is compatible with another range
    pub fn is_compatible_with(&self, other: &VersionRange) -> bool {
        self.min <= other.max && other.min <= self.max
    }
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ">={}, <{}", self.min, self.max)
    }
}

/// Core API version constant
///
/// This represents the current stable API version of the Nyx core.
/// All public APIs should be compatible with this version.
pub const CORE_API_VERSION: Version = Version {
    major: 0,
    minor: 1,
    patch: 0,
    pre: None,
    build: None,
};

/// Latest stable major version
pub const CORE_API_MAJOR_VERSION: u16 = CORE_API_VERSION.major;

/// Latest stable minor version
pub const CORE_API_MINOR_VERSION: u16 = CORE_API_VERSION.minor;

/// Latest stable patch version
pub const CORE_API_PATCH_VERSION: u16 = CORE_API_VERSION.patch;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_new() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.pre.is_none());
        assert!(v.build.is_none());
    }

    #[test]
    fn test_version_parse_basic() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_version_parse_prerelease() {
        let v = Version::parse("1.0.0-alpha.1").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.pre, Some("alpha.1".to_string()));
        assert!(v.is_prerelease());
    }

    #[test]
    fn test_version_parse_build() {
        let v = Version::parse("1.0.0+build.123").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.build, Some("build.123".to_string()));
    }

    #[test]
    fn test_version_parse_full() {
        let v = Version::parse("2.1.0-rc.1+202303151200").unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 0);
        assert_eq!(v.pre, Some("rc.1".to_string()));
        assert_eq!(v.build, Some("202303151200".to_string()));
    }

    #[test]
    fn test_version_display() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");

        let v_pre = Version::new_prerelease(1, 0, 0, "alpha.1");
        assert_eq!(v_pre.to_string(), "1.0.0-alpha.1");

        let v_build = Version::new_with_build(1, 0, 0, "build.123");
        assert_eq!(v_build.to_string(), "1.0.0+build.123");
    }

    #[test]
    fn test_version_comparison() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(1, 0, 1);
        let v3 = Version::new(2, 0, 0);

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }

    #[test]
    fn test_version_prerelease_comparison() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new_prerelease(1, 0, 0, "alpha");
        let v3 = Version::new_prerelease(1, 0, 0, "alpha.1");
        let v4 = Version::new_prerelease(1, 0, 0, "beta");

        assert!(v2 < v1); // Pre-release < release
        assert!(v2 < v3);
        assert!(v3 < v4);
    }

    #[test]
    fn test_version_compatibility() {
        let v1 = Version::new(1, 2, 0);

        // Strict policy
        assert!(v1.is_compatible(&Version::new(1, 3, 0), CompatibilityPolicy::Strict));
        assert!(!v1.is_compatible(&Version::new(2, 0, 0), CompatibilityPolicy::Strict));

        // Moderate policy
        assert!(v1.is_compatible(&Version::new(1, 3, 0), CompatibilityPolicy::Moderate));
        assert!(v1.is_compatible(&Version::new(1, 2, 5), CompatibilityPolicy::Moderate));
        assert!(!v1.is_compatible(&Version::new(1, 1, 0), CompatibilityPolicy::Moderate));

        // Lenient policy
        assert!(v1.is_compatible(&Version::new(1, 0, 0), CompatibilityPolicy::Lenient));
        assert!(v1.is_compatible(&Version::new(2, 0, 0), CompatibilityPolicy::Lenient));
    }

    #[test]
    fn test_version_bump() {
        let v = Version::new(1, 2, 3);

        assert_eq!(v.bump_major(), Version::new(2, 0, 0));
        assert_eq!(v.bump_minor(), Version::new(1, 3, 0));
        assert_eq!(v.bump_patch(), Version::new(1, 2, 4));
    }

    #[test]
    fn test_version_range() {
        let range = VersionRange::new(Version::new(1, 0, 0), Version::new(2, 0, 0));

        assert!(range.contains(&Version::new(1, 5, 0)));
        assert!(range.contains(&Version::new(1, 0, 0)));
        assert!(!range.contains(&Version::new(2, 0, 0)));
        assert!(!range.contains(&Version::new(0, 9, 0)));
    }

    #[test]
    fn test_version_from_str() {
        let v: Version = "2.0.0".parse().unwrap();
        assert_eq!(v.major, 2);

        let v2: Version = "1.2.3-rc.1+build.456".parse().unwrap();
        assert_eq!(v2.pre, Some("rc.1".to_string()));
        assert_eq!(v2.build, Some("build.456".to_string()));
    }
}
