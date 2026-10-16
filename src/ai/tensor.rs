// Tensor Mathematics from Scratch
// We will build a multi-dimensional array struct here and implement Matrix Multiplication (MatMul).
// Later, this will be optimized to use the Intel Arc 140T GPU via USM.

extern crate alloc;
use alloc::vec::Vec;
use alloc::vec;
use core::ops::Add;

#[derive(Clone, Debug)]
pub struct Tensor {
    // Basic CPU implementation stub
    pub data: Vec<f32>,
    pub shape: Vec<usize>,
}

impl Tensor {
    pub fn new(data: Vec<f32>, shape: Vec<usize>) -> Self {
        let size: usize = shape.iter().fold(1, |acc, &x| acc * x);
        assert_eq!(data.len(), size, "Data length must match shape product");
        Self { data, shape }
    }

    pub fn zeros(shape: Vec<usize>) -> Self {
        let size = shape.iter().fold(1, |acc, &x| acc * x);
        Self {
            data: vec![0.0; size],
            shape,
        }
    }

    pub fn ones(shape: Vec<usize>) -> Self {
        let size = shape.iter().fold(1, |acc, &x| acc * x);
        Self {
            data: vec![1.0; size],
            shape,
        }
    }

    /// Naive 2D Matrix Multiplication (CPU)
    /// To be accelerated by Arc 140T via Level Zero USM later.
    pub fn matmul(&self, other: &Tensor) -> Self {
        assert_eq!(self.shape.len(), 2, "Matmul currently supports 2D tensors");
        assert_eq!(other.shape.len(), 2, "Matmul currently supports 2D tensors");
        
        let m = self.shape[0];
        let k = self.shape[1];
        let k2 = other.shape[0];
        let n = other.shape[1];
        
        assert_eq!(k, k2, "Inner dimensions must match for matmul");

        let mut out_data = vec![0.0; m * n];
        
        // Naive O(n^3) implementation
        for i in 0..m {
            for j in 0..n {
                let mut sum = 0.0;
                for p in 0..k {
                    sum += self.data[i * k + p] * other.data[p * n + j];
                }
                out_data[i * n + j] = sum;
            }
        }
        
        Self::new(out_data, vec![m, n])
    }
    /// Transposes a 2D matrix (swaps rows and columns)
    pub fn transpose(&self) -> Self {
        assert_eq!(self.shape.len(), 2, "Transpose currently supports 2D tensors");
        let rows = self.shape[0];
        let cols = self.shape[1];
        let mut out_data = vec![0.0; rows * cols];

        for r in 0..rows {
            for c in 0..cols {
                out_data[c * rows + r] = self.data[r * cols + c];
            }
        }

        Self::new(out_data, vec![cols, rows])
    }

    /// Applies Softmax activation along the last dimension (row-wise for 2D)
    pub fn softmax(&self) -> Self {
        assert_eq!(self.shape.len(), 2, "Softmax currently supports 2D tensors");
        let rows = self.shape[0];
        let cols = self.shape[1];
        let mut out_data = vec![0.0; rows * cols];

        for r in 0..rows {
            let row_start = r * cols;
            let row_end = row_start + cols;
            
            // Find max for numerical stability
            let mut max_val = f32::MIN;
            for c in row_start..row_end {
                if self.data[c] > max_val {
                    max_val = self.data[c];
                }
            }

            // Compute exp and sum
            let mut sum_exp = 0.0f32;
            for c in row_start..row_end {
                let exp_val = fast_expf(self.data[c] - max_val);
                out_data[c] = exp_val;
                sum_exp += exp_val;
            }

            // Normalize
            for c in row_start..row_end {
                out_data[c] /= sum_exp;
            }
        }

        Self::new(out_data, vec![rows, cols])
    }

    /// Applies GELU (Gaussian Error Linear Unit) activation element-wise
    pub fn gelu(&self) -> Self {
        let out_data: Vec<f32> = self.data.iter().map(|&x| {
            // Approximation: 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
            let inner = 0.79788456 * (x + 0.044715 * x * x * x);
            0.5 * x * (1.0 + fast_tanhf(inner))
        }).collect();

        Self::new(out_data, self.shape.clone())
    }
}

/// Zero-dependency expf approximation for bare-metal no_std environment
fn fast_expf(mut x: f32) -> f32 {
    if x < -88.0 { return 0.0; }
    if x > 88.0 { x = 88.0; }
    let mut res = 1.0 + x / 256.0;
    for _ in 0..8 {
        res *= res;
    }
    res
}

/// Zero-dependency tanhf approximation for bare-metal no_std environment
pub fn fast_tanhf(x: f32) -> f32 {
    if x < -3.0 { return -1.0; }
    if x > 3.0 { return 1.0; }
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// Zero-dependency natural log approximation for bare-metal no_std environment
pub fn fast_logf(x: f32) -> f32 {
    if x <= 0.0 { return -16.0; }
    // Pade approximation around 1.0
    2.0 * (x - 1.0) / (x + 1.0)
}

impl Add for &Tensor {
    type Output = Tensor;

    fn add(self, other: Self) -> Tensor {
        assert_eq!(self.shape, other.shape, "Shapes must match for addition");
        let out_data: Vec<f32> = self.data.iter().zip(other.data.iter()).map(|(a, b)| a + b).collect();
        Tensor::new(out_data, self.shape.clone())
    }
}
