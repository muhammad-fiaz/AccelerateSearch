//! Quantization strategies for vector embeddings.
//!
//! Reduces memory footprint and accelerates approximate nearest neighbour
//! search at a small accuracy cost. The following strategies are
//! implemented:
//!
//! * [`ScalarQuantizer`] — 8-bit uniform quantization (4x compression).
//! * [`ProductQuantizer`] — split a vector into `m` sub-vectors and
//!   quantize each independently using k-means (16-32x compression).
//! * [`BinaryQuantizer`] — keep only the sign bit per dimension (32x
//!   compression; approximate via Hamming distance).
//!
//! Quantizers convert dense `f32` vectors into a compact byte payload that
//! can be persisted alongside the index. The [`distance`] helpers then
//! compute approximate distances on the reconstructed codes without ever
//! materialising the original `f32` values.

use serde::{Deserialize, Serialize};

/// Common quantizer surface. Each implementation owns the encoder state
/// (calibration buffers, codebooks) and produces a portable byte payload.
pub trait Quantizer: Send + Sync + 'static {
    /// Name of the strategy. Used in telemetry and persistence.
    fn name(&self) -> &'static str;
    /// Number of bytes one quantized code occupies.
    fn code_bytes(&self) -> usize;
    /// Encodes a single vector.
    fn encode(&self, vector: &[f32]) -> Vec<u8>;
    /// Estimates the distance between a query and a quantized code.
    fn distance(&self, query: &[f32], code: &[u8]) -> f32;
}

/// Scalar (uniform) 8-bit quantizer.
///
/// Each dimension is linearly mapped to the `[0, 255]` range using
/// per-dimension min/max calibration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalarQuantizer {
    /// Per-dimension minimum (length = dimensionality).
    pub mins: Vec<f32>,
    /// Per-dimension span (`max - min`, length = dimensionality).
    pub spans: Vec<f32>,
    /// Whether calibration is in place.
    pub calibrated: bool,
}

impl ScalarQuantizer {
    /// Creates an uncalibrated quantizer for the given dimensionality.
    #[must_use]
    pub fn new(dimensions: usize) -> Self {
        Self {
            mins: vec![f32::INFINITY; dimensions],
            spans: vec![0.0; dimensions],
            calibrated: false,
        }
    }

    /// Calibrates the quantizer by inspecting a sample of vectors.
    pub fn calibrate<I: IntoIterator<Item = Vec<f32>>>(&mut self, samples: I) {
        let mut mins = vec![f32::INFINITY; self.mins.len()];
        let mut maxs = vec![f32::NEG_INFINITY; self.mins.len()];
        let mut seen = 0usize;
        for v in samples {
            if v.len() != mins.len() {
                continue;
            }
            seen += 1;
            for (i, &x) in v.iter().enumerate() {
                if x < mins[i] {
                    mins[i] = x;
                }
                if x > maxs[i] {
                    maxs[i] = x;
                }
            }
        }
        if seen == 0 {
            return;
        }
        let spans: Vec<f32> = maxs
            .iter()
            .zip(mins.iter())
            .map(|(mx, mn)| (mx - mn).max(f32::EPSILON))
            .collect();
        self.mins = mins;
        self.spans = spans;
        self.calibrated = true;
    }

    fn reconstruct(&self, code: &[u8]) -> Vec<f32> {
        code.iter()
            .enumerate()
            .map(|(i, &b)| {
                let q = f32::from(b) / 255.0;
                self.mins.get(i).copied().unwrap_or_default()
                    + q * self.spans.get(i).copied().unwrap_or(1.0)
            })
            .collect()
    }
}

impl Quantizer for ScalarQuantizer {
    fn name(&self) -> &'static str {
        "scalar-int8"
    }

    fn code_bytes(&self) -> usize {
        self.mins.len()
    }

    fn encode(&self, vector: &[f32]) -> Vec<u8> {
        if vector.len() != self.mins.len() {
            return Vec::new();
        }
        vector
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                if !self.calibrated {
                    return 0u8;
                }
                let span = self.spans[i].max(f32::EPSILON);
                let min = self.mins[i];
                let norm = ((x - min) / span).clamp(0.0, 1.0);
                (norm * 255.0).round() as u8
            })
            .collect()
    }

    fn distance(&self, query: &[f32], code: &[u8]) -> f32 {
        let reconstructed = self.reconstruct(code);
        if query.len() != reconstructed.len() {
            return f32::INFINITY;
        }
        query
            .iter()
            .zip(reconstructed.iter())
            .map(|(a, b)| {
                let d = (*a as f64) - (*b as f64);
                (d * d) as f32
            })
            .sum::<f32>()
            .sqrt()
    }
}

