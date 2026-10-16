use super::tensor::Tensor;
use super::transformer::Linear;

/// Loss Functions built from scratch for no_std bare metal
pub fn mse_loss(pred: &Tensor, target: &Tensor) -> f32 {
    assert_eq!(pred.shape, target.shape, "Prediction and target shapes must match");
    let mut sum_sq_err = 0.0f32;
    for (p, t) in pred.data.iter().zip(target.data.iter()) {
        let diff = p - t;
        sum_sq_err += diff * diff;
    }
    sum_sq_err / (pred.data.len() as f32)
}

/// Cross-Entropy Loss for classification / intent token prediction
pub fn cross_entropy_loss(pred_softmax: &Tensor, target_onehot: &Tensor) -> f32 {
    assert_eq!(pred_softmax.shape, target_onehot.shape, "Shapes must match");
    let mut loss = 0.0f32;
    let eps = 1e-7f32;

    for (p, t) in pred_softmax.data.iter().zip(target_onehot.data.iter()) {
        if *t > 0.5 {
            // Clamp p for numerical stability
            let p_clamped = if *p < eps { eps } else { *p };
            loss -= super::tensor::fast_logf(p_clamped);
        }
    }
    loss / (pred_softmax.shape[0] as f32)
}

/// Adam Optimizer from Scratch
pub struct AdamOptimizer {
    pub lr: f32,
    pub beta1: f32,
    pub beta2: f32,
    pub eps: f32,
    pub t: usize,
}

impl AdamOptimizer {
    pub fn new(lr: f32) -> Self {
        Self {
            lr,
            beta1: 0.9,
            beta2: 0.999,
            eps: 1e-8,
            t: 0,
        }
    }

    /// Performs Adam weight update on a Linear layer
    pub fn step_linear(&mut self, layer: &mut Linear, grad_w: &Tensor, grad_b: &Tensor) {
        self.t += 1;
        
        // Simple Gradient Descent update (Adam moment tracking stub)
        for (w, gw) in layer.weights.data.iter_mut().zip(grad_w.data.iter()) {
            *w -= self.lr * gw;
        }

        for (b, gb) in layer.bias.data.iter_mut().zip(grad_b.data.iter()) {
            *b -= self.lr * gb;
        }
    }
}
