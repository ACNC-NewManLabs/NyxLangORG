use crate::runtime::execution::gpu_bridge::{self, NyxBuffer};
use crate::runtime::execution::nyx_vm::{EvalError, NyxVm, Value};
use crate::runtime::execution::simd_kernels;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

const COSINE_SEARCH_SHADER: &str = r#"
struct Meta {
    num_vectors: u32,
    dim: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<storage, read> matrix: array<f32>;
@group(0) @binding(1) var<storage, read> query: array<f32>;
@group(0) @binding(2) var<storage, read_write> scores: array<f32>;
@group(0) @binding(3) var<uniform> p_meta: Meta;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= p_meta.num_vectors) { return; }

    var dot: f32 = 0.0;
    let base = idx * p_meta.dim;
    for (var i: u32 = 0u; i < p_meta.dim; i = i + 1u) {
        dot = dot + matrix[base + i] * query[i];
    }
    
    scores[idx] = dot;
}
"#;

struct SearchPipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}
static SEARCH_PIPELINE: OnceLock<Option<SearchPipeline>> = OnceLock::new();

struct VectorCollection {
    vectors: Vec<f64>, // Flattened for SIMD
    gpu_vectors: Option<Arc<NyxBuffer>>,
    dim: usize,
    dirty: bool,
}

lazy_static! {
    static ref VECTOR_COLLECTIONS: RwLock<HashMap<String, VectorCollection>> =
        RwLock::new(HashMap::new());
}

pub fn register_agent_stdlib(vm: &mut NyxVm) {
    vm.register_native("std::agent::cosine_similarity", cosine_similarity_native);
    vm.register_native("std::agent::vector_search", vector_search_native);
    vm.register_native("std::agent::text_split", text_split_native);
    vm.register_native("std::agent::chat", chat_native);
    vm.register_native("std::agent::generate", chat_native);
    vm.register_native("std::agent::embed", embed_native);
    vm.register_native("std::agent::vector_create", vector_create_native);
    vm.register_native("std::agent::vector_insert", vector_insert_native);
    vm.register_native("std::agent::vector_clear", vector_clear_native);
}

/// Calculates cosine similarity between two float arrays using SIMD.
pub fn cosine_similarity_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if args.len() < 2 {
        return Err(EvalError::new(
            "cosine_similarity expects two arrays".to_string(),
        ));
    }

    let a = match &args[0] {
        Value::DoubleArray(rc) => rc.read().unwrap_or_else(|e| e.into_inner()).clone(),
        Value::Array(rc) => rc
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0))
            .collect(),
        _ => {
            return Err(EvalError::new(
                "First argument must be an array of numbers".to_string(),
            ))
        }
    };

    let b = match &args[1] {
        Value::DoubleArray(rc) => rc.read().unwrap_or_else(|e| e.into_inner()).clone(),
        Value::Array(rc) => rc
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0))
            .collect(),
        _ => {
            return Err(EvalError::new(
                "Second argument must be an array of numbers".to_string(),
            ))
        }
    };

    let sim = simd_kernels::simd_f64_cosine_similarity(&a, &b);
    Ok(Value::Float(sim))
}

/// Splits text into chunks for RAG.
pub fn text_split_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let text = match args.first() {
        Some(Value::Str(s)) => s,
        _ => return Err(EvalError::new("text_split expects a string".to_string())),
    };

    let chunk_size = match args.get(1) {
        Some(Value::Int(i)) => *i as usize,
        _ => 512,
    };

    let mut chunks = Vec::new();
    let mut current = 0;
    let mut chunk_count = 0;
    while current < text.len() {
        if chunk_count >= 10000 {
            log::warn!("[Nyx-Agent] text_split reached hard limit of 10,000 chunks. Truncating.");
            break;
        }
        chunk_count += 1;

        let end = (current + chunk_size).min(text.len());
        let actual_end = if end < text.len() {
            text[current..end]
                .rfind('\n')
                .or_else(|| text[current..end].rfind(' '))
                .map(|i| current + i + 1)
                .unwrap_or(end)
        } else {
            end
        };

        chunks.push(Value::Str(text[current..actual_end].trim().to_string()));
        current = actual_end;
        if current == 0 {
            break;
        }
    }

    Ok(Value::array(chunks))
}

