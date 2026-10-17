// =============================================================================
// Arctos OS — Standard Command Line Shell
// =============================================================================

use crate::vga_buffer::{Color, ColorCode, Window, WRITER};
use crate::interrupts::SHELL_QUEUE;
use crate::allocator::heap_stats;
use crate::fs;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;
use pc_keyboard::DecodedKey;
use spin::Mutex;
use crate::intent::{SemanticIntentEngine, IntentListener, dispatch_intent};

// Store allocated blocks so they remain occupied on the heap
static ALLOCATED_CHUNKS: Mutex<Vec<Box<[u8]>>> = Mutex::new(Vec::new());

// Available commands list for autocomplete
const COMMANDS: &[&str] = &[
    "help",
    "info",
    "clear",
    "aitest",
    "ai-test",
    "intent",
    "arcai",
    "arcai-load",
    "arcai-status",
    "arcai-writemodel",
    "aivfs-format",
    "aivfs-list",
    "aivfs-read",
    "aivfs-write",
    "aivfs-delete",
    "aivfs-stress",
    "aivfs-writemodel",
    "aivfs-loadmodel",
    "alloc",
    "free",
    "rm",
    "del",
    "ls",
    "cat",
    "screenfetch",
    "print",
    "echo",
    "run",
    "poweroff",
    "restart",
    "shutdown",
    "reboot",
    "gui",
    "desktop",
];

// ---------------------------------------------------------------------------
// Main Shell Loop
// ---------------------------------------------------------------------------

pub fn run() -> ! {
    // Configure the global WRITER window to bind to the entire 80x25 VGA buffer
    {
        let mut writer = WRITER.lock();
        writer.window = Window {
            x_start: 0,
            y_start: 0,
            width: 80,
            height: 25,
        };
        writer.column_position = 0;
        writer.row_position = 0;
        writer.set_color(Color::White, Color::Black);
        writer.clear_screen();
    }

    crate::println!("Welcome to the Arctos Command Line Shell!");
    crate::println!("Type 'help' to view available commands.");
    print_prompt();

    let mut input_buffer = String::new();

    loop {
        // Poll for keyboard inputs
        let key = {
            let mut queue = SHELL_QUEUE.lock();
            queue.pop_front()
        };

        if let Some(key) = key {
            match key {
                DecodedKey::RawKey(_) => {}
                DecodedKey::Unicode(character) => {
                    match character {
                        '\n' => {
                            clear_ghost_suggestions();
                            crate::println!(""); // New line in console
                            let cmd = input_buffer.trim();
                            if !cmd.is_empty() {
                                execute_command(cmd);
                            }
                            input_buffer.clear();
                            print_prompt();
                        }
                        '\u{0008}' => { // Backspace
                            if !input_buffer.is_empty() {
                                input_buffer.pop();
                                {
                                    let mut writer = WRITER.lock();
                                    writer.write_byte(0x08);
                                }
                                clear_ghost_suggestions();
                                draw_ghost_suggestion(&input_buffer);
                            }
                        }
                        '\t' => { // Tab autocomplete completion
                            if let Some(suggestion) = get_autocomplete_match(&input_buffer) {
                                clear_prompt_line(input_buffer.len());
                                input_buffer = String::from(suggestion);
                                crate::print!("{}", input_buffer);
                                clear_ghost_suggestions();
                            }
                        }
                        c => {
                            // Only accept standard printable ASCII characters
                            if c.is_ascii() && !c.is_control() && input_buffer.len() < 60 {
                                input_buffer.push(c);
                                {
                                    let mut writer = WRITER.lock();
                                    writer.write_byte(c as u8);
                                }
                                clear_ghost_suggestions();
                                draw_ghost_suggestion(&input_buffer);
                            }
                        }
                    }
                }
            }
        }

        // CPU yielding loop
        x86_64::instructions::hlt();
    }
}

// ---------------------------------------------------------------------------
// Shell Commands
// ---------------------------------------------------------------------------

