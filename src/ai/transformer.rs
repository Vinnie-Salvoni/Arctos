extern crate alloc;
use alloc::vec;
use super::tensor::Tensor;

/// Dense/Linear Layer (Y = X * W + B)
pub struct Linear {
    pub weights: Tensor,
    pub bias: Tensor,
}

impl Linear {
    pub fn new(in_features: usize, out_features: usize) -> Self {
        Self {
            weights: Tensor::ones(vec![in_features, out_features]),
            bias: Tensor::zeros(vec![1, out_features]),
        }
    }

    pub fn forward(&self, input: &Tensor) -> Tensor {
        let output = input.matmul(&self.weights);
        // Add bias (broadcasted over rows)
        let mut out_data = output.data.clone();
        let rows = output.shape[0];
        let cols = output.shape[1];

        for r in 0..rows {
            for c in 0..cols {
                out_data[r * cols + c] += self.bias.data[c];
            }
        }

        Tensor::new(out_data, vec![rows, cols])
    }
}

/// Multi-Head Self-Attention Layer from Scratch
pub struct MultiHeadAttention {
    pub q_proj: Linear,
    pub k_proj: Linear,
    pub v_proj: Linear,
    pub out_proj: Linear,
    pub d_model: usize,
}

impl MultiHeadAttention {
    pub fn new(d_model: usize) -> Self {
        Self {
            q_proj: Linear::new(d_model, d_model),
            k_proj: Linear::new(d_model, d_model),
            v_proj: Linear::new(d_model, d_model),
            out_proj: Linear::new(d_model, d_model),
            d_model,
        }
    }

    pub fn forward(&self, input: &Tensor) -> Tensor {
        // 1. Compute Q, K, V projections
        let q = self.q_proj.forward(input);
        let k = self.k_proj.forward(input);
        let v = self.v_proj.forward(input);

        // 2. Scaled Dot-Product Attention: Softmax((Q * K^T) / sqrt(d_k)) * V
        let k_t = k.transpose();
        let mut scores = q.matmul(&k_t);

        // Scale by sqrt(d_model)
        let scale = 1.0 / (self.d_model as f32);
        for val in scores.data.iter_mut() {
            *val *= scale;
        }

        let attn_weights = scores.softmax();
        let attn_output = attn_weights.matmul(&v);

        // 3. Final Linear projection
        self.out_proj.forward(&attn_output)
    }
}

/// Complete Transformer Encoder/Decoder Block
pub struct TransformerBlock {
    pub attention: MultiHeadAttention,
    pub ffn_linear1: Linear,
    pub ffn_linear2: Linear,
}

impl TransformerBlock {
    pub fn new(d_model: usize, d_ffn: usize) -> Self {
        Self {
            attention: MultiHeadAttention::new(d_model),
            ffn_linear1: Linear::new(d_model, d_ffn),
            ffn_linear2: Linear::new(d_ffn, d_model),
        }
    }

    pub fn forward(&self, input: &Tensor) -> Tensor {
        // 1. Self-Attention + Residual
        let attn_out = self.attention.forward(input);
        let x1 = input + &attn_out;

        // 2. Feed-Forward Network (GELU) + Residual
        let ffn_hidden = self.ffn_linear1.forward(&x1).gelu();
        let ffn_out = self.ffn_linear2.forward(&ffn_hidden);
        
        &x1 + &ffn_out
    }
}
