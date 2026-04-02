use crate::runtime::execution::nyx_vm::{NyxVm, Value, EvalError};
use crate::core::ast::ast_nodes::Expr;
use super::nyx_vm::TensorStorage;
use std::sync::{Arc, RwLock};
use crate::runtime::execution::df_engine::{self, LogicalPlan, DataChunk, Schema, Field};
use std::collections::HashMap;
use rayon::prelude::*;
use serde_json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub fn generate_synthetic_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Int(rows)), Some(Value::Object(schema_map_rc))) = (args.first(), args.get(1)) {
        let n = *rows as usize;
        let mut columns = Vec::new();
        let schema_map = schema_map_rc.read().unwrap_or_else(|e| e.into_inner());
        
        for (name, dtype_val) in schema_map.iter() {
            let dtype = match dtype_val {
                Value::Str(s) => s.as_str(),
                _ => "float",
            };
            
            let data = match dtype {
                "int" => {
                    let mut v = Vec::with_capacity(n);
                    for i in 0..n { v.push(i as i64); }
                    df_engine::ColumnData::I64(Arc::new(v))
                },
                "string" => {
                    let mut data = Vec::new();
                    let mut offsets = Vec::with_capacity(n);
                    for i in 0..n {
                        let s = format!("cat_{}", i % 10);
                        offsets.push(data.len());
                        data.extend_from_slice(s.as_bytes());
                    }
                    df_engine::ColumnData::Str {
                        data: Arc::new(data),
                        offsets: Arc::new(offsets),
                    }
                },
                _ => {
                    let mut v = Vec::with_capacity(n);
                    for i in 0..n { v.push(i as f64 * 0.1); }
                    df_engine::ColumnData::F64(Arc::new(v))
                }
            };
            columns.push(df_engine::Column::new(name.clone(), data, None));
        }

        // Wrap columns into a list of Column objects for the Nyx side
        let mut col_array = Vec::new();
        for col in columns {
            let mut c_map = HashMap::new();
            c_map.insert("_name".to_string(), Value::Str(col.name.clone()));
            
            let dtype_str = match &col.data {
                df_engine::ColumnData::F64(_) => "float",
                df_engine::ColumnData::I64(_) => "int",
                df_engine::ColumnData::Bitmap(_) => "bool",
                _ => "string",
            };

            let val_data = match &col.data {
                df_engine::ColumnData::F64(v) => Value::DoubleArray(Arc::new(RwLock::new((**v).clone()))),
                df_engine::ColumnData::I64(v) => {
                    Value::Array(Arc::new(RwLock::new(v.iter().map(|&x| Value::Int(x)).collect())))
                },
                df_engine::ColumnData::Bitmap(bm) => {
                    let mut flags = Vec::with_capacity(bm.len);
                    for i in 0..bm.len { flags.push(Value::Bool(bm.get(i))); }
                    Value::Array(Arc::new(RwLock::new(flags)))
                },
                df_engine::ColumnData::Str { data, offsets } => {
                    let mut strings = Vec::new();
                    for i in 0..offsets.len() {
                        let start = offsets[i];
                        let end = if i + 1 < offsets.len() { offsets[i+1] } else { data.len() };
                        strings.push(Value::Str(String::from_utf8_lossy(&data[start..end]).to_string()));
                    }
                    Value::Array(Arc::new(RwLock::new(strings)))
                },
                _ => Value::Null,
            };
            c_map.insert("_data".to_string(), val_data);
            c_map.insert("_dtype".to_string(), Value::Str(dtype_str.to_string()));
            col_array.push(Value::Object(Arc::new(RwLock::new(c_map))));
        }
        
        Ok(Value::Array(Arc::new(RwLock::new(col_array))))
    } else {
        Err(EvalError::new("generate_synthetic(rows, {col: type}) expected".to_string()))
    }
}


fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Float(f) => Some(*f),
        Value::Int(i) => Some(*i as f64),
        _ => None,
    }
}

fn extract_floats(arr: &Arc<RwLock<Vec<Value>>>) -> Vec<f64> {
    arr.read().unwrap_or_else(|e| e.into_inner()).iter().map(|v| as_f64(v).unwrap_or(0.0)).collect()
}

fn floats_to_value_arr(v: Vec<f64>) -> Value {
    let arr: Vec<Value> = v.into_iter().map(Value::Float).collect();
    Value::Array(Arc::new(RwLock::new(arr)))
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "".to_string(),
        _ => format!("{:?}", v),
    }
}


pub fn register_table_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Str(name)), Some(Value::Object(df_rc))) = (args.first(), args.get(1)) {
        let df = df_rc.read().unwrap_or_else(|e| e.into_inner());
        let chunk = df_to_data_chunk(&df)?;
        df_engine::register_table(name.clone(), Arc::new(vec![chunk]));
        return Ok(Value::Bool(true));
    }
    Ok(Value::Bool(false))
}

pub fn start_df_server_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Int(port)) = args.first() {
        let p = *port as u16;
        println!("[df.server] Starting HTTP server on 0.0.0.0:{}...", p);
        
        tokio::spawn(async move {
            let addr = format!("0.0.0.0:{}", p);
            match TcpListener::bind(&addr).await {
                Ok(listener) => {
                    loop {
                        match listener.accept().await {
                            Ok((mut socket, _)) => {
                                tokio::spawn(async move {
                                    let mut buf = [0; 4096];
                                    match socket.read(&mut buf).await {
                                        Ok(n) if n > 0 => {
                                            let req = String::from_utf8_lossy(&buf[..n]);
                                            let (res_line, res_body) = if req.contains("GET /tables") {
                                                let catalog = df_engine::global_catalog().lock().unwrap_or_else(|e| e.into_inner());
                                                let tables: Vec<_> = catalog.keys().cloned().collect();
                                                ("HTTP/1.1 200 OK", serde_json::to_string(&tables).unwrap_or_else(|_| "[]".to_string()))
                                            } else if req.contains("POST /query") {
                                                ("HTTP/1.1 200 OK", "{\"status\":\"ok\",\"info\":\"Nyx DataFrame Server v1.0 Ready\"}".to_string())
                                            } else {
                                                ("HTTP/1.1 404 Not Found", "{}".to_string())
                                            };
                                            let full_res = format!("{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", res_line, res_body.len(), res_body);
                                            let _ = socket.write_all(full_res.as_bytes()).await;
                                        }
                                        _ => {}
                                    }
                                });
                            }
                            Err(_) => break,
                        }
                    }
                }
                Err(e) => eprintln!("[df.server] Failed to bind to {}: {}", addr, e),
            }
        });
        return Ok(Value::Bool(true));
    }
    Ok(Value::Bool(false))
}

// ── arithmetic ────────────────────────────────────────────────────────────────

pub fn col_add_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Array(a)), Some(Value::Array(b))) = (args.first(), args.get(1)) {
        let av = extract_floats(a);
        let bv = extract_floats(b);
        let n = av.len().min(bv.len());
        let result: Vec<f64> = (0..n).map(|i| av[i] + bv[i]).collect();
        return Ok(floats_to_value_arr(result));
    }
    Ok(Value::Array(Arc::new(RwLock::new(vec![]))))
}

pub fn col_sub_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Array(a)), Some(Value::Array(b))) = (args.first(), args.get(1)) {
        let av = extract_floats(a);
        let bv = extract_floats(b);
        let n = av.len().min(bv.len());
        let result: Vec<f64> = (0..n).map(|i| av[i] - bv[i]).collect();
        return Ok(floats_to_value_arr(result));
    }
    Ok(Value::Array(Arc::new(RwLock::new(vec![]))))
}

pub fn col_mul_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Array(a)), Some(Value::Array(b))) = (args.first(), args.get(1)) {
        let av = extract_floats(a);
        let bv = extract_floats(b);
        let n = av.len().min(bv.len());
        let result: Vec<f64> = (0..n).map(|i| av[i] * bv[i]).collect();
        return Ok(floats_to_value_arr(result));
    }
    Ok(Value::Array(Arc::new(RwLock::new(vec![]))))
}

pub fn col_div_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Array(a)), Some(Value::Array(b))) = (args.first(), args.get(1)) {
        let av = extract_floats(a);
        let bv = extract_floats(b);
        let n = av.len().min(bv.len());
        let result: Vec<Value> = (0..n).map(|i| {
            if bv[i] == 0.0 { Value::Null } else { Value::Float(av[i] / bv[i]) }
        }).collect();
        return Ok(Value::Array(Arc::new(RwLock::new(result))));
    }
    Ok(Value::Array(Arc::new(RwLock::new(vec![]))))
}

// ── filter ────────────────────────────────────────────────────────────────────

pub fn col_filter_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Array(data)), Some(Value::Array(mask))) = (args.first(), args.get(1)) {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let m = mask.read().unwrap_or_else(|e| e.into_inner());
        let result: Vec<Value> = d.iter().zip(m.iter()).filter_map(|(val, flag)| {
            match flag {
                Value::Bool(true) => Some(val.clone()),
                Value::Int(i) if *i != 0 => Some(val.clone()),
                _ => None,
            }
        }).collect();
        return Ok(Value::Array(Arc::new(RwLock::new(result))));
    }
    Ok(Value::Array(Arc::new(RwLock::new(vec![]))))
}

// ── sort (stable argsort) ─────────────────────────────────────────────────────

pub fn col_sort_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let descending = matches!(args.get(1), Some(Value::Bool(true)));
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let mut indexed: Vec<(usize, f64)> = d.iter().enumerate()
            .map(|(i, v)| (i, as_f64(v).unwrap_or(0.0)))
            .collect();
        if descending {
            indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        } else {
            indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        }
        let indices: Vec<Value> = indexed.into_iter().map(|(i, _)| Value::Int(i as i64)).collect();
        return Ok(Value::Array(Arc::new(RwLock::new(indices))));
    }
    Ok(Value::Array(Arc::new(RwLock::new(vec![]))))
}

// ── aggregations ──────────────────────────────────────────────────────────────

pub fn col_min_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let min = d.iter().filter_map(as_f64)
            .fold(f64::INFINITY, f64::min);
        if min.is_infinite() { return Ok(Value::Null); }
        return Ok(Value::Float(min));
    }
    Ok(Value::Null)
}

