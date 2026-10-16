use std::fs::File;
use std::io::Write;
use std::path::Path;

struct Sample {
    input: String,
    output: String,
}

// Pseudo-random number generator for weight initialization
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_f32(&mut self) -> f32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let val = (self.state >> 33) as u32;
        (val as f32) / (u32::MAX as f32)
    }

    fn randn(&mut self) -> f32 {
        let u1 = self.next_f32().max(1e-7);
        let u2 = self.next_f32();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
    }
}

fn main() {
    println!("==================================================================");
    println!(" Arctos OS - Char-RNN Autoregressive LLM Trainer (Adagrad)       ");
    println!("==================================================================");

    let dataset_path = Path::new("arctos_dataset.json");
    let mut dataset_items = Vec::new();

    if dataset_path.exists() {
        if let Ok(content) = std::fs::read_to_string(dataset_path) {
            let mut cur_input = String::new();
            let mut cur_output = String::new();
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("\"input\":") {
                    if let Some(val) = trimmed.split(':').nth(1) {
                        cur_input = val.trim_matches(|c| c == ' ' || c == '"' || c == ',').to_string();
                    }
                } else if trimmed.starts_with("\"output\":") {
                    if let Some(val) = trimmed.split(':').nth(1) {
                        cur_output = val.trim_matches(|c| c == ' ' || c == '"' || c == ',').to_string();
                    }
                    if !cur_input.is_empty() && !cur_output.is_empty() {
                        dataset_items.push(Sample {
                            input: cur_input.clone(),
                            output: cur_output.clone(),
                        });
                        cur_input.clear();
                        cur_output.clear();
                    }
                }
            }
            println!("[ok] Loaded {} conversational samples", dataset_items.len());
        }
    }

    if dataset_items.is_empty() {
        for i in 0..1000 {
            dataset_items.push(Sample {
                input: format!("How do I safely write to memory {}", i),
                output: format!("Use write_volatile to write to memory offset 0x{:x}", i * 4),
            });
        }
    }

    let vocab_size = 257;
    let hidden_size = 128;
    
    println!("[trainer] Architecture: Recurrent Neural Network (Char-RNN)");
    println!("[trainer] Vocab: {}, Hidden Size: {}", vocab_size, hidden_size);
    println!();

    let mut rng = Rng::new(42);

    // RNN Weights (Xavier/He Init)
    let mut w_xh = vec![0.0f32; hidden_size * vocab_size];
    let mut w_hh = vec![0.0f32; hidden_size * hidden_size];
    let mut w_hy = vec![0.0f32; vocab_size * hidden_size];
    let mut b_h = vec![0.0f32; hidden_size];
    let mut b_y = vec![0.0f32; vocab_size];

    for i in 0..w_xh.len() { w_xh[i] = rng.randn() * 0.01; }
    for i in 0..w_hh.len() { w_hh[i] = rng.randn() * 0.01; }
    for i in 0..w_hy.len() { w_hy[i] = rng.randn() * 0.01; }

    // Adagrad Memory
    let mut m_w_xh = vec![0.0f32; w_xh.len()];
    let mut m_w_hh = vec![0.0f32; w_hh.len()];
    let mut m_w_hy = vec![0.0f32; w_hy.len()];
    let mut m_b_h = vec![0.0f32; b_h.len()];
    let mut m_b_y = vec![0.0f32; b_y.len()];

    let num_epochs = 150;
    let learning_rate = 0.1f32;
    let seq_length = 32;

    for epoch in 1..=num_epochs {
        let mut total_loss = 0.0f32;
        let mut tokens_processed = 0;

        for sample in dataset_items.iter().take(400) {
            let full_text = format!("{}->{}\n", sample.input, sample.output);
            let bytes = full_text.as_bytes();
            let mut h_prev = vec![0.0f32; hidden_size];

            let chunks = bytes.chunks(seq_length);
            for chunk in chunks {
                if chunk.len() < 2 { continue; }
                
                let mut inputs = Vec::new();
                let mut targets = Vec::new();
                for i in 0..chunk.len()-1 {
                    inputs.push(chunk[i] as usize);
                    targets.push(chunk[i+1] as usize);
                }
                
                let n_steps = inputs.len();
                let mut hs = vec![vec![0.0f32; hidden_size]; n_steps + 1];
                hs[0] = h_prev.clone();
                let mut ps = vec![vec![0.0f32; vocab_size]; n_steps];
                
                let mut loss = 0.0f32;

                // Forward pass
                for t in 0..n_steps {
                    let x = inputs[t];
                    // hs[t] = tanh(W_xh * x + W_hh * hs[t-1] + b_h)
                    for h in 0..hidden_size {
                        let mut val = b_h[h];
                        val += w_xh[h * vocab_size + x];
                        for h_prev_idx in 0..hidden_size {
                            val += w_hh[h * hidden_size + h_prev_idx] * hs[t][h_prev_idx];
                        }
                        hs[t+1][h] = val.tanh();
                    }
                    
                    // ys[t] = W_hy * hs[t] + b_y
                    let mut ys = vec![0.0f32; vocab_size];
                    let mut max_y = -1e9f32;
                    for v in 0..vocab_size {
                        let mut val = b_y[v];
                        for h in 0..hidden_size {
                            val += w_hy[v * hidden_size + h] * hs[t+1][h];
                        }
                        ys[v] = val;
                        if val > max_y { max_y = val; }
                    }
                    
                    // Softmax
                    let mut sum_exp = 0.0f32;
                    for v in 0..vocab_size {
                        let exp_val = (ys[v] - max_y).exp();
                        ps[t][v] = exp_val;
                        sum_exp += exp_val;
                    }
                    for v in 0..vocab_size {
                        ps[t][v] /= sum_exp;
                    }
                    
                    loss += -ps[t][targets[t]].max(1e-7).ln();
                }
                
                total_loss += loss;
                tokens_processed += n_steps;

                // Backward pass
                let mut dw_xh = vec![0.0f32; w_xh.len()];
                let mut dw_hh = vec![0.0f32; w_hh.len()];
                let mut dw_hy = vec![0.0f32; w_hy.len()];
                let mut db_h = vec![0.0f32; b_h.len()];
                let mut db_y = vec![0.0f32; b_y.len()];
                
                let mut dh_next = vec![0.0f32; hidden_size];
                
                for t in (0..n_steps).rev() {
                    let mut dy = ps[t].clone();
                    dy[targets[t]] -= 1.0;
                    
                    for v in 0..vocab_size {
                        db_y[v] += dy[v];
                        for h in 0..hidden_size {
                            dw_hy[v * hidden_size + h] += dy[v] * hs[t+1][h];
                        }
                    }
                    
                    let mut dh = vec![0.0f32; hidden_size];
                    for h in 0..hidden_size {
                        let mut val = dh_next[h];
                        for v in 0..vocab_size {
                            val += w_hy[v * hidden_size + h] * dy[v];
                        }
                        let dtanh = 1.0 - hs[t+1][h] * hs[t+1][h];
                        dh[h] = val * dtanh;
                    }
                    
                    for h in 0..hidden_size {
                        db_h[h] += dh[h];
                        dw_xh[h * vocab_size + inputs[t]] += dh[h];
                        for h_prev_idx in 0..hidden_size {
                            dw_hh[h * hidden_size + h_prev_idx] += dh[h] * hs[t][h_prev_idx];
                            dh_next[h_prev_idx] += w_hh[h * hidden_size + h_prev_idx] * dh[h];
                        }
                    }
                }

                // Adagrad Update
                for i in 0..w_xh.len() {
                    m_w_xh[i] += dw_xh[i] * dw_xh[i];
                    w_xh[i] -= learning_rate * dw_xh[i] / (m_w_xh[i] + 1e-8).sqrt();
                }
                for i in 0..w_hh.len() {
                    m_w_hh[i] += dw_hh[i] * dw_hh[i];
                    w_hh[i] -= learning_rate * dw_hh[i] / (m_w_hh[i] + 1e-8).sqrt();
                }
                for i in 0..w_hy.len() {
                    m_w_hy[i] += dw_hy[i] * dw_hy[i];
                    w_hy[i] -= learning_rate * dw_hy[i] / (m_w_hy[i] + 1e-8).sqrt();
                }
                for i in 0..b_h.len() {
                    m_b_h[i] += db_h[i] * db_h[i];
                    b_h[i] -= learning_rate * db_h[i] / (m_b_h[i] + 1e-8).sqrt();
                }
                for i in 0..b_y.len() {
                    m_b_y[i] += db_y[i] * db_y[i];
                    b_y[i] -= learning_rate * db_y[i] / (m_b_y[i] + 1e-8).sqrt();
                }
                
                h_prev = hs[n_steps].clone();
            }
        }

        let avg_loss = if tokens_processed > 0 { total_loss / (tokens_processed as f32) } else { 0.0 };
        let perplexity = avg_loss.exp();

        if epoch % 10 == 0 || epoch == 1 || epoch == num_epochs {
            println!(
                "  Epoch {:3}/{} [====================>] Loss: {:.4} | Perplexity: {:.2}",
                epoch, num_epochs, avg_loss, perplexity
            );
        }
    }

    println!();
    println!("[trainer] Char-RNN Training Complete!");
    
    // Serialize to arcai_weights.bin
    let mut weight_bytes = Vec::new();
    weight_bytes.extend_from_slice(b"AMOD");
    weight_bytes.extend_from_slice(&2u16.to_le_bytes()); // Version 2
    weight_bytes.extend_from_slice(&5u32.to_le_bytes());

    let total_weights = (w_xh.len() + w_hh.len() + w_hy.len() + b_h.len() + b_y.len()) as u64;
    weight_bytes.extend_from_slice(&total_weights.to_le_bytes());

    for &val in w_xh.iter().chain(w_hh.iter()).chain(w_hy.iter()).chain(b_h.iter()).chain(b_y.iter()) {
        weight_bytes.extend_from_slice(&val.to_le_bytes());
    }

    while weight_bytes.len() < 131072 { // expanded buffer
        weight_bytes.push(0x00);
    }

    let weights_file = "arcai_weights.bin";
    let mut file = File::create(weights_file).expect("Failed to create arcai_weights.bin");
    file.write_all(&weight_bytes).expect("Failed to write binary weights file");

    println!("[ok] Exported Char-RNN weights -> {} ({} bytes)", weights_file, weight_bytes.len());
}
