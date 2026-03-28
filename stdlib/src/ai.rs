//! NYX AI Layer

pub mod ai {
    pub mod tensor {
        use crate::error::{NyxError, ErrorCategory};
        use crate::collections::vec::Vec as NyxVec;

        pub struct Tensor {
            data: NyxVec<f64>,
            shape: NyxVec<usize>,
        }

        impl Tensor {
            pub fn new(shape_vec: Vec<usize>) -> Tensor {
                let size: usize = shape_vec.iter().product();
                let mut shape = NyxVec::new();
                for s in shape_vec { shape.push(s); }
                
                let mut data = NyxVec::with_capacity(size);
                for _ in 0..size { data.push(0.0); }
                
                Tensor {
                    data,
                    shape,
                }
            }

            pub fn from_vec(data_vec: Vec<f64>, shape_vec: Vec<usize>) -> Result<Tensor, NyxError> {
                let expected_size: usize = shape_vec.iter().product();
                if data_vec.len() != expected_size {
                    return Err(NyxError::new(
                        "AI001",
                        format!("Tensor data size mismatch: expected {}, found {}", expected_size, data_vec.len()),
                        ErrorCategory::Runtime
                    ));
                }
                let mut data = NyxVec::new();
                for d in data_vec { data.push(d); }
                let mut shape = NyxVec::new();
                for s in shape_vec { shape.push(s); }
                Ok(Tensor { data, shape })
            }

            pub fn shape(&self) -> &[usize] { self.shape.as_slice() }
            pub fn data(&self) -> &[f64] { self.data.as_slice() }
            
            pub fn add(&self, other: &Tensor) -> Result<Tensor, NyxError> {
                if self.shape.as_slice() != other.shape.as_slice() {
                    return Err(NyxError::new(
                        "AI002",
                        format!("Tensor shape mismatch for addition: {:?} vs {:?}", self.shape.as_slice(), other.shape.as_slice()),
                        ErrorCategory::Runtime
                    ));
                }
                
                let mut result_data = NyxVec::new();
                for (a, b) in self.data.as_slice().iter().zip(other.data.as_slice().iter()) {
                    result_data.push(a + b);
                }
                Ok(Tensor { data: result_data, shape: self.clone_shape() })
            }

            pub fn matmul(&self, other: &Tensor) -> Result<Tensor, NyxError> {
                let s_shape = self.shape.as_slice();
                let o_shape = other.shape.as_slice();
                
                if s_shape.len() != 2 || o_shape.len() != 2 {
                    return Err(NyxError::new("AI004", "Matmul requires 2D tensors", ErrorCategory::Runtime));
                }
                
                if s_shape[1] != o_shape[0] {
                    return Err(NyxError::new("AI005", format!("Matmul inner dimension mismatch: {} vs {}", s_shape[1], o_shape[0]), ErrorCategory::Runtime));
                }

                let rows = s_shape[0];
                let cols = o_shape[1];
                let inner = s_shape[1];
                // Compute directly into the target NyxVec — no intermediate allocation.
                let mut result_data = NyxVec::with_capacity(rows * cols);
                for i in 0..rows {
                    for j in 0..cols {
                        let mut sum = 0.0_f64;
                        for k in 0..inner {
                            sum += self.data.as_slice()[i * inner + k]
                                 * other.data.as_slice()[k * cols + j];
                        }
                        result_data.push(sum);
                    }
                }
                
                let mut final_shape = NyxVec::new();
                final_shape.push(rows);
                final_shape.push(cols);
                
                Ok(Tensor { data: result_data, shape: final_shape })
            }

            fn clone_shape(&self) -> NyxVec<usize> {
                let mut s = NyxVec::new();
                for val in self.shape.as_slice() { s.push(*val); }
                s
            }
        }
    }

    pub mod nn {
        pub mod layers {
            pub struct Layer {
                pub input_size: usize,
                pub output_size: usize,
                pub weights: Vec<f64>,
                pub bias: Vec<f64>,
            }

            impl Layer {
                pub fn new(input_size: usize, output_size: usize) -> Layer {
                    // Xavier uniform initialisation: keeps variance stable across layers.
                    let limit = (6.0_f64 / (input_size + output_size) as f64).sqrt();
                    let scale = 2.0 * limit;
                    // Simple deterministic init via LCG (no external dep needed here).
                    let mut state: u64 = 0xdeadbeef_cafef00d;
                    let mut next_f64 = move || {
                        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                        let hi = (state >> 33) as f64;
                        hi / (u32::MAX as f64) * scale - limit
                    };
                    let weights = (0..input_size * output_size).map(|_| next_f64()).collect();
                    Layer {
                        input_size,
                        output_size,
                        weights,
                        bias: vec![0.0; output_size],
                    }
                }

                pub fn forward(&self, input: &[f64]) -> Vec<f64> {
                    let mut output = self.bias.clone();
                    for (i, w) in self.weights.chunks(self.output_size).enumerate() {
                        for (j, &weight) in w.iter().enumerate() {
                            output[j] += input[i] * weight;
                        }
                    }
                    output
                }
            }

            pub struct Dense {
                layer: Layer,
            }

            impl Dense {
                pub fn new(input_size: usize, output_size: usize) -> Dense {
                    Dense { layer: Layer::new(input_size, output_size) }
                }
                pub fn forward(&self, input: &[f64]) -> Vec<f64> {
                    self.layer.forward(input)
                }
            }
        }