/// Product quantizer.
///
/// Splits each vector into `m` contiguous sub-vectors of equal length and
/// quantizes each sub-vector with a tiny k-means codebook of `k` centroids.
/// Distance is the asymmetric sum of squared distances to the nearest
/// centroid per sub-space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductQuantizer {
    /// Number of sub-vectors (sub-spaces).
    pub m: usize,
    /// Number of centroids per sub-space.
    pub k: usize,
    /// Per-sub-space centroids, flattened. Length = `m * k * sub_dim`.
    pub codebooks: Vec<f32>,
    /// Dimensionality of every input vector.
    pub dimensions: usize,
}

impl ProductQuantizer {
    /// Creates a quantizer with sensible defaults: `m = 8` sub-spaces and
    /// `k = 256` centroids per sub-space.
    #[must_use]
    pub fn new(dimensions: usize) -> Self {
        let m = 8usize.min(dimensions.max(1));
        Self {
            m,
            k: 256,
            codebooks: Vec::new(),
            dimensions,
        }
    }

    /// Calibrates the quantizer using k-means on every sub-space.
    ///
    /// The implementation is intentionally simple: it iterates k-means with
    /// random initial centroids for a small number of iterations. For a
    /// production deployment swap in a faster implementation.
    pub fn calibrate<I: IntoIterator<Item = Vec<f32>>>(&mut self, samples: I) {
        let sub_dim = self.dimensions.div_ceil(self.m);
        let mut per_subspace: Vec<Vec<Vec<f32>>> = vec![Vec::new(); self.m];
        for v in samples {
            if v.len() != self.dimensions {
                continue;
            }
            for (s, subspace) in per_subspace.iter_mut().enumerate().take(self.m) {
                let start = s * sub_dim;
                let end = ((s + 1) * sub_dim).min(self.dimensions);
                subspace.push(v[start..end].to_vec());
            }
        }
        let mut codebooks: Vec<f32> = Vec::with_capacity(self.m * self.k * sub_dim);
        for (_s, sub_samples) in per_subspace.iter().enumerate().take(self.m) {
            if sub_samples.is_empty() {
                codebooks.extend(std::iter::repeat_n(0.0, self.k * sub_dim));
                continue;
            }
            let effective_k = self.k.min(sub_samples.len()).max(1);
            let centroids = kmeans(sub_samples, effective_k, 8);
            for c in 0..self.k {
                if c < centroids.len() {
                    codebooks.extend_from_slice(&centroids[c]);
                } else {
                    codebooks.extend(std::iter::repeat_n(0.0, sub_dim));
                }
            }
        }
        self.codebooks = codebooks;
    }

    fn sub_dim(&self) -> usize {
        self.dimensions.div_ceil(self.m)
    }

    fn centroid(&self, s: usize, c: usize) -> &[f32] {
        let sub_dim = self.sub_dim();
        let start = (s * self.k + c) * sub_dim;
        let end = start + sub_dim;
        &self.codebooks[start..end]
    }

    fn nearest_centroid(&self, s: usize, sub_vec: &[f32]) -> u8 {
        let sub_dim = self.sub_dim();
        let mut best = 0u8;
        let mut best_dist = f32::INFINITY;
        for c in 0..self.k {
            let cen = self.centroid(s, c);
            if cen.len() != sub_dim {
                continue;
            }
            let d: f32 = sub_vec
                .iter()
                .zip(cen.iter())
                .map(|(a, b)| {
                    let diff = (*a as f64) - (*b as f64);
                    (diff * diff) as f32
                })
                .sum();
            if d < best_dist {
                best_dist = d;
                best = c as u8;
            }
        }
        best
    }
}