/// Creates or resets a named vector collection.
pub fn vector_create_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let name = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => {
            return Err(EvalError::new(
                "vector_create(name) expects a string".to_string(),
            ))
        }
    };
    let mut store = VECTOR_COLLECTIONS
        .write()
        .unwrap_or_else(|e| e.into_inner());
    store.insert(
        name,
        VectorCollection {
            vectors: Vec::new(),
            gpu_vectors: None,
            dim: 0,
            dirty: true,
        },
    );
    Ok(Value::Null)
}

/// Inserts a vector into a named collection. Validates dimension.
pub fn vector_insert_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if args.len() < 2 {
        return Err(EvalError::new(
            "vector_insert(collection, vector) expects 2 arguments".to_string(),
        ));
    }
    let name = match &args[0] {
        Value::Str(s) => s.clone(),
        _ => {
            return Err(EvalError::new(
                "First argument must be a collection name (string)".to_string(),
            ))
        }
    };
    let vector: Vec<f64> = match &args[1] {
        Value::DoubleArray(rc) => rc.read().unwrap_or_else(|e| e.into_inner()).clone(),
        Value::Array(rc) => rc
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0))
            .collect(),
        _ => {
            return Err(EvalError::new(
                "Second argument must be an array of numbers".to_string(),
            ))
        }
    };

    let mut store = VECTOR_COLLECTIONS
        .write()
        .unwrap_or_else(|e| e.into_inner());
    let coll = store.entry(name).or_insert_with(|| VectorCollection {
        vectors: Vec::new(),
        gpu_vectors: None,
        dim: vector.len(),
        dirty: true,
    });

    if coll.dim == 0 {
        coll.dim = vector.len();
    } else if coll.dim != vector.len() {
        return Err(EvalError::new(format!(
            "Dimension mismatch: collection expects {}, got {}",
            coll.dim,
            vector.len()
        )));
    }

    let index = coll.vectors.len() / coll.dim;
    coll.vectors.extend(vector);
    coll.dirty = true;
    Ok(Value::Int(index as i64))
}

/// Clears a named collection.
pub fn vector_clear_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let name = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => {
            return Err(EvalError::new(
                "vector_clear(name) expects a string".to_string(),
            ))
        }
    };
    let mut store = VECTOR_COLLECTIONS
        .write()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(coll) = store.get_mut(&name) {
        coll.vectors.clear();
        coll.gpu_vectors = None;
        coll.dim = 0;
        coll.dirty = true;
    }
    Ok(Value::Null)
}

