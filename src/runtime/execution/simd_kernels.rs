/// simd_kernels.rs — Parallel SIMD columnar kernels for Nyx DB.
///
/// Strategy: Rayon splits data across CPU cores (multi-threading),
/// AND each core processes 4 f64 values per instruction via `wide::f64x4`
/// (256-bit AVX2 on x86_64). This gives: N_cores × 4 throughput multiplier.
///
/// Every public function is safe, stable-Rust, and portable.

use std::sync::Arc;
use rayon::prelude::*;
use wide::{f64x4, i64x4, CmpGt, CmpLt, CmpEq};
use super::df_engine::Bitmap;

// ── SIMD lane width ────────────────────────────────────────────────────────
const LANES: usize = 4;
// Chunk size per rayon thread: process 256 SIMD batches = 1024 f64 values per task
const THREAD_CHUNK: usize = LANES * 256;

// ── Arithmetic kernels ─────────────────────────────────────────────────────

/// Rayon + SIMD F64 addition: l[i] + r[i] across all cores, 4 values/instruction.
pub fn simd_f64_add(l: &[f64], r: &[f64]) -> Vec<f64> {
    let n = l.len().min(r.len());
    let mut out = vec![0.0f64; n];

    out.par_chunks_mut(THREAD_CHUNK)
        .zip(l.par_chunks(THREAD_CHUNK))
        .zip(r.par_chunks(THREAD_CHUNK))
        .for_each(|((out_chunk, l_chunk), r_chunk)| {
            let len = out_chunk.len();
            let simd_len = len / LANES * LANES;
            for i in (0..simd_len).step_by(LANES) {
                let la = f64x4::new([l_chunk[i], l_chunk[i+1], l_chunk[i+2], l_chunk[i+3]]);
                let ra = f64x4::new([r_chunk[i], r_chunk[i+1], r_chunk[i+2], r_chunk[i+3]]);
                let res: [f64; 4] = (la + ra).into();
                out_chunk[i..i+LANES].copy_from_slice(&res);
            }
            for i in simd_len..len {
                out_chunk[i] = l_chunk[i] + r_chunk[i];
            }
        });
    out
}

/// Rayon + SIMD F64 subtraction.
pub fn simd_f64_sub(l: &[f64], r: &[f64]) -> Vec<f64> {
    let n = l.len().min(r.len());
    let mut out = vec![0.0f64; n];

    out.par_chunks_mut(THREAD_CHUNK)
        .zip(l.par_chunks(THREAD_CHUNK))
        .zip(r.par_chunks(THREAD_CHUNK))
        .for_each(|((out_chunk, l_chunk), r_chunk)| {
            let len = out_chunk.len();
            let simd_len = len / LANES * LANES;
            for i in (0..simd_len).step_by(LANES) {
                let la = f64x4::new([l_chunk[i], l_chunk[i+1], l_chunk[i+2], l_chunk[i+3]]);
                let ra = f64x4::new([r_chunk[i], r_chunk[i+1], r_chunk[i+2], r_chunk[i+3]]);
                let res: [f64; 4] = (la - ra).into();
                out_chunk[i..i+LANES].copy_from_slice(&res);
            }
            for i in simd_len..len { out_chunk[i] = l_chunk[i] - r_chunk[i]; }
        });
    out
}

/// Rayon + SIMD F64 multiplication.
pub fn simd_f64_mul(l: &[f64], r: &[f64]) -> Vec<f64> {
    let n = l.len().min(r.len());
    let mut out = vec![0.0f64; n];

    out.par_chunks_mut(THREAD_CHUNK)
        .zip(l.par_chunks(THREAD_CHUNK))
        .zip(r.par_chunks(THREAD_CHUNK))
        .for_each(|((out_chunk, l_chunk), r_chunk)| {
            let len = out_chunk.len();
            let simd_len = len / LANES * LANES;
            for i in (0..simd_len).step_by(LANES) {
                let la = f64x4::new([l_chunk[i], l_chunk[i+1], l_chunk[i+2], l_chunk[i+3]]);
                let ra = f64x4::new([r_chunk[i], r_chunk[i+1], r_chunk[i+2], r_chunk[i+3]]);
                let res: [f64; 4] = (la * ra).into();
                out_chunk[i..i+LANES].copy_from_slice(&res);
            }
            for i in simd_len..len { out_chunk[i] = l_chunk[i] * r_chunk[i]; }
        });
    out
}

