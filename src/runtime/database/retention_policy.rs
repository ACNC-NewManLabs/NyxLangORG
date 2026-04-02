#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionAction {
    RetainActive,
    MoveToWarmDisk,
    MoveToGlacier,
    PurgeCompletely,
}

pub struct RetentionPolicyManager {
    pub archive_sweep_enabled: bool,
    pub max_retention_years: u64,
    pub glacier_threshold_years: u64,
}

impl Default for RetentionPolicyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RetentionPolicyManager {
    pub fn new() -> Self {
        Self {
            archive_sweep_enabled: true,
            max_retention_years: 5,
            glacier_threshold_years: 3,
        }
    }

    /// Evaluates if a data block should be kept or purged per compliance laws (SOC2/GDPR).
    pub fn evaluate_row_retention(&self, age_in_years: u64) -> bool {
        age_in_years <= self.max_retention_years
    }

    /// Determines the lifecycle action for a data block based on its age.
    pub fn trigger_lifecycle_action(&self, block_age_years: u64) -> RetentionAction {
        if block_age_years > self.max_retention_years {
            RetentionAction::PurgeCompletely
        } else if block_age_years >= self.glacier_threshold_years {
            RetentionAction::MoveToGlacier
        } else if block_age_years >= 1 {
            RetentionAction::MoveToWarmDisk
        } else {
            RetentionAction::RetainActive
        }
    }

    pub fn trigger_deep_glacier_archival(&self, block_age_years: u64) -> &str {
        match self.trigger_lifecycle_action(block_age_years) {
            RetentionAction::PurgeCompletely => "PURGED_FROM_SYSTEM",
            RetentionAction::MoveToGlacier => "MOVED_TO_DEEP_GLACIER",
            RetentionAction::MoveToWarmDisk => "MOVED_TO_WARM_DISK",
            RetentionAction::RetainActive => "RETAINED_ACTIVE",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_window_enforcement() {
        let policy = RetentionPolicyManager::new();
        assert!(!policy.evaluate_row_retention(6)); // Over 5 years
        assert!(policy.evaluate_row_retention(4)); // Under 5 years
    }

    #[test]
    fn test_lifecycle_transitions() {
        let policy = RetentionPolicyManager::new();
        assert_eq!(
            policy.trigger_lifecycle_action(6),
            RetentionAction::PurgeCompletely
        );
        assert_eq!(
            policy.trigger_lifecycle_action(4),
            RetentionAction::MoveToGlacier
        );
        assert_eq!(
            policy.trigger_lifecycle_action(2),
            RetentionAction::MoveToWarmDisk
        );
        assert_eq!(
            policy.trigger_lifecycle_action(0),
            RetentionAction::RetainActive
        );
    }
}