        pub mod activations {
            pub fn relu(x: f64) -> f64 { x.max(0.0) }
            pub fn sigmoid(x: f64) -> f64 { 1.0 / (1.0 + (-x).exp()) }
            pub fn tanh(x: f64) -> f64 { x.tanh() }
        }

        pub mod losses {
            pub fn mse_loss(pred: &[f64], target: &[f64]) -> f64 {
                pred.iter().zip(target.iter())
                    .map(|(&p, &t)| (p - t).powi(2))
                    .sum::<f64>() / pred.len() as f64
            }

            pub fn cross_entropy_loss(pred: &[f64], target: &[f64]) -> f64 {
                // Clamp predictions to avoid log(0) = -inf which propagates NaN.
                const EPS: f64 = 1e-15;
                -pred.iter().zip(target.iter())
                    .map(|(&p, &t)| t * p.clamp(EPS, 1.0 - EPS).ln())
                    .sum::<f64>()
            }
        }
    }

    pub mod optimizer {
        pub struct SGD {
            pub lr: f64,
        }

        impl SGD {
            pub fn new(learning_rate: f64) -> SGD { SGD { lr: learning_rate } }
            pub fn step(&self, params: &mut [f64], grads: &[f64]) {
                for (p, g) in params.iter_mut().zip(grads.iter()) {
                    *p -= self.lr * g;
                }
            }
        }

        pub struct Adam {
            lr: f64,
            beta1: f64,
            beta2: f64,
            m: Vec<f64>,
            v: Vec<f64>,
            t: usize,
        }

        impl Adam {
            pub fn new(learning_rate: f64, params: usize) -> Adam {
                Adam {
                    lr: learning_rate,
                    beta1: 0.9,
                    beta2: 0.999,
                    m: vec![0.0; params],
                    v: vec![0.0; params],
                    t: 0,
                }
            }

            pub fn step(&mut self, params: &mut [f64], grads: &[f64]) {
                self.t += 1;
                for i in 0..params.len() {
                    self.m[i] = self.beta1 * self.m[i] + (1.0 - self.beta1) * grads[i];
                    self.v[i] = self.beta2 * self.v[i] + (1.0 - self.beta2) * grads[i].powi(2);
                    let m_hat = self.m[i] / (1.0 - self.beta1.powi(self.t as i32));
                    let v_hat = self.v[i] / (1.0 - self.beta2.powi(self.t as i32));
                    params[i] -= self.lr * m_hat / (v_hat.sqrt() + 1e-8);
                }
            }
        }
    }

    pub mod dataset {
        pub struct Dataset {
            pub data: Vec<Vec<f64>>,
            pub labels: Vec<Vec<f64>>,
        }

        impl Dataset {
            pub fn new() -> Dataset {
                Dataset { data: Vec::new(), labels: Vec::new() }
            }

            pub fn add(&mut self, data: Vec<f64>, label: Vec<f64>) {
                self.data.push(data);
                self.labels.push(label);
            }

            pub fn len(&self) -> usize { self.data.len() }
            pub fn is_empty(&self) -> bool { self.data.is_empty() }
        }
    }

    pub mod inference {
        pub struct InferenceEngine {
            model: Vec<crate::ai::nn::layers::Dense>,
        }

        impl InferenceEngine {
            pub fn new(model: Vec<crate::ai::nn::layers::Dense>) -> InferenceEngine {
                InferenceEngine { model }
            }

            pub fn predict(&self, input: &[f64]) -> Result<Vec<f64>, crate::error::NyxError> {
                let mut output = input.to_vec();
                for layer in &self.model {
                    output = layer.forward(&output);
                }
                Ok(output)
            }
        }
    }
}

pub use ai::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_ops() {
        let t1 = tensor::Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]).expect("Tensor 1 creation failed");
        let t2 = tensor::Tensor::from_vec(vec![5.0, 6.0, 7.0, 8.0], vec![2, 2]).expect("Tensor 2 creation failed");
        
        let t3 = t1.add(&t2).expect("Tensor addition failed");
        assert_eq!(t3.data(), &[6.0, 8.0, 10.0, 12.0]);
        assert_eq!(t3.shape(), &[2, 2]);

        let t4 = t1.matmul(&t2).expect("Matmul failed");
        // [1 2] * [5 6] = [1*5+2*7  1*6+2*8] = [19 22]
        // [3 4]   [7 8]   [3*5+4*7  3*6+4*8] = [43 50]
        assert_eq!(t4.data(), &[19.0, 22.0, 43.0, 50.0]);
    }

    #[test]
    fn test_nn_layers() {
        let layer = nn::layers::Dense::new(2, 1);
        // Initially weights are 0, bias is 0
        let input = vec![1.0, 1.0];
        let output = layer.forward(&input);
        assert_eq!(output, vec![0.0]);
    }

    #[test]
    fn test_activations() {
        assert_eq!(nn::activations::relu(5.0), 5.0);
        assert_eq!(nn::activations::relu(-5.0), 0.0);
        assert!(nn::activations::sigmoid(0.0) - 0.5 < 1e-6);
        assert!(nn::activations::tanh(0.0).abs() < 1e-6);
    }
}