pub fn col_max_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let max = d.iter().filter_map(as_f64)
            .fold(f64::NEG_INFINITY, f64::max);
        if max.is_infinite() { return Ok(Value::Null); }
        return Ok(Value::Float(max));
    }
    Ok(Value::Null)
}

pub fn col_std_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let vals: Vec<f64> = d.iter().filter_map(as_f64).collect();
        let n = vals.len() as f64;
        if n < 2.0 { return Ok(Value::Float(0.0)); }
        let mean = vals.iter().sum::<f64>() / n;
        let var = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
        return Ok(Value::Float(var.sqrt()));
    }
    Ok(Value::Null)
}

pub fn col_var_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let vals: Vec<f64> = d.iter().filter_map(as_f64).collect();
        let n = vals.len() as f64;
        if n < 2.0 { return Ok(Value::Float(0.0)); }
        let mean = vals.iter().sum::<f64>() / n;
        let var = vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
        return Ok(Value::Float(var));
    }
    Ok(Value::Null)
}

pub fn col_median_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let mut vals: Vec<f64> = d.iter().filter_map(as_f64).collect();
        if vals.is_empty() { return Ok(Value::Null); }
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = vals.len();
        let med = if n.is_multiple_of(2) { (vals[n/2 - 1] + vals[n/2]) / 2.0 } else { vals[n/2] };
        return Ok(Value::Float(med));
    }
    Ok(Value::Null)
}

pub fn col_quantile_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Array(data)), Some(q_val)) = (args.first(), args.get(1)) {
        let q = match q_val { Value::Float(f) => *f, Value::Int(i) => *i as f64, _ => return Ok(Value::Null) };
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let mut vals: Vec<f64> = d.iter().filter_map(as_f64).collect();
        if vals.is_empty() { return Ok(Value::Null); }
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((vals.len() - 1) as f64 * q).round() as usize;
        return Ok(Value::Float(vals[idx.min(vals.len() - 1)]));
    }
    Ok(Value::Null)
}

// ── rolling window ────────────────────────────────────────────────────────────

pub fn rolling_sum_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Array(data)), Some(Value::Int(w))) = (args.first(), args.get(1)) {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let vals = extract_floats(&Arc::new(RwLock::new(d.clone())));
        let window = *w as usize;
        let result: Vec<Value> = (0..vals.len()).map(|i| {
            if i + 1 < window { Value::Null }
            else { Value::Float(vals[(i + 1 - window)..=i].iter().sum()) }
        }).collect();
        return Ok(Value::Array(Arc::new(RwLock::new(result))));
    }
    Ok(Value::Null)
}

pub fn rolling_mean_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Array(data)), Some(Value::Int(w))) = (args.first(), args.get(1)) {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let vals = extract_floats(&Arc::new(RwLock::new(d.clone())));
        let window = *w as usize;
        let wf = window as f64;
        let result: Vec<Value> = (0..vals.len()).map(|i| {
            if i + 1 < window { Value::Null }
            else { Value::Float(vals[(i + 1 - window)..=i].iter().sum::<f64>() / wf) }
        }).collect();
        return Ok(Value::Array(Arc::new(RwLock::new(result))));
    }
    Ok(Value::Null)
}

// ── statistics ────────────────────────────────────────────────────────────────

pub fn pearson_corr_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Array(a)), Some(Value::Array(b))) = (args.first(), args.get(1)) {
        let av = extract_floats(a);
        let bv = extract_floats(b);
        let n = av.len().min(bv.len()) as f64;
        if n < 2.0 { return Ok(Value::Float(0.0)); }
        let mean_a = av.iter().sum::<f64>() / n;
        let mean_b = bv.iter().sum::<f64>() / n;
        let num: f64 = (0..n as usize).map(|i| (av[i] - mean_a) * (bv[i] - mean_b)).sum();
        let den_a: f64 = (0..n as usize).map(|i| (av[i] - mean_a).powi(2)).sum::<f64>().sqrt();
        let den_b: f64 = (0..n as usize).map(|i| (bv[i] - mean_b).powi(2)).sum::<f64>().sqrt();
        let denom = den_a * den_b;
        if denom == 0.0 { return Ok(Value::Float(0.0)); }
        return Ok(Value::Float(num / denom));
    }
    Ok(Value::Null)
}

// ── describe ──────────────────────────────────────────────────────────────────

pub fn col_describe_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let vals: Vec<f64> = d.iter().filter_map(as_f64).collect();
        let n = vals.len() as f64;
        let mut map = std::collections::HashMap::new();
        if n == 0.0 {
            map.insert("count".to_string(), Value::Float(0.0));
            return Ok(Value::Object(Arc::new(RwLock::new(map))));
        }
        let mut sorted = vals.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mean = vals.iter().sum::<f64>() / n;
        let var = if n > 1.0 { vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0) } else { 0.0 };
        let q = |p: f64| { let idx = ((n - 1.0) * p).round() as usize; sorted[idx.min(sorted.len()-1)] };
        map.insert("count".to_string(), Value::Float(n));
        map.insert("mean".to_string(), Value::Float(mean));
        map.insert("std".to_string(), Value::Float(var.sqrt()));
        map.insert("min".to_string(), Value::Float(*sorted.first().unwrap_or(&0.0)));
        map.insert("25%".to_string(), Value::Float(q(0.25)));
        map.insert("50%".to_string(), Value::Float(q(0.50)));
        map.insert("75%".to_string(), Value::Float(q(0.75)));
        map.insert("max".to_string(), Value::Float(*sorted.last().unwrap_or(&0.0)));
        return Ok(Value::Object(Arc::new(RwLock::new(map))));
    }
    Ok(Value::Null)
}

// ── CSV I/O ───────────────────────────────────────────────────────────────────

pub fn read_csv_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    // args: [path_str, delimiter_str, has_header_bool]
    let path = match args.first() { Some(Value::Str(s)) => s.clone(), _ => return Ok(Value::Null) };
    let delimiter = match args.get(1) { Some(Value::Str(s)) => s.chars().next().unwrap_or(','), _ => ',' };
    let has_header = !matches!(args.get(2), Some(Value::Bool(false)));

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Ok(Value::Null),
    };

    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() { return Ok(Value::Null); }

    let delim_str = delimiter.to_string();
    let (header_line, data_start) = if has_header { (lines[0], 1) } else { ("", 0) };

    // Parse the number of columns from first data line
    let first_data = if data_start < lines.len() { lines[data_start] } else { return Ok(Value::Null) };
    let ncols = first_data.split(&delim_str as &str).count();

    let headers: Vec<String> = if has_header {
        header_line.split(&delim_str as &str).map(|s| s.trim().to_string()).collect()
    } else {
        (0..ncols).map(|i| format!("col_{}", i)).collect()
    };

    let mut col_data: Vec<Vec<Value>> = (0..ncols).map(|_| Vec::new()).collect();

    for i in data_start..lines.len() {
        let line = lines[i].trim();
        if line.is_empty() { continue; }
        let fields: Vec<&str> = line.split(&delim_str as &str).collect();
        for (j, field) in fields.iter().enumerate() {
            if j >= ncols { break; }
            let s = field.trim();
            let val = if let Ok(f) = s.parse::<f64>() {
                Value::Float(f)
            } else if s.is_empty() || s.eq_ignore_ascii_case("null") || s.eq_ignore_ascii_case("na") {
                Value::Null
            } else {
                Value::Str(s.to_string())
            };
            col_data[j].push(val);
        }
    }

    // Return as Array of {name, data} objects
    let mut result_map = std::collections::HashMap::new();
    let columns_arr: Vec<Value> = headers.iter().zip(col_data).map(|(name, data)| {
        let mut col_map = std::collections::HashMap::new();
        col_map.insert("name".to_string(), Value::Str(name.clone()));
        col_map.insert("data".to_string(), Value::Array(Arc::new(RwLock::new(data))));
        Value::Object(Arc::new(RwLock::new(col_map)))
    }).collect();
    result_map.insert("columns".to_string(), Value::Array(Arc::new(RwLock::new(columns_arr))));
    Ok(Value::Object(Arc::new(RwLock::new(result_map))))
}

pub fn write_csv_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    // args: [col_names_array, col_data_array_of_arrays, path_str]
    let col_names_arr = match args.first() { Some(Value::Array(a)) => a.clone(), _ => return Ok(Value::Bool(false)) };
    let col_data_arr  = match args.get(1) { Some(Value::Array(a)) => a.clone(), _ => return Ok(Value::Bool(false)) };
    let path = match args.get(2) { Some(Value::Str(s)) => s.clone(), _ => return Ok(Value::Bool(false)) };

    let names = col_names_arr.read().unwrap_or_else(|e| e.into_inner());
    let cols = col_data_arr.read().unwrap_or_else(|e| e.into_inner());
    let ncols = names.len().min(cols.len());
    if ncols == 0 { return Ok(Value::Bool(false)); }

    // Number of rows from first column
    let nrows = match &cols[0] { Value::Array(a) => a.read().unwrap_or_else(|e| e.into_inner()).len(), _ => 0 };

    let mut out = String::new();
    // Header
    for (i, n) in names.iter().enumerate() {
        if i > 0 { out.push(','); }
        if let Value::Str(s) = n { out.push_str(s); }
    }
    out.push('\n');

    for r in 0..nrows {
        for c in 0..ncols {
            if c > 0 { out.push(','); }
            if let Value::Array(col_arr) = &cols[c] {
                let cd = col_arr.read().unwrap_or_else(|e| e.into_inner());
                if r < cd.len() {
                    match &cd[r] {
                        Value::Float(f) => out.push_str(&f.to_string()),
                        Value::Int(i)   => out.push_str(&i.to_string()),
                        Value::Str(s)   => { out.push('"'); out.push_str(s); out.push('"'); }
                        Value::Bool(b)  => out.push_str(&b.to_string()),
                        Value::Null     => {},
                        _ => {}
                    }
                }
            }
        }
        out.push('\n');
    }

    match std::fs::write(&path, out) {
        Ok(_) => Ok(Value::Bool(true)),
        Err(_) => Ok(Value::Bool(false)),
    }
}