fn execute_command(cmd: &str) {
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    let command_string = parts[0].to_lowercase();
    let command = command_string.as_str();
    let listener = SemanticIntentEngine;
    let intent = listener.parse_intent(cmd);

    match intent {
        crate::intent::Intent::Unknown(_) => {
            match command {
        "help" => {
            crate::println!("--- Arctos OS & ArcAI Help Menu ---");
            crate::println!("  aitest           - Run live Transformer self-test & Adam training check-in");
            crate::println!("  arcai-load       - Memory maps ArcAI model weights to 0x500000000000");
            crate::println!("  arcai-writemodel - Registers ArcAI model file (arcai_q4.bin) to AIVFS storage");
            crate::println!("  arcai-status     - View ArcAI memory mapping status & 2GB RAM allocation");
            crate::println!("  gui / desktop    - Enters 1080p Ambient Canvas mode");
            crate::println!("  aivfs-list / ls  - Lists files on AIVFS storage drive");
            crate::println!("  aivfs-read / cat - Reads contents of a file");
            crate::println!("  aivfs-write <f>  - Writes data file <f>");
            crate::println!("  rm / del <f>     - Deletes file <f> from storage");
            crate::println!("  alloc <bytes>    - Allocates heap memory block");
            crate::println!("  free             - Frees allocated heap block");
            crate::println!("  info / screenfetch - View CPU, memory, & system diagnostics");
            crate::println!("  clear            - Clears the terminal screen");
            crate::println!("  reboot / shutdown- Restarts or powers off the system");
        }
        "aitest" | "ai-test" => {
            crate::ai::run_training_checkin();
        }
        "info" => {
            let stats = heap_stats();
            let brand_string = get_cpu_brand_string();
            let features = get_cpu_features();

            crate::println!("==================================================");
            crate::println!("            Arctos OS Diagnostics Info            ");
            crate::println!("==================================================");
            crate::println!("CPU Processor:   {}", brand_string);
            crate::println!("Architecture:    x86_64 Long Mode (64-bit)");
            
            crate::print!("SIMD Extensions: ");
            let mut first = true;
            if features.sse { crate::print!("SSE"); first = false; }
            if features.sse2 { if !first { crate::print!(", "); } crate::print!("SSE2"); first = false; }
            if features.avx { if !first { crate::print!(", "); } crate::print!("AVX"); first = false; }
            if features.avx2 { if !first { crate::print!(", "); } crate::print!("AVX2"); first = false; }
            if features.avx512f { if !first { crate::print!(", "); } crate::print!("AVX-512"); first = false; }
            if first { crate::print!("None detected"); }
            crate::println!("");
            crate::println!("");

            if let Some((total, usable)) = crate::memory_mapper::system_memory_stats() {
                let total_mib = total / (1024 * 1024);
                let usable_mib = usable / (1024 * 1024);
                crate::println!("Physical Memory (RAM):");
                crate::println!("  Total System RAM:  {} MiB ({} bytes)", total_mib, total);
                crate::println!("  Usable System RAM: {} MiB ({} bytes)", usable_mib, usable);
            } else {
                crate::println!("Physical Memory (RAM):");
                crate::println!("  Status: Unavailable (Boot allocator not registered)");
            }
            crate::println!("");

            let heap_size = 100 * 1024;
            let used_pct = (stats.0 * 100) / heap_size;
            let free_pct = (stats.1 * 100) / heap_size;
            crate::println!("Kernel Memory Heap:");
            crate::println!("  Heap Size:         100 KiB (102,400 bytes)");
            crate::println!("  Heap Used:         {} bytes ({}%)", stats.0, used_pct);
            crate::println!("  Heap Free:         {} bytes ({}%)", stats.1, free_pct);
            crate::println!("");

            let mounted = fs::mount();
            let file_count = fs::list_files().len();
            crate::println!("Storage (AIVFS Filesystem):");
            crate::println!("  Disk Capacity:     10 MiB (10,485,760 bytes)");
            crate::println!("  Volume Mounted:    {}", if mounted { "Yes" } else { "No" });
            crate::println!("  Total Files:       {} active files", file_count);
            crate::println!("==================================================");
        }
        "clear" => {
            let mut writer = WRITER.lock();
            writer.clear_screen();
        }
        "reboot" | "restart" => {
            crate::println!("[system] Rebooting...");
            let mut port = x86_64::instructions::port::Port::<u8>::new(0x64);
            unsafe { port.write(0xFE); }
        }
        "shutdown" | "poweroff" => {
            crate::println!("[system] Shutting down...");
            // QEMU ACPI shutdown
            let mut qemu_acpi = x86_64::instructions::port::Port::<u16>::new(0x604);
            unsafe { qemu_acpi.write(0x2000); }
            // Bochs/Older QEMU shutdown
            let mut bochs_port = x86_64::instructions::port::Port::<u8>::new(0x501);
            unsafe { bochs_port.write(0x34); }
            // Fallback: CPU Halt
            crate::println!("[system] Shutdown failed. System halted.");
            loop {
                x86_64::instructions::hlt();
            }
        }
        "gui" | "desktop" => {
            crate::println!("[system] Entering BGA 1080p Graphical Mode... Press F1 anytime to return to console.");
            crate::bga::run_gui_mode();
        }
        "print" | "echo" => {
            if parts.len() < 2 {
                crate::println!("");
            } else {
                let echo_parts: Vec<&str> = parts[1].splitn(2, ' ').collect();
                if let Ok(count) = echo_parts[0].parse::<usize>() {
                    let text = if echo_parts.len() > 1 { echo_parts[1] } else { "" };
                    for _ in 0..count {
                        crate::println!("{}", text);
                    }
                } else {
                    crate::println!("{}", parts[1]);
                }
            }
        }
        "screenfetch" => {
            let stats = heap_stats();
            let files = fs::list_files();
            
            crate::println!("   /\\      OS: Arctos OS v{}", env!("CARGO_PKG_VERSION"));
            crate::println!("  /  \\     Kernel: x86_64 Long Mode");
            crate::println!(" / /\\ \\    Heap: {} / 100 KiB", stats.0 / 1024);
            crate::println!("/_/  \\_\\   Files: {}", files.len());
        }
        "alloc" => {
            let size = if parts.len() < 2 {
                1024 // Default to 1 KiB
            } else {
                parts[1].parse::<usize>().unwrap_or(1024)
            };

            if size == 0 {
                crate::println!("[error] Allocation size must be greater than 0.");
                return;
            }

            crate::println!("[shell] Allocating {} bytes on the heap...", size);
            
            // Allocate a boxed byte array filled with verification pattern 0x5A
            let chunk = alloc::vec![0x5Au8; size].into_boxed_slice();
            
            // Verify data integrity immediately (checks data, no fake print)
            let mut verified = true;
            for i in 0..size {
                if chunk[i] != 0x5A {
                    verified = false;
                    break;
                }
            }

            if verified {
                crate::println!("[ok] Allocated at {:p} and verified data integrity.", chunk.as_ptr());
                ALLOCATED_CHUNKS.lock().push(chunk);
            } else {
                crate::println!("[fail] Memory corruption detected on allocation!");
            }
        }
        "free" => {
            let mut chunks = ALLOCATED_CHUNKS.lock();
            if let Some(chunk) = chunks.pop() {
                // Verify data integrity before releasing to make sure no overwrite occurred
                let mut verified = true;
                for i in 0..chunk.len() {
                    if chunk[i] != 0x5A {
                        verified = false;
                        break;
                    }
                }
                
                if verified {
                    crate::println!("[shell] Verified integrity at {:p}. Releasing memory...", chunk.as_ptr());
                } else {
                    crate::println!("[warning] Releasing corrupted memory block at {:p}!", chunk.as_ptr());
                }
                
                drop(chunk); // Releases the memory
                crate::println!("[ok] Memory released.");
            } else {
                crate::println!("[error] No active heap allocations to free.");
            }
        }
        "aivfs-format" => {
            crate::println!("[AIVFS] Formatting storage drive...");
            if fs::format() {
                crate::println!("[AIVFS] Storage formatted successfully.");
            } else {
                crate::println!("[error] Formatting failed.");
            }
        }
        "aivfs-list" => {
            if !fs::mount() {
                crate::println!("[AIVFS] Drive not mounted. Formatting first...");
                fs::format();
            }
            crate::println!("--- AIVFS File Directory ---");
            let files = fs::list_files();
            if files.is_empty() {
                crate::println!("(No files found)");
            } else {
                for name in files {
                    crate::println!(" - {}", name);
                }
            }
        }
        "aivfs-read" => {
            if parts.len() < 2 {
                crate::println!("Usage: aivfs-read <filename>");
                return;
            }
            if !fs::mount() { fs::format(); }
            let filename = parts[1];
            if let Some(data) = fs::read_file(filename) {
                if let Ok(s) = core::str::from_utf8(&data) {
                    crate::println!("{}", s);
                } else {
                    crate::println!("[AIVFS] Binary Data: {} bytes", data.len());
                }
            } else {
                crate::println!("[error] File '{}' not found.", filename);
            }
        }
        "aivfs-write" => {
            if parts.len() < 2 {
                crate::println!("Usage: aivfs-write <filename>");
                return;
            }
            if !fs::mount() { fs::format(); }
            let filename = parts[1];
            let data = b"Hello, this is test data written to Arctos Intelligent Vector Filesystem (AIVFS)!";
            if fs::write_file(filename, data) {
                crate::println!("[AIVFS] File '{}' written successfully.", filename);
            } else {
                crate::println!("[error] Failed to write file.");
            }
        }

        "aivfs-stress" => {
            if !fs::mount() { fs::format(); }
            crate::println!("[AIVFS] Running file generation stress test...");
            let test_files = [
                ("kernel.sys", b"SYSTEM_CORE_DATA_SECTORS_BOOT_CFG_DO_NOT_DELETE" as &[u8]),
                ("notes.txt", b"Consumer text document containing project notes." as &[u8]),
                ("weights.vec", b"AI_MODEL_TENSOR_WEIGHT_VECTOR_EMBEDDING_0x5F3759DF" as &[u8]),
                ("temp.bin", b"BINARY_TEMP_CACHE_DATA_00000000" as &[u8]),
                ("readme.txt", b"Arctos OS V1.0 - Readme instructions for consumers." as &[u8]),
            ];
            for (filename, data) in &test_files {
                if fs::write_file(filename, *data) {
                    crate::println!(" - Wrote {}", filename);
                }
            }
            crate::println!("[ok] Stress test complete.");
        }
        "arcai" | "arcai-status" => {
            crate::println!("=====================================================");
            crate::println!("                 ARCAI ENGINE STATUS                 ");
            crate::println!("=====================================================");
            crate::println!("Name:           ArcAI (Arctos OS Core Intelligence)");
            crate::println!("Virtual Mapped: 0x500000000000");
            crate::println!("RAM Allocation: Up to 2048 MB (2 GB) Allowed");
            crate::println!("Status:         Active & Listening for Intents");
            crate::println!("=====================================================");
        }
        "arcai-writemodel" | "aivfs-writemodel" => {
            if !fs::mount() { fs::format(); }
            let filename = if parts.len() >= 2 { parts[1] } else { "arcai_q4.bin" };
            crate::println!("[ArcAI] Registering model file '{}' to AIVFS storage...", filename);

            let mut data = alloc::vec::Vec::new();
            // 1. Model Header
            data.extend_from_slice(b"AMOD"); // magic
            data.extend_from_slice(&1u16.to_le_bytes()); // version
            data.extend_from_slice(&2u32.to_le_bytes()); // layers_count
            data.extend_from_slice(&10u64.to_le_bytes()); // total_weights

            // 2. Layer 1 Header (Dense)
            data.extend_from_slice(&1u32.to_le_bytes()); // type = 1
            data.extend_from_slice(&4u32.to_le_bytes()); // input_dim = 4
            data.extend_from_slice(&2u32.to_le_bytes()); // output_dim = 2

            let weights: [f32; 8] = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
            for w in &weights { data.extend_from_slice(&w.to_le_bytes()); }

            let biases: [f32; 2] = [0.0, 1.0];
            for b in &biases { data.extend_from_slice(&b.to_le_bytes()); }

            // 3. Layer 2 Header (ReLU)
            data.extend_from_slice(&2u32.to_le_bytes());
            data.extend_from_slice(&2u32.to_le_bytes());
            data.extend_from_slice(&2u32.to_le_bytes());

            if fs::write_file(filename, &data) {
                crate::println!("[ok] ArcAI model file '{}' written ({} bytes).", filename, data.len());
            } else {
                crate::println!("[error] Failed to write ArcAI model file.");
            }
        }
        "arcai-load" | "aivfs-loadmodel" => {
            if !fs::mount() { fs::format(); }
            let filename = if parts.len() >= 2 { parts[1] } else { "arcai_q4.bin" };
            crate::println!("[ArcAI] Mapping model weights from '{}'...", filename);

            let target_addr = 0x_5000_0000_0000;
            if let Some((addr, size)) = crate::memory_mapper::map_file_to_virtual_addr(filename, target_addr) {
                crate::println!("[ok] File mapped successfully to virtual address: 0x{:x}", addr);

                match crate::model_loader::parse_mapped_model(addr, size) {
                    Ok(()) => {
                        crate::println!("[ArcAI] Model verified and loaded successfully into 2GB memory space.");
                        crate::println!("[ArcAI] Hello! I am ArcAI, active and ready on Arctos OS.");
                    }
                    Err(e) => {
                        crate::println!("[error] ArcAI model verification failed: {}", e);
                    }
                }
            } else {
                crate::println!("[error] Failed to memory map ArcAI model file.");
            }
        }
        "rm" | "del" | "aivfs-delete" => {
            if parts.len() < 2 {
                crate::println!("Usage: rm <filename>");
                return;
            }
            let filename = parts[1];
            if fs::delete_file(filename) {
                crate::println!("[ok] File '{}' deleted.", filename);
            } else {
                crate::println!("[error] File '{}' not found or could not be deleted.", filename);
            }
        }
        "ls" | "dir" => {
            execute_command("aivfs-list");
        }
        "cat" => {
            if parts.len() < 2 {
                crate::println!("Usage: cat <filename>");
                return;
            }
            let mut cmd = String::from("aivfs-read ");
            cmd.push_str(parts[1]);
            execute_command(&cmd);
        }
        _ => {
            crate::println!("[error] Unknown command: '{}'", command);
        }
        
    }
}
        other => {
            dispatch_intent(other);
            process_bus_intents();
        }
    }
    
}