impl Quantizer for ProductQuantizer {
    fn name(&self) -> &'static str {
        "product"
    }

    fn code_bytes(&self) -> usize {
        self.m
    }

    fn encode(&self, vector: &[f32]) -> Vec<u8> {
        if vector.len() != self.dimensions {
            return Vec::new();
        }
        let sub_dim = self.sub_dim();
        let mut out = Vec::with_capacity(self.m);
        for s in 0..self.m {
            let start = s * sub_dim;
            let end = ((s + 1) * sub_dim).min(self.dimensions);
            out.push(self.nearest_centroid(s, &vector[start..end]));
        }
        out
    }

    fn distance(&self, query: &[f32], code: &[u8]) -> f32 {
        if query.len() != self.dimensions || code.len() != self.m {
            return f32::INFINITY;
        }
        let sub_dim = self.sub_dim();
        let mut total = 0.0f32;
        for (s, code_byte) in code.iter().enumerate().take(self.m) {
            let c = *code_byte as usize;
            let start = s * sub_dim;
            let end = ((s + 1) * sub_dim).min(self.dimensions);
            let cen = self.centroid(s, c);
            for (i, q) in query[start..end].iter().enumerate() {
                let cv = cen.get(i).copied().unwrap_or_default();
                let diff = (*q as f64) - (cv as f64);
                total += (diff * diff) as f32;
            }
        }
        total.sqrt()
    }
}

/// Binary quantizer. Keeps only the sign bit per dimension. Approximates
/// the original distance with a normalised Hamming distance in `[-1, 1]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryQuantizer {
    /// Dimensionality of the input vectors.
    pub dimensions: usize,
}

impl BinaryQuantizer {
    /// Creates a new binary quantizer.
    #[must_use]
    pub fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }

    fn pack(bits: &[bool]) -> Vec<u8> {
        let mut bytes = vec![0u8; bits.len().div_ceil(8)];
        for (i, b) in bits.iter().enumerate() {
            if *b {
                bytes[i / 8] |= 1 << (i % 8);
            }
        }
        bytes
    }

    fn unpack(bytes: &[u8], len: usize) -> Vec<bool> {
        (0..len)
            .map(|i| (bytes[i / 8] >> (i % 8)) & 1 == 1)
            .collect()
    }
}

impl Quantizer for BinaryQuantizer {
    fn name(&self) -> &'static str {
        "binary"
    }

    fn code_bytes(&self) -> usize {
        self.dimensions.div_ceil(8)
    }

    fn encode(&self, vector: &[f32]) -> Vec<u8> {
        let bits: Vec<bool> = vector.iter().map(|x| *x > 0.0).collect();
        Self::pack(&bits)
    }

    fn distance(&self, query: &[f32], code: &[u8]) -> f32 {
        let query_bits: Vec<bool> = query.iter().map(|x| *x > 0.0).collect();
        if query_bits.len() != self.dimensions {
            return f32::INFINITY;
        }
        let code_bits = Self::unpack(code, self.dimensions);
        let mut diff = 0usize;
        for (a, b) in query_bits.iter().zip(code_bits.iter()) {
            if a != b {
                diff += 1;
            }
        }
        // Normalised Hamming distance to the [-1, 1] range: 0 when fully
        // similar, 1 when fully dissimilar.
        1.0 - (1.0 - (diff as f32) / self.dimensions.max(1) as f32) * 2.0
    }
}

/// Distance helper that runs the right kernel for a quantizer.
pub fn distance(q: &QuantizerDispatch, query: &[f32], code: &[u8]) -> f32 {
    match q {
        QuantizerDispatch::Scalar(s) => s.distance(query, code),
        QuantizerDispatch::Product(p) => p.distance(query, code),
        QuantizerDispatch::Binary(b) => b.distance(query, code),
    }
}

/// Sum type that lets callers erase the concrete quantizer at the API
/// boundary. Use [`Self::encode`] to produce codes and [`Self::distance`]
/// to score them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantizerDispatch {
    /// Scalar 8-bit quantizer.
    Scalar(ScalarQuantizer),
    /// Product quantizer.
    Product(ProductQuantizer),
    /// Binary quantizer.
    Binary(BinaryQuantizer),
}