// ── value_counts ──────────────────────────────────────────────────────────────
pub fn value_counts_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for v in d.iter() {
            let key = match v {
                Value::Str(s) => s.clone(),
                Value::Int(i) => i.to_string(),
                Value::Float(f) => f.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => "null".to_string(),
            };
            if !counts.contains_key(&key) { order.push(key.clone()); }
            *counts.entry(key).or_insert(0) += 1;
        }
        let result: Vec<Value> = order.iter().map(|k| {
            let mut m = std::collections::HashMap::new();
            m.insert("value".to_string(), Value::Str(k.clone()));
            m.insert("count".to_string(), Value::Int(*counts.get(k).unwrap_or(&0)));
            Value::Object(Arc::new(RwLock::new(m)))
        }).collect();
        return Ok(Value::Array(Arc::new(RwLock::new(result))));
    }
    Ok(Value::Null)
}

// ── t-test (Welch) ────────────────────────────────────────────────────────────
pub fn t_test_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Array(a)), Some(Value::Array(b))) = (args.first(), args.get(1)) {
        let av = extract_floats(a);
        let bv = extract_floats(b);
        let na = av.len() as f64;
        let nb = bv.len() as f64;
        if na < 2.0 || nb < 2.0 { return Ok(Value::Null); }
        let mean_a = av.iter().sum::<f64>() / na;
        let mean_b = bv.iter().sum::<f64>() / nb;
        let var_a = av.iter().map(|x| (x - mean_a).powi(2)).sum::<f64>() / (na - 1.0);
        let var_b = bv.iter().map(|x| (x - mean_b).powi(2)).sum::<f64>() / (nb - 1.0);
        let se = ((var_a / na) + (var_b / nb)).sqrt();
        let t = if se == 0.0 { 0.0 } else { (mean_a - mean_b) / se };
        let mut m = std::collections::HashMap::new();
        m.insert("t_stat".to_string(), Value::Float(t));
        m.insert("mean_a".to_string(), Value::Float(mean_a));
        m.insert("mean_b".to_string(), Value::Float(mean_b));
        return Ok(Value::Object(Arc::new(RwLock::new(m))));
    }
    Ok(Value::Null)
}

// ── normalize / standardize ───────────────────────────────────────────────────
pub fn col_normalize_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let vals: Vec<f64> = d.iter().filter_map(as_f64).collect();
        if vals.is_empty() { return Ok(Value::Array(Arc::new(RwLock::new(vec![])))); }
        let mn = vals.iter().cloned().fold(f64::INFINITY, f64::min);
        let mx = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = mx - mn;
        let result: Vec<f64> = vals.iter().map(|v| if range == 0.0 { 0.0 } else { (v - mn) / range }).collect();
        return Ok(floats_to_value_arr(result));
    }
    Ok(Value::Null)
}

pub fn col_standardize_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let vals: Vec<f64> = d.iter().filter_map(as_f64).collect();
        if vals.is_empty() { return Ok(Value::Array(Arc::new(RwLock::new(vec![])))); }
        let n = vals.len() as f64;
        let mean = vals.iter().sum::<f64>() / n;
        let std_dev = (vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt();
        let result: Vec<f64> = vals.iter().map(|v| if std_dev == 0.0 { 0.0 } else { (v - mean) / std_dev }).collect();
        return Ok(floats_to_value_arr(result));
    }
    Ok(Value::Null)
}

// ── encode categorical ────────────────────────────────────────────────────────
pub fn encode_categorical_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let mut mapping: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        let mut next_id: i64 = 0;
        let encoded: Vec<Value> = d.iter().map(|v| {
            let key = match v {
                Value::Str(s) => s.clone(),
                _ => format!("{:?}", v),
            };
            let id = *mapping.entry(key).or_insert_with(|| { let id = next_id; next_id += 1; id });
            Value::Int(id)
        }).collect();
        // Return {encoded, mapping}
        let mapping_val: Vec<Value> = mapping.iter().map(|(k, v)| {
            let mut m = std::collections::HashMap::new();
            m.insert("key".to_string(), Value::Str(k.clone()));
            m.insert("id".to_string(), Value::Int(*v));
            Value::Object(Arc::new(RwLock::new(m)))
        }).collect();
        let mut result = std::collections::HashMap::new();
        result.insert("encoded".to_string(), Value::Array(Arc::new(RwLock::new(encoded))));
        result.insert("mapping".to_string(), Value::Array(Arc::new(RwLock::new(mapping_val))));
        return Ok(Value::Object(Arc::new(RwLock::new(result))));
    }
    Ok(Value::Null)
}
// ── Engine V2 Bridge ────────────────────────────────────────────────────────

pub fn scan_csv_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let path = match args.first() { Some(Value::Str(s)) => s.clone(), _ => return Ok(Value::Null) };
    let mut options = std::collections::HashMap::new();
    if let Some(Value::Object(opt_rc)) = args.get(1) {
        let opts = opt_rc.read().unwrap_or_else(|e| e.into_inner());
        for (k, v) in opts.iter() {
            options.insert(k.clone(), value_to_string(v));
        }
    }
    
    let mut obj = std::collections::HashMap::new();
    obj.insert("_op".to_string(), Value::Str("p_scan".to_string()));
    obj.insert("source_id".to_string(), Value::Str(path));
    
    let mut opt_nyx = std::collections::HashMap::new();
    for (k, v) in options { opt_nyx.insert(k, Value::Str(v)); }
    obj.insert("options".to_string(), Value::Object(Arc::new(RwLock::new(opt_nyx))));
    
    Ok(Value::Object(Arc::new(RwLock::new(obj))))
}

pub fn scan_json_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let path = match args.first() { Some(Value::Str(s)) => s.clone(), _ => return Ok(Value::Null) };
    let mut obj = std::collections::HashMap::new();
    obj.insert("_op".to_string(), Value::Str("p_scan".to_string()));
    obj.insert("source_id".to_string(), Value::Str(path));
    Ok(Value::Object(Arc::new(RwLock::new(obj))))
}

pub fn execute_plan_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(plan_val) = args.first() {
        let logical_plan = value_to_logical_plan(plan_val)?;
        let mut ctx = df_engine::ExecutionContext { sources: HashMap::new() };
        
        // Extract sources if provided in 2nd arg
        if let Some(Value::Object(sources_rc)) = args.get(1) {
            let sources = sources_rc.read().unwrap_or_else(|e| e.into_inner());
            for (id, val) in sources.iter() {
                if let Value::Object(df_rc) = val {
                    let df = df_rc.read().unwrap_or_else(|e| e.into_inner());
                    let chunk = df_to_data_chunk(&df)?;
                    let fields = chunk.columns.iter().map(|c| Field { 
                    name: c.name.clone(), 
                    dtype: "dynamic".to_string(), 
                    nullable: true 
                }).collect();
                let schema = Schema::new(fields);
                    ctx.sources.insert(id.clone(), Box::new(df_engine::MemoryDataSource {
                        chunks: vec![chunk],
                        cursor: 0,
                        schema,
                    }));
                }
            }
        }

        let mut physical_plan = df_engine::create_physical_plan(&logical_plan, &mut ctx)
            .map_err(|e| EvalError { message: e, stack: vec![] })?;
        
        let mut all_chunks = Vec::new();
        while let Some(chunk) = physical_plan.next_chunk()
            .map_err(|e| EvalError { message: e, stack: vec![] })? {
            all_chunks.push(chunk);
        }
        
        return data_chunks_to_dataframe(all_chunks);
    }
    Err(EvalError { message: "execute_plan requires 1 or 2 arguments".to_string(), stack: vec![] })
}

