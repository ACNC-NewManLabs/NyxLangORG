use rayon::prelude::*;
use std::sync::{Arc, RwLock};

pub enum OpNode {
    Add,
    Sub,
    Mul,
    Div,
    ReLU,
    Sigmoid,
    Const(f64),
    Input(usize), // Index into input array list
}

pub struct FusedKernel {
    pub ops: Vec<OpNode>,
    pub num_inputs: usize,
}

impl Default for FusedKernel {
    fn default() -> Self {
        Self::new()
    }
}

impl FusedKernel {
    pub fn new() -> Self {
        Self { ops: Vec::new(), num_inputs: 0 }
    }

    pub fn execute(&self, inputs: &[&[f64]], length: usize) -> Vec<f64> {
        let mut out = vec![0.0; length];
        
        out.par_iter_mut().enumerate().for_each(|(i, val)| {
            let mut stack = Vec::with_capacity(8);
            for op in &self.ops {
                match op {
                    OpNode::Const(c) => stack.push(*c),
                    OpNode::Input(idx) => stack.push(inputs[*idx][i]),
                    OpNode::Add => {
                        let b = stack.pop().unwrap_or(0.0);
                        let a = stack.pop().unwrap_or(0.0);
                        stack.push(a + b);
                    }
                    OpNode::Sub => {
                        let b = stack.pop().unwrap_or(0.0);
                        let a = stack.pop().unwrap_or(0.0);
                        stack.push(a - b);
                    }
                    OpNode::Mul => {
                        let b = stack.pop().unwrap_or(0.0);
                        let a = stack.pop().unwrap_or(0.0);
                        stack.push(a * b);
                    }
                    OpNode::Div => {
                        let b = stack.pop().unwrap_or(0.0);
                        let a = stack.pop().unwrap_or(0.0);
                        stack.push(a / b);
                    }
                    OpNode::ReLU => {
                        let a = stack.pop().unwrap_or(0.0);
                        stack.push(if a > 0.0 { a } else { 0.0 });
                    }
                    OpNode::Sigmoid => {
                        let a = stack.pop().unwrap_or(0.0);
                        stack.push(1.0 / (1.0 + (-a).exp()));
                    }
                }
            }
            *val = stack.pop().unwrap_or(0.0);
        });
        
        out
    }
}

// Global cache for fused kernels to avoid re-compilation
static KERNEL_CACHE: std::sync::OnceLock<RwLock<std::collections::HashMap<String, Arc<FusedKernel>>>> = std::sync::OnceLock::new();

pub fn get_or_create_fused_kernel(signature: &str, builder_fn: impl FnOnce() -> FusedKernel) -> Arc<FusedKernel> {
    let cache = KERNEL_CACHE.get_or_init(|| RwLock::new(std::collections::HashMap::new()));
    
    {
        let read = cache.read().unwrap_or_else(|e| e.into_inner());
        if let Some(kernel) = read.get(signature) {
            return kernel.clone();
        }
    }
    
    let kernel = Arc::new(builder_fn());
    cache.write().unwrap_or_else(|e| e.into_inner()).insert(signature.to_string(), kernel.clone());
    kernel
}