/// Rayon + SIMD F64 division.
pub fn simd_f64_div(l: &[f64], r: &[f64]) -> Vec<f64> {
    let n = l.len().min(r.len());
    let mut out = vec![0.0f64; n];

    out.par_chunks_mut(THREAD_CHUNK)
        .zip(l.par_chunks(THREAD_CHUNK))
        .zip(r.par_chunks(THREAD_CHUNK))
        .for_each(|((out_chunk, l_chunk), r_chunk)| {
            let len = out_chunk.len();
            let simd_len = len / LANES * LANES;
            for i in (0..simd_len).step_by(LANES) {
                let la = f64x4::new([l_chunk[i], l_chunk[i+1], l_chunk[i+2], l_chunk[i+3]]);
                let ra = f64x4::new([r_chunk[i], r_chunk[i+1], r_chunk[i+2], r_chunk[i+3]]);
                let res: [f64; 4] = (la / ra).into();
                out_chunk[i..i+LANES].copy_from_slice(&res);
            }
            for i in simd_len..len { out_chunk[i] = l_chunk[i] / r_chunk[i]; }
        });
    out
}

/// SIMD I64 addition.
pub fn simd_i64_add(l: &[i64], r: &[i64]) -> Vec<i64> {
    let n = l.len().min(r.len());
    let mut out = vec![0i64; n];
    out.par_chunks_mut(THREAD_CHUNK).zip(l.par_chunks(THREAD_CHUNK)).zip(r.par_chunks(THREAD_CHUNK))
        .for_each(|((oc, lc), rc)| {
            let len = oc.len();
            let simd_len = len / LANES * LANES;
            for i in (0..simd_len).step_by(LANES) {
                let la = i64x4::new([lc[i], lc[i+1], lc[i+2], lc[i+3]]);
                let ra = i64x4::new([rc[i], rc[i+1], rc[i+2], rc[i+3]]);
                let res: [i64; 4] = (la + ra).into();
                oc[i..i+LANES].copy_from_slice(&res);
            }
            for i in simd_len..len { oc[i] = lc[i] + rc[i]; }
        });
    out
}

/// Parallelized hashing for U64/I64 using a high-performance bit-mixer.
pub fn simd_u64_hash(data: &[u64]) -> Vec<u64> {
    let n = data.len();
    let mut hashes = vec![0u64; n];
    
    hashes.par_chunks_mut(THREAD_CHUNK).zip(data.par_chunks(THREAD_CHUNK))
        .for_each(|(out, inp)| {
            for i in 0..out.len() {
                let mut x = inp[i];
                // Split-mix style bit mixer (fast and parallel-friendly)
                x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
                x = x ^ (x >> 31);
                out[i] = x;
            }
        });
    hashes
}

/// Parallelized hashing for F64.
pub fn simd_f64_hash(data: &[f64]) -> Vec<u64> {
    let bits: &[u64] = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u64, data.len()) };
    simd_u64_hash(bits)
}

/// SIMD I64 comparison → Bitmap (l[i] == r[i])
pub fn simd_i64_eq_bitmap(l: &[i64], r: &[i64]) -> Bitmap {
    let n = l.len().min(r.len());
    let byte_len = (n + 7) / 8;
    let data: Vec<u8> = (0..byte_len).into_par_iter().map(|byte_idx| {
        let mut byte = 0u8;
        let base = byte_idx * 8;
        let end = (base + 8).min(n);
        let simd_end = base + ((end - base) / LANES) * LANES;
        let mut i = base;
        while i + LANES <= simd_end {
            let la = i64x4::new([l[i], l[i+1], l[i+2], l[i+3]]);
            let ra = i64x4::new([r[i], r[i+1], r[i+2], r[i+3]]);
            let mask: [i64; 4] = la.cmp_eq(ra).into();
            for bit in 0..4 {
                if mask[bit] != 0 { byte |= 1 << (i + bit - base); }
            }
            i += LANES;
        }
        for i in simd_end..end {
            if l[i] == r[i] { byte |= 1 << (i - base); }
        }
        byte
    }).collect();
    Bitmap { data: Arc::new(data), len: n }
}

// ── Comparison → Bitmap kernels ────────────────────────────────────────────


fn build_bitmap_chunked<F>(l: &[f64], r: &[f64], n: usize, byte_len: usize, cmp: &F) -> Bitmap
where F: Fn(f64x4, f64x4) -> f64x4 + Sync
{
    // Parallel across bytes; each byte covers 8 rows.
    let data: Vec<u8> = (0..byte_len).into_par_iter().map(|byte_idx| {
        let mut byte = 0u8;
        let base = byte_idx * 8;
        // Process up to 8 elements in this byte (up to 2 SIMD checks of 4)
        let end = (base + 8).min(n);
        for i in base..end {
            let la = f64x4::splat(l[i]);
            let ra = f64x4::splat(r[i]);
            let mask: [f64; 4] = cmp(la, ra).into();
            if mask[0].to_bits() != 0 {
                byte |= 1 << (i - base);
            }
        }
        byte
    }).collect();
    Bitmap { data: Arc::new(data), len: n }
}