impl QuantizerDispatch {
    /// Returns the quantizer name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            QuantizerDispatch::Scalar(q) => q.name(),
            QuantizerDispatch::Product(q) => q.name(),
            QuantizerDispatch::Binary(q) => q.name(),
        }
    }
    /// Encodes a vector.
    #[must_use]
    pub fn encode(&self, v: &[f32]) -> Vec<u8> {
        match self {
            QuantizerDispatch::Scalar(q) => q.encode(v),
            QuantizerDispatch::Product(q) => q.encode(v),
            QuantizerDispatch::Binary(q) => q.encode(v),
        }
    }
    /// Approximate distance between a query and a code.
    #[must_use]
    pub fn distance(&self, q: &[f32], c: &[u8]) -> f32 {
        distance(self, q, c)
    }
}

/// Trivial k-means implementation. Returns `k` centroids (clamped to the
/// number of available samples).
fn kmeans(samples: &[Vec<f32>], k: usize, iterations: usize) -> Vec<Vec<f32>> {
    if samples.is_empty() || k == 0 {
        return Vec::new();
    }
    let dim = samples[0].len();
    let k = k.min(samples.len());
    let stride = samples.len() / k.max(1);
    let mut centroids: Vec<Vec<f32>> = (0..k)
        .map(|i| samples[i * stride.min(samples.len() - 1)].clone())
        .collect();
    for _ in 0..iterations {
        let mut sums = vec![vec![0.0f64; dim]; k];
        let mut counts = vec![0usize; k];
        for s in samples {
            let mut best = 0usize;
            let mut best_d = f32::INFINITY;
            for (ci, c) in centroids.iter().enumerate() {
                let d: f32 = s
                    .iter()
                    .zip(c.iter())
                    .map(|(a, b)| {
                        let diff = (*a as f64) - (*b as f64);
                        (diff * diff) as f32
                    })
                    .sum();
                if d < best_d {
                    best_d = d;
                    best = ci;
                }
            }
            for (i, &x) in s.iter().enumerate() {
                sums[best][i] += x as f64;
            }
            counts[best] += 1;
        }
        for ci in 0..k {
            if counts[ci] > 0 {
                for i in 0..dim {
                    centroids[ci][i] = (sums[ci][i] / counts[ci] as f64) as f32;
                }
            }
        }
    }
    centroids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(n: usize, dim: usize) -> Vec<Vec<f32>> {
        (0..n)
            .map(|i| {
                (0..dim)
                    .map(|d| ((i * 31 + d * 7) % 13) as f32 / 13.0)
                    .collect()
            })
            .collect()
    }

    #[test]
    fn scalar_quantizer_round_trip_is_close() {
        let mut q = ScalarQuantizer::new(4);
        q.calibrate(sample(64, 4));
        let v = vec![0.1, 0.5, 0.9, 0.3];
        let code = q.encode(&v);
        assert_eq!(code.len(), 4);
        let dist = q.distance(&v, &code);
        assert!(dist < 0.5, "distance should be small, got {dist}");
    }

    #[test]
    fn product_quantizer_produces_codes_within_range() {
        let mut q = ProductQuantizer::new(8);
        q.calibrate(sample(64, 8));
        let v = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        let code = q.encode(&v);
        assert_eq!(code.len(), q.m);
        for b in &code {
            assert!((*b as usize) < q.k);
        }
    }

    #[test]
    fn binary_quantizer_is_32x_compression() {
        let q = BinaryQuantizer::new(256);
        assert_eq!(q.code_bytes(), 32);
        let v: Vec<f32> = (0..256)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let code = q.encode(&v);
        assert_eq!(code.len(), 32);
    }

    #[test]
    fn quantizer_dispatch_routes_to_concrete() {
        let mut q = ScalarQuantizer::new(2);
        q.calibrate(sample(8, 2));
        let d = QuantizerDispatch::Scalar(q);
        let v = vec![0.2, 0.7];
        let code = d.encode(&v);
        let dist = d.distance(&v, &code);
        assert!(dist < 0.5);
    }
}
