pub mod tensor;
pub mod transformer;
pub mod train;
pub mod ddlgn;
pub mod infer;
pub mod level_zero;
pub mod hardware;

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;
use tensor::Tensor;
use transformer::TransformerBlock;
use train::{mse_loss, AdamOptimizer};

use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref PERSISTENT_MODEL: Mutex<transformer::Linear> = Mutex::new(transformer::Linear::new(4, 4));
    pub static ref TOTAL_EPOCHS: Mutex<usize> = Mutex::new(0);
}

/// Interactive check-in test to train our scratch-built Transformer inside the kernel
pub fn run_training_checkin() {
    crate::println!("--- [Arctos AI Engine Self-Test & Persistent Training] ---");
    
    // 1. Create input sequence and target tensors (shape [2, 4])
    let input = Tensor::new(vec![0.5, 0.2, 0.1, 0.9,  0.3, 0.8, 0.4, 0.6], vec![2, 4]);
    let target = Tensor::new(vec![1.0, 0.0, 0.0, 1.0,  0.0, 1.0, 0.0, 1.0], vec![2, 4]);

    // 2. Lock persistent model and optimizer
    let mut layer = PERSISTENT_MODEL.lock();
    let mut epoch_count = TOTAL_EPOCHS.lock();
    let mut opt = AdamOptimizer::new(0.08);

    crate::println!("[ai-train] Model weights loaded from persistent kernel memory.");
    crate::println!("[ai-train] Input shape: {:?}, Target shape: {:?}", input.shape, target.shape);
    crate::println!("[ai-train] Training 5 steps from Epoch {}...", *epoch_count + 1);

    for _ in 0..5 {
        *epoch_count += 1;
        let current_epoch = *epoch_count;

        // Forward pass: Y = X * W + B
        let pred = layer.forward(&input);
        
        // Compute MSE loss
        let loss = mse_loss(&pred, &target);
        
        // Compute exact backprop error: Error = (Pred - Target) / batch_size
        let diff_data: Vec<f32> = pred.data.iter().zip(target.data.iter()).map(|(p, t)| (p - t) / 2.0).collect();
        let error = Tensor::new(diff_data, pred.shape.clone());

        // dW = X^T * Error (shape [4, 2] * [2, 4] = [4, 4])
        let grad_w = input.transpose().matmul(&error);

        // Perform weight update using Adam optimizer
        opt.step_linear(&mut *layer, &grad_w, &Tensor::zeros(vec![1, 4]));

        // Print progress check-in
        let loss_int = (loss * 10000.0) as u32;
        crate::println!("  Epoch {:2}: MSE Loss = {}.{:04}", current_epoch, loss_int / 10000, loss_int % 10000);
    }

    crate::println!("[ok] Persistent neural network weights saved to memory! Next 'aitest' will continue from Epoch {}.", *epoch_count + 1);
    crate::println!();
}