fn value_to_logical_plan(v: &Value) -> Result<LogicalPlan, EvalError> {
    if let Value::Object(obj_rc) = v {
        let obj = obj_rc.read().unwrap_or_else(|e| e.into_inner());
        let op = obj.get("_op").and_then(|v| match v { Value::Str(s) => Some(s.as_str()), _ => None })
            .ok_or_else(|| EvalError { message: "Plan node missing '_op'".to_string(), stack: vec![] })?;
        
        match op {
            "p_projection" => {
                let input_val = obj.get("input").ok_or_else(|| EvalError { message: "Projection missing input".to_string(), stack: vec![] })?;
                let input = Box::new(value_to_logical_plan(input_val)?);
                let exprs_val = obj.get("exprs").and_then(|v| match v { Value::Array(a) => Some(a), _ => None })
                    .ok_or_else(|| EvalError { message: "Projection missing exprs array".to_string(), stack: vec![] })?;
                
                let mut exprs = Vec::new();
                let mut names = Vec::new();
                let e_arr = exprs_val.read().unwrap_or_else(|e| e.into_inner());
                for e in e_arr.iter() {
                    let (expr, name) = value_to_named_expr(e)?;
                    exprs.push(expr);
                    names.push(name);
                }
                Ok(LogicalPlan::Projection { input, exprs, names })
            }
            "p_filter" => {
                let input_val = obj.get("input").ok_or_else(|| EvalError { message: "Filter missing input".to_string(), stack: vec![] })?;
                let input = Box::new(value_to_logical_plan(input_val)?);
                let pred_val = obj.get("predicate").ok_or_else(|| EvalError { message: "Filter missing predicate".to_string(), stack: vec![] })?;
                let predicate = value_to_expr(pred_val)?;
                Ok(LogicalPlan::Filter { input, predicate })
            }
            "p_scan" => {
                let id = obj.get("source_id").and_then(|v| match v { Value::Str(s) => Some(s.clone()), _ => None })
                    .ok_or_else(|| EvalError { message: "Scan missing source_id".to_string(), stack: vec![] })?;
                let mut options = None;
                if let Some(Value::Object(opt_rc)) = obj.get("options") {
                    let opt_map = opt_rc.read().unwrap_or_else(|e| e.into_inner());
                    let mut res_map = std::collections::HashMap::new();
                    for (k, v) in opt_map.iter() {
                        if let Value::Str(s) = v { res_map.insert(k.clone(), s.clone()); }
                    }
                    options = Some(res_map);
                }
                Ok(LogicalPlan::Scan { source_id: id, projection: None, options, schema: None })
            }
            "p_aggregate" => {
                let input_val = obj.get("input").ok_or_else(|| EvalError { message: "Aggregate missing input".to_string(), stack: vec![] })?;
                let input = Box::new(value_to_logical_plan(input_val)?);
                let keys_val = obj.get("keys").and_then(|v| match v { Value::Array(a) => Some(a), _ => None })
                    .ok_or_else(|| EvalError { message: "Aggregate missing keys array".to_string(), stack: vec![] })?;
                let aggs_val = obj.get("aggs").and_then(|v| match v { Value::Array(a) => Some(a), _ => None })
                    .ok_or_else(|| EvalError { message: "Aggregate missing aggs array".to_string(), stack: vec![] })?;
                
                let mut keys = Vec::new();
                let mut key_names = Vec::new();
                for k in keys_val.read().unwrap_or_else(|e| e.into_inner()).iter() {
                    let (expr, name) = value_to_named_expr(k)?;
                    keys.push(expr);
                    key_names.push(name);
                }
                
                let mut aggs = Vec::new();
                let mut ops = Vec::new();
                let mut agg_names = Vec::new();
                for a in aggs_val.read().unwrap_or_else(|e| e.into_inner()).iter() { 
                    let (expr, name) = value_to_named_expr(a)?;
                    aggs.push(expr);
                    agg_names.push(name);
                    
                    let mut op_str = "sum".to_string();
                    if let Value::Object(obj_rc) = a {
                        let obj = obj_rc.read().unwrap_or_else(|e| e.into_inner());
                        let kind = obj.get("_kind").and_then(|v| match v { Value::Str(s) => Some(s.as_str()), _ => None }).unwrap_or("");
                        
                        if kind == "agg" {
                            op_str = obj.get("_value").and_then(|v| match v { Value::Str(s) => Some(s.clone()), _ => None }).unwrap_or_else(|| "sum".to_string());
                        } else if kind == "alias" {
                            let args = obj.get("_args").and_then(|v| match v { Value::Array(ar) => Some(ar), _ => None });
                            if let Some(ar_rc) = args {
                                let ar = ar_rc.read().unwrap_or_else(|e| e.into_inner());
                                if !ar.is_empty() {
                                    if let Value::Object(inner_rc) = &ar[0] {
                                        let inner = inner_rc.read().unwrap_or_else(|e| e.into_inner());
                                        let inner_kind = inner.get("_kind").and_then(|v| match v { Value::Str(s) => Some(s.as_str()), _ => None }).unwrap_or("");
                                        if inner_kind == "agg" {
                                            op_str = inner.get("_value").and_then(|v| match v { Value::Str(s) => Some(s.clone()), _ => None }).unwrap_or_else(|| "sum".to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    let agg_op = match op_str.as_str() {
                        "sum" => df_engine::AggregateOp::Sum,
                        "mean" | "avg" => df_engine::AggregateOp::Mean,
                        "count" => df_engine::AggregateOp::Count,
                        "min" => df_engine::AggregateOp::Min,
                        "max" => df_engine::AggregateOp::Max,
                        _ => df_engine::AggregateOp::Sum,
                    };
                    ops.push(agg_op);
                }
                
                Ok(LogicalPlan::Aggregate { input, keys, aggs, ops, key_names, agg_names })
            }
            "p_join" => {
                let left_val = obj.get("left").ok_or_else(|| EvalError { message: "Join missing left".to_string(), stack: vec![] })?;
                let right_val = obj.get("right").ok_or_else(|| EvalError { message: "Join missing right".to_string(), stack: vec![] })?;
                let left = Box::new(value_to_logical_plan(left_val)?);
                let right = Box::new(value_to_logical_plan(right_val)?);
                let on_left = obj.get("on_left").and_then(|v| match v { Value::Str(s) => Some(s.clone()), _ => None }).unwrap_or_default();
                let on_right = obj.get("on_right").and_then(|v| match v { Value::Str(s) => Some(s.clone()), _ => None }).unwrap_or_default();
                Ok(LogicalPlan::Join { left, right, on_left, on_right, join_type: df_engine::JoinType::Inner })
            }
            "p_sort" => {
                let input_val = obj.get("input").ok_or_else(|| EvalError { message: "Sort missing input".to_string(), stack: vec![] })?;
                let input = Box::new(value_to_logical_plan(input_val)?);
                let col = obj.get("column").and_then(|v| match v { Value::Str(s) => Some(s.clone()), _ => None }).unwrap_or_default();
                let asc = obj.get("ascending").and_then(|v| match v { Value::Bool(b) => Some(*b), _ => None }).unwrap_or(true);
                Ok(LogicalPlan::Sort { input, column: col, ascending: asc })
            }
            "p_limit" => {
                let input_val = obj.get("input").ok_or_else(|| EvalError { message: "Limit missing input".to_string(), stack: vec![] })?;
                let input = Box::new(value_to_logical_plan(input_val)?);
                let n = obj.get("n").and_then(|v| match v { Value::Int(i) => Some(*i as usize), _ => None }).unwrap_or(0);
                Ok(LogicalPlan::Limit { input, n })
            }
            _ => Err(EvalError { message: format!("Unknown logical op: {}", op), stack: vec![] }),
        }
    } else {
        Err(EvalError { message: "Plan node must be an object".to_string(), stack: vec![] })
    }
}

fn value_to_named_expr(v: &Value) -> Result<(Expr, String), EvalError> {
    if let Value::Str(s) = v {
        return Ok((Expr::ident(s.clone()), s.clone()));
    }
    if let Value::Object(obj_rc) = v {
        let obj = obj_rc.read().unwrap_or_else(|e| e.into_inner());
        let kind = obj.get("_kind").and_then(|v| match v { Value::Str(s) => Some(s.as_str()), _ => None })
            .ok_or_else(|| EvalError { message: "Expr node missing '_kind'".to_string(), stack: vec![] })?;
        
        if kind == "alias" {
            let name = obj.get("_value").and_then(|v| match v { Value::Str(s) => Some(s.clone()), _ => None })
                .ok_or_else(|| EvalError { message: "alias missing name".to_string(), stack: vec![] })?;
            let args_val = obj.get("_args").and_then(|v| match v { Value::Array(a) => Some(a), _ => None })
                .ok_or_else(|| EvalError { message: "alias missing args".to_string(), stack: vec![] })?;
            let args = args_val.read().unwrap_or_else(|e| e.into_inner());
            if args.is_empty() { return Err(EvalError { message: "alias requires 1 arg".to_string(), stack: vec![] }); }
            let (inner_expr, _) = value_to_named_expr(&args[0])?;
            return Ok((inner_expr, name));
        }
        
        let expr = value_to_expr(v)?;
        let name = match &expr {
            Expr::Identifier { name: s, .. } => s.clone(),
            _ => "column".to_string(),
        };
        Ok((expr, name))
    } else {
        let expr = value_to_expr(v)?;
        Ok((expr, "column".to_string()))
    }
}

fn value_to_expr(v: &Value) -> Result<Expr, EvalError> {
    if let Value::Str(s) = v {
        return Ok(Expr::ident(s.clone()));
    }
    if let Value::Object(obj_rc) = v {
        let obj = obj_rc.read().unwrap_or_else(|e| e.into_inner());
        let kind = obj.get("_kind").and_then(|v| match v { Value::Str(s) => Some(s.as_str()), _ => None })
            .ok_or_else(|| EvalError { message: "Expr node missing '_kind'".to_string(), stack: vec![] })?;
        
        match kind {
            "col" => {
                let name = obj.get("_value").and_then(|v| match v { Value::Str(s) => Some(s.clone()), _ => None })
                    .ok_or_else(|| EvalError { message: "col expr missing name".to_string(), stack: vec![] })?;
                Ok(Expr::ident(name))
            }
            "lit" => {
                let val = obj.get("_value").ok_or_else(|| EvalError { message: "lit expr missing value".to_string(), stack: vec![] })?;
                match val {
                    Value::Int(i) => Ok(Expr::int(*i)),
                    Value::Float(f) => Ok(Expr::float(*f)),
                    Value::Bool(b) => Ok(Expr::bool(*b)),
                    Value::Str(s) => Ok(Expr::string(s.clone())),
                    _ => Err(EvalError { message: "Unsupported literal value".to_string(), stack: vec![] }),
                }
            }
            "binop" => {
                let op_str = obj.get("_value").and_then(|v| match v { Value::Str(s) => Some(s.as_str()), _ => None })
                    .ok_or_else(|| EvalError { message: "binop missing op string".to_string(), stack: vec![] })?;
                
                let args_val = obj.get("_args").and_then(|v| match v { Value::Array(a) => Some(a), _ => None })
                    .ok_or_else(|| EvalError { message: "binop missing args".to_string(), stack: vec![] })?;
                let args = args_val.read().unwrap_or_else(|e| e.into_inner());
                if args.len() != 2 { return Err(EvalError { message: "binop must have 2 args".to_string(), stack: vec![] }); }
                
                let left = value_to_expr(&args[0])?;
                let right = value_to_expr(&args[1])?;
                Ok(Expr::Binary { left: Box::new(left), op: op_str.to_string(), right: Box::new(right), span: crate::core::diagnostics::Span::default() })
            }
            "agg" | "alias" | "cast" | "unary" | "fill_null" => {
                let args_val = obj.get("_args").and_then(|v| match v { Value::Array(a) => Some(a), _ => None })
                    .ok_or_else(|| EvalError { message: "expr missing args".to_string(), stack: vec![] })?;
                let args = args_val.read().unwrap_or_else(|e| e.into_inner());
                if args.is_empty() { return Err(EvalError { message: "expr must have at least 1 arg".to_string(), stack: vec![] }); }
                value_to_expr(&args[0])
            }
            _ => Err(EvalError { message: format!("Unsupported expr kind: {}", kind), stack: vec![] }),
        }
    } else {
        match v {
            Value::Int(i) => Ok(Expr::int(*i)),
            Value::Float(f) => Ok(Expr::float(*f)),
            Value::Bool(b) => Ok(Expr::bool(*b)),
            Value::Str(s) => Ok(Expr::string(s.clone())),
            _ => Err(EvalError { message: "Invalid expr value".to_string(), stack: vec![] }),
        }
    }
}

fn df_to_data_chunk(df: &HashMap<String, Value>) -> Result<DataChunk, EvalError> {
    // This is a bridge from AST DataFrame (Map of columns) to DataChunk (Vectorized)
    let columns_val = df.get("_columns").and_then(|v| match v { Value::Array(a) => Some(a), _ => None })
        .ok_or_else(|| EvalError { message: "DataFrame missing _columns".to_string(), stack: vec![] })?;
    
    let cols = columns_val.read().unwrap_or_else(|e| e.into_inner());
    let mut dc_cols = Vec::with_capacity(cols.len());
    let mut size = 0;
    
    for c_val in cols.iter() {
        if let Value::Object(c_rc) = c_val {
            let c = c_rc.read().unwrap_or_else(|e| e.into_inner());
            let name = match c.get("_name") { Some(Value::Str(s)) => s.clone(), _ => "unknown".to_string() };
            
            let data = match c.get("_data") {
                Some(Value::Array(a)) => a.read().unwrap_or_else(|e| e.into_inner()).clone(),
                Some(Value::DoubleArray(da)) => {
                    let d = da.read().unwrap_or_else(|e| e.into_inner());
                    d.iter().map(|&f| Value::Float(f)).collect::<Vec<_>>()
                },
                _ => return Err(EvalError { message: format!("Column '{}' missing _data or incorrect type", name), stack: vec![] }),
            };
            
            if size == 0 { size = data.len(); }
            
            let first_val = data.first().unwrap_or(&Value::Null);
            let dc_col = match first_val {
                Value::Str(_) => {
                    let mut cb = df_engine::ColumnBuilder::new("str");
                    for v in data.iter() {
                        match v {
                            Value::Str(s) => cb.append_str(s),
                            Value::Null => cb.append_null(),
                            _ => cb.append_str(&format!("{:?}", v)),
                        }
                    }
                    cb.build(name)
                }
                Value::Bool(_) => {
                    let mut cb = df_engine::ColumnBuilder::new("bool");
                    for v in data.iter() {
                        match v {
                            Value::Bool(b) => cb.append_bool(*b),
                            Value::Null => cb.append_null(),
                            _ => cb.append_bool(false),
                        }
                    }
                    cb.build(name)
                }
                Value::Int(_) => {
                    let mut cb = df_engine::ColumnBuilder::new("int");
                    for v in data.iter() {
                        match v {
                            Value::Int(i) => cb.append_int(*i),
                            Value::Float(f) => cb.append_int(*f as i64),
                            Value::Null => cb.append_null(),
                            _ => cb.append_int(0),
                        }
                    }
                    cb.build(name)
                }
                _ => {
                    let mut cb = df_engine::ColumnBuilder::new("float");
                    for v in data.iter() {
                        match v {
                            Value::Float(f) => cb.append_float(*f),
                            Value::Int(i) => cb.append_float(*i as f64),
                            Value::Null => cb.append_null(),
                            _ => cb.append_float(0.0),
                        }
                    }
                    cb.build(name)
                }
            };
            dc_cols.push(dc_col);
        }
    }
    
    Ok(DataChunk::new(dc_cols, size))
}

fn data_chunks_to_dataframe(chunks: Vec<DataChunk>) -> Result<Value, EvalError> {
    // Materialize chunks back into a Nyx DataFrame Map
    if chunks.is_empty() { return Ok(Value::Null); }
    
    let chunk = &chunks[0];
    let mut nyx_cols = Vec::new();
    
    for col in &chunk.columns {
        let mut nyx_c: HashMap<String, Value> = HashMap::new();
        nyx_c.insert("_name".to_string(), Value::Str(col.name.clone()));
        
        // Infer dtype from ColumnData
        let dtype = match &col.data {
            df_engine::ColumnData::I64(_) => "int",
            df_engine::ColumnData::F64(_) => "float",
            df_engine::ColumnData::Str { .. } => "str",
            df_engine::ColumnData::Bool(_) => "bool",
            df_engine::ColumnData::Bitmap(_) => "bool",
            df_engine::ColumnData::Categorical { .. } => "cat",
        };
        nyx_c.insert("_dtype".to_string(), Value::Str(dtype.to_string()));
        
        let mut vals = Vec::with_capacity(col.len());
        for i in 0..col.len() {
            vals.push(col.get_value(i));
        }
        
        nyx_c.insert("_data".to_string(), Value::Array(Arc::new(RwLock::new(vals))));
        nyx_cols.push(Value::Object(Arc::new(RwLock::new(nyx_c))));
    }
    
    let mut df: HashMap<String, Value> = HashMap::new();
    df.insert("_columns".to_string(), Value::Array(Arc::new(RwLock::new(nyx_cols))));
    Ok(Value::Object(Arc::new(RwLock::new(df))))
}

pub fn scan_parquet_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Str(path)) = args.first() {
        let mut obj = std::collections::HashMap::new();
        obj.insert("_op".to_string(), Value::Str("p_scan".to_string()));
        obj.insert("source_id".to_string(), Value::Str(path.clone()));
        return Ok(Value::Object(Arc::new(RwLock::new(obj))));
    }
    Err(EvalError { message: "scan_parquet expects a path string".to_string(), stack: vec![] })
}

pub fn check_health_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Object(df_rc)) = args.first() {
        let df = df_rc.read().unwrap_or_else(|e| e.into_inner());
        let mut report = std::collections::HashMap::new();
        let cols_val = df.get("_columns").and_then(|v| match v { Value::Array(a) => Some(a), _ => None }).ok_or_else(|| EvalError{message:"Invalid DF".to_string(), stack:vec![]})?;
        let cols = cols_val.read().unwrap_or_else(|e| e.into_inner());
        
        for c_val in cols.iter() {
            if let Value::Object(c_rc) = c_val {
                let c = c_rc.read().unwrap_or_else(|e| e.into_inner());
                let name = c.get("_name").and_then(|v| match v { Value::Str(s) => Some(s.clone()), _ => None }).unwrap_or_default();
                let data_val = c.get("_data").and_then(|v| match v { Value::Array(a) => Some(a), _ => None }).ok_or_else(|| EvalError{message:"Column missing data".to_string(), stack:vec![]})?;
                let data = data_val.read().unwrap_or_else(|e| e.into_inner());
                
                let mut nans = 0;
                let mut nulls = 0;
                for v in data.iter() {
                    match v {
                        Value::Float(f) if f.is_nan() => nans += 1,
                        Value::Null => nulls += 1,
                        _ => {}
                    }
                }
                let mut col_report = std::collections::HashMap::new();
                col_report.insert("nans".to_string(), Value::Int(nans as i64));
                col_report.insert("nulls".to_string(), Value::Int(nulls as i64));
                report.insert(name, Value::Object(Arc::new(RwLock::new(col_report))));
            }
        }
        return Ok(Value::Object(Arc::new(RwLock::new(report))));
    }
    Ok(Value::Null)
}

pub fn write_json_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if args.len() < 2 { return Ok(Value::Null); }
    let df_obj = &args[0];
    let path = match &args[1] {
        Value::Str(s) => s.clone(),
        _ => return Err(EvalError { message: "Path must be a string".to_string(), stack: vec![] }),
    };

    // Extract columns from DataFrame object
    let columns = if let Value::Object(obj) = df_obj {
        let obj_read = obj.read().unwrap_or_else(|e| e.into_inner());
        if let Some(Value::Array(cols)) = obj_read.get("_columns") {
            cols.read().unwrap_or_else(|e| e.into_inner()).clone()
        } else {
            return Err(EvalError { message: "Invalid DataFrame object: _columns missing".to_string(), stack: vec![] });
        }
    } else {
        return Err(EvalError { message: "Expected DataFrame object".to_string(), stack: vec![] });
    };

    if columns.is_empty() {
        return Ok(Value::Bool(true));
    }

    let mut file = std::fs::File::create(&path).map_err(|e| EvalError { message: e.to_string(), stack: vec![] })?;
    use std::io::Write;

    let num_rows = {
        if let Value::Object(col) = &columns[0] {
            let col_read = col.read().unwrap_or_else(|e| e.into_inner());
            if let Some(Value::Array(data)) = col_read.get("_data") {
                data.read().unwrap_or_else(|e| e.into_inner()).len()
            } else { 0 }
        } else { 0 }
    };

    for i in 0..num_rows {
        let mut row_obj = serde_json::Map::new();
        for col_val in &columns {
            if let Value::Object(col) = col_val {
                let col_read = col.read().unwrap_or_else(|e| e.into_inner());
                let name = match col_read.get("_name") {
                    Some(Value::Str(s)) => s.clone(),
                    _ => "unknown".to_string(),
                };
                if let Some(Value::Array(data)) = col_read.get("_data") {
                    let data_read = data.read().unwrap_or_else(|e| e.into_inner());
                    if i < data_read.len() {
                        let val = match &data_read[i] {
                            Value::Int(v) => serde_json::Value::Number((*v).into()),
                            Value::Float(v) => {
                                if let Some(n) = serde_json::Number::from_f64(*v) {
                                    serde_json::Value::Number(n)
                                } else {
                                    serde_json::Value::Null
                                }
                            }
                            Value::Str(v) => serde_json::Value::String(v.clone()),
                            Value::Bool(v) => serde_json::Value::Bool(*v),
                            Value::Null => serde_json::Value::Null,
                            _ => serde_json::Value::String(format!("{:?}", data_read[i])),
                        };
                        row_obj.insert(name, val);
                    }
                }
            }
        }
        let json_str = serde_json::to_string(&row_obj).map_err(|e| EvalError { message: e.to_string(), stack: vec![] })?;
        writeln!(file, "{}", json_str).map_err(|e| EvalError { message: e.to_string(), stack: vec![] })?;
    }

    Ok(Value::Bool(true))
}

// ── Phase 4: RFC 4180 CSV field parser ───────────────────────────────────────

/// Parse a CSV line respecting double-quoted fields (RFC 4180).
pub fn parse_csv_line_rfc4180(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_quotes {
                    if chars.peek() == Some(&'"') { chars.next(); current.push('"'); }
                    else { in_quotes = false; }
                } else { in_quotes = true; }
            }
            ch if ch == delimiter && !in_quotes => {
                fields.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(c),
        }
    }
    fields.push(current.trim().to_string());
    fields
}

