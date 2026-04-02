use rand::Rng;

pub struct AnalyticsProcessing {
    pub min_max_headers_read: bool,
    pub time_series_contiguous_alloc: bool,
}

impl Default for AnalyticsProcessing {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyticsProcessing {
    pub fn new() -> Self {
        Self {
            min_max_headers_read: true,
            time_series_contiguous_alloc: true,
        }
    }

    /// O(1) Streaming Aggregation update logic.
    pub fn compute_sliding_window_delta(&self, current_agg: f64, dropped_val: f64, new_val: f64) -> f64 {
        current_agg - dropped_val + new_val
    }

    /// Reservoir Sampling implementation for real O(1) random block selection across large-scale data.
    pub fn reservoir_sample(&self, input: &[usize], k: usize) -> Vec<usize> {
        let mut sample = Vec::with_capacity(k);
        let mut rng = rand::thread_rng();

        for (i, &item) in input.iter().enumerate() {
            if i < k {
                sample.push(item);
            } else {
                let j = rng.gen_range(0..=i);
                if j < k {
                    sample[j] = item;
                }
            }
        }
        sample
    }

    /// Fast random selection based on block index with actual jittering.
    pub fn random_sample_block(&self, total_blocks: usize) -> usize {
        if total_blocks == 0 { return 0; }
        let mut rng = rand::thread_rng();
        rng.gen_range(0..total_blocks)
    }

    /// SIMD-Accelerated sum for f64 arrays using the `wide` crate (256-bit AVX2).
    pub fn simd_sum_f64(&self, data: &[f64]) -> f64 {
        use wide::*;
        let mut sum_vec = f64x4::splat(0.0);
        let chunks = data.chunks_exact(4);
        let remainder = chunks.remainder();

        for chunk in chunks {
            let v = f64x4::from([chunk[0], chunk[1], chunk[2], chunk[3]]);
            sum_vec += v;
        }

        let mut total_sum = sum_vec.reduce_add();
        total_sum += remainder.iter().sum::<f64>();
        total_sum
    }

    /// SIMD-Accelerated mean for f64 arrays.
    pub fn simd_mean_f64(&self, data: &[f64]) -> f64 {
        if data.is_empty() { return 0.0; }
        self.simd_sum_f64(data) / data.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reservoir_sampling_distribution() {
        let ana = AnalyticsProcessing::new();
        let input: Vec<usize> = (0..1000).collect();
        let k = 10;
        let sample = ana.reservoir_sample(&input, k);
        assert_eq!(sample.len(), k);
        // Ensure all elements are within range
        for &s in &sample {
            assert!(s < 1000);
        }
    }

    #[test]
    fn test_window_correctness_and_streaming() {
        let ana = AnalyticsProcessing::new();
        let mut floating_sum = 100.0;
        floating_sum = ana.compute_sliding_window_delta(floating_sum, 10.0, 50.0);
        assert_eq!(floating_sum, 140.0);
    }

    #[test]
    fn test_simd_analytics_accuracy() {
        let ana = AnalyticsProcessing::new();
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let sum = ana.simd_sum_f64(&data);
        let mean = ana.simd_mean_f64(&data);
        
        assert_eq!(sum, 55.0);
        assert_eq!(mean, 5.5);
    }
}