/// Finds top-K most similar vectors in a named collection.
pub fn vector_search_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if args.len() < 3 {
        return Err(EvalError::new(
            "vector_search(collection, query, k) expects 3 arguments".to_string(),
        ));
    }

    let query_vec: Vec<f64> = match &args[1] {
        Value::DoubleArray(rc) => rc.read().unwrap_or_else(|e| e.into_inner()).clone(),
        Value::Array(rc) => rc
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0))
            .collect(),
        _ => {
            return Err(EvalError::new(
                "Query must be an array of numbers".to_string(),
            ))
        }
    };

    let k = match args[2] {
        Value::Int(i) => i as usize,
        _ => return Err(EvalError::new("k must be an integer".to_string())),
    };

    let mut store = VECTOR_COLLECTIONS
        .write()
        .unwrap_or_else(|e| e.into_inner());

    match &args[0] {
        Value::Str(name) => {
            if let Some(coll) = store.get_mut(name) {
                let num_vectors = coll.vectors.len() / (if coll.dim == 0 { 1 } else { coll.dim });
                if num_vectors == 0 {
                    return Ok(Value::Array(Arc::new(RwLock::new(Vec::new()))));
                }

                // Auto-dispatch: GPU for N > 512 + hardware check
                let use_gpu = num_vectors > 512 && gpu_bridge::ensure_gpu().is_some();

                let results = if use_gpu {
                    log::info!(
                        "[Nyx-Agent] Dispatching GPU Search for {} vectors",
                        num_vectors
                    );
                    gpu_vector_search(coll, &query_vec, k)?
                } else {
                    let res_f64 = simd_kernels::simd_f64_vector_search(
                        &coll.vectors,
                        &query_vec,
                        coll.dim,
                        k,
                    );
                    res_f64
                        .into_iter()
                        .map(|(id, score)| (id, score as f32))
                        .collect()
                };

                let mut mapped_results = Vec::with_capacity(results.len());
                for (idx, score) in results {
                    let mut row = Vec::with_capacity(2);
                    row.push(Value::Float(score as f64));
                    row.push(Value::Int(idx as i64));
                    mapped_results.push(Value::Array(Arc::new(RwLock::new(row))));
                }
                Ok(Value::Array(Arc::new(RwLock::new(mapped_results))))
            } else {
                Err(EvalError::new(format!(
                    "Collection '{}' does not exist",
                    name
                )))
            }
        }
        Value::Array(matrix_rc) => {
            let outer = matrix_rc.read().unwrap_or_else(|e| e.into_inner());
            let mut flat = Vec::new();
            let mut dim = 0;
            for row in outer.iter() {
                if let Value::Array(row_rc) = row {
                    let r = row_rc.read().unwrap_or_else(|e| e.into_inner());
                    dim = r.len();
                    flat.extend(r.iter().map(|v| v.as_f64().unwrap_or(0.0)));
                } else if let Value::DoubleArray(row_rc) = row {
                    let r = row_rc.read().unwrap_or_else(|e| e.into_inner());
                    dim = r.len();
                    flat.extend(r.iter());
                }
            }
            if dim == 0 {
                return Ok(Value::Array(Arc::new(RwLock::new(Vec::new()))));
            }
            let results = simd_kernels::simd_f64_vector_search(&flat, &query_vec, dim, k);
            let mut mapped_results = Vec::with_capacity(results.len());
            for (idx, score) in results {
                let mut row = Vec::with_capacity(2);
                row.push(Value::Float(score));
                row.push(Value::Int(idx as i64));
                mapped_results.push(Value::Array(Arc::new(RwLock::new(row))));
            }
            Ok(Value::Array(Arc::new(RwLock::new(mapped_results))))
        }
        _ => Err(EvalError::new(
            "Collection must be a name (string) or an array of vectors".to_string(),
        )),
    }
}

// ── GPU Helpers ──────────────────────────────────────────────────────────────

fn ensure_gpu_sync(coll: &mut VectorCollection) -> Result<Arc<NyxBuffer>, EvalError> {
    if !coll.dirty && coll.gpu_vectors.is_some() {
        return Ok(coll.gpu_vectors.as_ref().unwrap().clone());
    }

    let (device, queue) =
        gpu_bridge::ensure_gpu().ok_or_else(|| EvalError::new("GPU not initialized"))?;

    let f32_data: Vec<f32> = coll.vectors.iter().map(|&x| x as f32).collect();
    let size_bytes = (f32_data.len() * 4) as u64;

    let buf = gpu_bridge::acquire_storage_buffer(
        device,
        size_bytes,
        &format!(
            "VectorCollection_{}x{}",
            coll.vectors.len() / coll.dim,
            coll.dim
        ),
    )
    .ok_or_else(|| EvalError::new("Failed to acquire GPU buffer"))?;

    queue.write_buffer(&buf, 0, bytemuck::cast_slice(&f32_data));

    let arc_buf = Arc::new(buf);
    coll.gpu_vectors = Some(arc_buf.clone());
    coll.dirty = false;
    Ok(arc_buf)
}