pub fn read_parquet_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let path = match args.first() { Some(Value::Str(s)) => s.clone(), _ => return Ok(Value::Null) };
    println!("[df.io] Parquet reader: path='{}' — real Parquet requires the 'parquet2' feature. Returning null.", path);
    Ok(Value::Null)
}

pub fn write_parquet_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    println!("[df.io] Parquet writer: real Parquet requires the 'parquet2' feature. Use write_csv for now.");
    Ok(Value::Bool(false))
}

// ── Phase 5: Window Functions ─────────────────────────────────────────────────

/// Rank values (1-based, dense rank within ties) in a column array.
pub fn window_rank_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let ascending = !matches!(args.get(1), Some(Value::Bool(false)));
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let sorted_vals: Vec<f64> = d.iter().map(|v| as_f64(v).unwrap_or(f64::NAN)).collect();
        let n = sorted_vals.len();
        let mut indexed: Vec<(usize, f64)> = sorted_vals.iter().cloned().enumerate().collect();
        if ascending {
            indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        } else {
            indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        }
        let mut rank_map = vec![0i64; n];
        let mut rank = 1i64;
        let mut i = 0;
        while i < indexed.len() {
            let cur = indexed[i].1;
            let mut j = i;
            while j < indexed.len() && (indexed[j].1 - cur).abs() < 1e-12 {
                rank_map[indexed[j].0] = rank;
                j += 1;
            }
            rank += (j - i) as i64;
            i = j;
        }
        let result: Vec<Value> = rank_map.into_iter().map(Value::Int).collect();
        return Ok(Value::Array(Arc::new(RwLock::new(result))));
    }
    Ok(Value::Null)
}

