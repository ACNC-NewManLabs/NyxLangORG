#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudProvider {
    AWS,
    GCP,
    Azure,
    OnPrem,
}

pub struct CloudNativeScale {
    pub provider: CloudProvider,
    pub auto_scaling_active: bool,
    pub replica_count: u32,
    pub last_scale_timestamp: u64,
}

impl Default for CloudNativeScale {
    fn default() -> Self {
        Self::new()
    }
}

impl CloudNativeScale {
    pub fn new() -> Self {
        Self {
            provider: CloudProvider::OnPrem,
            auto_scaling_active: true,
            replica_count: 1,
            last_scale_timestamp: 0,
        }
    }

    /// Evaluates if a scale-up or scale-down is required based on CPU/Memory pressure.
    pub fn evaluate_scaling_need(&mut self, cpu_usage: f64, mem_usage: f64) -> i32 {
        if !self.auto_scaling_active {
            return 0;
        }

        if cpu_usage > 0.8 || mem_usage > 0.85 {
            // Scale up: Return 1
            if self.replica_count < 10 {
                self.replica_count += 1;
                return 1;
            }
        } else if cpu_usage < 0.2 && self.replica_count > 1 {
            // Scale down: Return -1
            self.replica_count -= 1;
            return -1;
        }
        0 // Steady state
    }

    /// Determines the optimal S3/GCS endpoint based on the cloud provider.
    pub fn get_blob_storage_endpoint(&self) -> String {
        match self.provider {
            CloudProvider::AWS => "https://s3.amazonaws.com".to_string(),
            CloudProvider::GCP => "https://storage.googleapis.com".to_string(),
            CloudProvider::Azure => "https://blob.core.windows.net".to_string(),
            CloudProvider::OnPrem => "http://minio.local".to_string(),
        }
    }

    pub fn set_provider(&mut self, provider: CloudProvider) {
        self.provider = provider;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_scaling_logic() {
        let mut cloud = CloudNativeScale::new();
        // High CPU triggers scale up
        assert_eq!(cloud.evaluate_scaling_need(0.9, 0.5), 1);
        assert_eq!(cloud.replica_count, 2);
        // Low CPU triggers scale down
        assert_eq!(cloud.evaluate_scaling_need(0.1, 0.1), -1);
        assert_eq!(cloud.replica_count, 1);
    }

    #[test]
    fn test_cloud_provider_endpoints() {
        let mut cloud = CloudNativeScale::new();
        cloud.set_provider(CloudProvider::GCP);
        assert_eq!(
            cloud.get_blob_storage_endpoint(),
            "https://storage.googleapis.com"
        );
    }
}