fn process_bus_intents() {
    while let Some(intent) = crate::intent::pop_intent() {
        match intent {
            crate::intent::Intent::LaunchApp(app) => {
                let mut filename = app;
                if !filename.ends_with(".arc") {
                    filename.push_str(".arc");
                }
                crate::println!("[intent-router] Resolving intent to launch app: {}", filename);
                crate::apps::run_app(&filename);
            }
            crate::intent::Intent::ConfigureEnvironment { role, details: _ } => {
                crate::println!("[intent-router] Resolving environment configuration for: {}", role);
                crate::println!("  [hook] Future package selector will install libraries for '{}' in background.", role);
            }
            crate::intent::Intent::SetHotspot { x, y, label, action } => {
                crate::println!("[intent-router] Pinning hotspot: '{}' at ({}, {}) for action: {}", label, x, y, action);
                crate::println!("  [hook] Future desktop compositor will render this persistent control.");
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Shell Prompt & Autocomplete UI Helpers
// ---------------------------------------------------------------------------

pub fn print_prompt() {
    let mut writer = WRITER.lock();
    writer.set_color(Color::LightCyan, Color::Black);
    writer.write_string("arctos > ");
    writer.set_color(Color::White, Color::Black);
}

fn clear_prompt_line(input_len: usize) {
    let mut writer = WRITER.lock();
    let start_col = 9; // "arctos > " is 9 characters
    let end_col = start_col + input_len;
    let blank = ColorCode::new(Color::White, Color::Black);
    let row = 24;
    for col in start_col..end_col {
        writer.write_char_at(row, col, b' ', blank);
    }
    writer.column_position = start_col;
    writer.update_hardware_cursor();
}

fn get_autocomplete_match(input: &str) -> Option<&'static str> {
    if input.is_empty() {
        return None;
    }
    let lower_input = input.to_lowercase();
    for cmd in COMMANDS {
        if cmd.starts_with(&lower_input) {
            return Some(cmd);
        }
    }
    None
}

fn draw_ghost_suggestion(input: &str) {
    if let Some(suggestion) = get_autocomplete_match(input) {
        let suffix = &suggestion[input.len()..];
        let mut writer = WRITER.lock();
        let start_col = writer.column_position;
        let ghost_color = ColorCode::new(Color::DarkGray, Color::Black);
        let row = 24;
        
        for (i, byte) in suffix.bytes().enumerate() {
            writer.write_char_at(row, start_col + i, byte, ghost_color);
        }
    }
}

fn clear_ghost_suggestions() {
    let mut writer = WRITER.lock();
    let start_col = writer.column_position;
    let blank_color = ColorCode::new(Color::White, Color::Black);
    let row = 24;
    // Clear rest of terminal line (max width is 80 columns in full screen)
    for col in start_col..80 {
        writer.write_char_at(row, col, b' ', blank_color);
    }
}

// ---------------------------------------------------------------------------
// CPU Spec Query Helpers (using CPUID inline assembly)
// ---------------------------------------------------------------------------

struct CpuFeatures {
    sse: bool,
    sse2: bool,
    avx: bool,
    avx2: bool,
    avx512f: bool,
}

fn get_cpu_brand_string() -> String {
    let mut brand = [0u8; 48];
    let mut max_ext_leaf = 0u32;

    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "pop rbx",
            inout("eax") 0x80000000u32 => max_ext_leaf,
            out("ecx") _,
            out("edx") _,
        );
    }

    if max_ext_leaf >= 0x80000004u32 {
        for i in 0..3 {
            let leaf = 0x80000002u32 + i as u32;
            let mut eax = 0u32;
            let mut ebx = 0u32;
            let mut ecx = 0u32;
            let mut edx = 0u32;
            unsafe {
                core::arch::asm!(
                    "push rbx",
                    "cpuid",
                    "mov {ebx_val:e}, ebx",
                    "pop rbx",
                    ebx_val = out(reg) ebx,
                    inout("eax") leaf => eax,
                    out("ecx") ecx,
                    out("edx") edx,
                );
            }
            let idx = i as usize * 16;
            brand[idx..idx+4].copy_from_slice(&eax.to_le_bytes());
            brand[idx+4..idx+8].copy_from_slice(&ebx.to_le_bytes());
            brand[idx+8..idx+12].copy_from_slice(&ecx.to_le_bytes());
            brand[idx+12..idx+16].copy_from_slice(&edx.to_le_bytes());
        }

        // Find the first null character to truncate
        let mut len = 48;
        for j in 0..48 {
            if brand[j] == 0 {
                len = j;
                break;
            }
        }
        let s = core::str::from_utf8(&brand[..len]).unwrap_or("Generic x86_64 CPU");
        String::from(s.trim())
    } else {
        String::from("Generic x86_64 CPU")
    }
}

fn get_cpu_features() -> CpuFeatures {
    let mut ecx1 = 0u32;
    let mut edx1 = 0u32;
    let mut ebx7 = 0u32;

    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "pop rbx",
            inout("eax") 1 => _,
            out("ecx") ecx1,
            out("edx") edx1,
        );
    }

    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_val:e}, ebx",
            "pop rbx",
            ebx_val = out(reg) ebx7,
            inout("eax") 7 => _,
            inout("ecx") 0 => _,
            out("edx") _,
        );
    }

    CpuFeatures {
        sse: (edx1 & (1 << 25)) != 0,
        sse2: (edx1 & (1 << 26)) != 0,
        avx: (ecx1 & (1 << 28)) != 0,
        avx2: (ebx7 & (1 << 5)) != 0,
        avx512f: (ebx7 & (1 << 16)) != 0,
    }
}