/// Assign sequential row numbers (1-based).
pub fn window_row_number_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let n = data.read().unwrap_or_else(|e| e.into_inner()).len();
        let result: Vec<Value> = (1..=(n as i64)).map(Value::Int).collect();
        return Ok(Value::Array(Arc::new(RwLock::new(result))));
    }
    Ok(Value::Null)
}

/// Apply a Nyx closure (row value → value) to each element — row-by-row UDF.
pub fn apply_native(vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Array(data)), Some(Value::Closure(clo))) = (args.first(), args.get(1)) {
        let d = data.read().unwrap_or_else(|e| e.into_inner()).clone();
        let mut result = Vec::with_capacity(d.len());
        for val in d.iter() {
            let out = vm.call_closure(clo, vec![val.clone()])?;
            result.push(out);
        }
        return Ok(Value::Array(Arc::new(RwLock::new(result))));
    }
    Ok(Value::Null)
}

/// Apply a Nyx closure to the entire column array at once — column-level UDF.
pub fn apply_col_native(vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(arr), Some(Value::Closure(clo))) = (args.first(), args.get(1)) {
        let out = vm.call_closure(clo, vec![arr.clone()])?;
        return Ok(out);
    }
    Ok(Value::Null)
}

// ── Phase 5: Time-Series Kernels ─────────────────────────────────────────────

/// Percentage change: (val[i] - val[i-n]) / val[i-n]
pub fn pct_change_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let n = match args.get(1) { Some(Value::Int(i)) => *i as usize, _ => 1 };
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let vals: Vec<f64> = d.iter().map(|v| as_f64(v).unwrap_or(f64::NAN)).collect();
        let result: Vec<Value> = (0..vals.len()).map(|i| {
            if i < n { Value::Null }
            else {
                let prev = vals[i - n];
                if prev == 0.0 { Value::Null } else { Value::Float((vals[i] - prev) / prev) }
            }
        }).collect();
        return Ok(Value::Array(Arc::new(RwLock::new(result))));
    }
    Ok(Value::Null)
}

/// Exponential weighted mean with smoothing factor alpha.
pub fn ewm_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let alpha = match args.get(1) {
            Some(Value::Float(f)) => *f,
            Some(Value::Int(i))   => *i as f64,
            _ => 0.5,
        };
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let mut result = Vec::with_capacity(d.len());
        let mut ema = f64::NAN;
        for v in d.iter() {
            let x = as_f64(v).unwrap_or(f64::NAN);
            if ema.is_nan() { ema = x; } else { ema = alpha * x + (1.0 - alpha) * ema; }
            result.push(Value::Float(ema));
        }
        return Ok(Value::Array(Arc::new(RwLock::new(result))));
    }
    Ok(Value::Null)
}

/// Resample: group by integer bucket index and aggregate.
/// args: [data_array, bucket_size: int, agg_op: str ("sum"|"mean"|"count"|"min"|"max")]
pub fn resample_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Array(data)) = args.first() {
        let bucket_size = match args.get(1) { Some(Value::Int(i)) => (*i as usize).max(1), _ => 1 };
        let agg_op = match args.get(2) { Some(Value::Str(s)) => s.as_str().to_string(), _ => "sum".to_string() };
        let d = data.read().unwrap_or_else(|e| e.into_inner());
        let vals: Vec<f64> = d.iter().map(|v| as_f64(v).unwrap_or(0.0)).collect();
        let mut result = Vec::new();
        let mut i = 0;
        while i < vals.len() {
            let end = (i + bucket_size).min(vals.len());
            let bucket = &vals[i..end];
            let agg = match agg_op.as_str() {
                "sum"   => bucket.iter().sum::<f64>(),
                "mean"  => bucket.iter().sum::<f64>() / bucket.len() as f64,
                "count" => bucket.len() as f64,
                "min"   => bucket.iter().cloned().fold(f64::INFINITY, f64::min),
                "max"   => bucket.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                _       => bucket.iter().sum::<f64>(),
            };
            result.push(Value::Float(agg));
            i += bucket_size;
        }
        return Ok(Value::Array(Arc::new(RwLock::new(result))));
    }
    Ok(Value::Null)
}

// ── Phase 6: ML / Tensor Bridge ──────────────────────────────────────────────

/// Convert a Nyx tensor {data:[], shape:[rows,cols]} to a DataFrame-like object.
pub fn to_tensor_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Object(df_rc)), Some(Value::Str(col_name))) = (args.first(), args.get(1)) {
        let df = df_rc.read().unwrap_or_else(|e| e.into_inner());
        let cols = match df.get("_columns") { 
            Some(Value::Array(a)) => a.read().unwrap_or_else(|e| e.into_inner()),
            _ => return Ok(Value::Null),
        };
        
        for c_val in cols.iter() {
            if let Value::Object(c_rc) = c_val {
                let c = c_rc.read().unwrap_or_else(|e| e.into_inner());
                let name = match c.get("_name") { Some(Value::Str(s)) => s, _ => "" };
                if name == col_name {
                    let data_val = c.get("_data").and_then(|v| match v { Value::Array(a) => Some(a.read().unwrap_or_else(|e| e.into_inner())), _ => None })
                        .ok_or_else(|| EvalError{message:"Column missing data".to_string(), stack:vec![]})?;
                    
                    let mut float_data = Vec::with_capacity(data_val.len());
                    for v in data_val.iter() {
                        float_data.push(v.as_f64().unwrap_or(0.0) as f32);
                    }
                    
                    let shape = vec![float_data.len()];
                    let storage = TensorStorage::Cpu(Arc::new(RwLock::new(float_data)));
                    return Ok(Value::Tensor(storage, shape));
                }
            }
        }
    }
    Ok(Value::Null)
}

/// Convert a Nyx tensor {data:[], shape:[rows,cols]} to a DataFrame-like object.
pub fn from_tensor_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Object(tensor_rc)) = args.first() {
        let tensor = tensor_rc.read().unwrap_or_else(|e| e.into_inner());
        let data_arr = match tensor.get("data").or_else(|| tensor.get("_data")) {
            Some(Value::Array(a)) => a.clone(),
            _ => return Ok(Value::Null),
        };
        let shape_arr = match tensor.get("shape").or_else(|| tensor.get("_shape")) {
            Some(Value::Array(a)) => a.clone(),
            _ => return Ok(Value::Null),
        };
        let shape = shape_arr.read().unwrap_or_else(|e| e.into_inner());
        let rows = match shape.first() { Some(Value::Int(i)) => *i as usize, _ => return Ok(Value::Null) };
        let cols_count = match shape.get(1) { Some(Value::Int(i)) => *i as usize, _ => return Ok(Value::Null) };
        let flat = data_arr.read().unwrap_or_else(|e| e.into_inner());

        let nyx_cols: Vec<Value> = (0..cols_count).map(|c| {
            let col_name = match args.get(1) {
                Some(Value::Array(names)) => {
                    let n = names.read().unwrap_or_else(|e| e.into_inner());
                    match n.get(c) { Some(Value::Str(s)) => s.clone(), _ => format!("col_{}", c) }
                }
                _ => format!("col_{}", c),
            };
            let col_data: Vec<Value> = (0..rows).map(|r| {
                flat.get(r * cols_count + c).cloned().unwrap_or(Value::Null)
            }).collect();
            let mut col_map: HashMap<String, Value> = HashMap::new();
            col_map.insert("_name".to_string(), Value::Str(col_name));
            col_map.insert("_dtype".to_string(), Value::Str("float".to_string()));
            col_map.insert("_data".to_string(), Value::Array(Arc::new(RwLock::new(col_data))));
            Value::Object(Arc::new(RwLock::new(col_map)))
        }).collect();

        let mut df: HashMap<String, Value> = HashMap::new();
        df.insert("_columns".to_string(), Value::Array(Arc::new(RwLock::new(nyx_cols))));
        return Ok(Value::Object(Arc::new(RwLock::new(df))));
    }
    Ok(Value::Null)
}

/// Serialize DataFrame columns to Arrow IPC-compatible JSON bytes (stub for Python interop).
pub fn export_arrow_ipc_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Object(df_rc)) = args.first() {
        let df = df_rc.read().unwrap_or_else(|e| e.into_inner());
        let cols = match df.get("_columns") {
            Some(Value::Array(a)) => a.read().unwrap_or_else(|e| e.into_inner()).clone(),
            _ => return Ok(Value::Null),
        };
        let num_rows = if let Some(Value::Object(c0)) = cols.first() {
            let cr = c0.read().unwrap_or_else(|e| e.into_inner());
            if let Some(Value::Array(d)) = cr.get("_data") { d.read().unwrap_or_else(|e| e.into_inner()).len() } else { 0 }
        } else { 0 };

        let mut records: Vec<serde_json::Value> = Vec::with_capacity(num_rows);
        for row in 0..num_rows {
            let mut obj = serde_json::Map::new();
            for col_v in &cols {
                if let Value::Object(col_rc) = col_v {
                    let col = col_rc.read().unwrap_or_else(|e| e.into_inner());
                    let name = match col.get("_name") { Some(Value::Str(s)) => s.clone(), _ => "col".to_string() };
                    if let Some(Value::Array(data)) = col.get("_data") {
                        let d = data.read().unwrap_or_else(|e| e.into_inner());
                        let jv = match d.get(row) {
                            Some(Value::Float(f)) => serde_json::Number::from_f64(*f).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null),
                            Some(Value::Int(i))   => serde_json::Value::Number((*i).into()),
                            Some(Value::Str(s))   => serde_json::Value::String(s.clone()),
                            Some(Value::Bool(b))  => serde_json::Value::Bool(*b),
                            _ => serde_json::Value::Null,
                        };
                        obj.insert(name, jv);
                    }
                }
            }
            records.push(serde_json::Value::Object(obj));
        }
        let bytes = serde_json::to_vec(&records).unwrap_or_default();
        return Ok(Value::Bytes(Arc::new(RwLock::new(bytes))));
    }
    Ok(Value::Null)
}