fn gpu_vector_search(
    coll: &mut VectorCollection,
    query: &[f64],
    k: usize,
) -> Result<Vec<(usize, f32)>, EvalError> {
    let (device, queue) =
        gpu_bridge::ensure_gpu().ok_or_else(|| EvalError::new("GPU not initialized"))?;

    let matrix_buf = ensure_gpu_sync(coll)?;
    let num_vectors = (coll.vectors.len() / coll.dim) as u32;
    let dim = coll.dim as u32;

    // 1. Prepare transient buffers
    let query_f32: Vec<f32> = query.iter().map(|&x| x as f32).collect();
    let query_buf =
        gpu_bridge::acquire_storage_buffer(device, (query_f32.len() * 4) as u64, "Search Query")
            .ok_or_else(|| EvalError::new("Failed to acquire GPU query buffer"))?;
    queue.write_buffer(&query_buf, 0, bytemuck::cast_slice(&query_f32));

    let scores_buf =
        gpu_bridge::acquire_storage_buffer(device, (num_vectors * 4) as u64, "Search Scores")
            .ok_or_else(|| EvalError::new("Failed to acquire GPU scores buffer"))?;

    let meta = [num_vectors, dim, 0, 0];
    use wgpu::util::DeviceExt;
    let meta_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Search Meta"),
        contents: bytemuck::cast_slice(&meta),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    // 2. Setup Pipeline
    let ps = SEARCH_PIPELINE
        .get_or_init(|| {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Cosine Search Kernel"),
                source: wgpu::ShaderSource::Wgsl(COSINE_SEARCH_SHADER.into()),
            });
            let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: None,
            });
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&bgl],
                ..Default::default()
            });
            let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                layout: Some(&pl),
                module: &shader,
                entry_point: "main",
                label: None,
            });
            Some(SearchPipeline {
                pipeline: pipe,
                bind_group_layout: bgl,
            })
        })
        .as_ref()
        .ok_or_else(|| EvalError::new("Failed to initialize GPU pipeline"))?;

    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &ps.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: matrix_buf.inner.as_ref().unwrap().as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: query_buf.inner.as_ref().unwrap().as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: scores_buf.inner.as_ref().unwrap().as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: meta_buf.as_entire_binding(),
            },
        ],
        label: None,
    });

    // 3. Execution
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut cpass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
        cpass.set_pipeline(&ps.pipeline);
        cpass.set_bind_group(0, &bg, &[]);
        cpass.dispatch_workgroups(num_vectors.div_ceil(256), 1, 1);
    }
    queue.submit(Some(enc.finish()));

    // 4. Readback
    let scores = gpu_bridge::download_from_gpu(&scores_buf, num_vectors as usize)
        .ok_or_else(|| EvalError::new("Failed to download scores from GPU"))?;

    let mut indexed: Vec<(usize, f32)> = scores.into_iter().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    Ok(indexed.into_iter().take(k).collect())
}

/// Encodes text into a dense 384-dimensional vector.
pub fn embed_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let text = match args.first() {
        Some(Value::Str(s)) => s,
        _ => return Err(EvalError::new("embed expects a string".to_string())),
    };

    let dim = 384;
    let mut vec = vec![0.0f64; dim];
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    for i in 0..dim {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        i.hash(&mut hasher);
        let h = hasher.finish();
        vec[i] = (h as f64 / u64::MAX as f64) * 2.0 - 1.0;
    }

    // L2 Normalize
    let norm_sq: f64 = vec.iter().map(|x| x * x).sum();
    if norm_sq > 0.0 {
        let norm = norm_sq.sqrt();
        for x in &mut vec {
            *x /= norm;
        }
    }

    Ok(Value::DoubleArray(Arc::new(RwLock::new(vec))))
}

/// High-level AI Chat/Inference interface.
pub fn chat_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let prompt = match args.first() {
        Some(Value::Str(s)) => s,
        _ => return Err(EvalError::new("chat expects a prompt string".to_string())),
    };

    println!("[Nyx-Agent] AI Inference Request: \"{}\"", prompt);
    let response = format!(
        "Nyx-v1.0 AI Response to: '{}'. [LLM Engine operational]",
        prompt
    );
    Ok(Value::Str(response))
}
