use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub struct GovernanceSecurity {
    pub rls_active: bool,
    pub authorized_tenants: HashSet<String>,
    pub audit_log: Vec<String>,
}

impl Default for GovernanceSecurity {
    fn default() -> Self {
        Self::new()
    }
}

impl GovernanceSecurity {
    pub fn new() -> Self {
        let mut tenants = HashSet::new();
        tenants.insert("admin".to_string());
        tenants.insert("authorized_tenant".to_string());

        Self {
            rls_active: true,
            authorized_tenants: tenants,
            audit_log: Vec::new(),
        }
    }

    /// Evaluates row-level access based on the caller's tenant/role context.
    pub fn evaluate_row_level_policy(&mut self, context_role: &str, table_name: &str) -> bool {
        let success = self.authorized_tenants.contains(context_role);

        // Audit the access attempt
        let event = format!(
            "[Audit] Role '{}' accessed table '{}' - Success: {}",
            context_role, table_name, success
        );
        self.audit_log.push(event);

        success
    }

    /// Physically hashes the payload to redact it securely (GDPR compliant).
    pub fn mask_pii_payload(&self, payload: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(payload.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)
    }

    /// Partially masks sensitive strings (e.g. "SSN-123-456" -> "SSN-XXX-456").
    pub fn partial_mask(&self, payload: &str) -> String {
        if payload.len() < 8 {
            return "****".to_string();
        }
        let mut masked = payload.to_string();
        masked.replace_range(4..payload.len() - 3, "XXX");
        masked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_control_and_audit() {
        let mut gov = GovernanceSecurity::new();
        assert!(gov.evaluate_row_level_policy("admin", "users"));
        assert!(!gov.evaluate_row_level_policy("guest", "users"));
        assert_eq!(gov.audit_log.len(), 2);
    }

    #[test]
    fn test_data_masking_variants() {
        let gov = GovernanceSecurity::new();
        let masked = gov.mask_pii_payload("secret");
        assert_eq!(masked.len(), 64);

        let partial = gov.partial_mask("MY-SSN-1234");
        assert_eq!(partial, "MY-SXXX234");
    }
}