// ── Phase 7: SIMD / Rayon High-Throughput Kernels ────────────────────────────

pub fn sum_simd_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    match args.first() {
        Some(Value::Array(arr_rc)) => {
            let arr = arr_rc.read().unwrap_or_else(|e| e.into_inner());
            let sum: f64 = if arr.len() > 1000 {
                arr.par_iter().map(|v| v.as_f64().unwrap_or(0.0)).sum()
            } else {
                arr.iter().map(|v| v.as_f64().unwrap_or(0.0)).sum()
            };
            Ok(Value::Float(sum))
        }
        Some(Value::DoubleArray(arr_rc)) => {
            let arr = arr_rc.read().unwrap_or_else(|e| e.into_inner());
            let sum: f64 = if arr.len() > 1000 {
                arr.par_iter().sum()
            } else {
                arr.iter().sum()
            };
            Ok(Value::Float(sum))
        }
        Some(Value::FloatArray(arr_rc)) => {
            let arr = arr_rc.read().unwrap_or_else(|e| e.into_inner());
            let sum: f64 = if arr.len() > 1000 {
                arr.par_iter().map(|&x| x as f64).sum()
            } else {
                arr.iter().map(|&x| x as f64).sum()
            };
            Ok(Value::Float(sum))
        }
        _ => Ok(Value::Float(0.0)),
    }
}

pub fn dot_simd_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Array(a_rc)), Some(Value::Array(b_rc))) = (args.first(), args.get(1)) {
        let a = a_rc.read().unwrap_or_else(|e| e.into_inner());
        let b = b_rc.read().unwrap_or_else(|e| e.into_inner());
        let len = a.len().min(b.len());
        
        let dot: f64 = if len > 1000 {
            (0..len).into_par_iter().map(|i| {
                let va = a[i].as_f64().unwrap_or(0.0);
                let vb = b[i].as_f64().unwrap_or(0.0);
                va * vb
            }).sum()
        } else {
            (0..len).map(|i| {
                let va = a[i].as_f64().unwrap_or(0.0);
                let vb = b[i].as_f64().unwrap_or(0.0);
                va * vb
            }).sum()
        };
        Ok(Value::Float(dot))
    } else {
        Ok(Value::Float(0.0))
    }
}


pub fn set_memory_limit_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let limit = match args.first() {
        Some(Value::Int(i))   => *i as usize,
        Some(Value::Float(f)) => *f as usize,
        _ => return Ok(Value::Bool(false)),
    };
    df_engine::set_memory_limit(limit);
    println!("[df] Memory limit set to {} bytes ({:.1} MB)", limit, limit as f64 / 1_048_576.0);
    Ok(Value::Bool(true))
}

// ── Phase 8: Observability ────────────────────────────────────────────────────

thread_local! {
    static LAST_METRICS: std::cell::RefCell<Vec<(String, f64)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Execute a logical plan and return execution metrics (total_ms, chunks, rows).
pub fn profile_plan_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(plan_val) = args.first() {
        let logical = value_to_logical_plan(plan_val)?;
        let mut ctx = df_engine::ExecutionContext { sources: HashMap::new() };
        
        // Extract sources if provided in 2nd arg
        if let Some(Value::Object(sources_rc)) = args.get(1) {
            let sources = sources_rc.read().unwrap_or_else(|e| e.into_inner());
            for (id, val) in sources.iter() {
                if let Value::Object(df_rc) = val {
                    let df = df_rc.read().unwrap_or_else(|e| e.into_inner());
                    let chunk = df_to_data_chunk(&df)?;
                    let fields = chunk.columns.iter().map(|c| Field { 
                        name: c.name.clone(), 
                        dtype: "dynamic".to_string(), 
                        nullable: true 
                    }).collect();
                    let schema = Schema::new(fields);
                    ctx.sources.insert(id.clone(), Box::new(df_engine::MemoryDataSource {
                        chunks: vec![chunk],
                        cursor: 0,
                        schema,
                    }));
                }
            }
        }

        let t0 = std::time::Instant::now();
        let mut phys = df_engine::create_physical_plan(&logical, &mut ctx)
            .map_err(|e| EvalError { message: e, stack: vec![] })?;
        let mut chunk_count = 0i64;
        let mut row_count = 0i64;
        while let Some(chunk) = phys.next_chunk().map_err(|e| EvalError { message: e, stack: vec![] })? {
            chunk_count += 1;
            row_count += chunk.size as i64;
        }
        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        LAST_METRICS.with(|m| {
            let mut b = m.borrow_mut();
            b.clear();
            b.push(("total_ms".to_string(), elapsed));
            b.push(("chunks".to_string(), chunk_count as f64));
            b.push(("rows".to_string(), row_count as f64));
        });
        let mut row_map: HashMap<String, Value> = HashMap::new();
        row_map.insert("total_ms".to_string(), Value::Float(elapsed));
        row_map.insert("chunks".to_string(), Value::Int(chunk_count));
        row_map.insert("rows".to_string(), Value::Int(row_count));
        return Ok(Value::Object(Arc::new(RwLock::new(row_map))));
    }
    Ok(Value::Null)
}

/// Returns last execution metrics from thread-local storage.
pub fn get_metrics_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    let mut result: HashMap<String, Value> = HashMap::new();
    LAST_METRICS.with(|m| {
        for (k, v) in m.borrow().iter() {
            result.insert(k.clone(), Value::Float(*v));
        }
    });
    Ok(Value::Object(Arc::new(RwLock::new(result))))
}

/// Render an ASCII logical plan tree.
pub fn explain_plan_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(plan_val) = args.first() {
        let tree = render_plan_ascii(plan_val, 0);
        println!("{}", tree);
        return Ok(Value::Str(tree));
    }
    Ok(Value::Null)
}

fn render_plan_ascii(v: &Value, depth: usize) -> String {
    let prefix = "  ".repeat(depth);
    let arrow = if depth == 0 { "".to_string() } else { format!("{}└─ ", "  ".repeat(depth - 1)) };
    match v {
        Value::Object(obj_rc) => {
            let obj = obj_rc.read().unwrap_or_else(|e| e.into_inner());
            let op = match obj.get("_op") { Some(Value::Str(s)) => s.clone(), _ => "?".to_string() };
            let mut lines = format!("{}[{}]", arrow, op.to_ascii_uppercase());
            if let Some(col_val) = obj.get("column") {
                if let Value::Str(c) = col_val { lines.push_str(&format!(" col={}", c)); }
            }
            if let Some(n_val) = obj.get("n") {
                if let Value::Int(n) = n_val { lines.push_str(&format!(" n={}", n)); }
            }
            if let Some(input) = obj.get("input") {
                lines.push('\n'); lines.push_str(&render_plan_ascii(input, depth + 1));
            }
            if let Some(left) = obj.get("left") {
                lines.push_str(&format!("\n{}  [LEFT]", prefix));
                lines.push('\n'); lines.push_str(&render_plan_ascii(left, depth + 2));
            }
            if let Some(right) = obj.get("right") {
                lines.push_str(&format!("\n{}  [RIGHT]", prefix));
                lines.push('\n'); lines.push_str(&render_plan_ascii(right, depth + 2));
            }
            lines
        }
        _ => format!("{}(source)", arrow),
    }
}

// ── Phase 8: Sandboxed Expression Evaluator ──────────────────────────────────

/// Evaluate a simple expression string against a data array in a secure sandbox.
/// Only allows: numeric literals and binary operators (+, -, *, /, >, <, >=, <=, ==, !=).
pub fn sandboxed_eval_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    let expr_str = match args.first() { Some(Value::Str(s)) => s.trim().to_string(), _ => return Ok(Value::Null) };
    let data = match args.get(1) { Some(Value::Array(a)) => a.clone(), _ => return Ok(Value::Null) };
    let d = data.read().unwrap_or_else(|e| e.into_inner()).clone();
    // Try operators in descending length order to avoid prefix matching bugs
    let ops: &[(&str, bool)] = &[
        (">=", false), ("<=", false), ("==", false), ("!=", false),
        (">", false), ("<", false), ("+", true), ("-", true), ("*", true), ("/", true),
    ];
    for &(op, _is_arith) in ops {
        if let Some(pos) = expr_str.rfind(op) {
            let rhs_str = expr_str[(pos + op.len())..].trim();
            if let Ok(rhs) = rhs_str.parse::<f64>() {
                let result: Vec<Value> = d.iter().map(|v| {
                    let lhs = as_f64(v).unwrap_or(0.0);
                    match op {
                        ">="  => Value::Bool(lhs >= rhs),
                        "<="  => Value::Bool(lhs <= rhs),
                        "=="  => Value::Bool((lhs - rhs).abs() < f64::EPSILON),
                        "!="  => Value::Bool((lhs - rhs).abs() >= f64::EPSILON),
                        ">"   => Value::Bool(lhs > rhs),
                        "<"   => Value::Bool(lhs < rhs),
                        "+"   => Value::Float(lhs + rhs),
                        "-"   => Value::Float(lhs - rhs),
                        "*"   => Value::Float(lhs * rhs),
                        "/"   => if rhs == 0.0 { Value::Null } else { Value::Float(lhs / rhs) },
                        _     => Value::Null,
                    }
                }).collect();
                return Ok(Value::Array(Arc::new(RwLock::new(result))));
            }
        }
    }
    Err(EvalError { message: format!("sandboxed_eval: unsupported expression '{}'", expr_str), stack: vec![] })
}

// ── Phase 6: DISTINCT dedup ───────────────────────────────────────────────────