/// Rayon + SIMD F64 GT comparison → Bitmap (l[i] > r[i])
pub fn simd_f64_gt_bitmap(l: &[f64], r: &[f64]) -> Bitmap {
    let n = l.len().min(r.len());
    let byte_len = (n + 7) / 8;
    build_bitmap_chunked(l, r, n, byte_len, &|la: f64x4, ra: f64x4| la.cmp_gt(ra))
}

/// Rayon + SIMD F64 LT comparison → Bitmap
pub fn simd_f64_lt_bitmap(l: &[f64], r: &[f64]) -> Bitmap {
    let n = l.len().min(r.len());
    let byte_len = (n + 7) / 8;
    build_bitmap_chunked(l, r, n, byte_len, &|la: f64x4, ra: f64x4| la.cmp_lt(ra))
}

/// Rayon + SIMD F64 EQ comparison → Bitmap
pub fn simd_f64_eq_bitmap(l: &[f64], r: &[f64]) -> Bitmap {
    let n = l.len().min(r.len());
    let byte_len = (n + 7) / 8;
    build_bitmap_chunked(l, r, n, byte_len, &|la: f64x4, ra: f64x4| la.cmp_eq(ra))
}

/// Scalar-broadcast GT: each l[i] vs. a constant threshold (common filter pattern).
pub fn simd_f64_gt_scalar_bitmap(l: &[f64], threshold: f64) -> Bitmap {
    let n = l.len();
    let byte_len = (n + 7) / 8;
    let ra = f64x4::splat(threshold);
    let data: Vec<u8> = (0..byte_len).into_par_iter().map(|byte_idx| {
        let mut byte = 0u8;
        let base = byte_idx * 8;
        let end = (base + 8).min(n);
        // Process in SIMD chunks of 4 within the byte window
        let simd_end = base + ((end - base) / LANES) * LANES;
        let mut i = base;
        while i + LANES <= simd_end {
            let la = f64x4::new([l[i], l[i+1], l[i+2], l[i+3]]);
            let mask: [f64; 4] = la.cmp_gt(ra).into();
            for bit in 0..4 {
                if mask[bit].to_bits() != 0 { byte |= 1 << (i + bit - base); }
            }
            i += LANES;
        }
        for i in simd_end..end {
            if l[i] > threshold { byte |= 1 << (i - base); }
        }
        byte
    }).collect();
    Bitmap { data: Arc::new(data), len: n }
}

/// Scalar-broadcast LT.
pub fn simd_f64_lt_scalar_bitmap(l: &[f64], threshold: f64) -> Bitmap {
    let n = l.len();
    let byte_len = (n + 7) / 8;
    let ra = f64x4::splat(threshold);
    let data: Vec<u8> = (0..byte_len).into_par_iter().map(|byte_idx| {
        let mut byte = 0u8;
        let base = byte_idx * 8;
        let end = (base + 8).min(n);
        let simd_end = base + ((end - base) / LANES) * LANES;
        let mut i = base;
        while i + LANES <= simd_end {
            let la = f64x4::new([l[i], l[i+1], l[i+2], l[i+3]]);
            let mask: [f64; 4] = la.cmp_lt(ra).into();
            for bit in 0..4 {
                if mask[bit].to_bits() != 0 { byte |= 1 << (i + bit - base); }
            }
            i += LANES;
        }
        for i in simd_end..end {
            if l[i] < threshold { byte |= 1 << (i - base); }
        }
        byte
    }).collect();
    Bitmap { data: Arc::new(data), len: n }
}

/// Scalar-broadcast EQ.
pub fn simd_f64_eq_scalar_bitmap(l: &[f64], threshold: f64) -> Bitmap {
    let n = l.len();
    let byte_len = (n + 7) / 8;
    let ra = f64x4::splat(threshold);
    let data: Vec<u8> = (0..byte_len).into_par_iter().map(|byte_idx| {
        let mut byte = 0u8;
        let base = byte_idx * 8;
        let end = (base + 8).min(n);
        let simd_end = base + ((end - base) / LANES) * LANES;
        let mut i = base;
        while i + LANES <= simd_end {
            let la = f64x4::new([l[i], l[i+1], l[i+2], l[i+3]]);
            let mask: [f64; 4] = la.cmp_eq(ra).into();
            for bit in 0..4 {
                if mask[bit].to_bits() != 0 { byte |= 1 << (i + bit - base); }
            }
            i += LANES;
        }
        for i in simd_end..end {
            if (l[i] - threshold).abs() < f64::EPSILON { byte |= 1 << (i - base); }
        }
        byte
    }).collect();
    Bitmap { data: Arc::new(data), len: n }
}
