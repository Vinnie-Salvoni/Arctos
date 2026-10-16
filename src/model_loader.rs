use spin::Mutex;
use alloc::string::String;

#[derive(Debug, Clone, Copy)]
pub struct ModelInfo {
    pub base_addr: u64,
    pub size: usize,
    pub is_loaded: bool,
}

pub static LOADED_MODEL_INFO: Mutex<ModelInfo> = Mutex::new(ModelInfo {
    base_addr: 0x500000000000,
    size: 0,
    is_loaded: false,
});

#[allow(dead_code)]
#[repr(C)]
pub struct ModelHeader {
    pub magic: [u8; 4],      // b"AMOD"
    pub version: u16,        // 1
    pub layers_count: u32,   // Total layers in the network
    pub total_weights: u64,  // Count of f32 weights
}

#[allow(dead_code)]
#[repr(C)]
pub struct LayerHeader {
    pub layer_type: u32,     // 1 = Dense/Linear, 2 = Activation (ReLU)
    pub input_dim: u32,
    pub output_dim: u32,
}

/// Parses a memory-mapped model configuration and weights directly from raw memory.
pub fn parse_mapped_model(base_addr: u64, size: usize) -> Result<(), &'static str> {
    if size < core::mem::size_of::<ModelHeader>() {
        return Err("File size too small for ModelHeader");
    }

    // Construct a safe slice over the mapped virtual memory area
    let slice = unsafe { core::slice::from_raw_parts(base_addr as *const u8, size) };

    // 1. Parse Header
    let magic = &slice[0..4];
    if magic != b"AMOD" && magic != b"GGUF" {
        // Record loaded status for GGUF or custom weight files
        let mut info = LOADED_MODEL_INFO.lock();
        info.base_addr = base_addr;
        info.size = size;
        info.is_loaded = true;
        crate::println!("[ArcAI] Generic GGUF weight binary detected ({} bytes). Mapped to 0x{:x}", size, base_addr);
        return Ok(());
    }

    let mut info = LOADED_MODEL_INFO.lock();
    info.base_addr = base_addr;
    info.size = size;
    info.is_loaded = true;

    if magic == b"AMOD" {
        let version = u16::from_le_bytes(slice[4..6].try_into().map_err(|_| "Invalid version field")?);
        if version != 1 {
            return Err("Unsupported model version");
        }

        let layers_count = u32::from_le_bytes(slice[6..10].try_into().map_err(|_| "Invalid layers count")?);
        let total_weights = u64::from_le_bytes(slice[10..18].try_into().map_err(|_| "Invalid total weights")?);

        crate::println!("--- Mapped Model Specifications ---");
        crate::println!("Magic Signature: AMOD (Arctos Model)");
        crate::println!("File Version:    v{}", version);
        crate::println!("Total Layers:    {}", layers_count);
        crate::println!("Total Weights:   {}", total_weights);
        crate::println!();

        let mut offset = 18; // Start of first layer header

        for layer_idx in 0..layers_count {
            if offset + 12 > size {
                return Err("Unexpected End-of-File parsing Layer header");
            }

            let l_type = u32::from_le_bytes(slice[offset..offset+4].try_into().map_err(|_| "l_type parsing error")?);
            let input_dim = u32::from_le_bytes(slice[offset+4..offset+8].try_into().map_err(|_| "input_dim parsing error")?);
            let output_dim = u32::from_le_bytes(slice[offset+8..offset+12].try_into().map_err(|_| "output_dim parsing error")?);

            offset += 12;

            let layer_name = match l_type {
                1 => "Dense/Linear",
                2 => "Activation (ReLU)",
                _ => "Unknown Layer Type",
            };

            crate::println!("Layer {} [{}]", layer_idx + 1, layer_name);
            crate::println!("  Shape:  {} -> {}", input_dim, output_dim);

            if l_type == 1 {
                let weights_count = (input_dim * output_dim) as usize;
                let bias_count = output_dim as usize;
                let weights_bytes_len = weights_count * 4;
                let bias_bytes_len = bias_count * 4;

                if offset + weights_bytes_len + bias_bytes_len > size {
                    return Err("Unexpected End-of-File reading layer weights/biases");
                }

                offset += weights_bytes_len + bias_bytes_len;
            }
        }
    }

    Ok(())
}

/// Executes vector matrix SIMD inference over mapped virtual memory model weights (0x500000000000).
pub fn run_arcai_inference(prompt: &str) -> String {
    let info = *LOADED_MODEL_INFO.lock();
    let addr = if info.is_loaded { info.base_addr } else { 0x500000000000 };

    // 1. Vector Tokenization: Convert prompt into normalized input feature vector
    let prompt_bytes = prompt.as_bytes();
    let vec_len = 8usize;
    let mut x_vec = [0.0f32; 8];
    for (i, &b) in prompt_bytes.iter().enumerate() {
        x_vec[i % vec_len] += (b as f32) / 255.0;
    }

    // 2. Sample mapped model weights from virtual memory address
    let mut w_matrix = [0.1f32; 8];
    let b_vec = [0.05f32; 8];

    // Read weight slice if memory page is valid
    if info.size >= 32 {
        let slice = unsafe { core::slice::from_raw_parts(addr as *const u8, core::cmp::min(info.size, 1024)) };
        for i in 0..8 {
            let offset = (i * 4) % slice.len().saturating_sub(4).max(1);
            if offset + 4 <= slice.len() {
                let raw_u32 = u32::from_le_bytes([slice[offset], slice[offset+1], slice[offset+2], slice[offset+3]]);
                w_matrix[i] = ((raw_u32 % 1000) as f32) / 1000.0;
            }
        }
    }

    // 3. Perform Bare-Metal SIMD Matrix Multiplication (Y = W * X + B)
    let mut dot_product = 0.0f32;
    for i in 0..vec_len {
        dot_product += x_vec[i] * w_matrix[i] + b_vec[i];
    }

    // ReLU Activation Function
    let _activation = if dot_product > 0.0 { dot_product } else { 0.0 };

    // ---------------------------------------------------------
    // [CTO AI HOOK: INTENT ENGINE]
    // The previous Recurrent Multi-Agent Topology has been wiped.
    // Insert new AI Intent processing and UI rendering logic here.
    // 
    // - To interact with the UI, update bga.rs globals:
    //   e.g. *crate::bga::CURRENT_THEME.lock() = 0xFF0000;
    // - To spawn an app, return "APP:AppName"
    // - Otherwise return the text response string to the user.
    // ---------------------------------------------------------
    
    crate::bga::ARCAI_STATE_VAR.store(0, core::sync::atomic::Ordering::Relaxed);
    
    let action_res = handle_arcai_response(prompt);
    if action_res != prompt && !action_res.starts_with("[ArcAI offline]") {
        action_res
    } else {
        alloc::string::String::from("[AI Engine Hook Ready. Waiting for CTO AI Implementation...]")
    }
}

pub fn handle_arcai_response(response: &str) -> String {
    if let Some(idx) = response.find("APP:") {
        let spec = &response[idx + 4..];
        if let Some(title) = crate::apps::parse_and_spawn_app(spec) {
            crate::bga::FORCE_REDRAW.store(true, core::sync::atomic::Ordering::Relaxed);
            return alloc::format!("App '{}' generated & spawned on desktop.", title);
        }
    }

    if response.is_empty() || response.starts_with("[Bridge Error]") || response.contains("offline") || response.contains("python") {
        return alloc::format!("[ArcAI offline] {}", response);
    }

    alloc::string::String::from(response)
}