/// Remove duplicate rows from a DataFrame-like object.
pub fn df_distinct_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Object(df_rc)) = args.first() {
        let df = df_rc.read().unwrap_or_else(|e| e.into_inner());
        let cols = match df.get("_columns") {
            Some(Value::Array(a)) => a.read().unwrap_or_else(|e| e.into_inner()).clone(),
            _ => return Ok(Value::Object(Arc::new(RwLock::new(df.clone())))),
        };
        if cols.is_empty() { return Ok(Value::Object(Arc::new(RwLock::new(df.clone())))); }

        let num_rows = if let Some(Value::Object(c0)) = cols.first() {
            let cr = c0.read().unwrap_or_else(|e| e.into_inner());
            if let Some(Value::Array(d)) = cr.get("_data") { d.read().unwrap_or_else(|e| e.into_inner()).len() } else { 0 }
        } else { 0 };

        let mut seen = std::collections::HashSet::<String>::new();
        let mut kept: Vec<usize> = Vec::new();

        for row in 0..num_rows {
            let key: String = cols.iter().map(|cv| {
                if let Value::Object(cr) = cv {
                    let col = cr.read().unwrap_or_else(|e| e.into_inner());
                    if let Some(Value::Array(da)) = col.get("_data") {
                        value_to_string(da.read().unwrap_or_else(|e| e.into_inner()).get(row).unwrap_or(&Value::Null))
                    } else { String::new() }
                } else { String::new() }
            }).collect::<Vec<_>>().join("\x00");
            if seen.insert(key) { kept.push(row); }
        }

        let new_cols: Vec<Value> = cols.iter().map(|cv| {
            if let Value::Object(cr) = cv {
                let col = cr.read().unwrap_or_else(|e| e.into_inner());
                let name  = match col.get("_name")  { Some(Value::Str(s)) => s.clone(), _ => "col".to_string() };
                let dtype = match col.get("_dtype") { Some(Value::Str(s)) => s.clone(), _ => "str".to_string() };
                if let Some(Value::Array(da)) = col.get("_data") {
                    let d = da.read().unwrap_or_else(|e| e.into_inner());
                    let nd: Vec<Value> = kept.iter().map(|&r| d.get(r).cloned().unwrap_or(Value::Null)).collect();
                    let mut nc: HashMap<String, Value> = HashMap::new();
                    nc.insert("_name".to_string(),  Value::Str(name));
                    nc.insert("_dtype".to_string(), Value::Str(dtype));
                    nc.insert("_data".to_string(),  Value::Array(Arc::new(RwLock::new(nd))));
                    Value::Object(Arc::new(RwLock::new(nc)))
                } else { cv.clone() }
            } else { cv.clone() }
        }).collect();

        let mut new_df: HashMap<String, Value> = HashMap::new();
        new_df.insert("_columns".to_string(), Value::Array(Arc::new(RwLock::new(new_cols))));
        return Ok(Value::Object(Arc::new(RwLock::new(new_df))));
    }
    Ok(Value::Null)
}

/// Differentiable Soft-Join
/// Computes a weighted sum of right-hand-side features based on ID similarity.
pub fn soft_join_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if args.len() < 5 { return Ok(Value::Null); }
    let left_df = match &args[0] { Value::Object(o) => o.read().unwrap_or_else(|e| e.into_inner()), _ => return Ok(Value::Null) };
    let right_df = match &args[1] { Value::Object(o) => o.read().unwrap_or_else(|e| e.into_inner()), _ => return Ok(Value::Null) };
    let left_on = match &args[2] { Value::Str(s) => s, _ => return Ok(Value::Null) };
    let right_on = match &args[3] { Value::Str(s) => s, _ => return Ok(Value::Null) };
    let temp = args[4].as_f64().unwrap_or(0.1) as f32;

    // 1. Extract ID columns
    let left_ids = get_col_data(&left_df, left_on)?;
    let right_ids = get_col_data(&right_df, right_on)?;
    
    // 2. Compute similarity matrix (Softmax over distances)
    // For Phase 15, we implement a CPU-based differentiable version.
    // In production, this would be a tiled GPU kernel.
    let mut joined_cols = Vec::new();
    
    // Join logic: for each row in left, find weighted average of right features
    // We'll join all columns from right_df except the ID column.
    let right_cols = match right_df.get("_columns") {
        Some(Value::Array(a)) => a.read().unwrap_or_else(|e| e.into_inner()),
        _ => return Ok(Value::Null),
    };

    for r_col_val in right_cols.iter() {
        if let Value::Object(c_rc) = r_col_val {
            let c = c_rc.read().unwrap_or_else(|e| e.into_inner());
            let name = match c.get("_name") { Some(Value::Str(s)) => s, _ => "" };
            if name == right_on { continue; }
            
            let data = match c.get("_data") {
                Some(Value::Array(a)) => a.read().unwrap_or_else(|e| e.into_inner()),
                _ => continue,
            };
            
            let mut new_data = Vec::with_capacity(left_ids.len());
            for l_id in &left_ids {
                let mut weighted_sum = 0.0;
                let mut weight_total = 0.0;
                
                for (j, r_id) in right_ids.iter().enumerate() {
                    let dist = (l_id - r_id).abs();
                    let weight = ( -dist / temp as f64 ).exp();
                    
                    let val = match &data[j] {
                        Value::Float(f) => *f,
                        Value::Int(i) => *i as f64,
                        _ => 0.0,
                    };
                    
                    weighted_sum += weight * val;
                    weight_total += weight;
                }
                
                new_data.push(Value::Float(if weight_total > 0.0 { weighted_sum / weight_total } else { 0.0 }));
            }
            
            let mut new_col = HashMap::new();
            new_col.insert("_name".to_string(), Value::Str(format!("{}_joined", name)));
            new_col.insert("_data".to_string(), Value::Array(Arc::new(RwLock::new(new_data))));
            joined_cols.push(Value::Object(Arc::new(RwLock::new(new_col))));
        }
    }
    
    let mut result_df = HashMap::new();
    result_df.insert("_columns".to_string(), Value::Array(Arc::new(RwLock::new(joined_cols))));
    Ok(Value::Object(Arc::new(RwLock::new(result_df))))
}

fn get_col_data(df: &HashMap<String, Value>, name: &str) -> Result<Vec<f64>, EvalError> {
    let cols = match df.get("_columns") { 
        Some(Value::Array(a)) => a.read().unwrap_or_else(|e| e.into_inner()),
        _ => return Err(EvalError{message:"Invalid DF".to_string(), stack:vec![]}),
    };
    for c_val in cols.iter() {
        if let Value::Object(c_rc) = c_val {
            let c = c_rc.read().unwrap_or_else(|e| e.into_inner());
            if let Some(Value::Str(n)) = c.get("_name") {
                if n == name {
                    let data = match c.get("_data") {
                        Some(Value::Array(a)) => a.read().unwrap_or_else(|e| e.into_inner()),
                        _ => return Err(EvalError{message:"Column missing data".to_string(), stack:vec![]}),
                    };
                    return Ok(data.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect());
                }
            }
        }
}
    Err(EvalError{message:format!("Column {} not found", name), stack:vec![]})
}

// ── Phase 42: Transactional Integrity & Persistence ──────────────────────────

pub fn db_begin_transaction_native(_vm: &mut NyxVm, _args: &[Value]) -> Result<Value, EvalError> {
    let tx_id = df_engine::global_tx_context().begin_transaction();
    Ok(Value::Int(tx_id as i64))
}

pub fn db_commit_transaction_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Int(tx_id)) = args.first() {
        let pending = df_engine::global_tx_context().commit(*tx_id as u64)
            .map_err(|e| EvalError { message: e, stack: vec![] })?;
        
        // Apply pending changes to global catalog
        for (name, (_schema, chunks)) in pending {
            df_engine::register_table(name, Arc::new(chunks));
        }
        return Ok(Value::Bool(true));
    }
    Ok(Value::Bool(false))
}

pub fn db_abort_transaction_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let Some(Value::Int(tx_id)) = args.first() {
        df_engine::global_tx_context().abort(*tx_id as u64)
            .map_err(|e| EvalError { message: e, stack: vec![] })?;
        return Ok(Value::Bool(true));
    }
    Ok(Value::Bool(false))
}

pub fn save_table_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Str(name)), Some(Value::Str(path))) = (args.first(), args.get(1)) {
        let catalog = df_engine::global_catalog().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(chunks) = catalog.get(name) {
            // Need schema
            if chunks.is_empty() { return Ok(Value::Bool(false)); }
            let fields = chunks[0].columns.iter().map(|c| Field { 
                name: c.name.clone(), 
                dtype: "dynamic".to_string(), 
                nullable: true 
            }).collect();
            let schema = Schema::new(fields);
            
            crate::runtime::execution::nyx_table_writer::NyxTableWriter::write_to_file(path, &schema, chunks)
                .map_err(|e| EvalError { message: e.to_string(), stack: vec![] })?;
            return Ok(Value::Bool(true));
        }
    }
    Ok(Value::Bool(false))
}

pub fn load_table_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Str(name)), Some(Value::Str(path))) = (args.first(), args.get(1)) {
        let (_schema, chunks) = crate::runtime::execution::nyx_table_writer::NyxTableWriter::read_from_file(path)
            .map_err(|e| EvalError { message: e.to_string(), stack: vec![] })?;
        df_engine::register_table(name.clone(), Arc::new(chunks));
        return Ok(Value::Bool(true));
    }
    Ok(Value::Bool(false))
}

pub fn db_add_pending_table_native(_vm: &mut NyxVm, args: &[Value]) -> Result<Value, EvalError> {
    if let (Some(Value::Int(tx_id)), Some(Value::Str(name)), Some(Value::Object(df_rc))) = (args.first(), args.get(1), args.get(2)) {
        let df = df_rc.read().unwrap_or_else(|e| e.into_inner());
        let chunk = df_to_data_chunk(&df)?;
        let fields = chunk.columns.iter().map(|c| Field { 
            name: c.name.clone(), 
            dtype: "dynamic".to_string(), 
            nullable: true 
        }).collect();
        let schema = Schema::new(fields);
        
        df_engine::global_tx_context().add_pending_table(*tx_id as u64, name.clone(), schema, vec![chunk])
            .map_err(|e| EvalError { message: e, stack: vec![] })?;
        return Ok(Value::Bool(true));
    }
    Ok(Value::Bool(false))
}

